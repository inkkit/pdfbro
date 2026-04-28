//! Pluggable PDF backend trait used by the chromium routes.
//!
//! Production code wires [`ChromiumBackend`] (a wrapper around
//! [`engine::ChromiumEngine`]) into [`crate::AppState`]; tests provide a
//! mock implementation that records inputs and returns canned outputs.
//!
//! Splitting the chromium-shaped surface into a trait lets the router
//! tests in `tests/router.rs` exercise every code path without spawning
//! Chrome.

use std::sync::Arc;

use async_trait::async_trait;
use engine::{ChromiumEngine, EngineResult, PdfOptions, RequestContext, ScreenshotOptions};

/// Minimal trait surface mirroring the parts of [`ChromiumEngine`] that
/// the server invokes from request handlers.
#[async_trait]
pub trait PdfBackend: Send + Sync + 'static {
    /// Render an HTML string to PDF bytes.
    async fn html_to_pdf(
        &self,
        html: &str,
        base_url: Option<&str>,
        opts: &PdfOptions,
        ctx: &RequestContext,
    ) -> EngineResult<Vec<u8>>;

    /// Navigate to a URL and render to PDF bytes.
    async fn url_to_pdf(
        &self,
        url: &str,
        opts: &PdfOptions,
        ctx: &RequestContext,
    ) -> EngineResult<Vec<u8>>;

    /// Render Markdown to PDF.
    async fn markdown_to_pdf(
        &self,
        markdown: &str,
        opts: &PdfOptions,
        ctx: &RequestContext,
    ) -> EngineResult<Vec<u8>>;

    /// Liveness probe.
    async fn healthy(&self) -> bool;

    /// Render HTML to screenshot image.
    async fn html_to_screenshot(&self, html: &str, opts: &ScreenshotOptions) -> EngineResult<Vec<u8>>;

    /// Navigate to URL and capture screenshot.
    async fn url_to_screenshot(&self, url: &str, opts: &ScreenshotOptions) -> EngineResult<Vec<u8>>;
}

/// Production [`PdfBackend`] backed by the real Chromium engine.
#[derive(Clone)]
pub struct ChromiumBackend {
    inner: Arc<ChromiumEngine>,
}

impl ChromiumBackend {
    /// Wrap an existing [`ChromiumEngine`] handle.
    pub fn new(engine: ChromiumEngine) -> Self {
        Self {
            inner: Arc::new(engine),
        }
    }

    /// Borrow the inner engine (e.g. for shutdown).
    pub fn engine(&self) -> &ChromiumEngine {
        &self.inner
    }
}

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

    async fn html_to_screenshot(&self, html: &str, opts: &ScreenshotOptions) -> EngineResult<Vec<u8>> {
        use engine::chromium::screenshot::html_to_screenshot;
        html_to_screenshot(&self.inner, html, opts).await
    }

    async fn url_to_screenshot(&self, url: &str, opts: &ScreenshotOptions) -> EngineResult<Vec<u8>> {
        use engine::chromium::screenshot::url_to_screenshot;
        url_to_screenshot(&self.inner, url, opts).await
    }
}
