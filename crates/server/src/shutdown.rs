//! Graceful shutdown plumbing.
//!
//! Awaits either `SIGINT` (Ctrl-C) or `SIGTERM` and resolves; passed to
//! `axum::serve(...).with_graceful_shutdown(...)`. After this future
//! resolves, axum stops accepting new connections and waits for in-flight
//! requests up to a deadline before exiting.

use std::time::Duration;

/// Default in-flight drain budget after a shutdown signal.
pub const DEFAULT_DRAIN: Duration = Duration::from_secs(30);

/// Resolves on the first of `SIGINT` or `SIGTERM`.
pub async fn shutdown_signal() {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };

    #[cfg(unix)]
    let terminate = async {
        if let Ok(mut sig) =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        {
            sig.recv().await;
        }
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => tracing::info!("received SIGINT, beginning graceful shutdown"),
        _ = terminate => tracing::info!("received SIGTERM, beginning graceful shutdown"),
    }
}
