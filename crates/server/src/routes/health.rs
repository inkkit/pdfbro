//! `/health`, `/version`, and `/prometheus/metrics` routes.

use axum::Json;
use axum::extract::State;
use axum::http::header;
use axum::response::{IntoResponse, Response};
use serde_json::json;

use crate::AppState;
use crate::metrics::export_metrics;

/// Liveness summary. Always returns 200 OK; the body indicates per-engine
/// availability (matching Gotenberg's convention).
pub async fn health(State(state): State<AppState>) -> impl IntoResponse {
    let chromium_up = match state.chromium.as_ref() {
        Some(be) => be.healthy().await,
        None => false,
    };

    #[cfg(feature = "libreoffice")]
    let lo_up = match state.libreoffice.as_ref() {
        Some(lo) => lo.healthy().await,
        None => false,
    };
    #[cfg(not(feature = "libreoffice"))]
    let lo_up = false;

    // Update engine health metrics
    state.metrics.update_engine_health(chromium_up, lo_up);

    let uptime_secs = state.started_at.elapsed().as_secs();
    Json(json!({
        "status": "up",
        "uptime_secs": uptime_secs,
        "chromium": if chromium_up { "up" } else { "down" },
        "libreoffice": if lo_up { "up" } else { "down" },
    }))
}

/// Crate version, plain text.
pub async fn version() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
        env!("CARGO_PKG_VERSION"),
    )
}

/// Prometheus metrics endpoint.
pub async fn metrics_handler() -> impl IntoResponse {
    let metrics = export_metrics();
    Response::builder()
        .status(axum::http::StatusCode::OK)
        .header(
            header::CONTENT_TYPE,
            "text/plain; version=0.0.4; charset=utf-8",
        )
        .body(metrics)
        .unwrap()
}
