//! Pluggable PDF backend trait used by the chromium routes.
//!
//! Production code wires [`ChromiumBackend`] (a wrapper around
//! [`supervised_engine::SupervisedChromiumEngine`]) into [`crate::AppState`]; tests provide a
//! mock implementation that records inputs and returns canned outputs.
//!
//! Splitting the chromium-shaped surface into a trait lets the router
//! tests in `tests/router.rs` exercise every code path without spawning
//! Chrome.

use async_trait::async_trait;
use engine::{EngineResult, PdfOptions};

#[cfg(feature = "chromium")]
use engine::{RequestContext, ScreenshotOptions};
#[cfg(feature = "chromium")]
use crate::supervised_engine::SupervisedChromiumEngine;

/// Minimal trait surface mirroring the parts of [`ChromiumEngine`] that
/// the server invokes from request handlers.
#[async_trait]
pub trait PdfBackend: Send + Sync + 'static {
    /// Render an HTML string to PDF bytes.
    #[cfg(feature = "chromium")]
    async fn html_to_pdf(
        &self,
        html: &str,
        base_url: Option<&str>,
        opts: &PdfOptions,
        ctx: &RequestContext,
    ) -> EngineResult<Vec<u8>>;

    /// Navigate to a URL and render to PDF bytes.
    #[cfg(feature = "chromium")]
    async fn url_to_pdf(
        &self,
        url: &str,
        opts: &PdfOptions,
        ctx: &RequestContext,
    ) -> EngineResult<Vec<u8>>;

    /// Render Markdown to PDF.
    #[cfg(feature = "chromium")]
    async fn markdown_to_pdf(
        &self,
        markdown: &str,
        opts: &PdfOptions,
        ctx: &RequestContext,
    ) -> EngineResult<Vec<u8>>;

    /// Liveness probe.
    async fn healthy(&self) -> bool;

    /// Non-blocking liveness check based on an atomic flag — safe to call from
    /// the console sampler without competing for the engine's internal mutex.
    fn is_alive(&self) -> bool { true }

    /// Seconds since the last conversion handled by this engine. Returns 0 if never used.
    fn idle_secs(&self) -> u64 { 0 }

    /// Render HTML to screenshot image.
    #[cfg(feature = "chromium")]
    async fn html_to_screenshot(&self, html: &str, opts: &ScreenshotOptions) -> EngineResult<Vec<u8>>;

    /// Navigate to URL and capture screenshot.
    #[cfg(feature = "chromium")]
    async fn url_to_screenshot(&self, url: &str, opts: &ScreenshotOptions) -> EngineResult<Vec<u8>>;
}

/// Production [`PdfBackend`] backed by the supervised Chromium engine.
#[cfg(feature = "chromium")]
#[derive(Clone)]
pub struct ChromiumBackend {
    inner: SupervisedChromiumEngine,
}

#[cfg(feature = "chromium")]
impl ChromiumBackend {
    /// Wrap an existing [`SupervisedChromiumEngine`] handle.
    pub fn new(engine: SupervisedChromiumEngine) -> Self {
        Self {
            inner: engine,
        }
    }

    /// Borrow the inner engine (e.g. for shutdown).
    pub fn engine(&self) -> &SupervisedChromiumEngine {
        &self.inner
    }
}

#[cfg(feature = "chromium")]
#[async_trait]
impl PdfBackend for ChromiumBackend {
    async fn html_to_pdf(
        &self,
        html: &str,
        base_url: Option<&str>,
        opts: &PdfOptions,
        ctx: &RequestContext,
    ) -> EngineResult<Vec<u8>> {
        self.inner.html_to_pdf(html, base_url, opts, ctx).await
    }

    async fn url_to_pdf(
        &self,
        url: &str,
        opts: &PdfOptions,
        ctx: &RequestContext,
    ) -> EngineResult<Vec<u8>> {
        self.inner.url_to_pdf(url, opts, ctx).await
    }

    async fn markdown_to_pdf(
        &self,
        markdown: &str,
        opts: &PdfOptions,
        ctx: &RequestContext,
    ) -> EngineResult<Vec<u8>> {
        self.inner.markdown_to_pdf(markdown, opts, ctx).await
    }

    async fn healthy(&self) -> bool {
        self.inner.healthy().await
    }

    fn is_alive(&self) -> bool {
        self.inner.is_running()
    }

    fn idle_secs(&self) -> u64 {
        self.inner.idle_secs()
    }

    async fn html_to_screenshot(&self, html: &str, opts: &ScreenshotOptions) -> EngineResult<Vec<u8>> {
        self.inner.html_to_screenshot(html, opts).await
    }

    async fn url_to_screenshot(&self, url: &str, opts: &ScreenshotOptions) -> EngineResult<Vec<u8>> {
        self.inner.url_to_screenshot(url, opts).await
    }
}
