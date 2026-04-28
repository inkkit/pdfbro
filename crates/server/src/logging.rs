//! Structured logging initialization.
//!
//! Implements `docs/specs/35-logging.md`. Provides `init_logging()` which
//! configures `tracing-subscriber` for text or JSON output with support
//! for environment variable filtering and span events.

use tracing_subscriber::{
    self,
    fmt,
    EnvFilter,
    layer::SubscriberExt,
    util::SubscriberInitExt,
    fmt::format::FmtSpan,
};

/// Initialize the global tracing subscriber.
///
/// # Arguments
///
/// * `log_format` – `"text"` or `"json"` (anything else defaults to text).
/// * `log_level`  – fallback directive if `RUST_LOG` is not set.
///
/// Environment variables respected:
///
/// * `RUST_LOG` – `EnvFilter` directive (same syntax as `env_logger`).
/// * `FOLIO_LOG_SPAN_EVENTS` – if truthy (`1/true/yes`), span enter/exit
///   events are emitted.
///
/// # Errors
///
/// Returns `anyhow::Error` if the subscriber cannot be installed (e.g. it
/// was already initialized elsewhere).
pub fn init_logging(log_format: &str, log_level: &str) -> anyhow::Result<()> {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(log_level));

    let span_events = std::env::var("FOLIO_LOG_SPAN_EVENTS")
        .map(|v| is_truthy(&v))
        .unwrap_or(false);

    let fmt_layer = fmt::layer()
        .with_writer(std::io::stdout);

    let fmt_layer = if span_events {
        fmt_layer.with_span_events(FmtSpan::NEW | FmtSpan::CLOSE)
    } else {
        fmt_layer
    };

    match log_format {
        "json" => {
            tracing_subscriber::registry()
                .with(filter)
                .with(fmt_layer.json())
                .try_init()
                .map_err(|e| anyhow::anyhow!("logging already initialized: {e}"))?;
        }
        _ => {
            tracing_subscriber::registry()
                .with(filter)
                .with(fmt_layer)
                .try_init()
                .map_err(|e| anyhow::anyhow!("logging already initialized: {e}"))?;
        }
    }

    Ok(())
}

fn is_truthy(s: &str) -> bool {
    matches!(
        s.trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "on"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_truthy_recognizes_valid_values() {
        for v in &["1", "true", "TRUE", "yes", "on"] {
            assert!(is_truthy(v), "expected true for `{v}`");
        }
        for v in &["", "0", "false", "no", "off", "foo"] {
            assert!(!is_truthy(v), "expected false for `{v}`");
        }
    }
}
