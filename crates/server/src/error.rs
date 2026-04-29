//! Error → HTTP response mapping.
//!
//! Two layers:
//! * [`ApiError`] — server-internal error type covering both
//!   [`engine::EngineError`] and the small set of HTTP-shaped errors raised
//!   from the multipart layer (missing fields, bad JSON, oversized bodies).
//! * Implementation of [`axum::response::IntoResponse`] for [`ApiError`]
//!   that maps every variant onto the spec's documented status / JSON body.

use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use engine::EngineError;
use serde_json::{Value, json};

/// Convenient `Result` alias for handlers and helpers.
pub type ApiResult<T> = Result<T, ApiError>;

/// All error shapes the server can emit.
#[derive(Debug)]
pub enum ApiError {
    /// An engine call returned a typed [`EngineError`].
    Engine(EngineError),
    /// Multipart body could not be parsed (malformed framing, etc).
    BadMultipart(String),
    /// A required multipart field is missing.
    MissingField(&'static str),
    /// A required multipart file is missing.
    MissingFile(String),
    /// A field value failed to parse / decode (e.g. non-JSON `cookies`).
    InvalidField {
        /// Logical name of the field.
        field: &'static str,
        /// Human-readable parse error.
        message: String,
    },
    /// Multipart filename contained a path-traversal component.
    UnsafeFilename(String),
    /// Body exceeded the configured maximum size.
    BodyTooLarge,
    /// Unsupported media type on a route that requires multipart.
    UnsupportedMediaType,
    /// Catch-all for unexpected internal failures.
    Internal(String),
    /// Webhook configuration or delivery error.
    Webhook(String),
    /// Resource not found (e.g., batch ID doesn't exist).
    NotFound,
    /// Resource gone (e.g., batch already downloaded and removed).
    Gone,
}

impl ApiError {
    /// Maps the error to its documented (status, code) pair.
    pub fn status_and_code(&self) -> (StatusCode, &'static str) {
        match self {
            ApiError::Engine(e) => engine_status_and_code(e),
            ApiError::BadMultipart(_) => (StatusCode::BAD_REQUEST, "BAD_MULTIPART"),
            ApiError::MissingField(_) => (StatusCode::BAD_REQUEST, "MISSING_FIELD"),
            ApiError::MissingFile(_) => (StatusCode::BAD_REQUEST, "MISSING_FILE"),
            ApiError::InvalidField { .. } => (StatusCode::BAD_REQUEST, "INVALID_OPTION"),
            ApiError::UnsafeFilename(_) => (StatusCode::BAD_REQUEST, "UNSAFE_FILENAME"),
            ApiError::BodyTooLarge => (StatusCode::PAYLOAD_TOO_LARGE, "BODY_TOO_LARGE"),
            ApiError::UnsupportedMediaType => {
                (StatusCode::UNSUPPORTED_MEDIA_TYPE, "UNSUPPORTED_MEDIA_TYPE")
            }
            ApiError::Internal(_) => (StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL"),
            ApiError::Webhook(_) => (StatusCode::BAD_REQUEST, "WEBHOOK_ERROR"),
            ApiError::NotFound => (StatusCode::NOT_FOUND, "NOT_FOUND"),
            ApiError::Gone => (StatusCode::GONE, "GONE"),
        }
    }

    /// Renders the error as the documented JSON body.
    pub fn body(&self) -> Value {
        let (_, code) = self.status_and_code();
        match self {
            ApiError::Engine(EngineError::Navigation { url, reason }) => json!({
                "error": format!("navigation failed for {url}: {reason}"),
                "code": code,
                "url": url,
                "reason": reason,
            }),
            ApiError::Engine(e) => json!({
                "error": e.to_string(),
                "code": code,
            }),
            ApiError::BadMultipart(msg) => json!({ "error": msg, "code": code }),
            ApiError::MissingField(name) => json!({
                "error": format!("missing required field '{name}'"),
                "code": code,
            }),
            ApiError::MissingFile(name) => json!({
                "error": format!("missing required file '{name}'"),
                "code": code,
            }),
            ApiError::InvalidField { field, message } => json!({
                "error": format!("{field} is not valid: {message}"),
                "code": code,
                "field": field,
            }),
            ApiError::UnsafeFilename(name) => json!({
                "error": format!("unsafe filename: {name}"),
                "code": code,
            }),
            ApiError::BodyTooLarge => json!({
                "error": "request body exceeds configured limit",
                "code": code,
            }),
            ApiError::UnsupportedMediaType => json!({
                "error": "expected Content-Type: multipart/form-data",
                "code": code,
            }),
            ApiError::Internal(msg) => json!({ "error": msg, "code": code }),
            ApiError::Webhook(msg) => json!({ "error": msg, "code": code }),
            ApiError::NotFound => json!({
                "error": "resource not found",
                "code": code,
            }),
            ApiError::Gone => json!({
                "error": "resource no longer available",
                "code": code,
            }),
        }
    }
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ApiError::Engine(e) => write!(f, "engine: {e}"),
            ApiError::BadMultipart(m) => write!(f, "bad multipart: {m}"),
            ApiError::MissingField(n) => write!(f, "missing required field {n}"),
            ApiError::MissingFile(n) => write!(f, "missing required file {n}"),
            ApiError::InvalidField { field, message } => write!(f, "{field}: {message}"),
            ApiError::UnsafeFilename(n) => write!(f, "unsafe filename: {n}"),
            ApiError::BodyTooLarge => write!(f, "request body too large"),
            ApiError::UnsupportedMediaType => write!(f, "unsupported media type"),
            ApiError::Internal(m) => write!(f, "internal: {m}"),
            ApiError::Webhook(m) => write!(f, "webhook: {m}"),
            ApiError::NotFound => write!(f, "resource not found"),
            ApiError::Gone => write!(f, "resource gone"),
        }
    }
}

impl std::error::Error for ApiError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        if let ApiError::Engine(e) = self {
            Some(e)
        } else {
            None
        }
    }
}

impl From<EngineError> for ApiError {
    fn from(e: EngineError) -> Self {
        ApiError::Engine(e)
    }
}

impl From<crate::webhook::WebhookError> for ApiError {
    fn from(e: crate::webhook::WebhookError) -> Self {
        ApiError::Webhook(e.to_string())
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, _) = self.status_and_code();
        let body = self.body();
        (status, Json(body)).into_response()
    }
}

fn engine_status_and_code(e: &EngineError) -> (StatusCode, &'static str) {
    match e {
        EngineError::InvalidOption(_) => (StatusCode::BAD_REQUEST, "INVALID_OPTION"),
        EngineError::InvalidPageRange(_) => (StatusCode::BAD_REQUEST, "INVALID_PAGE_RANGE"),
        EngineError::Navigation { .. } => (StatusCode::BAD_GATEWAY, "NAVIGATION"),
        EngineError::Timeout(_) => (StatusCode::GATEWAY_TIMEOUT, "TIMEOUT"),
        EngineError::ChromeNotFound { .. } | EngineError::ChromeLaunch(_) => {
            (StatusCode::INTERNAL_SERVER_ERROR, "ENGINE_UNAVAILABLE")
        }
        EngineError::Cdp(_) | EngineError::Internal(_) | EngineError::Io(_) | EngineError::Pdf(_) => {
            (StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    async fn body_value_async(err: ApiError) -> (StatusCode, Value) {
        let resp = err.into_response();
        let status = resp.status();
        let bytes = axum::body::to_bytes(resp.into_body(), 1 << 16)
            .await
            .unwrap();
        let v: Value = serde_json::from_slice(&bytes).unwrap();
        (status, v)
    }

    fn body_value(err: ApiError) -> (StatusCode, Value) {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        rt.block_on(body_value_async(err))
    }

    #[test]
    fn invalid_option_maps_to_400_invalid_option() {
        let (status, body) =
            body_value(ApiError::Engine(EngineError::InvalidOption("scale".into())));
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_OPTION");
        assert!(body["error"].as_str().unwrap().contains("scale"));
    }

    #[test]
    fn invalid_page_range_maps_to_400() {
        let (status, body) = body_value(ApiError::Engine(EngineError::InvalidPageRange(
            "empty".into(),
        )));
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_PAGE_RANGE");
    }

    #[test]
    fn navigation_maps_to_502_with_url_reason() {
        let err = ApiError::Engine(EngineError::Navigation {
            url: "https://example.com".to_string(),
            reason: "net::ERR_NAME_NOT_RESOLVED".to_string(),
        });
        let (status, body) = body_value(err);
        assert_eq!(status, StatusCode::BAD_GATEWAY);
        assert_eq!(body["code"], "NAVIGATION");
        assert_eq!(body["url"], "https://example.com");
        assert_eq!(body["reason"], "net::ERR_NAME_NOT_RESOLVED");
    }

    #[test]
    fn timeout_maps_to_504() {
        let (status, body) = body_value(ApiError::Engine(EngineError::Timeout(
            Duration::from_secs(30),
        )));
        assert_eq!(status, StatusCode::GATEWAY_TIMEOUT);
        assert_eq!(body["code"], "TIMEOUT");
    }

    #[test]
    fn chrome_launch_failures_map_to_500_engine_unavailable() {
        let (s1, b1) = body_value(ApiError::Engine(EngineError::ChromeNotFound {
            searched: vec![],
        }));
        let (s2, b2) = body_value(ApiError::Engine(EngineError::ChromeLaunch("boom".into())));
        assert_eq!(s1, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(s2, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(b1["code"], "ENGINE_UNAVAILABLE");
        assert_eq!(b2["code"], "ENGINE_UNAVAILABLE");
    }

    #[test]
    fn cdp_internal_io_map_to_500_internal() {
        let (s1, b1) = body_value(ApiError::Engine(EngineError::Cdp("bad".into())));
        let (s2, b2) = body_value(ApiError::Engine(EngineError::Internal("bad".into())));
        let (s3, b3) = body_value(ApiError::Engine(EngineError::Io(std::io::Error::other(
            "disk",
        ))));
        assert_eq!(s1, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(s2, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(s3, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(b1["code"], "INTERNAL");
        assert_eq!(b2["code"], "INTERNAL");
        assert_eq!(b3["code"], "INTERNAL");
    }

    #[test]
    fn missing_file_message_includes_name() {
        let (status, body) = body_value(ApiError::MissingFile("index.html".into()));
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "MISSING_FILE");
        assert!(body["error"].as_str().unwrap().contains("index.html"));
    }

    #[test]
    fn body_too_large_returns_413() {
        let (status, body) = body_value(ApiError::BodyTooLarge);
        assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE);
        assert_eq!(body["code"], "BODY_TOO_LARGE");
    }
}
