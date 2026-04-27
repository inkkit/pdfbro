//! Chromium-backed PDF engine.
//!
//! Implementation of `docs/specs/11-engine-chromium.md`. Drives a real
//! Chrome / Chromium instance via the Chrome DevTools Protocol
//! ([`chromiumoxide`]) to render HTML, URLs, or Markdown to PDF byte
//! streams.
//!
//! See the spec for the full public-API contract; this module exposes
//! only [`ChromiumEngine`], [`RequestContext`], and [`Cookie`].

mod launch;
mod markdown;
mod pdf_params;
mod render;
mod wait;

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use chromiumoxide::Browser;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;

use crate::types::{BrowserConfig, EngineError, EngineResult, PdfOptions};

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// One Chromium browser instance shared across many concurrent renders.
///
/// Cheap to clone — internally an [`Arc`] of the browser handle.
///
/// ## Example
///
/// ```no_run
/// use engine::{ChromiumEngine, PdfOptions, RequestContext};
///
/// # async fn run() -> engine::EngineResult<()> {
/// let engine = ChromiumEngine::launch().await?;
/// let pdf = engine
///     .html_to_pdf(
///         "<h1>hello</h1>",
///         None,
///         &PdfOptions::default(),
///         &RequestContext::default(),
///     )
///     .await?;
/// engine.shutdown().await?;
/// # let _ = pdf;
/// # Ok(())
/// # }
/// ```
#[derive(Clone)]
pub struct ChromiumEngine {
    inner: Arc<Inner>,
}

pub(crate) struct Inner {
    /// `None` once `shutdown` has run; renders observe `None` and return
    /// `EngineError::Internal("engine shut down")`.
    pub(crate) browser: Mutex<Option<Browser>>,
    /// Set to `true` at the start of `shutdown`; used to disambiguate
    /// CDP errors caused by intentional teardown from real CDP errors.
    pub(crate) is_shutdown: AtomicBool,
    /// Handle to the chromiumoxide event-loop task; aborted on shutdown.
    pub(crate) handler_task: std::sync::Mutex<Option<JoinHandle<()>>>,
    /// Frozen browser-level configuration used for every render.
    pub(crate) config: BrowserConfig,
}

/// Per-render context describing user-agent, headers, cookies, and
/// fail-on-status policy applied before a render.
///
/// All fields are optional: an empty [`RequestContext::default`] means
/// "use the page defaults".
#[derive(Debug, Clone, Default)]
pub struct RequestContext {
    /// Override for the page's `User-Agent` header. `None` keeps the
    /// browser default.
    pub user_agent: Option<String>,
    /// Extra HTTP headers attached to every request issued by the page.
    pub extra_headers: HashMap<String, String>,
    /// Cookies installed on the page before navigation/setContent.
    pub cookies: Vec<Cookie>,
    /// HTTP statuses (on the main-frame response) that fail the render
    /// with [`crate::EngineError::Navigation`]. Empty means no statuses
    /// fail.
    pub fail_on_status: Vec<u16>,
}

/// A single cookie installed on the page before a render.
///
/// Mirrors the relevant subset of Chrome's `Network.setCookie` parameters.
#[derive(Debug, Clone)]
pub struct Cookie {
    /// Cookie name.
    pub name: String,
    /// Cookie value.
    pub value: String,
    /// Domain the cookie applies to. `None` means "the page origin".
    pub domain: Option<String>,
    /// Path the cookie applies to. `None` means `/`.
    pub path: Option<String>,
    /// `Secure` flag.
    pub secure: bool,
    /// `HttpOnly` flag.
    pub http_only: bool,
}

// ---------------------------------------------------------------------------
// Public methods
// ---------------------------------------------------------------------------

impl ChromiumEngine {
    /// Launch a new Chrome / Chromium instance with default
    /// [`BrowserConfig`].
    ///
    /// # Errors
    ///
    /// Returns [`EngineError::ChromeNotFound`] if no executable can be
    /// located via `BrowserConfig::executable`, the `BROWSER_PATH`
    /// environment variable, `$PATH`, or platform-typical defaults; or
    /// [`EngineError::ChromeLaunch`] if the executable was found but
    /// failed to start.
    pub async fn launch() -> EngineResult<Self> {
        Self::launch_with(BrowserConfig::default()).await
    }

    /// Launch a new Chrome / Chromium instance with explicit
    /// [`BrowserConfig`].
    ///
    /// # Errors
    ///
    /// See [`ChromiumEngine::launch`].
    pub async fn launch_with(config: BrowserConfig) -> EngineResult<Self> {
        launch::launch_with(config).await
    }

    /// Render an HTML string to a PDF byte stream.
    ///
    /// `base_url`, when `Some`, is used as the document's base URL so
    /// relative `<img>`, `<link>`, etc. resolve against it. When
    /// `None`, the content is rendered against `about:blank`.
    ///
    /// # Errors
    ///
    /// Returns [`EngineError::InvalidOption`] if `opts.validate()` rejects
    /// the option set; [`EngineError::Cdp`] if a CDP call fails;
    /// [`EngineError::Navigation`] if the base URL fails to load or
    /// `request.fail_on_status` matches; [`EngineError::Timeout`] if the
    /// render exceeds `BrowserConfig::timeout`.
    pub async fn html_to_pdf(
        &self,
        html: &str,
        base_url: Option<&str>,
        opts: &PdfOptions,
        request: &RequestContext,
    ) -> EngineResult<Vec<u8>> {
        render::html_to_pdf(self, html, base_url, opts, request).await
    }

    /// Navigate to `url` and render the resulting page to a PDF byte
    /// stream.
    ///
    /// # Errors
    ///
    /// See [`ChromiumEngine::html_to_pdf`].
    pub async fn url_to_pdf(
        &self,
        url: &str,
        opts: &PdfOptions,
        request: &RequestContext,
    ) -> EngineResult<Vec<u8>> {
        render::url_to_pdf(self, url, opts, request).await
    }

    /// Convert a Markdown string to a PDF byte stream.
    ///
    /// CommonMark plus tables, strikethrough, and task lists are
    /// supported (per [`pulldown_cmark::Options::all`]). The rendered
    /// HTML is wrapped in a small built-in stylesheet and then handed
    /// off to [`ChromiumEngine::html_to_pdf`].
    ///
    /// # Errors
    ///
    /// See [`ChromiumEngine::html_to_pdf`].
    pub async fn markdown_to_pdf(
        &self,
        markdown_input: &str,
        opts: &PdfOptions,
        request: &RequestContext,
    ) -> EngineResult<Vec<u8>> {
        let html = markdown::render(markdown_input);
        self.html_to_pdf(&html, None, opts, request).await
    }

    /// Best-effort liveness probe.
    ///
    /// Returns `true` iff the browser process responds to
    /// `Browser.getVersion` within `BrowserConfig::timeout`. Always
    /// returns `false` after [`ChromiumEngine::shutdown`].
    pub async fn healthy(&self) -> bool {
        if self.inner.is_shutdown.load(Ordering::SeqCst) {
            return false;
        }
        let timeout = self.inner.config.timeout;
        let guard = match tokio::time::timeout(timeout, self.inner.browser.lock()).await {
            Ok(g) => g,
            Err(_) => return false,
        };
        let Some(browser) = guard.as_ref() else {
            return false;
        };
        tokio::time::timeout(timeout, browser.version())
            .await
            .map(|r| r.is_ok())
            .unwrap_or(false)
    }

    /// Close the browser. Idempotent.
    ///
    /// In-flight renders observe the shutdown and resolve to
    /// [`EngineError::Internal`] (`"engine shut down"`); subsequent
    /// renders fail the same way. Calling `shutdown` on a separate
    /// clone is a no-op.
    ///
    /// # Errors
    ///
    /// Returns [`EngineError::Cdp`] if Chrome reports an error while
    /// closing. The browser is dropped regardless.
    pub async fn shutdown(self) -> EngineResult<()> {
        // Mark shutdown first so concurrent renders can interpret CDP
        // errors as intentional teardown.
        let was_running = !self.inner.is_shutdown.swap(true, Ordering::SeqCst);

        // Take the browser out of the option and drop it.
        let mut close_err: Option<chromiumoxide::error::CdpError> = None;
        {
            let mut guard = self.inner.browser.lock().await;
            if let Some(mut browser) = guard.take() {
                if let Err(e) = browser.close().await {
                    close_err = Some(e);
                }
                // Drop the browser explicitly to terminate the chrome
                // process even if `close` failed.
                drop(browser);
            }
        }

        // Abort the chromiumoxide event-loop task.
        if let Ok(mut g) = self.inner.handler_task.lock()
            && let Some(handle) = g.take()
        {
            handle.abort();
        }

        if was_running && let Some(e) = close_err {
            return Err(EngineError::Cdp(e.to_string()));
        }
        Ok(())
    }
}

impl std::fmt::Debug for ChromiumEngine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ChromiumEngine")
            .field("shutdown", &self.inner.is_shutdown.load(Ordering::SeqCst))
            .field("config", &self.inner.config)
            .finish()
    }
}

// ---------------------------------------------------------------------------
// Internal helpers exposed to submodules
// ---------------------------------------------------------------------------

impl ChromiumEngine {
    /// Construct from already-launched browser parts. Internal helper
    /// used by `launch`.
    pub(crate) fn from_parts(
        browser: Browser,
        handler_task: JoinHandle<()>,
        config: BrowserConfig,
    ) -> Self {
        Self {
            inner: Arc::new(Inner {
                browser: Mutex::new(Some(browser)),
                is_shutdown: AtomicBool::new(false),
                handler_task: std::sync::Mutex::new(Some(handler_task)),
                config,
            }),
        }
    }

    pub(crate) fn inner(&self) -> &Inner {
        &self.inner
    }

    /// Map a CDP error to the engine's error model, accounting for
    /// intentional shutdown.
    pub(crate) fn map_cdp_error(&self, err: chromiumoxide::error::CdpError) -> EngineError {
        if self.inner.is_shutdown.load(Ordering::SeqCst) {
            EngineError::Internal("engine shut down".into())
        } else {
            EngineError::Cdp(err.to_string())
        }
    }
}

// ---------------------------------------------------------------------------
// Trait assertions: the public type must be Send + Sync + Clone.
// ---------------------------------------------------------------------------

#[cfg(test)]
mod assertions {
    use super::*;
    use static_assertions::assert_impl_all;

    assert_impl_all!(ChromiumEngine: Send, Sync, Clone);
    assert_impl_all!(RequestContext: Send, Sync, Clone);
    assert_impl_all!(Cookie: Send, Sync, Clone);
}
