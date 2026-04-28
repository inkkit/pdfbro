//! Logging helpers for the engine crate.
//!
//! Provides utility functions for structured logging of conversion
//! results, matching the patterns described in `docs/specs/35-logging.md`.

use std::time::Duration;

use tracing::{info, error};

/// Helper to log conversion result with consistent fields.
///
/// # Examples
///
/// ```
/// use engine::logging::log_conversion_result;
/// use std::time::Duration;
///
/// log_conversion_result(
///     "chromium",
///     "url_to_pdf",
///     Duration::from_millis(542),
///     true,
///     None,
/// );
/// ```
pub fn log_conversion_result(
    engine: &str,
    operation: &str,
    duration: Duration,
    success: bool,
    error: Option<&str>,
) {
    if success {
        info!(
            engine = engine,
            operation = operation,
            duration_ms = duration.as_millis() as u64,
            "Conversion completed successfully"
        );
    } else {
        error!(
            engine = engine,
            operation = operation,
            duration_ms = duration.as_millis() as u64,
            error = error.unwrap_or("unknown"),
            "Conversion failed"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn log_conversion_result_emits_events() {
        // Initialize a test subscriber that captures events
        let _ = tracing_subscriber::fmt()
            .with_env_filter(tracing_subscriber::EnvFilter::new("trace"))
            .try_init();

        // These should not panic
        log_conversion_result(
            "chromium",
            "url_to_pdf",
            Duration::from_millis(100),
            true,
            None,
        );

        log_conversion_result(
            "libreoffice",
            "convert",
            Duration::from_millis(200),
            false,
            Some("timeout"),
        );
    }
}
