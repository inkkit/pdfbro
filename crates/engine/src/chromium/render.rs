//! Render orchestration for [`super::ChromiumEngine`].
//!
//! Implements `html_to_pdf` and `url_to_pdf` per
//! `docs/specs/11-engine-chromium.md`. The whole render — from page
//! creation to `Page.printToPDF` — is wrapped in a single
//! `tokio::time::timeout(BrowserConfig::timeout, ...)` so wait
//! conditions and slow networks are bounded uniformly.

use std::collections::HashSet;
use std::sync::{Arc, Mutex};

use chromiumoxide::Page;
use chromiumoxide::cdp::browser_protocol::dom::Rgba;
use chromiumoxide::cdp::browser_protocol::emulation::{
    SetDefaultBackgroundColorOverrideParams, SetEmulatedMediaParams,
};
use chromiumoxide::cdp::browser_protocol::network::{
    CookieParam, EventLoadingFailed, EventResponseReceived, Headers, ResourceType,
    SetExtraHttpHeadersParams, SetUserAgentOverrideParams,
};
use chromiumoxide::cdp::browser_protocol::page::{EventDomContentEventFired, EventLoadEventFired};
use chromiumoxide::cdp::js_protocol::runtime::EventExceptionThrown;
use futures_util::StreamExt;
use tokio::task::JoinHandle;
use tracing::debug;

use crate::types::{EngineError, EngineResult, PdfOptions};

use super::pdf_params::{build_printtopdf_params, media_kind};
use super::wait;
use super::{ChromiumEngine, Cookie, RequestContext};

/// `html_to_pdf` entrypoint. See the spec for the step-by-step
/// behavior contract.
pub(crate) async fn html_to_pdf(
    engine: &ChromiumEngine,
    html: &str,
    base_url: Option<&str>,
    opts: &PdfOptions,
    request: &RequestContext,
) -> EngineResult<Vec<u8>> {
    opts.validate()?;
    let timeout = engine.inner().config.timeout;
    let fut = render_html(engine, html, base_url, opts, request);
    match tokio::time::timeout(timeout, fut).await {
        Ok(r) => r,
        Err(_) => Err(EngineError::Timeout(timeout)),
    }
}

/// `url_to_pdf` entrypoint.
pub(crate) async fn url_to_pdf(
    engine: &ChromiumEngine,
    url: &str,
    opts: &PdfOptions,
    request: &RequestContext,
) -> EngineResult<Vec<u8>> {
    opts.validate()?;
    let timeout = engine.inner().config.timeout;
    let fut = render_url(engine, url, opts, request);
    match tokio::time::timeout(timeout, fut).await {
        Ok(r) => r,
        Err(_) => Err(EngineError::Timeout(timeout)),
    }
}

// ---------------------------------------------------------------------------
// Internal rendering pipeline
// ---------------------------------------------------------------------------

async fn render_html(
    engine: &ChromiumEngine,
    html: &str,
    base_url: Option<&str>,
    opts: &PdfOptions,
    request: &RequestContext,
) -> EngineResult<Vec<u8>> {
    let page = open_page(engine).await?;
    let result = render_html_on(engine, &page, html, base_url, opts, request).await;
    close_page_best_effort(page).await;
    result
}

async fn render_html_on(
    engine: &ChromiumEngine,
    page: &Page,
    html: &str,
    base_url: Option<&str>,
    opts: &PdfOptions,
    request: &RequestContext,
) -> EngineResult<Vec<u8>> {
    apply_request_context(engine, page, request).await?;

    if let Some(url) = base_url {
        page.goto(url).await.map_err(|e| navigation_error(url, e))?;
        page.set_content(html)
            .await
            .map_err(|e| engine.map_cdp_error(e))?;
    } else {
        page.set_content(html)
            .await
            .map_err(|e| engine.map_cdp_error(e))?;
    }

    apply_emulated_media(engine, page, opts).await?;
    apply_omit_background(engine, page, opts.omit_background).await?;
    inject_print_color_adjust(engine, page, opts.print_background).await?;
    wait::apply(page, &opts.wait).await?;
    print_to_pdf(engine, page, opts).await
}

async fn render_url(
    engine: &ChromiumEngine,
    url: &str,
    opts: &PdfOptions,
    request: &RequestContext,
) -> EngineResult<Vec<u8>> {
    let page = open_page(engine).await?;
    let result = render_url_on(engine, &page, url, opts, request).await;
    close_page_best_effort(page).await;
    result
}

async fn render_url_on(
    engine: &ChromiumEngine,
    page: &Page,
    url: &str,
    opts: &PdfOptions,
    request: &RequestContext,
) -> EngineResult<Vec<u8>> {
    debug!("render_url_on: start");
    apply_request_context(engine, page, request).await?;
    debug!("render_url_on: request context applied");

    // Set up event listeners before navigation.
    let main_frame_status = if request.fail_on_status.is_empty() {
        None
    } else {
        Some(spawn_main_frame_status_capture(page).await?)
    };

    let resource_status = if request.fail_on_resource_status.is_empty() {
        None
    } else {
        Some(spawn_resource_status_capture(page, &request.fail_on_resource_status).await?)
    };

    let console_exceptions = if request.fail_on_console_exceptions {
        Some(spawn_console_exception_capture(page).await?)
    } else {
        None
    };

    let resource_loading = if request.fail_on_resource_loading_failed {
        Some(spawn_resource_loading_capture(page).await?)
    } else {
        None
    };

    // Navigate with lifecycle event waits.
    debug!("render_url_on: navigating to {}" , url);
    navigate_with_lifecycle(page, url, engine.inner().config.network_idle_timeout)
        .await
        .map_err(|e| navigation_error(url, e))?;
    debug!("render_url_on: navigation complete");

    // Check main frame status.
    if let Some((status, task)) = main_frame_status {
        task.abort();
        let observed = *status.lock().expect("captured status mutex poisoned");
        if let Some(code) = observed
            && request.fail_on_status.contains(&code)
        {
            return Err(EngineError::Navigation {
                url: url.into(),
                reason: format!("status {code}"),
            });
        }
    }

    // Check resource status.
    if let Some((errors, task)) = resource_status {
        task.abort();
        let errors = errors.lock().expect("resource status mutex poisoned");
        if !errors.is_empty() {
            let msg = errors.join(", ");
            return Err(EngineError::Navigation {
                url: url.into(),
                reason: format!("resource HTTP status failed: {msg}"),
            });
        }
    }

    // Check console exceptions.
    if let Some((exceptions, task)) = console_exceptions {
        task.abort();
        let exceptions = exceptions.lock().expect("console exceptions mutex poisoned");
        if !exceptions.is_empty() {
            let msg = exceptions.join(", ");
            return Err(EngineError::Navigation {
                url: url.into(),
                reason: format!("console exceptions: {msg}"),
            });
        }
    }

    // Check resource loading failures.
    if let Some((failures, task)) = resource_loading {
        task.abort();
        let failures = failures.lock().expect("resource loading mutex poisoned");
        if !failures.is_empty() {
            let msg = failures.join(", ");
            return Err(EngineError::Navigation {
                url: url.into(),
                reason: format!("resource loading failed: {msg}"),
            });
        }
    }

    debug!("render_url_on: applying emulated media");
    apply_emulated_media(engine, page, opts).await?;
    apply_omit_background(engine, page, opts.omit_background).await?;
    inject_print_color_adjust(engine, page, opts.print_background).await?;
    debug!("render_url_on: applying wait condition {:?}", opts.wait);
    wait::apply(page, &opts.wait).await?;
    debug!("render_url_on: printing to PDF");
    print_to_pdf(engine, page, opts).await
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

async fn open_page(engine: &ChromiumEngine) -> EngineResult<Page> {
    let guard = engine.inner().browser.lock().await;
    let browser = guard
        .as_ref()
        .ok_or_else(|| EngineError::Internal("engine shut down".into()))?;
    browser
        .new_page("about:blank")
        .await
        .map_err(|e| engine.map_cdp_error(e))
}

async fn close_page_best_effort(page: Page) {
    if let Err(e) = page.close().await {
        tracing::debug!(error = %e, "page close failed (ignored)");
    }
}

async fn apply_request_context(
    engine: &ChromiumEngine,
    page: &Page,
    request: &RequestContext,
) -> EngineResult<()> {
    if let Some(ua) = &request.user_agent {
        page.execute(SetUserAgentOverrideParams {
            user_agent: ua.clone(),
            accept_language: None,
            platform: None,
            user_agent_metadata: None,
        })
        .await
        .map_err(|e| engine.map_cdp_error(e))?;
    }

    if !request.extra_headers.is_empty() {
        let mut obj = serde_json::Map::with_capacity(request.extra_headers.len());
        for (k, v) in &request.extra_headers {
            obj.insert(k.clone(), serde_json::Value::String(v.clone()));
        }
        page.execute(SetExtraHttpHeadersParams {
            headers: Headers::new(serde_json::Value::Object(obj)),
        })
        .await
        .map_err(|e| engine.map_cdp_error(e))?;
    }

    for cookie in &request.cookies {
        page.set_cookie(cookie_to_param(cookie))
            .await
            .map_err(|e| engine.map_cdp_error(e))?;
    }

    Ok(())
}

fn cookie_to_param(c: &Cookie) -> CookieParam {
    let path = c.path.clone();
    let mut b = CookieParam::builder()
        .name(c.name.clone())
        .value(c.value.clone())
        .secure(c.secure)
        .http_only(c.http_only);
    if let Some(d) = &c.domain {
        // CDP refuses Network.setCookie on about:blank when only a
        // domain is supplied — the cookie has no origin to attach to.
        // Synthesize a URL from scheme + domain + path so cookies can
        // be installed before any navigation has occurred.
        let scheme = if c.secure { "https" } else { "http" };
        let url_path = path.as_deref().unwrap_or("/");
        b = b.domain(d.clone()).url(format!("{scheme}://{d}{url_path}"));
    }
    if let Some(p) = path {
        b = b.path(p);
    }
    // The builder's `build` is infallible when name+value are set.
    b.build().unwrap_or_else(|_| {
        // Defensive: should not occur because name and value are
        // always set above. Fall back to the minimal constructor.
        CookieParam::new(c.name.clone(), c.value.clone())
    })
}

async fn apply_emulated_media(
    engine: &ChromiumEngine,
    page: &Page,
    opts: &PdfOptions,
) -> EngineResult<()> {
    if let Some(media) = opts.emulate_media {
        page.execute(SetEmulatedMediaParams {
            media: Some(media_kind(media).to_string()),
            features: None,
        })
        .await
        .map_err(|e| engine.map_cdp_error(e))?;
    }
    Ok(())
}

/// Inject CSS that matches gotenberg's `forceExactColorsActionFunc`:
/// - Always sets `-webkit-print-color-adjust: exact` so Chrome honours the
///   `printToPDF` `print_background` flag (without it, the property defaults
///   to `economy` and Chrome may strip backgrounds regardless of the flag).
/// - When `print_background` is false, also injects
///   `html, body { background: none !important }` to clear any page-level
///   background colour (e.g. example.com's `body { background: #eee }`).
async fn inject_print_color_adjust(
    engine: &ChromiumEngine,
    page: &Page,
    print_background: bool,
) -> EngineResult<()> {
    let mut css = "html { -webkit-print-color-adjust: exact !important; }".to_string();
    if !print_background {
        css.push_str(" html, body { background: none !important; }");
    }
    let script = format!(
        r#"(() => {{
    const s = document.createElement('style');
    s.appendChild(document.createTextNode('{css}'));
    document.head.appendChild(s);
}})()"#
    );
    page.evaluate(script)
        .await
        .map_err(|e| engine.map_cdp_error(e))?;
    Ok(())
}

/// Override the default background color to transparent (RGBA 0,0,0,0) when
/// `omit_background` is `true`. This is the CDP equivalent of Puppeteer's
/// `page.setBackgroundColor({r:0,g:0,b:0,a:0})` that Gotenberg uses.
async fn apply_omit_background(
    engine: &ChromiumEngine,
    page: &Page,
    omit_background: bool,
) -> EngineResult<()> {
    if omit_background {
        page.execute(SetDefaultBackgroundColorOverrideParams {
            color: Some(Rgba {
                r: 0,
                g: 0,
                b: 0,
                a: Some(0.0),
            }),
        })
        .await
        .map_err(|e| engine.map_cdp_error(e))?;
    }
    Ok(())
}

async fn print_to_pdf(
    engine: &ChromiumEngine,
    page: &Page,
    opts: &PdfOptions,
) -> EngineResult<Vec<u8>> {
    let params = build_printtopdf_params(opts);
    page.pdf(params).await.map_err(|e| engine.map_cdp_error(e))
}

fn navigation_error(url: &str, err: chromiumoxide::error::CdpError) -> EngineError {
    EngineError::Navigation {
        url: url.into(),
        reason: err.to_string(),
    }
}

/// Spawn a background task that records the most recent main-frame
/// (`ResourceType::Document`) response status into the returned
/// `Mutex`. The returned `JoinHandle` should be aborted by the caller
/// once it has read the captured status.
async fn spawn_main_frame_status_capture(
    page: &Page,
) -> EngineResult<(Arc<Mutex<Option<u16>>>, tokio::task::JoinHandle<()>)> {
    let mut events = page
        .event_listener::<EventResponseReceived>()
        .await
        .map_err(|e| EngineError::Cdp(e.to_string()))?;
    let captured: Arc<Mutex<Option<u16>>> = Arc::new(Mutex::new(None));
    let writer = captured.clone();
    let task = tokio::spawn(async move {
        while let Some(ev) = events.next().await {
            if matches!(ev.r#type, ResourceType::Document) {
                let status = ev.response.status as u16;
                if let Ok(mut g) = writer.lock() {
                    *g = Some(status);
                }
            }
        }
    });
    Ok((captured, task))
}

/// Spawn a background task that records resource loading failures.
async fn spawn_resource_loading_capture(
    page: &Page,
) -> EngineResult<(Arc<Mutex<Vec<String>>>, JoinHandle<()>)> {
    let mut events = page
        .event_listener::<EventLoadingFailed>()
        .await
        .map_err(|e| EngineError::Cdp(e.to_string()))?;
    let failures: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let writer = failures.clone();
    let task = tokio::spawn(async move {
        while let Some(ev) = events.next().await {
            // Only capture resource loading failures, not main document failures.
            if !matches!(ev.r#type, ResourceType::Document) {
                let msg = format!("{:?}: {}", ev.r#type, ev.error_text);
                if let Ok(mut g) = writer.lock() {
                    g.push(msg);
                }
            }
        }
    });
    Ok((failures, task))
}

/// Spawn a background task that records console exceptions.
async fn spawn_console_exception_capture(
    page: &Page,
) -> EngineResult<(Arc<Mutex<Vec<String>>>, JoinHandle<()>)> {
    let mut events = page
        .event_listener::<EventExceptionThrown>()
        .await
        .map_err(|e| EngineError::Cdp(e.to_string()))?;
    let exceptions: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let writer = exceptions.clone();
    let task = tokio::spawn(async move {
        while let Some(ev) = events.next().await {
            let msg = ev
                .exception_details
                .exception
                .as_ref()
                .map(|e| {
                    e.description.clone().unwrap_or_else(|| {
                        e.value.as_ref().map(|v| v.to_string()).unwrap_or_else(|| "unknown".to_string())
                    })
                })
                .unwrap_or_else(|| "unknown exception".to_string());
            if let Ok(mut g) = writer.lock() {
                g.push(msg);
            }
        }
    });
    Ok((exceptions, task))
}

/// Spawn a background task that records resource HTTP status codes that match
/// the given list of failing statuses.
async fn spawn_resource_status_capture(
    page: &Page,
    fail_statuses: &[u16],
) -> EngineResult<(Arc<Mutex<Vec<String>>>, JoinHandle<()>)> {
    let mut events = page
        .event_listener::<EventResponseReceived>()
        .await
        .map_err(|e| EngineError::Cdp(e.to_string()))?;
    let fail_set: HashSet<u16> = fail_statuses.iter().copied().collect();
    let errors: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let writer = errors.clone();
    let task = tokio::spawn(async move {
        while let Some(ev) = events.next().await {
            // Only check resources, not main document.
            if !matches!(ev.r#type, ResourceType::Document) {
                let status = ev.response.status as u16;
                if fail_set.contains(&status) {
                    let msg = format!("{}: {}", ev.response.url, status);
                    if let Ok(mut g) = writer.lock() {
                        g.push(msg);
                    }
                }
            }
        }
    });
    Ok((errors, task))
}

/// Navigate to URL and wait for lifecycle events.
///
/// Always waits for `domContentLoaded` and `load`.
/// If `network_idle_timeout` is `Some(t)`, races `networkIdle` against `t`
/// after load — proceeds whichever fires first.
/// If `None`, skips networkIdle entirely (default, matches gotenberg).
async fn navigate_with_lifecycle(
    page: &Page,
    url: &str,
    network_idle_timeout: Option<std::time::Duration>,
) -> Result<(), chromiumoxide::error::CdpError> {
    debug!("navigate_with_lifecycle: registering event listeners");
    let mut dom_content_events = page.event_listener::<EventDomContentEventFired>().await?;
    let mut load_events = page.event_listener::<EventLoadEventFired>().await?;
    debug!("navigate_with_lifecycle: listeners registered");

    debug!("navigate_with_lifecycle: calling page.goto({})", url);
    page.goto(url).await?;
    debug!("navigate_with_lifecycle: page.goto returned");

    debug!("navigate_with_lifecycle: waiting for domContentLoaded and load events");
    let dom_fut = async {
        dom_content_events.next().await;
        debug!("navigate_with_lifecycle: domContentLoaded received");
    };
    let load_fut = async {
        load_events.next().await;
        debug!("navigate_with_lifecycle: load event received");
    };
    tokio::join!(dom_fut, load_fut);
    debug!("navigate_with_lifecycle: domContentLoaded and load done");

    if let Some(timeout) = network_idle_timeout {
        debug!("navigate_with_lifecycle: racing networkIdle against {:?}", timeout);
        tokio::select! {
            result = wait_lifecycle_event(page, "networkIdle") => {
                result?;
                debug!("navigate_with_lifecycle: networkIdle fired");
            }
            _ = tokio::time::sleep(timeout) => {
                debug!("navigate_with_lifecycle: networkIdle timeout, proceeding");
            }
        }
    }

    debug!("navigate_with_lifecycle: complete");
    Ok(())
}

/// Wait for a specific lifecycle event name.
async fn wait_lifecycle_event(
    page: &Page,
    event_name: &str,
) -> Result<(), chromiumoxide::error::CdpError> {
    use chromiumoxide::cdp::browser_protocol::page::EventLifecycleEvent;
    debug!("wait_lifecycle_event: registering listener for {}", event_name);
    let mut events = page.event_listener::<EventLifecycleEvent>().await?;
    debug!("wait_lifecycle_event: polling for {}", event_name);
    while let Some(ev) = events.next().await {
        debug!("wait_lifecycle_event: received lifecycle event name={}", ev.name);
        if ev.name == event_name {
            debug!("wait_lifecycle_event: matched {}", event_name);
            return Ok(());
        }
    }
    debug!("wait_lifecycle_event: stream ended without {}", event_name);
    Ok(())
}
