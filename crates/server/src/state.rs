//! Shared application state attached to every request via
//! [`axum::extract::State`].
//!
//! `AppState` is `Clone + Send + Sync` and cheap (every field is an
//! `Arc<_>` or trivially copyable).

use std::sync::Arc;
use std::time::Instant;

use engine::LibreOfficeEngine;
use tokio::sync::Semaphore;

use crate::ServerConfig;
use crate::backend::PdfBackend;

/// Per-process server state.
#[derive(Clone)]
pub struct AppState {
    /// PDF rendering backend (production = chromium).
    pub chromium: Arc<dyn PdfBackend>,
    /// LibreOffice engine; `None` in tests that don't need it.
    pub libreoffice: Option<Arc<LibreOfficeEngine>>,
    /// Outer concurrency cap.
    pub sem: Arc<Semaphore>,
    /// Resolved server config (for body limits, paths, etc.).
    pub config: Arc<ServerConfig>,
    /// Process start time, for `/health` uptime reporting.
    pub started_at: Instant,
}

impl AppState {
    /// Build a new state given concrete components.
    pub fn new(
        chromium: Arc<dyn PdfBackend>,
        libreoffice: Option<Arc<LibreOfficeEngine>>,
        config: ServerConfig,
    ) -> Self {
        let sem = Arc::new(Semaphore::new(config.concurrency));
        Self {
            chromium,
            libreoffice,
            sem,
            config: Arc::new(config),
            started_at: Instant::now(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use static_assertions::assert_impl_all;

    assert_impl_all!(AppState: Clone, Send, Sync);
}
