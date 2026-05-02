//! HTTP Header injection prevention for `extraHttpHeaders` validation.
//!
//! Blocks CRLF injection attacks and prevents overriding of security-critical headers.

use std::collections::HashMap;

use axum::http::HeaderName;

use crate::error::ApiError;

/// Headers that cannot be overridden for security reasons.
const BLOCKED_HEADERS: &[&str] = &[
    // Request control
    "host",
    "content-length",
    "transfer-encoding",
    "connection",
    "keep-alive",
    "upgrade",
    "upgrade-insecure-requests",
    // Proxy/security
    "proxy-authorization",
    "proxy-authenticate",
    "proxy-connection",
    "te",
    "trailer",
    "http2-settings",
    // Authentication/cookies (should be via dedicated fields)
    "cookie",
    "set-cookie",
    "authorization",
    // Security headers (should not be overridden)
    "strict-transport-security",
    "content-security-policy",
    "x-frame-options",
    "x-content-type-options",
    "referrer-policy",
    "permissions-policy",
];

/// Validate a single header name and value.
///
/// # Arguments
///
/// * `name` - Header name
/// * `value` - Header value
///
/// # Returns
///
/// Validated `(HeaderName, String)` pair or `ApiError`.
///
/// # Errors
///
/// Returns `ApiError::InvalidField` if:
/// - Header name contains CRLF
/// - Header value contains CRLF
/// - Header name is invalid HTTP syntax
/// - Header is in the blocked list
pub fn validate_header(name: &str, value: &str) -> Result<(HeaderName, String), ApiError> {
    // Check for CRLF injection in name
    if name.contains('\r') || name.contains('\n') {
        return Err(ApiError::InvalidField {
            field: "extraHttpHeaders",
            message: format!(
                "Header name '{}' contains illegal CRLF character - possible header injection attack",
                name.chars().map(|c| if c == '\r' || c == '\n' { '?' } else { c }).collect::<String>()
            ),
        });
    }

    // Check for CRLF injection in value
    if value.contains('\r') || value.contains('\n') {
        return Err(ApiError::InvalidField {
            field: "extraHttpHeaders",
            message: format!(
                "Header '{}' value contains illegal CRLF character - possible header injection attack",
                name
            ),
        });
    }

    // Check for null bytes
    if name.contains('\0') || value.contains('\0') {
        return Err(ApiError::InvalidField {
            field: "extraHttpHeaders",
            message: format!(
                "Header '{}' contains null byte - possible header injection attack",
                name
            ),
        });
    }

    // Validate header name format
    let header_name = HeaderName::from_bytes(name.as_bytes()).map_err(|e| ApiError::InvalidField {
        field: "extraHttpHeaders",
        message: format!("Invalid header name '{}': {}", name, e),
    })?;

    // Check against blocked headers (case-insensitive)
    let name_lower = name.to_lowercase();
    for blocked in BLOCKED_HEADERS {
        if name_lower == *blocked {
            return Err(ApiError::InvalidField {
                field: "extraHttpHeaders",
                message: format!(
                    "Header '{}' cannot be overridden for security reasons (blocked: {})",
                    name, blocked
                ),
            });
        }
    }

    // Validate header value length (reasonable limit)
    if value.len() > 8192 {
        return Err(ApiError::InvalidField {
            field: "extraHttpHeaders",
            message: format!(
                "Header '{}' value too long: {} bytes (max 8192)",
                name, value.len()
            ),
        });
    }

    Ok((header_name, value.to_string()))
}

/// Validate a map of headers from JSON.
///
/// # Arguments
///
/// * `headers` - Map of header names to values (typically from `extraHttpHeaders` JSON)
///
/// # Returns
///
/// Validated headers as `Vec<(HeaderName, String)>` or `ApiError` on first failure.
///
/// # Errors
///
/// Returns `ApiError::InvalidField` if any header fails validation.
pub fn validate_headers_map(
    headers: &HashMap<String, String>,
) -> Result<Vec<(HeaderName, String)>, ApiError> {
    let mut result = Vec::with_capacity(headers.len());

    for (name, value) in headers {
        let validated = validate_header(name, value)?;
        result.push(validated);
    }

    Ok(result)
}

/// Parse and validate JSON-formatted extra headers.
///
/// Expects JSON like: `{"X-Custom": "value", "Accept-Language": "en"}`
///
/// # Arguments
///
/// * `json_str` - JSON string containing header map
///
/// # Returns
///
/// Validated headers or `ApiError` if JSON is invalid or headers fail validation.
pub fn parse_and_validate_extra_headers(
    json_str: &str,
) -> Result<Vec<(HeaderName, String)>, ApiError> {
    let parsed: HashMap<String, String> =
        serde_json::from_str(json_str).map_err(|e| ApiError::InvalidField {
            field: "extraHttpHeaders",
            message: format!("Invalid JSON for extraHttpHeaders: {}", e),
        })?;

    validate_headers_map(&parsed)
}

/// Maximum number of custom headers allowed.
pub const MAX_EXTRA_HEADERS: usize = 50;

/// Validate that the number of extra headers is within limits.
pub fn validate_header_count(count: usize) -> Result<(), ApiError> {
    if count > MAX_EXTRA_HEADERS {
        return Err(ApiError::InvalidField {
            field: "extraHttpHeaders",
            message: format!(
                "Too many extra headers: {} (max {})",
                count, MAX_EXTRA_HEADERS
            ),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_header_passes() {
        let result = validate_header("X-Custom-Header", "some value");
        assert!(result.is_ok());
        let (name, value) = result.unwrap();
        assert_eq!(name.as_str(), "x-custom-header");
        assert_eq!(value, "some value");
    }

    #[test]
    fn blocks_crlf_in_name() {
        let result = validate_header("X-Evil\r\nHost", "value");
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("CRLF"));
    }

    #[test]
    fn blocks_crlf_in_value() {
        let result = validate_header("X-Custom", "value\r\nHost: evil.com");
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("CRLF"));
    }

    #[test]
    fn blocks_null_byte() {
        let result = validate_header("X-Custom\0", "value");
        assert!(result.is_err());
    }

    #[test]
    fn blocks_security_headers() {
        let blocked = vec![
            "Host",
            "Content-Length",
            "Transfer-Encoding",
            "Connection",
            "Cookie",
            "Authorization",
            "Proxy-Authorization",
        ];

        for header in blocked {
            let result = validate_header(header, "value");
            assert!(result.is_err(), "Should block '{}'", header);
        }
    }

    #[test]
    fn allows_common_safe_headers() {
        let allowed = vec![
            "Accept",
            "Accept-Language",
            "Accept-Encoding",
            "Cache-Control",
            "If-Modified-Since",
            "If-None-Match",
            "User-Agent",
            "X-Requested-With",
            "X-Custom-Header",
            "Origin",
            "Referer",
        ];

        for header in allowed {
            let result = validate_header(header, "value");
            assert!(result.is_ok(), "Should allow '{}'", header);
        }
    }

    #[test]
    fn header_value_length_limit() {
        let long_value = "a".repeat(9000);
        let result = validate_header("X-Custom", &long_value);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("too long"));
    }

    #[test]
    fn validate_headers_map_test() {
        let mut headers = HashMap::new();
        headers.insert("X-Custom".to_string(), "value1".to_string());
        headers.insert("Accept-Language".to_string(), "en-US".to_string());

        let result = validate_headers_map(&headers);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 2);
    }

    #[test]
    fn parse_json_headers() {
        let json = r#"{"X-Custom": "value", "Accept": "application/json"}"#;
        let result = parse_and_validate_extra_headers(json);
        assert!(result.is_ok());
        let headers = result.unwrap();
        assert_eq!(headers.len(), 2);
    }

    #[test]
    fn parse_invalid_json_fails() {
        let json = r#"{"invalid": json}"#;
        let result = parse_and_validate_extra_headers(json);
        assert!(result.is_err());
    }

    #[test]
    fn header_count_limit() {
        assert!(validate_header_count(10).is_ok());
        assert!(validate_header_count(50).is_ok());
        assert!(validate_header_count(51).is_err());
    }
}
