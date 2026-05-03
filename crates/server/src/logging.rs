//! Structured logging initialization.
//!
//! Implements `docs/specs/35-logging.md`. Provides `init_logging()` which
//! configures `tracing-subscriber` for text or JSON output with support
//! for environment variable filtering and span events. Optionally enables
//! an OpenTelemetry OTLP trace exporter layer.

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
/// * `otel_enabled` – whether to register an OpenTelemetry trace layer.
/// * `otel_endpoint` – OTLP HTTP endpoint URL for trace export.
///
/// Environment variables respected:
///
/// * `RUST_LOG` – `EnvFilter` directive (same syntax as `env_logger`).
/// * `PDFBRO_LOG_SPAN_EVENTS` – if truthy (`1/true/yes`), span enter/exit
///   events are emitted.
///
/// # Errors
///
/// Returns `anyhow::Error` if the subscriber cannot be installed (e.g. it
/// was already initialized elsewhere) or the OTLP exporter fails to build.
pub fn init_logging(
    log_format: &str,
    log_level: &str,
    otel_enabled: bool,
    otel_endpoint: &str,
) -> anyhow::Result<()> {
    // When RUST_LOG is set, use it verbatim (user-controlled).
    // When using the default level, suppress chromiumoxide::handler at warn —
    // it logs benign "WS Invalid message" noise for CDP events not in its
    // protocol schema (harmless compatibility gap with newer Chrome versions).
    let filter = if let Ok(f) = EnvFilter::try_from_default_env() {
        f
    } else {
        EnvFilter::new(log_level)
            .add_directive("chromiumoxide::handler=error".parse().unwrap())
    };

    let span_events = std::env::var("PDFBRO_LOG_SPAN_EVENTS")
        .map(|v| is_truthy(&v))
        .unwrap_or(false);

    let fmt_layer = fmt::layer()
        .with_writer(std::io::stdout);

    let fmt_layer = if span_events {
        fmt_layer.with_span_events(FmtSpan::NEW | FmtSpan::CLOSE)
    } else {
        fmt_layer
    };

    if otel_enabled {
        init_otel_layer(otel_endpoint)?;
    }

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

/// Initialise the OpenTelemetry global tracer provider and a
/// `tracing-opentelemetry` layer.  Because the concrete layer type depends
/// on the exact SDK tracer type, we install it *before* the global
/// `tracing_subscriber` registry so that `init_logging` only needs to deal
/// with `Layer<Registry>` trait objects.
fn init_otel_layer(endpoint: &str) -> anyhow::Result<()> {
    use opentelemetry::trace::TracerProvider;
    use opentelemetry::KeyValue;
    use opentelemetry_otlp::WithExportConfig;
    use opentelemetry_sdk::trace::TracerProvider as SdkTracerProvider;
    use opentelemetry_sdk::Resource;

    let exporter = opentelemetry_otlp::SpanExporter::builder()
        .with_http()
        .with_endpoint(endpoint)
        .build()
        .map_err(|e| anyhow::anyhow!("failed to build OTLP span exporter: {e}"))?;

    let provider = SdkTracerProvider::builder()
        .with_batch_exporter(exporter, opentelemetry_sdk::runtime::Tokio)
        .with_resource(Resource::new([KeyValue::new(
            "service.name",
            "pdfbro-server",
        )]))
        .build();

    let tracer = provider.tracer("pdfbro-server");
    opentelemetry::global::set_tracer_provider(provider);

    let otel_layer = tracing_opentelemetry::layer().with_tracer(tracer);

    // Install as an independent subscriber; tracing_subscriber::registry()
    // will compose with it when called later in the same thread.
    tracing_subscriber::registry()
        .with(otel_layer)
        .try_init()
        .map_err(|e| anyhow::anyhow!("OpenTelemetry layer already initialized: {e}"))?;

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
