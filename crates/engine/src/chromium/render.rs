//! Render orchestration for [`super::ChromiumEngine`].
//!
//! Implements `html_to_pdf` and `url_to_pdf` per
//! `docs/specs/11-engine-chromium.md`. The whole render — from page
//! creation to `Page.printToPDF` — is wrapped in a single
//! `tokio::time::timeout(BrowserConfig::timeout, ...)` so wait
//! conditions and slow networks are bounded uniformly.

use std::sync::{Arc, Mutex};

use chromiumoxide::Page;
use chromiumoxide::cdp::browser_protocol::emulation::SetEmulatedMediaParams;
use chromiumoxide::cdp::browser_protocol::network::{
    CookieParam, EventResponseReceived, Headers, ResourceType, SetExtraHttpHeadersParams,
    SetUserAgentOverrideParams,
};
use futures_util::StreamExt;

use crate::types::{EngineError, EngineResult, PdfOptions};

use crate::types::WaitCondition;

use super::pdf_params::{build_printtopdf_params, media_kind};
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
    apply_wait(&opts.wait)?;
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
    apply_request_context(engine, page, request).await?;

    // Set up the fail-on-status watcher only if requested.
    let captured = if request.fail_on_status.is_empty() {
        None
    } else {
        Some(spawn_main_frame_status_capture(page).await?)
    };

    page.goto(url).await.map_err(|e| navigation_error(url, e))?;

    if let Some((status, task)) = captured {
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

    apply_emulated_media(engine, page, opts).await?;
    apply_wait(&opts.wait)?;
    print_to_pdf(engine, page, opts).await
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Placeholder wait handler: only `WaitCondition::Load` is supported in
/// this commit. Other variants are implemented by the follow-up
/// `feat(engine/11): implement wait conditions` commit.
fn apply_wait(wait: &WaitCondition) -> EngineResult<()> {
    match wait {
        WaitCondition::Load => Ok(()),
        _ => Err(EngineError::Internal(
            "wait conditions other than Load are not yet implemented (spec 11 follow-up)".into(),
        )),
    }
}

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
    let mut b = CookieParam::builder()
        .name(c.name.clone())
        .value(c.value.clone())
        .secure(c.secure)
        .http_only(c.http_only);
    if let Some(d) = &c.domain {
        b = b.domain(d.clone());
    }
    if let Some(p) = &c.path {
        b = b.path(p.clone());
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
    page.execute(SetEmulatedMediaParams {
        media: Some(media_kind(opts.emulate_media).to_string()),
        features: None,
    })
    .await
    .map_err(|e| engine.map_cdp_error(e))?;
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
