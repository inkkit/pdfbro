# Spec 44 — Crystal-Clear Error Messages

> Replace generic "500 Internal Server Error" with actionable,
> structured error responses. Addresses Gotenberg issues #1356,
> #921, #1926 where users get opaque errors with no guidance.

## Goal

Transform error handling from generic HTTP status codes to
rich, actionable error responses that tell users exactly
what went wrong and how to fix it. This is the #3 complaint
across all PDF generation tools.

## Problem Analysis

### Real User Quotes

> "Including web fonts in header or footer will cause 500
> Error / Printing failed (-32000)... I feel Gotenberg should
> ignore it without performance impact or we should update the
> docs to reflect that."
> — Issue #1356

> "I've noticed some problems with converting html to pdf: for
> some reason the numbers 6 and 8 get a bigger font size
> than other numbers... I suppose a workaround could be to
> rebuild the Docker container"
> — Issue #921

> "Testing HTML / CSS fails to render correctly... it fails
> to render correctly. I am not sure where to start because
> it generated no error messages."
> — WeasyPrint issue #1926

### Current State (Bad)

```json
{
  "error": "Printing failed (-32000)",
  "code": "INTERNAL"
}
```

### Desired State (Good)

```json
{
  "error": "PDF generation failed: image not loaded",
  "code": "RESOURCE_TIMEOUT",
  "details": {
    "url": "https://cdn.example.com/image.png",
    "timeout_ms": 30000,
    "suggestion": "Add --form 'waitDelay=5s' or check URL accessibility"
  },
  "documentation": "https://folio.dev/docs/troubleshooting#image-not-loaded"
}
```

## Scope

**In:**

- Structured error responses with suggestions
- Error code taxonomy (not just INTERNAL)
- Suggestions field with fix instructions
- Documentation links for each error type
- Field-level validation errors
- Resource-level error details (which URL failed)
- Stack trace in debug mode only

**Out:**

- Exposing internal paths (security risk)
- Full Chromium logs in production
- Arbitrary error message from engine (sanitisation needed)

## Error Code Taxonomy

### Conversion Errors

| Code | HTTP Status | Description | Suggestion |
|------|-------------|-------------|------------|
| `NAVIGATION` | 502 | Failed to navigate to URL | Check URL accessibility |
| `TIMEOUT` | 504 | Conversion timed out | Increase `--request-timeout` |
| `INVALID_OPTION` | 400 | Bad form field value | Check field format |
| `INVALID_PAGE_RANGE` | 400 | Bad page range syntax | Use format "1-5,7" |
| `RESOURCE_TIMEOUT` | 502 | Sub-resource failed to load | Check CDN/network |
| `RESOURCE_404` | 502 | Sub-resource not found | Fix missing images/CSS |
| `CHROMIUM_CRASH` | 503 | Chromium process died | Restart or check memory |
| `LIBREOFFICE_CRASH` | 503 | LibreOffice failed | Check document format |
| `FONT_MISSING` | 200 + warning | Font not installed | Install font in Docker |
| `WEB_FONT_BLOAT` | 200 + warning | Web font increases size | Use local fonts |

### Validation Errors

| Code | Description | Suggestion |
|------|-------------|------------|
| `MISSING_FIELD` | Required field not provided | Add `files` or `url` field |
| `INVALID_PAPER_SIZE` | Bad paper dimensions | Use format "8.5,11" or "A4" |
| `INVALID_MARGIN` | Bad margin value | Use float like "1.0" |
| `INVALID_BOOL` | Not true/false | Use "true" or "false" |
| `INVALID_JSON` | Bad JSON in field | Check JSON syntax |

## Implementation

### 1. Enhanced Error Type

```rust
// crates/engine/src/error.rs

#[derive(Debug, Clone, Serialize)]
pub struct ApiErrorResponse {
    pub error: String,
    pub code: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<ErrorDetails>,
    #[serde(skip_serialising_if = "Option::is_none")]
    pub suggestion: Option<String>,
    #[serde(skip_serialising_if = "Option::is_none")]
    pub documentation: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ErrorDetails {
    pub url: Option<String>,
    pub timeout_ms: Option<u64>,
    pub field: Option<String>,
    pub value: Option<String>,
    pub resource_errors: Option<Vec<ResourceError>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ResourceError {
    pub url: String,
    pub status_code: Option<u16>,
    pub error: String,
}

impl ApiError {
    pub fn to_response(&self) -> (StatusCode, Json<ApiErrorResponse>) {
        match self {
            ApiError::Navigation { url, reason } => (
                StatusCode::BAD_GATEWAY,
                Json(ApiErrorResponse {
                    error: format!("Navigation failed: {}", reason),
                    code: "NAVIGATION".into(),
                    details: Some(ErrorDetails {
                        url: Some(url.clone()),
                        ..Default::default()
                    }),
                    suggestion: Some(format!(
                        "Check that {} is accessible. Try with waitDelay=5s",
                        url
                    )),
                    documentation: Some(
                        "https://folio.dev/docs/troubleshooting#navigation-failed".into()
                    ),
                })
            ),

            ApiError::Timeout(duration) => (
                StatusCode::GATEWAY_TIMEOUT,
                Json(ApiErrorResponse {
                    error: "Conversion timed out".into(),
                    code: "TIMEOUT".into(),
                    details: Some(ErrorDetails {
                        timeout_ms: Some(duration.as_millis() as u64),
                        ..Default::default()
                    }),
                    suggestion: Some(format!(
                        "Increase timeout: --request-timeout {}s",
                        duration.as_secs() * 2
                    )),
                    documentation: Some(
                        "https://folio.dev/docs/troubleshooting#timeout".into()
                    ),
                })
            ),

            ApiError::InvalidOption(msg) => (
                StatusCode::BAD_REQUEST,
                Json(ApiErrorResponse {
                    error: msg.clone(),
                    code: "INVALID_OPTION".into(),
                    suggestion: Some(
                        "Check field format in documentation".into()
                    ),
                    documentation: Some(
                        "https://folio.dev/docs/api#form-fields".into()
                    ),
                    ..Default::default()
                })
            ),

            // ... handle all error variants
        }
    }
}
```

### 2. Resource Error Collection

```rust
// In chromium/mod.rs - collect resource errors

struct ResourceErrorCollector {
    errors: Vec<ResourceError>,
}

impl ResourceErrorCollector {
    fn new() -> Self {
        Self { errors: Vec::new() }
    }

    async fn monitor_page(&mut self, page: &Page) {
        // Listen for failed requests
        page.event_listener::<RequestFailed>()
            .await
            .for_each(|event| {
                if let Some(status) = event.response_status {
                    if status >= 400 {
                        self.errors.push(ResourceError {
                            url: event.request_url.unwrap_or_default(),
                            status_code: Some(status),
                            error: format!("HTTP {}", status),
                        });
                    }
                }
            });
    }

    fn into_api_error(self) -> Option<ApiError> {
        if self.errors.is_empty() {
            None
        } else {
            Some(ApiError::ResourceErrors(self.errors))
        }
    }
}
```

### 3. Field-Level Validation

```rust
// Improved form parsing with field-level errors

pub fn parse_paper_size(form: &HashMap<String, String>) -> Result<(f64, f64), ApiError> {
    let value = form.get("paperSize").ok_or_else(|| {
        ApiError::InvalidOption(
            "paperSize field is required".into()
        )
    })?;

    // Try named sizes
    let dimensions = match value.as_str() {
        "A4" => (210.0, 297.0),
        "Letter" => (215.9, 279.4),
        "Legal" => (215.9, 355.6),
        _ => {
            // Try "W,H" format
            let parts: Vec<&str> = value.split(',').collect();
            if parts.len() != 2 {
                return Err(ApiError::InvalidOption(
                    format!(
                        "Invalid paperSize: '{}'. Use 'A4', 'Letter', or 'W,H' format (e.g., '8.5,11')",
                        value
                    )
                ));
            }

            let w = parts[0].parse::<f64>().map_err(|_| {
                ApiError::InvalidOption(format!(
                    "Invalid paperSize width: '{}'. Must be a number",
                    parts[0]
                ))
            })?;

            let h = parts[1].parse::<f64>().map_err(|_| {
                ApiError::InvalidOption(format!(
                    "Invalid paperSize height: '{}'. Must be a number",
                    parts[1]
                ))
            })?;

            (w, h)
        }
    };

    Ok(dimensions)
}
```

### 4. Documentation Links

```rust
// Auto-generate documentation links

fn documentation_link(error_code: &str) -> String {
    match error_code {
        "NAVIGATION" => "https://folio.dev/docs/troubleshooting#navigation-failed",
        "TIMEOUT" => "https://folio.dev/docs/troubleshooting#timeout",
        "INVALID_OPTION" => "https://folio.dev/docs/api#form-fields",
        "RESOURCE_TIMEOUT" => "https://folio.dev/docs/troubleshooting#resource-failed",
        "CHROMIUM_CRASH" => "https://folio.dev/docs/troubleshooting#chromium-crash",
        _ => "https://folio.dev/docs/troubleshooting",
    }.into()
}
```

## Expected Behaviour

### Good Error (Resource Failed)

```json
{
  "error": "Image not loaded",
  "code": "RESOURCE_TIMEOUT",
  "details": {
    "url": "https://cdn.example.com/image.png",
    "timeout_ms": 30000
  },
  "suggestion": "Add --form 'waitDelay=5s' or check URL accessibility. CDN may be blocking requests.",
  "documentation": "https://folio.dev/docs/troubleshooting#resource-timeout"
}
```

### Good Error (Invalid Option)

```json
{
  "error": "Invalid paperSize: 'A5'. Use 'A4', 'Letter', or 'W,H' format (e.g., '8.5,11')",
  "code": "INVALID_OPTION",
  "details": {
    "field": "paperSize",
    "value": "A5"
  },
  "suggestion": "Valid values: A4, Letter, Legal, or 'W,H' (e.g., '8.5,11')",
  "documentation": "https://folio.dev/docs/api#form-fields"
}
```

### Warning (Not Error)

```json
{
  "result": "ok",
  "warnings": [
    {
      "code": "FONT_MISSING",
      "message": "Font 'Comic Sans' not installed",
      "suggestion": "Install in Docker: apt-get install fonts-comic-sans"
    }
  ]
}
```

## Test Plan

### Unit Tests

- `error_response_has_suggestion_field`
- `resource_error_collection_captures_failed_requests`
- `field_validation_returns_helpful_message`
- `documentation_link_matches_error_code`

### Integration Tests

- `navigation_error_returns_url_in_details`
- `timeout_error_suggests_increasing_timeout`
- `invalid_option_error_shows_valid_values`
- `resource_errors_list_all_failed_urls`

## Acceptance

- [ ] `ApiErrorResponse` struct with all fields
- [ ] All error variants return structured responses
- [ ] Resource error collection in Chromium
- [ ] Field-level validation with suggestions
- [ ] Documentation links for each error type
- [ ] Unit tests for error formatting
- [ ] Integration tests for all error scenarios
- [ ] `cargo clippy -p server -- -D warnings` clean

## References

- Gotenberg issue #1356: https://github.com/gotenberg/gotenberg/issues/1356
- Gotenberg issue #921: https://github.com/gotenberg/gotenberg/issues/921
- WeasyPrint issue #1926: https://github.com/Kozea/WeasyPrint/issues/1926
- RFC 7807: Problem Details for HTTP APIs: https://tools.ietf.org/html/rfc7807
