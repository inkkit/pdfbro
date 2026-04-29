//! Shared application state attached to every request via
//! [`axum::extract::State`].
//!
//! `AppState` is `Clone + Send + Sync` and cheap (every field is an
//! `Arc<_>` or trivially copyable).

use std::sync::Arc;
use std::time::Instant;

#[cfg(feature = "libreoffice")]
use engine::LibreOfficeEngine;
use tokio::sync::Semaphore;

use crate::ServerConfig;
use crate::backend::PdfBackend;
use crate::metrics::FolioMetrics;
use crate::routes::batch_state::BatchStateManager;
use crate::webhook::WebhookQueue;
use prometheus;

/// Per-process server state.
#[derive(Clone)]
pub struct AppState {
    /// PDF rendering backend (production = chromium).
    pub chromium: Option<Arc<dyn PdfBackend>>,
    /// LibreOffice engine; `None` in tests that don't need it.
    #[cfg(feature = "libreoffice")]
    pub libreoffice: Option<Arc<LibreOfficeEngine>>,
    /// Outer concurrency cap.
    pub sem: Arc<Semaphore>,
    /// Resolved server config (for body limits, paths, etc).
    pub config: Arc<ServerConfig>,
    /// Process start time, for `/health` uptime reporting.
    pub started_at: Instant,
    /// Webhook job queue; `None` when webhook workers are not started.
    pub webhook_queue: Option<WebhookQueue>,
    /// Prometheus metrics for monitoring.
    pub metrics: Arc<FolioMetrics>,
    /// Batch state manager for batch API.
    pub batch_manager: Option<BatchStateManager>,
}

impl AppState {
    /// Build a new state given concrete components.
    pub fn new(
        chromium: Option<Arc<dyn PdfBackend>>,
        config: ServerConfig,
    ) -> Self {
        let sem = Arc::new(Semaphore::new(config.concurrency));
        // Use global metrics instance (registered once via Lazy)
        let metrics = Arc::new((*crate::metrics::METRICS).clone());
        Self {
            chromium,
            #[cfg(feature = "libreoffice")]
            libreoffice: None,
            sem,
            config: Arc::new(config),
            started_at: Instant::now(),
            webhook_queue: None,
            metrics,
            batch_manager: None,
        }
    }

    #[cfg(feature = "libreoffice")]
    /// Attach a LibreOffice engine.
    pub fn with_libreoffice(mut self, libreoffice: Option<Arc<LibreOfficeEngine>>) -> Self {
        self.libreoffice = libreoffice;
        self
    }

    /// Attach a webhook queue for async processing.
    pub fn with_webhook_queue(mut self, queue: WebhookQueue) -> Self {
        self.webhook_queue = Some(queue);
        self
    }

    /// Attach a batch state manager for batch API.
    pub fn with_batch_manager(mut self, manager: BatchStateManager) -> Self {
        self.batch_manager = Some(manager);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use static_assertions::assert_impl_all;

    assert_impl_all!(AppState: Clone, Send, Sync);
}
