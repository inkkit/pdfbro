//! Router construction + middleware stack.
//!
//! The router is built in three layers:
//! 1. Route table (handlers from [`crate::routes`]).
//! 2. Per-route logic (e.g. timeout-bypass on `/health` and `/version`).
//! 3. Outer cross-cutting middleware (request-id, body limit, CORS,
//!    tracing, metrics).

use std::time::Duration;

use axum::Router;
use axum::error_handling::HandleErrorLayer;
use axum::extract::DefaultBodyLimit;
use axum::http::{HeaderName, Request};
use axum::response::IntoResponse;
use axum::routing::{get, post};
use engine::EngineError;
use tower::BoxError;
use tower::ServiceBuilder;
use tower::timeout::TimeoutLayer;
use tower_http::cors::CorsLayer;
use tower_http::request_id::{
    MakeRequestId, PropagateRequestIdLayer, RequestId, SetRequestIdLayer,
};
use tower_http::trace::{
    MakeSpan, TraceLayer,
};
use tracing::Level;

use crate::error::ApiError;

use crate::routes::{batch, health, pdfengines};
#[cfg(feature = "chromium")]
use crate::routes::chromium;
#[cfg(feature = "libreoffice")]
use crate::routes::libreoffice;
use crate::state::AppState;

const REQUEST_ID_HEADER: &str = "x-request-id";

/// Generates a UUIDv4 for every incoming request that did not already
/// carry an `X-Request-Id` header.
#[derive(Clone, Default)]
pub struct UuidRequestId;

impl MakeRequestId for UuidRequestId {
    fn make_request_id<B>(&mut self, _request: &Request<B>) -> Option<RequestId> {
        let id = uuid::Uuid::new_v4().to_string();
        let header = id.parse::<axum::http::HeaderValue>().ok()?;
        Some(RequestId::new(header))
    }
}

/// Custom [`MakeSpan`] that includes `request_id` (set by
/// [`SetRequestIdLayer`]) as a structured field on every request span.
#[derive(Clone)]
struct RequestIdMakeSpan;

impl<B> MakeSpan<B> for RequestIdMakeSpan {
    fn make_span(&mut self, request: &Request<B>) -> tracing::Span {
        let request_id = request
            .headers()
            .get(REQUEST_ID_HEADER)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("unknown")
            .to_string();

        tracing::info_span!(
            "request",
            request_id = %request_id,
            method = %request.method(),
            path = %request.uri().path(),
        )
    }
}

/// Build the full HTTP router for the given [`AppState`].
///
/// The middleware stack (outer → inner) is:
/// `Trace → SetRequestId → PropagateRequestId → RequestBodyLimit → Timeout
/// → CORS → routes`. `/health` and `/version` are served from a separate
/// sub-router that bypasses the timeout layer (they must always respond
/// quickly even under heavy load).
pub fn build_router(state: AppState) -> Router {
    let max_body = state.config.max_body_bytes;
    let request_timeout = state.config.request_timeout;

    let mut timed = Router::new();

    #[cfg(feature = "chromium")]
    {
        timed = timed
            .route(
                "/forms/chromium/convert/html",
                post(chromium::chromium_html),
            )
            .route("/forms/chromium/convert/url", post(chromium::chromium_url))
            .route(
                "/forms/chromium/convert/markdown",
                post(chromium::chromium_markdown),
            )
            .route(
                "/forms/chromium/screenshot/html",
                post(chromium::chromium_screenshot_html),
            )
            .route(
                "/forms/chromium/screenshot/url",
                post(chromium::chromium_screenshot_url),
            )
            .route(
                "/forms/chromium/screenshot/markdown",
                post(chromium::chromium_screenshot_markdown),
            );
    }

    #[cfg(feature = "libreoffice")]
    {
        timed = timed.route(
            "/forms/libreoffice/convert",
            post(libreoffice::libreoffice_convert),
        );
    }

    timed = timed.route(
            "/forms/pdfengines/merge",
            post(pdfengines::pdfengines_merge),
        )
        .route(
            "/forms/pdfengines/split",
            post(pdfengines::pdfengines_split),
        )
        .route(
            "/forms/pdfengines/flatten",
            post(pdfengines::pdfengines_flatten),
        )
        .route(
            "/forms/pdfengines/metadata/read",
            post(pdfengines::pdfengines_metadata_read),
        )
        .route(
            "/forms/pdfengines/metadata/write",
            post(pdfengines::pdfengines_metadata_write),
        )
        .route(
            "/forms/pdfengines/convert",
            post(pdfengines::pdfengines_convert),
        )
        .route(
            "/forms/pdfengines/bookmarks/read",
            post(pdfengines::pdfengines_bookmarks_read),
        )
        .route(
            "/forms/pdfengines/bookmarks/write",
            post(pdfengines::pdfengines_bookmarks_write),
        )
        .route(
            "/forms/pdfengines/watermark",
            post(pdfengines::pdfengines_watermark),
        )
        .route(
            "/forms/pdfengines/stamp",
            post(pdfengines::pdfengines_stamp),
        )
        .route(
            "/forms/pdfengines/encrypt",
            post(pdfengines::pdfengines_encrypt),
        )
        .route(
            "/forms/pdfengines/decrypt",
            post(pdfengines::pdfengines_decrypt),
        )
        .route(
            "/forms/pdfengines/rotate",
            post(pdfengines::pdfengines_rotate),
        )
        // Batch API routes
        .route("/forms/batch/submit", post(batch::batch_submit))
        .route("/forms/batch/{id}/status", get(batch::batch_status))
        .route("/forms/batch/{id}/download", get(batch::batch_download))
        .layer(
            ServiceBuilder::new()
                .layer(HandleErrorLayer::new(handle_timeout_error))
                .layer(TimeoutLayer::new(request_timeout)),
        )
        .layer(DefaultBodyLimit::max(max_body));

    let untimed = Router::new()
        .route("/health", get(health::health))
        .route("/version", get(health::version))
        .route("/prometheus/metrics", get(health::metrics_handler));

    let header_name = HeaderName::from_static(REQUEST_ID_HEADER);

    Router::new()
        .merge(timed)
        .merge(untimed)
        .with_state(state)
        .layer(
            ServiceBuilder::new()
                .layer(
                    TraceLayer::new_for_http()
                        .make_span_with(RequestIdMakeSpan)
                        .on_response(
                            tower_http::trace::DefaultOnResponse::new().level(Level::INFO),
                        )
                        .on_failure(
                            tower_http::trace::DefaultOnFailure::new().level(Level::WARN),
                        ),
                )
                .layer(SetRequestIdLayer::new(header_name.clone(), UuidRequestId))
                .layer(PropagateRequestIdLayer::new(header_name))
                .layer(CorsLayer::permissive()),
                // metrics_middleware removed - handlers record metrics directly
        )
}

/// Default request timeout exposed for integration tests.
#[allow(dead_code)]
pub fn default_request_timeout() -> Duration {
    Duration::from_secs(120)
}

/// Maps `tower::timeout::error::Elapsed` (and any other boxed error
/// raised by middleware) into the documented JSON shape.
async fn handle_timeout_error(err: BoxError) -> impl IntoResponse {
    if err.is::<tower::timeout::error::Elapsed>() {
        ApiError::Engine(EngineError::Timeout(default_request_timeout()))
    } else {
        ApiError::Internal(err.to_string())
    }
}
