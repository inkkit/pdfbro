//! Webhook configuration extraction from request headers.

use axum::http::{HeaderMap, HeaderValue};

use super::WebhookError;

/// Webhook configuration extracted from request headers.
#[derive(Debug, Clone)]
pub struct WebhookConfig {
    /// Primary webhook URL for success notifications.
    pub webhook_url: String,
    /// Optional separate URL for error notifications.
    pub error_url: Option<String>,
    /// Extra headers to include in webhook requests.
    pub extra_headers: HeaderMap,
    /// Force synchronous mode even if webhooks configured.
    pub sync_mode: bool,
}

/// Header names (case-insensitive).
pub const HEADER_ASYNC: &str = "gotenberg-async";
pub const HEADER_WEBHOOK_URL: &str = "gotenberg-webhook-url";
pub const HEADER_ERROR_URL: &str = "gotenberg-webhook-error-url";
pub const HEADER_EXTRA_HEADERS: &str = "gotenberg-webhook-extra-http-headers";
pub const HEADER_SYNC_MODE: &str = "gotenberg-webhook-enable-sync-mode";

/// Extract webhook configuration from request headers.
///
/// Returns `None` if no webhook URL is provided (synchronous mode).
/// Returns `Some(WebhookConfig)` if webhook headers are present.
pub fn extract_webhook_config(headers: &HeaderMap) -> Result<Option<WebhookConfig>, WebhookError> {
    // Check if async mode is explicitly enabled
    let async_enabled = headers
        .get(HEADER_ASYNC)
        .and_then(|v| v.to_str().ok())
        .map(|v| matches!(v.trim().to_lowercase().as_str(), "true" | "1" | "yes"))
        .unwrap_or(false);

    // Get webhook URL (required for async mode)
    let webhook_url = match headers.get(HEADER_WEBHOOK_URL) {
        Some(url) => url.to_str().map_err(|_| {
            WebhookError::InvalidUrl("Invalid UTF-8 in webhook URL".into())
        })?.to_string(),
        None => {
            // No webhook URL, not in async mode
            if async_enabled {
                return Err(WebhookError::InvalidUrl(
                    "Gotenberg-Async: true requires Gotenberg-Webhook-Url header".into(),
                ));
            }
            return Ok(None);
        }
    };

    // Check sync mode override
    let sync_mode = headers
        .get(HEADER_SYNC_MODE)
        .and_then(|v| v.to_str().ok())
        .map(|v| matches!(v.trim().to_lowercase().as_str(), "true" | "1" | "yes"))
        .unwrap_or(false);

    // Get error URL (optional)
    let error_url = headers
        .get(HEADER_ERROR_URL)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    // Parse extra headers (JSON object)
    let extra_headers = headers
        .get(HEADER_EXTRA_HEADERS)
        .and_then(|v| v.to_str().ok())
        .and_then(|json_str| parse_extra_headers(json_str).ok())
        .unwrap_or_default();

    Ok(Some(WebhookConfig {
        webhook_url,
        error_url,
        extra_headers,
        sync_mode,
    }))
}

/// Parse extra headers from JSON string.
fn parse_extra_headers(json_str: &str) -> Result<HeaderMap, WebhookError> {
    let map: serde_json::Map<String, serde_json::Value> =
        serde_json::from_str(json_str).map_err(|e| WebhookError::InvalidUrl(format!("Invalid extra headers JSON: {}", e)))?;

    let mut headers = HeaderMap::new();
    for (key, value) in map {
        let header_name = axum::http::HeaderName::from_bytes(key.as_bytes())
            .map_err(|e| WebhookError::InvalidUrl(format!("Invalid header name '{}': {}", key, e)))?;
        let header_value = match value {
            serde_json::Value::String(s) => HeaderValue::from_str(&s)
                .map_err(|e| WebhookError::InvalidUrl(format!("Invalid header value for '{}': {}", key, e)))?,
            other => HeaderValue::from_str(&other.to_string())
                .map_err(|e| WebhookError::InvalidUrl(format!("Invalid header value for '{}': {}", key, e)))?,
        };
        headers.insert(header_name, header_value);
    }

    Ok(headers)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_headers(pairs: &[(&str, &str)]) -> HeaderMap {
        let mut headers = HeaderMap::new();
        for (key, value) in pairs {
            headers.insert(
                axum::http::HeaderName::from_bytes(key.as_bytes()).unwrap(),
                HeaderValue::from_str(value).unwrap(),
            );
        }
        headers
    }

    #[test]
    fn extract_config_no_webhook_returns_none() {
        let headers = create_headers(&[]);
        assert!(extract_webhook_config(&headers).unwrap().is_none());
    }

    #[test]
    fn extract_config_with_webhook_url() {
        let headers = create_headers(&[
            ("gotenberg-webhook-url", "https://example.com/webhook"),
        ]);
        let config = extract_webhook_config(&headers).unwrap().unwrap();
        assert_eq!(config.webhook_url, "https://example.com/webhook");
        assert!(config.error_url.is_none());
        assert!(!config.sync_mode);
    }

    #[test]
    fn extract_config_with_error_url() {
        let headers = create_headers(&[
            ("gotenberg-webhook-url", "https://example.com/webhook"),
            ("gotenberg-webhook-error-url", "https://example.com/error"),
        ]);
        let config = extract_webhook_config(&headers).unwrap().unwrap();
        assert_eq!(config.webhook_url, "https://example.com/webhook");
        assert_eq!(config.error_url, Some("https://example.com/error".into()));
    }

    #[test]
    fn extract_config_async_mode_requires_webhook_url() {
        let headers = create_headers(&[
            ("gotenberg-async", "true"),
        ]);
        let result = extract_webhook_config(&headers);
        assert!(result.is_err());
    }

    #[test]
    fn extract_config_with_extra_headers() {
        let headers = create_headers(&[
            ("gotenberg-webhook-url", "https://example.com/webhook"),
            ("gotenberg-webhook-extra-http-headers", r#"{"Authorization":"Bearer token123","X-Custom":"value"}"#),
        ]);
        let config = extract_webhook_config(&headers).unwrap().unwrap();
        assert!(config.extra_headers.contains_key("authorization"));
        assert!(config.extra_headers.contains_key("x-custom"));
    }

    #[test]
    fn extract_config_sync_mode() {
        let headers = create_headers(&[
            ("gotenberg-webhook-url", "https://example.com/webhook"),
            ("gotenberg-webhook-enable-sync-mode", "true"),
        ]);
        let config = extract_webhook_config(&headers).unwrap().unwrap();
        assert!(config.sync_mode);
    }
}
