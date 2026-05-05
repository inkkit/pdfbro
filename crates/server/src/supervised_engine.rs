//! Supervised engine wrapper with lazy/eager start and idle shutdown.
//!
//! This module provides wrappers around ChromiumEngine and LibreOfficeEngine
//! that implement:
//! - Lazy start: Engine starts on first request (default: eager start at server startup)
//! - Idle shutdown: Engine automatically shuts down after period of inactivity

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};

use engine::{BrowserConfig, ChromiumEngine, LibreOfficeConfig, LibreOfficeEngine};
use engine::{EngineError, EngineResult};
use tokio::sync::Mutex;
use tokio::time::interval;
use tracing::{info, warn};

/// Wrapper around ChromiumEngine with auto-start and idle shutdown.
#[derive(Clone)]
pub struct SupervisedChromiumEngine {
    inner: Arc<SupervisedEngineInner<ChromiumEngine, BrowserConfig>>,
}

struct SupervisedEngineInner<E, C> {
    /// The engine instance, created lazily if lazy_start is true
    engine: Mutex<Option<E>>,
    /// Configuration for creating the engine
    config: C,
    /// Whether to use lazy initialization (start on first request)
    lazy_start: bool,
    /// Idle timeout duration
    idle_timeout: Option<Duration>,
    /// Last activity timestamp (seconds since epoch)
    last_activity: AtomicU64,
    /// Whether the engine is currently running
    is_running: AtomicBool,
}

impl SupervisedChromiumEngine {
    /// Create a new supervised engine wrapper.
    pub fn new(config: BrowserConfig) -> Self {
        let lazy_start = config.lazy_start;
        let idle_timeout = config.idle_shutdown_timeout;

        Self {
            inner: Arc::new(SupervisedEngineInner {
                engine: Mutex::new(None),
                config,
                lazy_start,
                idle_timeout,
                last_activity: AtomicU64::new(0),
                is_running: AtomicBool::new(false), // Will be set to true after eager start or lazy start
            }),
        }
    }

    /// Eagerly start the engine at server startup.
    pub async fn start(&self) -> EngineResult<()> {
        let mut guard = self.inner.engine.lock().await;
        if guard.is_none() {
            info!("Starting Chromium engine");
            let engine = ChromiumEngine::launch_with(self.inner.config.clone()).await?;
            *guard = Some(engine);
            self.inner.is_running.store(true, Ordering::SeqCst);
            self.update_activity();
            info!("Chromium engine started successfully");
        }
        Ok(())
    }

    /// Get or create the engine, starting it if necessary (lazy init).
    async fn get_engine(&self) -> EngineResult<tokio::sync::MutexGuard<'_, Option<ChromiumEngine>>> {
        let mut guard = self.inner.engine.lock().await;

        if guard.is_none() && self.inner.lazy_start {
            info!("Lazy-starting Chromium engine on first request");
            let engine = ChromiumEngine::launch_with(self.inner.config.clone()).await?;
            *guard = Some(engine);
            self.inner.is_running.store(true, Ordering::SeqCst);
            self.update_activity();
            info!("Chromium engine lazy-started successfully");
        }

        Ok(guard)
    }

    /// Update last activity timestamp.
    fn update_activity(&self) {
        let now = Instant::now().elapsed().as_secs();
        self.inner.last_activity.store(now, Ordering::SeqCst);
    }

    /// Start the idle shutdown monitor task.
    pub fn start_idle_monitor(&self) {
        if let Some(timeout) = self.inner.idle_timeout {
            let inner = Arc::clone(&self.inner);
            tokio::spawn(async move {
                let mut ticker = interval(Duration::from_secs(30)); // Check every 30s
                loop {
                    ticker.tick().await;
                    
                    if !inner.is_running.load(Ordering::SeqCst) {
                        continue; // Engine not running, nothing to do
                    }
                    
                    let last = inner.last_activity.load(Ordering::SeqCst);
                    let now = Instant::now().elapsed().as_secs();
                    let idle_duration = Duration::from_secs(now.saturating_sub(last));
                    
                    if idle_duration >= timeout {
                        warn!(
                            idle_seconds = idle_duration.as_secs(),
                            "Chromium engine idle timeout reached, shutting down"
                        );
                        
                        let mut guard = inner.engine.lock().await;
                        if let Some(engine) = guard.take() {
                            // We can't easily await shutdown here since we're in a task
                            // Mark as not running - actual shutdown happens on drop
                            drop(engine); // This triggers shutdown
                        }
                        inner.is_running.store(false, Ordering::SeqCst);
                        info!("Chromium engine shut down due to idle timeout");
                    }
                }
            });
        }
    }

    /// Check if the engine is healthy.
    ///
    /// Only probes an already-running engine — does NOT trigger lazy start.
    pub async fn healthy(&self) -> bool {
        if !self.inner.is_running.load(Ordering::SeqCst) {
            return false;
        }
        let guard = self.inner.engine.lock().await;
        match guard.as_ref() {
            Some(engine) => engine.healthy().await,
            None => false,
        }
    }

    /// HTML to PDF conversion.
    pub async fn html_to_pdf(
        &self,
        html: &str,
        base_url: Option<&str>,
        opts: &engine::PdfOptions,
        request: &engine::RequestContext,
    ) -> EngineResult<Vec<u8>> {
        self.update_activity();
        let guard = self.get_engine().await?;
        match guard.as_ref() {
            Some(engine) => engine.html_to_pdf(html, base_url, opts, request).await,
            None => Err(EngineError::Internal("Chromium engine not available".into())),
        }
    }

    /// URL to PDF conversion.
    pub async fn url_to_pdf(
        &self,
        url: &str,
        opts: &engine::PdfOptions,
        request: &engine::RequestContext,
    ) -> EngineResult<Vec<u8>> {
        self.update_activity();
        let guard = self.get_engine().await?;
        match guard.as_ref() {
            Some(engine) => engine.url_to_pdf(url, opts, request).await,
            None => Err(EngineError::Internal("Chromium engine not available".into())),
        }
    }

    /// Markdown to PDF conversion.
    pub async fn markdown_to_pdf(
        &self,
        markdown: &str,
        opts: &engine::PdfOptions,
        request: &engine::RequestContext,
    ) -> EngineResult<Vec<u8>> {
        self.update_activity();
        let guard = self.get_engine().await?;
        match guard.as_ref() {
            Some(engine) => engine.markdown_to_pdf(markdown, opts, request).await,
            None => Err(EngineError::Internal("Chromium engine not available".into())),
        }
    }

    /// HTML to screenshot conversion.
    pub async fn html_to_screenshot(
        &self,
        html: &str,
        opts: &engine::chromium::screenshot::ScreenshotOptions,
    ) -> EngineResult<Vec<u8>> {
        self.update_activity();
        let guard = self.get_engine().await?;
        match guard.as_ref() {
            Some(engine) => engine::chromium::screenshot::html_to_screenshot(engine, html, opts).await,
            None => Err(EngineError::Internal("Chromium engine not available".into())),
        }
    }

    /// URL to screenshot conversion.
    pub async fn url_to_screenshot(
        &self,
        url: &str,
        opts: &engine::chromium::screenshot::ScreenshotOptions,
    ) -> EngineResult<Vec<u8>> {
        self.update_activity();
        let guard = self.get_engine().await?;
        match guard.as_ref() {
            Some(engine) => engine::chromium::screenshot::url_to_screenshot(engine, url, opts).await,
            None => Err(EngineError::Internal("Chromium engine not available".into())),
        }
    }

    /// Returns true if the Chromium engine is currently running.
    pub fn is_running(&self) -> bool {
        self.inner.is_running.load(Ordering::SeqCst)
    }

    /// Seconds since this engine last handled a request. Returns 0 if never used.
    pub fn idle_secs(&self) -> u64 {
        let last = self.inner.last_activity.load(Ordering::SeqCst);
        if last == 0 { return 0; }
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        now.saturating_sub(last)
    }

    /// Shutdown the engine.
    pub async fn shutdown(&self) {
        let mut guard = self.inner.engine.lock().await;
        if let Some(engine) = guard.take() {
            drop(engine); // Triggers shutdown via ChromiumEngine's Drop
        }
        self.inner.is_running.store(false, Ordering::SeqCst);
    }
}

/// Wrapper around LibreOfficeEngine with lazy/eager start and idle shutdown.
#[derive(Clone)]
pub struct SupervisedLibreOfficeEngine {
    inner: Arc<SupervisedEngineInner<LibreOfficeEngine, LibreOfficeConfig>>,
}

impl SupervisedLibreOfficeEngine {
    /// Create a new supervised LibreOffice engine wrapper.
    pub fn new(config: LibreOfficeConfig) -> Self {
        let lazy_start = config.lazy_start;
        let idle_timeout = config.idle_shutdown_timeout;

        Self {
            inner: Arc::new(SupervisedEngineInner {
                engine: Mutex::new(None),
                config,
                lazy_start,
                idle_timeout,
                last_activity: AtomicU64::new(0),
                is_running: AtomicBool::new(false), // Will be set to true after eager start or lazy start
            }),
        }
    }

    /// Eagerly start the engine at server startup.
    pub async fn start(&self) -> EngineResult<()> {
        let mut guard = self.inner.engine.lock().await;
        if guard.is_none() {
            info!("Starting LibreOffice engine");
            let engine = LibreOfficeEngine::launch(self.inner.config.clone()).await?;
            *guard = Some(engine);
            self.inner.is_running.store(true, Ordering::SeqCst);
            self.update_activity();
            info!("LibreOffice engine started successfully");
        }
        Ok(())
    }

    /// Get or create the engine, starting it if necessary (lazy init).
    async fn get_engine(&self) -> EngineResult<tokio::sync::MutexGuard<'_, Option<LibreOfficeEngine>>> {
        let mut guard = self.inner.engine.lock().await;

        if guard.is_none() && self.inner.lazy_start {
            info!("Lazy-starting LibreOffice engine on first request");
            let engine = LibreOfficeEngine::launch(self.inner.config.clone()).await?;
            *guard = Some(engine);
            self.inner.is_running.store(true, Ordering::SeqCst);
            self.update_activity();
            info!("LibreOffice engine lazy-started successfully");
        }

        Ok(guard)
    }

    /// Update last activity timestamp.
    fn update_activity(&self) {
        let now = Instant::now().elapsed().as_secs();
        self.inner.last_activity.store(now, Ordering::SeqCst);
    }

    /// Start the idle shutdown monitor task.
    pub fn start_idle_monitor(&self) {
        if let Some(timeout) = self.inner.idle_timeout {
            let inner = Arc::clone(&self.inner);
            tokio::spawn(async move {
                let mut ticker = interval(Duration::from_secs(30));
                loop {
                    ticker.tick().await;
                    
                    if !inner.is_running.load(Ordering::SeqCst) {
                        continue;
                    }
                    
                    let last = inner.last_activity.load(Ordering::SeqCst);
                    let now = Instant::now().elapsed().as_secs();
                    let idle_duration = Duration::from_secs(now.saturating_sub(last));
                    
                    if idle_duration >= timeout {
                        warn!(
                            idle_seconds = idle_duration.as_secs(),
                            "LibreOffice engine idle timeout reached, shutting down"
                        );
                        
                        let mut guard = inner.engine.lock().await;
                        if let Some(engine) = guard.take() {
                            drop(engine);
                        }
                        inner.is_running.store(false, Ordering::SeqCst);
                        info!("LibreOffice engine shut down due to idle timeout");
                    }
                }
            });
        }
    }

    /// Check if the engine is healthy.
    /// Check if the engine is healthy.
    ///
    /// Only probes an already-running engine — does NOT trigger lazy start.
    pub async fn healthy(&self) -> bool {
        if !self.inner.is_running.load(Ordering::SeqCst) {
            return false;
        }
        let guard = self.inner.engine.lock().await;
        match guard.as_ref() {
            Some(engine) => engine.healthy().await,
            None => false,
        }
    }

    /// Convert a file to PDF.
    pub async fn convert(
        &self,
        input: &std::path::Path,
        opts: &engine::OfficeOptions,
    ) -> EngineResult<Vec<u8>> {
        self.update_activity();
        let guard = self.get_engine().await?;
        match guard.as_ref() {
            Some(engine) => engine.convert(input, opts).await,
            None => Err(EngineError::Internal("LibreOffice engine not available".into())),
        }
    }

    /// Returns true if the LibreOffice engine is currently running.
    pub fn is_running(&self) -> bool {
        self.inner.is_running.load(Ordering::SeqCst)
    }

    /// Seconds since this engine last handled a request. Returns 0 if never used.
    pub fn idle_secs(&self) -> u64 {
        let last = self.inner.last_activity.load(Ordering::SeqCst);
        if last == 0 { return 0; }
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        now.saturating_sub(last)
    }

    /// Convert many files to PDFs in parallel.
    pub async fn convert_many(
        &self,
        inputs: &[std::path::PathBuf],
        opts: &engine::OfficeOptions,
    ) -> EngineResult<Vec<Vec<u8>>> {
        self.update_activity();
        let guard = self.get_engine().await?;
        match guard.as_ref() {
            Some(engine) => engine.convert_many(inputs, opts).await,
            None => Err(EngineError::Internal("LibreOffice engine not available".into())),
        }
    }

    /// Drain in-flight conversions, call `LibreOfficeEngine::shutdown()` on
    /// the inner engine if present, and clear the running flag. This
    /// supersedes the supervised wrapper's idle-shutdown drop path during
    /// graceful server shutdown — calling `LibreOfficeEngine::shutdown()`
    /// guarantees the worker thread runs `mem::forget(office)` to bypass
    /// LO >= 6.5's atexit teardown segfault.
    pub async fn shutdown(&self) {
        let mut guard = self.inner.engine.lock().await;
        if let Some(engine) = guard.take() {
            if let Err(e) = engine.shutdown().await {
                tracing::warn!(error = %e, "LibreOfficeEngine shutdown returned error");
            }
        }
        self.inner.is_running.store(false, Ordering::SeqCst);
    }
}
