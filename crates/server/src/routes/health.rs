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

/// `GET /` — lists all registered routes, matching Gotenberg's convention.
pub async fn root() -> impl IntoResponse {
    const ROUTES: &str = "\
GET /
GET /health
GET /version
GET /prometheus/metrics
POST /forms/chromium/convert/html
POST /forms/chromium/convert/url
POST /forms/chromium/convert/markdown
POST /forms/chromium/screenshot/html
POST /forms/chromium/screenshot/url
POST /forms/chromium/screenshot/markdown
POST /forms/libreoffice/convert
POST /forms/pdfengines/merge
POST /forms/pdfengines/split
POST /forms/pdfengines/flatten
POST /forms/pdfengines/convert
POST /forms/pdfengines/metadata/read
POST /forms/pdfengines/metadata/write
POST /forms/pdfengines/bookmarks/read
POST /forms/pdfengines/bookmarks/write
POST /forms/pdfengines/watermark
POST /forms/pdfengines/stamp
POST /forms/pdfengines/encrypt
POST /forms/pdfengines/decrypt
POST /forms/pdfengines/rotate
POST /forms/pdfengines/embed
POST /forms/batch/submit
GET /forms/batch/{id}/status
GET /forms/batch/{id}/download
";
    (
        [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
        ROUTES,
    )
}

/// Debug endpoint - exposes server configuration and state.
/// Only available when --api-enable-debug-route is set.
pub async fn debug(State(state): State<AppState>) -> impl IntoResponse {
    let uptime_secs = state.started_at.elapsed().as_secs();
    
    Json(json!({
        "version": env!("CARGO_PKG_VERSION"),
        "uptime_secs": uptime_secs,
        "config": {
            "host": state.config.host.to_string(),
            "port": state.config.port,
            "concurrency": state.config.concurrency,
            "max_body_bytes": state.config.max_body_bytes,
            "request_timeout_secs": state.config.request_timeout.as_secs(),
            "chromium_lazy_start": state.config.chromium_lazy_start,
            "chromium_idle_shutdown_timeout_secs": state.config.chromium_idle_shutdown_timeout.map(|d| d.as_secs()),
            "libreoffice_lazy_start": state.config.libreoffice_lazy_start,
            "libreoffice_idle_shutdown_timeout_secs": state.config.libreoffice_idle_shutdown_timeout.map(|d| d.as_secs()),
        },
        "features": {
            "chromium": cfg!(feature = "chromium"),
            "libreoffice": cfg!(feature = "libreoffice"),
        }
    }))
}

#[cfg(test)]
mod tests {
    use axum::body::to_bytes;
    use axum::http::StatusCode;
    use axum::response::IntoResponse;

    #[tokio::test]
    async fn root_returns_200_with_route_list() {
        let resp = super::root().await.into_response();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = to_bytes(resp.into_body(), 64 * 1024).await.unwrap();
        let text = std::str::from_utf8(&body).unwrap();
        assert!(text.contains("/health"), "missing /health in root listing");
        assert!(text.contains("/forms/chromium/convert/html"), "missing html route");
        assert!(text.contains("/forms/pdfengines/merge"), "missing merge route");
        assert!(text.contains("/forms/pdfengines/embed"), "missing embed route");
    }
}
