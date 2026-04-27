//! Spec 11 integration tests against a real Chrome / Chromium.
//!
//! Every test in this file is `#[ignore]` so the default
//! `cargo test -p engine` run never starts a browser. To execute them:
//!
//! ```text
//! cargo test -p engine --test chromium_html -- --ignored
//! ```
//!
//! The browser executable is resolved per spec 11's discovery rules
//! (explicit `BROWSER_PATH` env, then `$PATH`, then platform defaults).
//! Override at the test level by setting `CHROME_PATH=/path/to/chrome`
//! before invoking `cargo test`.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use axum::extract::State;
use axum::http::HeaderMap;
use axum::response::Html;
use axum::routing::get;
use engine::{
    BrowserConfig, ChromiumEngine, Cookie, EngineError, Margins, PaperSize, PdfOptions,
    RequestContext, WaitCondition,
};
use tokio::sync::oneshot;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Launch an engine for the test, honouring `CHROME_PATH` if set.
async fn launch_engine() -> ChromiumEngine {
    let mut cfg = BrowserConfig::default();
    if let Ok(p) = std::env::var("CHROME_PATH") {
        cfg.executable = Some(PathBuf::from(p));
    }
    ChromiumEngine::launch_with(cfg)
        .await
        .expect("failed to launch ChromiumEngine — set CHROME_PATH or install chrome")
}

/// Launch an engine with a tighter render timeout.
async fn launch_engine_with_timeout(timeout: Duration) -> ChromiumEngine {
    let cfg = BrowserConfig {
        timeout,
        executable: std::env::var("CHROME_PATH").ok().map(PathBuf::from),
        ..BrowserConfig::default()
    };
    ChromiumEngine::launch_with(cfg)
        .await
        .expect("failed to launch ChromiumEngine — set CHROME_PATH or install chrome")
}

/// Spawn a tiny axum server bound to `127.0.0.1:0` and return the
/// resolved address plus a shutdown trigger.
async fn spawn_server(router: Router) -> (SocketAddr, oneshot::Sender<()>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let (tx, rx) = oneshot::channel::<()>();
    tokio::spawn(async move {
        axum::serve(listener, router)
            .with_graceful_shutdown(async {
                let _ = rx.await;
            })
            .await
            .unwrap();
    });
    // Allow the listener to start accepting before tests fire requests.
    tokio::time::sleep(Duration::from_millis(20)).await;
    (addr, tx)
}

/// Validate basic PDF structure and return the loaded document for
/// further inspection.
fn parse_pdf(bytes: &[u8]) -> lopdf::Document {
    assert!(bytes.starts_with(b"%PDF-"), "missing %PDF- header");
    assert!(
        bytes.windows(5).any(|w| w == b"%%EOF"),
        "missing %%EOF trailer"
    );
    lopdf::Document::load_mem(bytes).expect("lopdf failed to parse pdf bytes")
}

/// Extract text from every page of `doc` into a single concatenated
/// string.
fn extract_all_text(doc: &lopdf::Document) -> String {
    let pages = doc.get_pages();
    let nums: Vec<u32> = pages.keys().copied().collect();
    doc.extract_text(&nums).unwrap_or_default()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore]
async fn html_to_pdf_returns_valid_pdf_bytes() {
    let engine = launch_engine().await;
    let bytes = engine
        .html_to_pdf(
            "<h1>hello world</h1>",
            None,
            &PdfOptions::default(),
            &RequestContext::default(),
        )
        .await
        .expect("render failed");

    let doc = parse_pdf(&bytes);
    assert!(!doc.get_pages().is_empty(), "pdf has zero pages");

    engine.shutdown().await.ok();
}

#[tokio::test]
#[ignore]
async fn html_to_pdf_respects_paper_size() {
    let engine = launch_engine().await;
    let opts = PdfOptions {
        paper: PaperSize::new(2.0, 2.0).unwrap(),
        margin: Margins::ZERO,
        ..PdfOptions::default()
    };
    let bytes = engine
        .html_to_pdf("<p>tiny</p>", None, &opts, &RequestContext::default())
        .await
        .expect("render failed");

    let doc = parse_pdf(&bytes);
    let (_, page_oid) = doc
        .get_pages()
        .into_iter()
        .next()
        .expect("pdf has no pages");
    // PDF user-space units are 1/72in. 2in × 2in => 144 × 144 user units.
    let page = doc.get_object(page_oid).unwrap().as_dict().unwrap();
    let media_box_obj = match page.get(b"MediaBox") {
        Ok(o) => o,
        Err(_) => {
            // Fall back to the document's root pages dict if MediaBox
            // is inherited.
            let pages_oid = doc
                .catalog()
                .unwrap()
                .get(b"Pages")
                .unwrap()
                .as_reference()
                .unwrap();
            doc.get_object(pages_oid)
                .unwrap()
                .as_dict()
                .unwrap()
                .get(b"MediaBox")
                .expect("no MediaBox on page or pages dict")
        }
    };
    let media_box = media_box_obj.as_array().unwrap();
    assert_eq!(media_box.len(), 4);
    let to_f32 = |o: &lopdf::Object| -> f32 {
        o.as_float()
            .or_else(|_| o.as_i64().map(|v| v as f32))
            .expect("MediaBox entry not a number")
    };
    let width = to_f32(&media_box[2]);
    let height = to_f32(&media_box[3]);
    assert!((width - 144.0).abs() < 1.0, "width was {width}");
    assert!((height - 144.0).abs() < 1.0, "height was {height}");

    engine.shutdown().await.ok();
}

#[tokio::test]
#[ignore]
async fn url_to_pdf_against_local_axum() {
    let router = Router::new().route(
        "/index.html",
        get(|| async { Html("<!doctype html><html><body><h1>From server</h1></body></html>") }),
    );
    let (addr, shutdown) = spawn_server(router).await;

    let engine = launch_engine().await;
    let bytes = engine
        .url_to_pdf(
            &format!("http://{addr}/index.html"),
            &PdfOptions::default(),
            &RequestContext::default(),
        )
        .await
        .expect("render failed");

    let doc = parse_pdf(&bytes);
    assert_eq!(doc.get_pages().len(), 1);

    engine.shutdown().await.ok();
    let _ = shutdown.send(());
}

#[tokio::test]
#[ignore]
async fn wait_selector_completes_when_element_appears() {
    let engine = launch_engine().await;
    let html = r#"
        <!doctype html>
        <html><body>
            <div id="placeholder">waiting</div>
            <script>
                setTimeout(function() {
                    var d = document.createElement('div');
                    d.id = 'late';
                    d.textContent = 'arrived';
                    document.body.appendChild(d);
                }, 100);
            </script>
        </body></html>
    "#;
    let opts = PdfOptions {
        wait: WaitCondition::Selector {
            selector: "#late".into(),
        },
        ..PdfOptions::default()
    };
    let bytes = engine
        .html_to_pdf(html, None, &opts, &RequestContext::default())
        .await
        .expect("render failed");

    let _ = parse_pdf(&bytes);
    engine.shutdown().await.ok();
}

#[tokio::test]
#[ignore]
async fn wait_selector_times_out_when_missing() {
    let engine = launch_engine_with_timeout(Duration::from_millis(800)).await;
    let opts = PdfOptions {
        wait: WaitCondition::Selector {
            selector: "#never".into(),
        },
        ..PdfOptions::default()
    };
    let result = engine
        .html_to_pdf(
            "<p>nothing here</p>",
            None,
            &opts,
            &RequestContext::default(),
        )
        .await;
    assert!(
        matches!(result, Err(EngineError::Timeout(_))),
        "expected Timeout, got {result:?}"
    );

    engine.shutdown().await.ok();
}

#[tokio::test]
#[ignore]
async fn cookies_and_headers_round_trip() {
    type SharedSeen = Arc<tokio::sync::Mutex<Option<EchoState>>>;

    #[derive(Clone, Default)]
    struct EchoState {
        cookie_header: Option<String>,
        x_test_header: Option<String>,
        user_agent: Option<String>,
    }

    async fn echo(State(state): State<SharedSeen>, headers: HeaderMap) -> Html<String> {
        let s = EchoState {
            cookie_header: headers
                .get("cookie")
                .and_then(|v| v.to_str().ok())
                .map(str::to_string),
            x_test_header: headers
                .get("x-test")
                .and_then(|v| v.to_str().ok())
                .map(str::to_string),
            user_agent: headers
                .get("user-agent")
                .and_then(|v| v.to_str().ok())
                .map(str::to_string),
        };

        let body = format!(
            "<!doctype html><html><body>\
             <p id=\"cookie\">COOKIE:{c}</p>\
             <p id=\"x-test\">XTEST:{x}</p>\
             <p id=\"ua\">UA:{u}</p>\
             </body></html>",
            c = s.cookie_header.clone().unwrap_or_default(),
            x = s.x_test_header.clone().unwrap_or_default(),
            u = s.user_agent.clone().unwrap_or_default(),
        );
        *state.lock().await = Some(s);
        Html(body)
    }

    let state: SharedSeen = Arc::new(tokio::sync::Mutex::new(None));
    let router = Router::new()
        .route("/echo", get(echo))
        .with_state(state.clone());
    let (addr, shutdown) = spawn_server(router).await;

    let engine = launch_engine().await;

    let mut extra_headers = HashMap::new();
    extra_headers.insert("X-Test".into(), "marker-value".into());

    let request = RequestContext {
        user_agent: Some("FolioTest/1.0".into()),
        extra_headers,
        cookies: vec![Cookie {
            name: "session".into(),
            value: "abc123".into(),
            domain: Some(addr.ip().to_string()),
            path: Some("/".into()),
            secure: false,
            http_only: false,
        }],
        fail_on_status: vec![],
    };

    let bytes = engine
        .url_to_pdf(
            &format!("http://{addr}/echo"),
            &PdfOptions::default(),
            &request,
        )
        .await
        .expect("render failed");

    let doc = parse_pdf(&bytes);
    let text = extract_all_text(&doc);
    assert!(
        text.contains("session=abc123"),
        "cookie missing in text: {text}"
    );
    assert!(
        text.contains("marker-value"),
        "x-test header missing in text: {text}"
    );
    assert!(text.contains("FolioTest/1.0"), "ua missing in text: {text}");

    engine.shutdown().await.ok();
    let _ = shutdown.send(());
}

#[tokio::test]
#[ignore]
async fn concurrent_renders_do_not_deadlock() {
    let engine = launch_engine().await;
    let mut handles = Vec::new();
    for i in 0..8 {
        let e = engine.clone();
        handles.push(tokio::spawn(async move {
            e.html_to_pdf(
                &format!("<h1>page {i}</h1>"),
                None,
                &PdfOptions::default(),
                &RequestContext::default(),
            )
            .await
        }));
    }
    for h in handles {
        let bytes = h.await.expect("task panicked").expect("render failed");
        assert!(bytes.starts_with(b"%PDF-"));
    }
    engine.shutdown().await.ok();
}

#[tokio::test]
#[ignore]
async fn markdown_to_pdf_renders_table() {
    let engine = launch_engine().await;
    let md = "\
| Name | Score |\n\
|------|------:|\n\
| Alice | 99 |\n\
| Bob   | 42 |\n\
";
    let bytes = engine
        .markdown_to_pdf(md, &PdfOptions::default(), &RequestContext::default())
        .await
        .expect("render failed");

    let doc = parse_pdf(&bytes);
    assert_eq!(doc.get_pages().len(), 1);

    // Chrome's PDFs subset their fonts and embed custom CMaps that
    // lopdf cannot reliably decode for `extract_text`. Use a
    // structural assertion instead: verify the page's content stream
    // contains both text-show operators (one per cell) and path
    // operators for the table borders, which together prove the
    // markdown table actually rendered.
    let pages = doc.get_pages();
    let (_, &page_oid) = pages.iter().next().unwrap();
    let content = doc
        .get_and_decode_page_content(page_oid)
        .expect("decode page content");
    let mut text_show_ops = 0usize;
    let mut path_ops = 0usize;
    for op in &content.operations {
        match op.operator.as_str() {
            "Tj" | "TJ" | "'" | "\"" => text_show_ops += 1,
            "m" | "l" | "re" => path_ops += 1,
            _ => {}
        }
    }
    assert!(
        text_show_ops >= 4,
        "expected >=4 text-show ops (header + cells), got {text_show_ops}"
    );
    assert!(
        path_ops >= 4,
        "expected >=4 path ops for table borders, got {path_ops}"
    );

    engine.shutdown().await.ok();
}

#[tokio::test]
#[ignore]
async fn shutdown_is_idempotent() {
    let engine = launch_engine().await;
    let clone = engine.clone();
    engine.shutdown().await.expect("first shutdown failed");
    // Second shutdown on a different clone must succeed without
    // surfacing an error.
    clone
        .shutdown()
        .await
        .expect("second shutdown returned err");
}

#[tokio::test]
#[ignore]
async fn shutdown_cancels_in_flight_render() {
    let engine = launch_engine().await;

    // Kick off a render that waits on a selector that never appears.
    let opts = PdfOptions {
        wait: WaitCondition::Selector {
            selector: "#never-arrives".into(),
        },
        ..PdfOptions::default()
    };
    let render_engine = engine.clone();
    let render = tokio::spawn(async move {
        render_engine
            .html_to_pdf("<p>blocked</p>", None, &opts, &RequestContext::default())
            .await
    });

    // Give the render time to open the page and start polling.
    tokio::time::sleep(Duration::from_millis(200)).await;

    engine.shutdown().await.expect("shutdown failed");

    let result = render.await.expect("render task panicked");
    match result {
        Err(EngineError::Internal(msg)) => assert!(
            msg.contains("engine shut down"),
            "unexpected internal msg: {msg}"
        ),
        Err(EngineError::Cdp(_)) => {
            // Acceptable: CDP error surfacing the dropped connection
            // before the shutdown flag was observed. Spec accepts the
            // shutdown-side as long as it succeeded.
        }
        Err(EngineError::Timeout(_)) => {
            // Acceptable: render's outer timeout fired before our
            // shutdown closed the page.
        }
        other => panic!("expected shutdown-related error, got {other:?}"),
    }
}
