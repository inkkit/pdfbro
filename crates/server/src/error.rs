//! Error → HTTP response mapping.
//!
//! Three layers:
//! * [`ApiErrorResponse`] — structured JSON response with actionable guidance.
//! * [`ApiError`] — server-internal error type covering both
//!   [`engine::EngineError`] and the small set of HTTP-shaped errors raised
//!   from the multipart layer (missing fields, bad JSON, oversized bodies).
//! * Implementation of [`axum::response::IntoResponse`] for [`ApiError`]
//!   that maps every variant onto the spec's documented status / JSON body.
//!
//! Implementation of `docs/specs/44-crystal-clear-errors.md`.

use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use engine::EngineError;
use serde::Serialize;
use serde_json::{Value, json};

/// Convenient `Result` alias for handlers and helpers.
pub type ApiResult<T> = Result<T, ApiError>;

/// Structured error response with actionable guidance.
///
/// This is the public-facing error format returned in all API responses.
#[derive(Debug, Clone, Serialize)]
pub struct ApiErrorResponse {
    /// Human-readable error message.
    pub error: String,
    /// Machine-readable error code.
    pub code: String,
    /// Additional error context (URLs, field names, etc.).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<ErrorDetails>,
    /// Actionable suggestion for fixing the error.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggestion: Option<String>,
    /// Link to documentation about this error.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub documentation: Option<String>,
}

/// Additional context for error responses.
#[derive(Debug, Clone, Default, Serialize)]
pub struct ErrorDetails {
    /// URL that failed (for navigation/resource errors).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// Timeout duration in milliseconds.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout_ms: Option<u64>,
    /// Field name that caused the error.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub field: Option<String>,
    /// Invalid value that was provided.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    /// Resource loading errors (failed images, CSS, etc.).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resource_errors: Option<Vec<ResourceError>>,
    /// Reason for navigation failure.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

/// A single resource loading error.
#[derive(Debug, Clone, Serialize)]
pub struct ResourceError {
    /// URL that failed to load.
    pub url: String,
    /// HTTP status code if available.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status_code: Option<u16>,
    /// Error description.
    pub error: String,
}

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
    /// Sub-resource(s) failed to load (images, CSS, fonts, etc.).
    ResourceErrors(Vec<ResourceError>),
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
            ApiError::ResourceErrors(_) => (StatusCode::BAD_GATEWAY, "RESOURCE_ERROR"),
        }
    }

    /// Convert to a structured response with suggestions and documentation.
    pub fn to_response(&self) -> (StatusCode, ApiErrorResponse) {
        let (status, code) = self.status_and_code();
        let response = match self {
            // Navigation errors with full context
            ApiError::Engine(EngineError::Navigation { url, reason }) => ApiErrorResponse {
                error: format!("Navigation failed: {reason}"),
                code: code.to_string(),
                details: Some(ErrorDetails {
                    url: Some(url.clone()),
                    reason: Some(reason.clone()),
                    ..Default::default()
                }),
                suggestion: Some(format!(
                    "Check that {url} is accessible. Try adding --form 'waitDelay=5s' to allow resources to load."
                )),
                documentation: Some(documentation_link("NAVIGATION")),
            },

            // Timeout errors with retry guidance
            ApiError::Engine(EngineError::Timeout(duration)) => ApiErrorResponse {
                error: "Conversion timed out".to_string(),
                code: code.to_string(),
                details: Some(ErrorDetails {
                    timeout_ms: Some(duration.as_millis() as u64),
                    ..Default::default()
                }),
                suggestion: Some(format!(
                    "Increase the timeout: --request-timeout {}s. For slow pages, try --wait-for-idle or --wait-for-selector.",
                    duration.as_secs().saturating_mul(2)
                )),
                documentation: Some(documentation_link("TIMEOUT")),
            },

            // Invalid option with field context
            ApiError::Engine(EngineError::InvalidOption(msg)) => ApiErrorResponse {
                error: msg.clone(),
                code: code.to_string(),
                details: None,
                suggestion: Some(
                    "Check the field format in the documentation. Valid examples: paperSize='A4', scale=1.0, marginTop=0.5".to_string()
                ),
                documentation: Some(documentation_link("INVALID_OPTION")),
            },

            // Invalid page range with format hint
            ApiError::Engine(EngineError::InvalidPageRange(msg)) => ApiErrorResponse {
                error: format!("Invalid page range: {msg}"),
                code: code.to_string(),
                details: None,
                suggestion: Some(
                    "Use format: '1-3,5,7-' (pages 1-3, page 5, pages 7 to end). Page numbers are 1-indexed.".to_string()
                ),
                documentation: Some(documentation_link("INVALID_PAGE_RANGE")),
            },

            // Chrome not found - installation issue
            ApiError::Engine(EngineError::ChromeNotFound { searched }) => ApiErrorResponse {
                error: format!(
                    "Chrome executable not found. Searched: {}",
                    searched.iter().map(|p| p.display().to_string()).collect::<Vec<_>>().join(", ")
                ),
                code: code.to_string(),
                details: None,
                suggestion: Some(
                    "Install Chrome/Chromium or set CHROME_PATH. In Docker: apt-get install chromium.".to_string()
                ),
                documentation: Some(documentation_link("CHROME_NOT_FOUND")),
            },

            // Chrome launch failure
            ApiError::Engine(EngineError::ChromeLaunch(msg)) => ApiErrorResponse {
                error: format!("Chrome failed to launch: {msg}"),
                code: code.to_string(),
                details: None,
                suggestion: Some(
                    "Check Chrome installation and permissions. In Docker, use --no-sandbox flag.".to_string()
                ),
                documentation: Some(documentation_link("CHROME_LAUNCH")),
            },

            // CDP errors
            ApiError::Engine(EngineError::Cdp(msg)) => ApiErrorResponse {
                error: format!("Chrome DevTools Protocol error: {msg}"),
                code: code.to_string(),
                details: None,
                suggestion: Some(
                    "This may be a temporary Chrome issue. Try restarting the server or reducing concurrent requests.".to_string()
                ),
                documentation: Some(documentation_link("CDP_ERROR")),
            },

            // IO errors
            ApiError::Engine(EngineError::Io(e)) => ApiErrorResponse {
                error: format!("I/O error: {e}"),
                code: code.to_string(),
                details: None,
                suggestion: Some(
                    "Check disk space and permissions. Ensure the temp directory is writable.".to_string()
                ),
                documentation: Some(documentation_link("IO_ERROR")),
            },

            // PDF errors
            ApiError::Engine(EngineError::Pdf(msg)) => ApiErrorResponse {
                error: format!("PDF error: {msg}"),
                code: code.to_string(),
                details: None,
                suggestion: Some(
                    "The PDF may be corrupted or password-protected. Try validating the input PDF.".to_string()
                ),
                documentation: Some(documentation_link("PDF_ERROR")),
            },

            // Internal engine errors
            ApiError::Engine(EngineError::Internal(msg)) => ApiErrorResponse {
                error: format!("Internal error: {msg}"),
                code: code.to_string(),
                details: None,
                suggestion: Some(
                    "This is a bug. Please report it with the error details.".to_string()
                ),
                documentation: Some(documentation_link("INTERNAL")),
            },

            // Resource loading errors
            ApiError::ResourceErrors(errors) => ApiErrorResponse {
                error: format!("{} resource(s) failed to load", errors.len()),
                code: code.to_string(),
                details: Some(ErrorDetails {
                    resource_errors: Some(errors.clone()),
                    ..Default::default()
                }),
                suggestion: Some(
                    "Check that all images, CSS, and fonts are accessible. Consider using waitDelay or waitForIdle.".to_string()
                ),
                documentation: Some(documentation_link("RESOURCE_ERROR")),
            },

            // Multipart parsing errors
            ApiError::BadMultipart(msg) => ApiErrorResponse {
                error: msg.clone(),
                code: code.to_string(),
                details: None,
                suggestion: Some(
                    "Ensure the request uses Content-Type: multipart/form-data with proper boundary.".to_string()
                ),
                documentation: Some(documentation_link("BAD_MULTIPART")),
            },

            // Missing field
            ApiError::MissingField(name) => ApiErrorResponse {
                error: format!("Missing required field '{name}'"),
                code: code.to_string(),
                details: Some(ErrorDetails {
                    field: Some(name.to_string()),
                    ..Default::default()
                }),
                suggestion: Some(format!(
                    "Add the field: --form '{name}=<value>'. Check the API documentation for required fields."
                )),
                documentation: Some(documentation_link("MISSING_FIELD")),
            },

            // Missing file
            ApiError::MissingFile(name) => ApiErrorResponse {
                error: format!("Missing required file '{name}'"),
                code: code.to_string(),
                details: Some(ErrorDetails {
                    field: Some(name.clone()),
                    ..Default::default()
                }),
                suggestion: Some(format!(
                    "Add the file: --form 'files=@path/to/{name}'. For multiple files, use multiple --form 'files=@...' arguments."
                )),
                documentation: Some(documentation_link("MISSING_FILE")),
            },

            // Invalid field with specific guidance
            ApiError::InvalidField { field, message } => ApiErrorResponse {
                error: format!("{field} is not valid: {message}"),
                code: code.to_string(),
                details: Some(ErrorDetails {
                    field: Some(field.to_string()),
                    ..Default::default()
                }),
                suggestion: Some(format!(
                    "Check the format for '{field}' in the documentation."
                )),
                documentation: Some(documentation_link("INVALID_FIELD")),
            },

            // Unsafe filename
            ApiError::UnsafeFilename(name) => ApiErrorResponse {
                error: format!("Unsafe filename detected: {name}"),
                code: code.to_string(),
                details: Some(ErrorDetails {
                    value: Some(name.clone()),
                    ..Default::default()
                }),
                suggestion: Some(
                    "Remove path traversal characters (../, ./) from filenames. Use simple names like 'file.html'.".to_string()
                ),
                documentation: Some(documentation_link("UNSAFE_FILENAME")),
            },

            // Body too large
            ApiError::BodyTooLarge => ApiErrorResponse {
                error: "Request body exceeds configured limit".to_string(),
                code: code.to_string(),
                details: None,
                suggestion: Some(
                    "Reduce request size or increase limit: FOLIO_MAX_BODY_SIZE=100MB. Consider uploading large files separately.".to_string()
                ),
                documentation: Some(documentation_link("BODY_TOO_LARGE")),
            },

            // Unsupported media type
            ApiError::UnsupportedMediaType => ApiErrorResponse {
                error: "Expected Content-Type: multipart/form-data".to_string(),
                code: code.to_string(),
                details: None,
                suggestion: Some(
                    "Use: -H 'Content-Type: multipart/form-data' with curl, or let curl set it automatically with --form.".to_string()
                ),
                documentation: Some(documentation_link("UNSUPPORTED_MEDIA_TYPE")),
            },

            // Webhook errors
            ApiError::Webhook(msg) => ApiErrorResponse {
                error: msg.clone(),
                code: code.to_string(),
                details: None,
                suggestion: Some(
                    "Check webhook URL format and ensure it's accessible. For async webhooks, verify the endpoint accepts POST requests.".to_string()
                ),
                documentation: Some(documentation_link("WEBHOOK_ERROR")),
            },

            // Internal server error
            ApiError::Internal(msg) => ApiErrorResponse {
                error: format!("Internal server error: {msg}"),
                code: code.to_string(),
                details: None,
                suggestion: Some(
                    "This is a server bug. Please report it with steps to reproduce.".to_string()
                ),
                documentation: Some(documentation_link("INTERNAL")),
            },

            // Not found
            ApiError::NotFound => ApiErrorResponse {
                error: "Resource not found".to_string(),
                code: code.to_string(),
                details: None,
                suggestion: Some(
                    "Check the resource ID in the URL. Resources may expire after a configured TTL.".to_string()
                ),
                documentation: Some(documentation_link("NOT_FOUND")),
            },

            // Gone
            ApiError::Gone => ApiErrorResponse {
                error: "Resource no longer available".to_string(),
                code: code.to_string(),
                details: None,
                suggestion: Some(
                    "The resource was deleted or expired. Re-submit the original request to regenerate it.".to_string()
                ),
                documentation: Some(documentation_link("GONE")),
            },
        };

        (status, response)
    }

    /// Legacy: Renders the error as a simple JSON Value.
    /// Prefer `to_response()` for new code.
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
            ApiError::ResourceErrors(errors) => json!({
                "error": format!("{} resource(s) failed to load", errors.len()),
                "code": code,
                "resource_errors": errors,
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
            ApiError::ResourceErrors(errors) => {
                write!(f, "resource errors: {} failed", errors.len())
            }
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
        let (status, response) = self.to_response();
        (status, Json(response)).into_response()
    }
}

/// Maps engine errors to HTTP status and error codes.
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

/// Generate documentation links for error codes.
fn documentation_link(error_code: &str) -> String {
    let base_url = "https://folio.dev/docs";
    let path = match error_code {
        "NAVIGATION" => "/troubleshooting#navigation-failed",
        "TIMEOUT" => "/troubleshooting#timeout",
        "INVALID_OPTION" | "INVALID_FIELD" => "/api#form-fields",
        "INVALID_PAGE_RANGE" => "/api#page-ranges",
        "MISSING_FIELD" | "MISSING_FILE" => "/api#required-fields",
        "BAD_MULTIPART" => "/api#multipart-requests",
        "BODY_TOO_LARGE" => "/configuration#body-size-limits",
        "RESOURCE_ERROR" => "/troubleshooting#resource-failed",
        "CHROME_NOT_FOUND" | "ENGINE_UNAVAILABLE" => "/installation#chrome",
        "CHROME_LAUNCH" => "/troubleshooting#chrome-launch",
        "CDP_ERROR" => "/troubleshooting#chrome-devtools",
        "PDF_ERROR" => "/troubleshooting#pdf-issues",
        "WEBHOOK_ERROR" => "/webhooks#troubleshooting",
        "UNSAFE_FILENAME" => "/security#filename-validation",
        "UNSUPPORTED_MEDIA_TYPE" => "/api#content-type",
        _ => "/troubleshooting",
    };
    format!("{base_url}{path}")
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
        // New structured format puts URL/reason in details
        assert_eq!(body["details"]["url"], "https://example.com");
        assert_eq!(body["details"]["reason"], "net::ERR_NAME_NOT_RESOLVED");
        // Also verify suggestion and documentation are present
        assert!(body["suggestion"].is_string());
        assert!(body["documentation"].as_str().unwrap().contains("folio.dev"));
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

    // Tests for spec 44 - Crystal-Clear Error Messages

    #[test]
    fn error_response_has_suggestion_field() {
        let (_, body) = body_value(ApiError::MissingField("files"));
        assert!(body["suggestion"].is_string(), "suggestion field should be present");
        let suggestion = body["suggestion"].as_str().unwrap();
        assert!(
            suggestion.contains("--form") || suggestion.contains("files"),
            "suggestion should guide user to fix: {suggestion}"
        );
    }

    #[test]
    fn error_response_has_documentation_link() {
        let (_, body) = body_value(ApiError::MissingField("url"));
        assert!(body["documentation"].is_string(), "documentation field should be present");
        let doc = body["documentation"].as_str().unwrap();
        assert!(doc.starts_with("https://folio.dev/docs"), "documentation should link to folio docs: {doc}");
    }

    #[test]
    fn navigation_error_has_details_with_url() {
        let err = ApiError::Engine(EngineError::Navigation {
            url: "https://example.com/bad".to_string(),
            reason: "net::ERR_CONNECTION_REFUSED".to_string(),
        });
        let (_, body) = body_value(err);

        assert_eq!(body["code"], "NAVIGATION");
        assert!(body["details"].is_object(), "details should be an object");
        assert_eq!(body["details"]["url"], "https://example.com/bad");
        assert_eq!(body["details"]["reason"], "net::ERR_CONNECTION_REFUSED");
        assert!(body["suggestion"].as_str().unwrap().contains("waitDelay"));
    }

    #[test]
    fn timeout_error_has_timeout_ms_in_details() {
        let err = ApiError::Engine(EngineError::Timeout(Duration::from_secs(30)));
        let (_, body) = body_value(err);

        assert_eq!(body["code"], "TIMEOUT");
        assert!(body["details"]["timeout_ms"].is_number());
        assert_eq!(body["details"]["timeout_ms"], 30000);
        let suggestion = body["suggestion"].as_str().unwrap();
        assert!(suggestion.contains("60s"), "should suggest doubling timeout: {suggestion}");
    }

    #[test]
    fn resource_errors_includes_failed_urls() {
        let resource_errors = vec![
            ResourceError {
                url: "https://cdn.example.com/image.png".to_string(),
                status_code: Some(404),
                error: "HTTP 404".to_string(),
            },
            ResourceError {
                url: "https://fonts.google.com/font.woff".to_string(),
                status_code: None,
                error: "net::ERR_TIMED_OUT".to_string(),
            },
        ];
        let err = ApiError::ResourceErrors(resource_errors);
        let (_, body) = body_value(err);

        assert_eq!(body["code"], "RESOURCE_ERROR");
        assert!(body["details"]["resource_errors"].is_array());
        let errors = body["details"]["resource_errors"].as_array().unwrap();
        assert_eq!(errors.len(), 2);
        assert_eq!(errors[0]["url"], "https://cdn.example.com/image.png");
        assert_eq!(errors[0]["status_code"], 404);
    }

    #[test]
    fn invalid_option_includes_suggestion() {
        let (_, body) = body_value(ApiError::Engine(EngineError::InvalidOption(
            "scale out of range".to_string()
        )));
        assert_eq!(body["code"], "INVALID_OPTION");
        let suggestion = body["suggestion"].as_str().unwrap();
        assert!(suggestion.contains("paperSize") || suggestion.contains("scale"));
    }

    #[test]
    fn unsafe_filename_includes_security_guidance() {
        let (_, body) = body_value(ApiError::UnsafeFilename("../../../etc/passwd".to_string()));
        assert_eq!(body["code"], "UNSAFE_FILENAME");
        let suggestion = body["suggestion"].as_str().unwrap();
        assert!(suggestion.contains("path traversal") || suggestion.contains("../"));
    }

    #[test]
    fn documentation_link_generation() {
        assert!(documentation_link("NAVIGATION").contains("navigation-failed"));
        assert!(documentation_link("TIMEOUT").contains("timeout"));
        assert!(documentation_link("INVALID_OPTION").contains("form-fields"));
        assert!(documentation_link("UNKNOWN_CODE").contains("troubleshooting"));
    }

    #[test]
    fn to_response_returns_structured_format() {
        let err = ApiError::BodyTooLarge;
        let (status, response) = err.to_response();

        assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE);
        assert_eq!(response.code, "BODY_TOO_LARGE");
        assert!(response.suggestion.is_some());
        assert!(response.documentation.is_some());
    }
}
