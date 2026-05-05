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
use axum::extract::{DefaultBodyLimit, State};
use axum::http::{HeaderName, Request};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
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

use crate::auth::{BasicAuthConfig, BasicAuthLayer};
use crate::config::ServerConfig;
use crate::routes::{batch, health, pdfengines};
#[cfg(feature = "chromium")]
use crate::routes::chromium;
#[cfg(feature = "libreoffice")]
use crate::routes::libreoffice;
use crate::state::AppState;

/// Generates a ULID for every incoming request that did not already
/// carry an `X-Request-Id` header.
///
/// ULID provides lexicographic sorting (chronological order), is URL-safe,
/// and uses 26 lowercase characters (Crockford base32).
#[derive(Clone, Default)]
pub struct UlidRequestId;

impl MakeRequestId for UlidRequestId {
    fn make_request_id<B>(&mut self, _request: &Request<B>) -> Option<RequestId> {
        let id = ulid::Ulid::new().to_string().to_lowercase();
        let header = id.parse::<axum::http::HeaderValue>().ok()?;
        Some(RequestId::new(header))
    }
}

/// Custom [`MakeSpan`] that includes `request_id` (set by
/// [`SetRequestIdLayer`]) as a structured field on every request span.
#[derive(Clone)]
struct RequestIdMakeSpan {
    /// Whether to disable telemetry for health route.
    disable_health: bool,
    /// Whether to disable telemetry for root route.
    disable_root: bool,
    /// Whether to disable telemetry for debug route.
    disable_debug: bool,
    /// Whether to disable telemetry for version route.
    disable_version: bool,
    /// Header name to read the request ID from.
    header_name: String,
}

impl RequestIdMakeSpan {
    fn new(config: &ServerConfig) -> Self {
        Self {
            disable_health: config.api_disable_health_route_telemetry,
            disable_root: config.api_disable_root_route_telemetry,
            disable_debug: config.api_disable_debug_route_telemetry,
            disable_version: config.api_disable_version_route_telemetry,
            header_name: config.api_correlation_id_header.clone(),
        }
    }

    fn telemetry_disabled_for(&self, path: &str) -> bool {
        match path {
            "/health" => self.disable_health,
            "/" => self.disable_root,
            "/debug" => self.disable_debug,
            "/version" => self.disable_version,
            _ => false,
        }
    }
}

impl<B> MakeSpan<B> for RequestIdMakeSpan {
    fn make_span(&mut self, request: &Request<B>) -> tracing::Span {
        let path = request.uri().path();

        // Return disabled span if telemetry is disabled for this route
        if self.telemetry_disabled_for(path) {
            return tracing::Span::none();
        }

        let request_id = request
            .headers()
            .get(self.header_name.as_str())
            .and_then(|v| v.to_str().ok())
            .unwrap_or("unknown")
            .to_string();

        tracing::info_span!(
            "request",
            request_id = %request_id,
            method = %request.method(),
            path = %path,
        )
    }
}

/// Build the full HTTP router for the given [`AppState`] and [`ServerConfig`].
///
/// The middleware stack (outer → inner) is:
/// `Trace → SetRequestId → PropagateRequestId → RequestBodyLimit → Timeout
/// → CORS → routes`. `/health` and `/version` are served from a separate
/// sub-router that bypasses the timeout layer (they must always respond
/// quickly even under heavy load).
pub fn build_router(state: AppState, config: &ServerConfig) -> Router {
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
            "/forms/pdfengines/optimise",
            post(pdfengines::pdfengines_optimise),
        )
        .route(
            "/forms/pdfengines/rotate",
            post(pdfengines::pdfengines_rotate),
        )
        .route(
            "/forms/pdfengines/embed",
            post(pdfengines::pdfengines_embed),
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

    let mut untimed = Router::new()
        .route("/", get(health::root))
        .route("/health", get(health::health))
        .route("/health", axum::routing::head(health::health_head))
        .route("/version", get(health::version))
        .route("/prometheus/metrics", get(health::metrics_handler))
        .route("/favicon.ico", get(health::favicon));

    // Conditionally add debug route
    if config.api_enable_debug_route {
        untimed = untimed.route("/debug", get(health::debug));
    }

    // Font Doctor diagnostic routes (always enabled)
    use crate::routes::debug;
    untimed = untimed
        .route("/debug/fonts", get(debug::debug_list_fonts))
        .route("/debug/validate-fonts", post(debug::debug_validate_fonts))
        .route("/debug/diagnose-html", post(debug::debug_diagnose_html));

    // Live Preview Mode routes (Spec 45)
    use crate::routes::preview;
    untimed = untimed
        .route("/preview/url", get(preview::preview_url))
        .route("/preview/html", post(preview::preview_html))
        .route("/preview/markdown", post(preview::preview_markdown))
        .route("/preview/compare", post(preview::preview_compare));

    // PDF Size Estimator routes (Spec 46)
    use crate::routes::estimate;
    untimed = untimed
        .route("/estimate", post(estimate::estimate))
        .route("/estimate/form", post(estimate::estimate_form))
        .route("/estimate/batch", post(estimate::estimate_batch));

    // OpenAPI spec for Scalar documentation
    use crate::routes::openapi;
    untimed = untimed.route("/openapi.json", get(openapi::openapi_spec));

    // Operator console SSE stream and one-shot metrics JSON (long-lived, no timeout)
    use crate::routes::console;
    untimed = untimed
        .route("/_",             get(|| async { axum::response::Redirect::permanent("/_/") }))
        .route("/_/api/stream",  get(console::console_stream))
        .route("/_/api/metrics", get(console::console_metrics_json))
        .route("/_/",            get(console::console_asset_root))
        .route("/_/{*path}",     get(console::console_asset));

    // Scalar interactive API documentation
    use axum::response::Html;
    use scalar_api_reference::scalar_html_default;
    use serde_json::json;

    let scalar_config = json!({
        "url": "/openapi.json",
        "metaData": {
            "title": "pdfbro API",
            "description": "PDF generation API (Gotenberg-compatible)",
            "favicon": "/favicon.ico"
        },
        "theme": "purple",
        "darkMode": true,
        "layout": "modern",
        "searchHotKey": "k",
        "defaultHttpClient": {
            "targetKey": "curl",
            "clientKey": "curl"
        }
    });

    // Create Scalar HTML handler
    let scalar_html = scalar_html_default(&scalar_config);
    untimed = untimed.route("/docs", get(|| async move {
        Html(scalar_html)
    }));

    let header_name = HeaderName::from_bytes(
        config.api_correlation_id_header.as_bytes(),
    )
    .expect("api_correlation_id_header was validated in ServerConfig::resolve");

    // Keep a clone for the console log middleware (state is moved into with_state below).
    let state_for_console = state.clone();

    let mut router = Router::new()
        .merge(timed)
        .merge(untimed)
        .with_state(state)
        .layer(
            ServiceBuilder::new()
                .layer(SetRequestIdLayer::new(header_name.clone(), UlidRequestId))
                .layer(PropagateRequestIdLayer::new(header_name.clone()))
                .layer(
                    TraceLayer::new_for_http()
                        .make_span_with(RequestIdMakeSpan::new(config))
                        .on_response(
                            tower_http::trace::DefaultOnResponse::new().level(Level::INFO),
                        )
                        .on_failure(
                            tower_http::trace::DefaultOnFailure::new().level(Level::WARN),
                        ),
                )
                .layer(CorsLayer::permissive())
                // metrics_middleware removed - handlers record metrics directly
        );

    // Add Basic Auth middleware if configured
    if let (Some(username), Some(password)) = (&config.api_basic_auth_username, &config.api_basic_auth_password) {
        router = router.layer(BasicAuthLayer::new(BasicAuthConfig::new(
            username.clone(),
            password.clone(),
        )));
    }

    // Console log middleware — outermost layer so it captures every request
    // (including auth-rejected ones) and records them into the ConsoleStore
    // ring buffer. Added last so it wraps the full stack.
    router = router.layer(middleware::from_fn_with_state(state_for_console, console_log_middleware));

    // Optional path-prefix mount (--root-path / API_ROOT_PATH). When
    // empty, the router is returned untouched; otherwise every route
    // is reachable under the prefix.
    if !config.api_root_path.is_empty() {
        router = Router::new().nest(&config.api_root_path, router);
    }

    router
}

/// Default request timeout exposed for integration tests.
#[allow(dead_code)]
pub fn default_request_timeout() -> Duration {
    Duration::from_secs(120)
}

/// Middleware that records every non-console HTTP request into the
/// [`ConsoleStore`] ring buffer for the operator console UI.
///
/// Requests to `/_/` (the console API itself) are skipped to avoid noise.
async fn console_log_middleware(
    State(state): State<AppState>,
    req: axum::extract::Request,
    next: Next,
) -> Response {
    use std::time::Instant;

    let method = req.method().to_string();
    let path = req.uri().path().to_string();

    // Skip the console routes themselves to avoid noise.
    if path.starts_with("/_/") {
        return next.run(req).await;
    }

    use std::sync::atomic::Ordering;
    state.console.active_requests.fetch_add(1, Ordering::SeqCst);
    {
        let mut map = state.console.active_per_route.lock().await;
        *map.entry(path.clone()).or_insert(0) += 1;
    }
    let start = Instant::now();
    let response = next.run(req).await;
    state.console.active_requests.fetch_sub(1, Ordering::SeqCst);
    {
        let mut map = state.console.active_per_route.lock().await;
        if let Some(c) = map.get_mut(&path) {
            *c = c.saturating_sub(1);
        }
    }
    let elapsed = start.elapsed();
    let duration_ms = elapsed.as_millis() as u64;
    let status = response.status().as_u16();

    // Record into ring buffer for the console UI
    state.console.record_request(method.clone(), path.clone(), status, duration_ms).await;
    // Record into Prometheus counters + histogram for RPS / routes / error% calculations
    state.metrics.record_http_request(&method, &path, status, elapsed.as_secs_f64());
    response
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
