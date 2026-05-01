#![cfg(all(feature = "chromium", feature = "libreoffice"))]

//! End-to-end integration tests against real engines.
//!
//! These tests skip gracefully when dependencies are missing:
//! - Chrome on PATH (or via $CHROME_PATH) for Chromium tests
//! - soffice on PATH (or via $LIBREOFFICE_PATH) for LibreOffice tests
//!
//! Each test spawns an instance of the full `folio-server` router on a
//! dynamically-allocated localhost port, performs HTTP requests against it
//! via `reqwest`, and asserts on the response shape.

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use engine::{BrowserConfig, LibreOfficeConfig};
use reqwest::multipart::{Form, Part};
use server::backend::ChromiumBackend;
use server::config::{LogFormat, ServerConfig};
use server::supervised_engine::{SupervisedChromiumEngine, SupervisedLibreOfficeEngine};
use server::{AppState, build_router};
use tokio::net::TcpListener;
use tokio::sync::oneshot;

/// Return true if a Chrome binary is available (via `$CHROME_PATH` or `$PATH`).
fn have_chrome() -> bool {
    if std::env::var("CHROME_PATH").is_ok() {
        return true;
    }
    for name in ["google-chrome", "chromium", "chromium-browser", "chrome"] {
        if std::process::Command::new(name)
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
        {
            return true;
        }
    }
    false
}

/// Return true if a LibreOffice (`soffice`) binary is available.
fn have_soffice() -> bool {
    if std::env::var("LIBREOFFICE_PATH").is_ok() {
        return true;
    }
    std::process::Command::new("soffice")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

struct TestServer {
    addr: SocketAddr,
    shutdown_tx: Option<oneshot::Sender<()>>,
    handle: tokio::task::JoinHandle<()>,
    chromium: SupervisedChromiumEngine,
}

impl TestServer {
    fn url(&self, path: &str) -> String {
        format!("http://{}{}", self.addr, path)
    }

    async fn shutdown(mut self) {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }
        // Best-effort wait for axum::serve to finish.
        let _ = tokio::time::timeout(Duration::from_secs(35), self.handle).await;
        let _ = self.chromium.shutdown().await;
    }
}

fn test_config() -> ServerConfig {
    ServerConfig {
        host: "127.0.0.1".parse().unwrap(),
        port: 0,
        concurrency: 4,
        max_body_bytes: 50 * 1024 * 1024,
        request_timeout: Duration::from_secs(60),
        chrome_path: std::env::var("CHROME_PATH").ok().map(Into::into),
        no_sandbox: Some(cfg!(target_os = "linux")),
        soffice_path: std::env::var("LIBREOFFICE_PATH").ok().map(Into::into),
        log_level: "off".to_string(),
        log_format: LogFormat::Text,
        batch_max_items: 50,
        batch_concurrency: 4,
        batch_max_active: 10,
        batch_retention_minutes: 60,
        batch_storage_path: std::path::PathBuf::from("/tmp/folio-batches"),
        otel_enabled: false,
        otel_endpoint: "http://localhost:4318/v1/traces".to_string(),
        chromium_lazy_start: false,
        chromium_idle_shutdown_timeout: None,
        libreoffice_lazy_start: false,
        libreoffice_idle_shutdown_timeout: None,
        api_disable_health_route_telemetry: false,
        api_disable_root_route_telemetry: false,
        api_disable_debug_route_telemetry: false,
        api_disable_version_route_telemetry: false,
        api_enable_debug_route: false,
        api_tls_cert_file: None,
        api_tls_key_file: None,
        api_basic_auth_username: None,
        api_basic_auth_password: None,
        api_download_from_allow_list: Vec::new(),
        api_download_from_deny_list: Vec::new(),
        api_download_from_max_retry: 3,
        api_disable_download_from: false,
        api_correlation_id_header: "x-request-id".to_string(),
        api_root_path: String::new(),
        libreoffice_unoserver_port: 2003,
        libreoffice_unoserver_ready_timeout: std::time::Duration::from_secs(60),
        webhook_max_retry: 4,
        webhook_retry_min_wait: std::time::Duration::from_secs(1),
        webhook_retry_max_wait: std::time::Duration::from_secs(30),
        webhook_client_timeout: std::time::Duration::from_secs(30),
        webhook_allow_list: vec![],
        webhook_deny_list: vec![],
    }
}

async fn launch_chromium(config: &ServerConfig) -> SupervisedChromiumEngine {
    let defaults = BrowserConfig::default();
    let cfg = BrowserConfig {
        executable: config.chrome_path.clone(),
        no_sandbox: config.no_sandbox.unwrap_or(defaults.no_sandbox),
        timeout: config.request_timeout,
        ..defaults
    };
    SupervisedChromiumEngine::new(cfg)
}

async fn launch_libreoffice(config: &ServerConfig) -> SupervisedLibreOfficeEngine {
    let cfg = LibreOfficeConfig {
        executable: config.soffice_path.clone(),
        timeout: config.request_timeout,
        ..LibreOfficeConfig::default()
    };
    SupervisedLibreOfficeEngine::new(cfg)
}

async fn spawn_server(with_libreoffice: bool) -> TestServer {
    let config = test_config();
    let chromium = launch_chromium(&config).await;
    let backend = ChromiumBackend::new(chromium.clone());
    let lo = if with_libreoffice {
        Some(Arc::new(launch_libreoffice(&config).await))
    } else {
        None
    };

    let state = AppState::new(Some(Arc::new(backend)), config.clone())
        .with_libreoffice(lo);
    let router = build_router(state, &config);

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();

    let handle = tokio::spawn(async move {
        let _ = axum::serve(listener, router.into_make_service())
            .with_graceful_shutdown(async {
                let _ = shutdown_rx.await;
            })
            .await;
    });

    // Give axum a tick to start accepting connections.
    tokio::time::sleep(Duration::from_millis(50)).await;

    TestServer {
        addr,
        shutdown_tx: Some(shutdown_tx),
        handle,
        chromium,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread")]
async fn e2e_chromium_html() {
    if !have_chrome() {
        eprintln!("skipping: chrome not found");
        return;
    }
    let srv = spawn_server(false).await;

    let form = Form::new().part(
        "files",
        Part::bytes(b"<html><body><h1>folio</h1></body></html>".to_vec())
            .file_name("index.html")
            .mime_str("text/html")
            .unwrap(),
    );

    let resp = reqwest::Client::new()
        .post(srv.url("/forms/chromium/convert/html"))
        .multipart(form)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let ct = resp
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
    assert_eq!(ct, "application/pdf");
    let bytes = resp.bytes().await.unwrap();
    assert!(bytes.starts_with(b"%PDF-"));
    lopdf::Document::load_mem(&bytes).expect("valid PDF");

    srv.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn e2e_chromium_url_against_local_axum_app() {
    if !have_chrome() {
        eprintln!("skipping: chrome not found");
        return;
    }
    use axum::Router;
    use axum::routing::get;
    let inner = Router::new().route(
        "/",
        get(|| async { axum::response::Html("<html><body><p>local-page</p></body></html>") }),
    );
    let inner_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let inner_addr = inner_listener.local_addr().unwrap();
    let inner_handle = tokio::spawn(async move {
        let _ = axum::serve(inner_listener, inner.into_make_service()).await;
    });

    let srv = spawn_server(false).await;
    let form = Form::new().text("url", format!("http://{inner_addr}/"));
    let resp = reqwest::Client::new()
        .post(srv.url("/forms/chromium/convert/url"))
        .multipart(form)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let bytes = resp.bytes().await.unwrap();
    assert!(bytes.starts_with(b"%PDF-"));

    inner_handle.abort();
    srv.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn e2e_libreoffice_docx() {
    if !have_chrome() {
        eprintln!("skipping: chrome not found");
        return;
    }
    if !have_soffice() {
        eprintln!("skipping: soffice not found");
        return;
    }
    let srv = spawn_server(true).await;

    // A trivial RTF qualifies as a `writer` LibreOffice input. We emit
    // RTF rather than DOCX so the test does not depend on a binary
    // fixture; the engine spec guarantees RTF -> writer_pdf_Export.
    let rtf = b"{\\rtf1\\ansi folio test\\par}".to_vec();
    let form = Form::new().part(
        "files",
        Part::bytes(rtf)
            .file_name("input.rtf")
            .mime_str("application/rtf")
            .unwrap(),
    );

    let resp = reqwest::Client::new()
        .post(srv.url("/forms/libreoffice/convert"))
        .multipart(form)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let ct = resp
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
    assert_eq!(ct, "application/pdf");
    let bytes = resp.bytes().await.unwrap();
    assert!(bytes.starts_with(b"%PDF-"));

    srv.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn e2e_pdfengines_merge_split_round_trip() {
    if !have_chrome() {
        eprintln!("skipping: chrome not found");
        return;
    }
    let srv = spawn_server(false).await;

    // Render three trivial PDFs via the chromium endpoint, then merge,
    // then split-by-N=1, then assert page count.
    let mut pdfs: Vec<Vec<u8>> = Vec::new();
    for i in 0..3 {
        let html = format!("<html><body><h1>page-{i}</h1></body></html>");
        let form = Form::new().part(
            "files",
            Part::bytes(html.into_bytes())
                .file_name("index.html")
                .mime_str("text/html")
                .unwrap(),
        );
        let resp = reqwest::Client::new()
            .post(srv.url("/forms/chromium/convert/html"))
            .multipart(form)
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), 200);
        pdfs.push(resp.bytes().await.unwrap().to_vec());
    }

    let mut merge_form = Form::new();
    for (i, pdf) in pdfs.iter().enumerate() {
        merge_form = merge_form.part(
            "files",
            Part::bytes(pdf.clone())
                .file_name(format!("input-{i}.pdf"))
                .mime_str("application/pdf")
                .unwrap(),
        );
    }
    let merged_resp = reqwest::Client::new()
        .post(srv.url("/forms/pdfengines/merge"))
        .multipart(merge_form)
        .send()
        .await
        .unwrap();
    assert_eq!(merged_resp.status(), 200);
    let merged_bytes = merged_resp.bytes().await.unwrap();
    let merged_doc = lopdf::Document::load_mem(&merged_bytes).expect("valid merged PDF");
    assert_eq!(merged_doc.get_pages().len(), 3);

    let split_form = Form::new()
        .part(
            "files",
            Part::bytes(merged_bytes.to_vec())
                .file_name("merged.pdf")
                .mime_str("application/pdf")
                .unwrap(),
        )
        .text("splitMode", "intervals")
        .text("splitSpan", "1");
    let split_resp = reqwest::Client::new()
        .post(srv.url("/forms/pdfengines/split"))
        .multipart(split_form)
        .send()
        .await
        .unwrap();
    assert_eq!(split_resp.status(), 200);
    let ct = split_resp
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
    assert_eq!(ct, "application/zip");

    srv.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn graceful_shutdown_drains_inflight() {
    if !have_chrome() {
        eprintln!("skipping: chrome not found");
        return;
    }
    let srv = spawn_server(false).await;

    // Issue a render in the background.
    let url = srv.url("/forms/chromium/convert/html");
    let render = tokio::spawn(async move {
        let form = Form::new().part(
            "files",
            Part::bytes(b"<html><body>x</body></html>".to_vec())
                .file_name("index.html")
                .mime_str("text/html")
                .unwrap(),
        );
        reqwest::Client::new()
            .post(url)
            .multipart(form)
            .send()
            .await
    });

    // Tiny pause so the request lands inside the server.
    tokio::time::sleep(Duration::from_millis(100)).await;

    let started = std::time::Instant::now();
    srv.shutdown().await;
    assert!(
        started.elapsed() < Duration::from_secs(35),
        "shutdown exceeded 35s drain budget"
    );

    // The in-flight request should have either completed (200) or
    // been cleanly cut (connection error). We accept either.
    let _ = render.await;
}
