//! `/health` and `/version` routes.

use axum::Json;
use axum::extract::State;
use axum::http::header;
use axum::response::IntoResponse;
use serde_json::json;

use crate::AppState;

/// Liveness summary. Always returns 200 OK; the body indicates per-engine
/// availability (matching Gotenberg's convention).
pub async fn health(State(state): State<AppState>) -> impl IntoResponse {
    let chromium_up = state.chromium.healthy().await;
    let lo_up = match state.libreoffice.as_ref() {
        Some(lo) => lo.healthy().await,
        None => false,
    };
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
