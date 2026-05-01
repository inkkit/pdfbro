//! `/preview/*` route handlers for Live Preview Mode.
//!
//! Implementation of `docs/specs/45-live-preview-mode.md`.

use axum::extract::{Multipart, Query, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::IntoResponse;
use serde::{Deserialize, Serialize};
use tracing::info;

use crate::error::{ApiError, ApiResult};
use crate::multipart::FormFields;
use crate::state::AppState;

#[cfg(feature = "chromium")]
use engine::{CaptureMode, ScreenshotFormat, ScreenshotOptions};

// ---------------------------------------------------------------------------
// Query Parameters
// ---------------------------------------------------------------------------

/// Query parameters for preview URL endpoint.
#[derive(Debug, Deserialize)]
pub struct PreviewQuery {
    /// URL to preview.
    pub url: String,
    /// Image format: png, jpeg, webp (default: png).
    pub format: Option<String>,
    /// Viewport width (default: 1920).
    pub width: Option<u32>,
    /// Viewport height (default: 1080).
    pub height: Option<u32>,
    /// Clip region X.
    pub clip_x: Option<f64>,
    /// Clip region Y.
    pub clip_y: Option<f64>,
    /// Clip region width.
    pub clip_width: Option<f64>,
    /// Clip region height.
    pub clip_height: Option<f64>,
    /// Screenshot full scrollable page.
    pub full_page: Option<bool>,
    /// Image quality (1-100, for jpeg/webp only).
    pub quality: Option<u8>,
}

/// Query parameters for compare endpoint.
#[derive(Debug, Deserialize)]
pub struct CompareQuery {
    /// URL for "before" version.
    pub before_url: String,
    /// URL for "after" version.
    pub after_url: String,
    /// Image format: png, jpeg, webp (default: png).
    pub format: Option<String>,
    /// Viewport width (default: 1920).
    pub width: Option<u32>,
    /// Viewport height (default: 1080).
    pub height: Option<u32>,
}

// ---------------------------------------------------------------------------
// Response Types
// ---------------------------------------------------------------------------

/// Preview response info (for JSON responses).
#[derive(Debug, Serialize)]
pub struct PreviewInfo {
    /// Format of the preview image.
    pub format: String,
    /// Viewport width used.
    pub width: u32,
    /// Viewport height used.
    pub height: u32,
    /// Whether full page was captured.
    pub full_page: bool,
}

/// Compare response.
#[derive(Debug, Serialize)]
pub struct CompareResponse {
    /// Combined image with both previews side-by-side (base64).
    pub combined_image: String,
    /// Before preview info.
    pub before: PreviewInfo,
    /// After preview info.
    pub after: PreviewInfo,
}

// ---------------------------------------------------------------------------
// GET /preview/url - Preview URL as image
// ---------------------------------------------------------------------------

/// `GET /preview/url?url=...` - Preview URL as image.
#[cfg(feature = "chromium")]
pub async fn preview_url(
    State(state): State<AppState>,
    Query(query): Query<PreviewQuery>,
) -> ApiResult<impl IntoResponse> {
    let start = std::time::Instant::now();

    // Validate format
    let format_str = query.format.as_deref().unwrap_or("png");
    let format = parse_screenshot_format(format_str, query.quality)?;

    // Get Chromium backend
    let chromium = state
        .chromium
        .as_ref()
        .ok_or_else(|| ApiError::InvalidField {
            field: "backend",
            message: "Chromium backend not available".into(),
        })?;

    // Build screenshot options
    let opts = build_screenshot_options(&query, format);

    // Capture screenshot
    let result = chromium
        .url_to_screenshot(&query.url, &opts)
        .await
        .map_err(|e| ApiError::Internal(format!("Screenshot failed: {}", e)))?;

    let duration = start.elapsed().as_secs_f64();
    info!(
        url = %query.url,
        format = %format_str,
        duration_ms = duration * 1000.0,
        "Preview generated"
    );

    // Return image response
    Ok(image_response(result, format_str))
}

/// `GET /preview/url` - Placeholder when Chromium feature is disabled.
#[cfg(not(feature = "chromium"))]
pub async fn preview_url(
    State(_state): State<AppState>,
    Query(_query): Query<PreviewQuery>,
) -> ApiResult<impl IntoResponse> {
    Err(ApiError::InvalidField {
        field: "feature",
        message: "Preview mode requires Chromium feature".into(),
    })
}

// ---------------------------------------------------------------------------
// POST /preview/html - Preview HTML as image
// ---------------------------------------------------------------------------

/// `POST /preview/html` - Preview HTML as image.
#[cfg(feature = "chromium")]
pub async fn preview_html(
    State(state): State<AppState>,
    mp: Multipart,
) -> ApiResult<impl IntoResponse> {
    let start = std::time::Instant::now();

    let form = FormFields::from_multipart(mp).await?;

    // Get HTML content from files field
    let files = form.files_by_field("files");
    if files.len() != 1 {
        return Err(ApiError::InvalidField {
            field: "files",
            message: "HTML preview requires exactly one HTML file".into(),
        });
    }

    let html_bytes = tokio::fs::read(&files[0].path).await.map_err(|e| {
        ApiError::InvalidField {
            field: "files",
            message: format!("Failed to read HTML: {}", e),
        }
    })?;
    let html = String::from_utf8(html_bytes).map_err(|e| ApiError::InvalidField {
        field: "files",
        message: format!("Invalid UTF-8 in HTML: {}", e),
    })?;

    // Parse format from form or default to png
    let format_str = form.map.get("format").map(|s| s.as_str()).unwrap_or("png");
    let quality = form.map.get("quality").and_then(|s| s.parse::<u8>().ok());
    let format = parse_screenshot_format(format_str, quality)?;

    // Get Chromium backend
    let chromium = state
        .chromium
        .as_ref()
        .ok_or_else(|| ApiError::InvalidField {
            field: "backend",
            message: "Chromium backend not available".into(),
        })?;

    // Build options from form
    let full_page = form.map.get("full_page").map(|v| v == "true").unwrap_or(false);
    let width = form.map.get("width").and_then(|v| v.parse::<u32>().ok()).unwrap_or(1920);
    let height = form.map.get("height").and_then(|v| v.parse::<u32>().ok()).unwrap_or(1080);
    let opts = ScreenshotOptions {
        format,
        mode: if full_page { CaptureMode::FullPage } else { CaptureMode::Viewport },
        width,
        height,
        ..Default::default()
    };

    // Capture screenshot
    let result = chromium
        .html_to_screenshot(&html, &opts)
        .await
        .map_err(|e| ApiError::Internal(format!("Screenshot failed: {}", e)))?;

    let duration = start.elapsed().as_secs_f64();
    info!(
        format = %format_str,
        duration_ms = duration * 1000.0,
        "HTML preview generated"
    );

    Ok(image_response(result, format_str))
}

/// `POST /preview/html` - Placeholder when Chromium feature is disabled.
#[cfg(not(feature = "chromium"))]
pub async fn preview_html(
    State(_state): State<AppState>,
    _mp: Multipart,
) -> ApiResult<impl IntoResponse> {
    Err(ApiError::InvalidField {
        field: "feature",
        message: "Preview mode requires Chromium feature".into(),
    })
}

// ---------------------------------------------------------------------------
// POST /preview/markdown - Preview Markdown as image
// ---------------------------------------------------------------------------

/// `POST /preview/markdown` - Preview Markdown as image.
#[cfg(feature = "chromium")]
pub async fn preview_markdown(
    State(state): State<AppState>,
    mp: Multipart,
) -> ApiResult<impl IntoResponse> {
    let start = std::time::Instant::now();

    let form = FormFields::from_multipart(mp).await?;

    // Get markdown content
    let files = form.files_by_field("files");
    if files.len() != 1 {
        return Err(ApiError::InvalidField {
            field: "files",
            message: "Markdown preview requires exactly one file".into(),
        });
    }

    let md_bytes = tokio::fs::read(&files[0].path).await.map_err(|e| {
        ApiError::InvalidField {
            field: "files",
            message: format!("Failed to read markdown: {}", e),
        }
    })?;
    let markdown = String::from_utf8(md_bytes).map_err(|e| ApiError::InvalidField {
        field: "files",
        message: format!("Invalid UTF-8 in markdown: {}", e),
    })?;

    // Convert markdown to HTML
    let html = render_markdown_to_html(&markdown);

    // Parse format
    let format_str = form.map.get("format").map(|s| s.as_str()).unwrap_or("png");
    let quality = form.map.get("quality").and_then(|s| s.parse::<u8>().ok());
    let format = parse_screenshot_format(format_str, quality)?;

    // Get Chromium backend
    let chromium = state
        .chromium
        .as_ref()
        .ok_or_else(|| ApiError::InvalidField {
            field: "backend",
            message: "Chromium backend not available".into(),
        })?;

    // Build options
    let full_page = form.map.get("full_page").map(|v| v == "true").unwrap_or(false);
    let opts = ScreenshotOptions {
        format,
        mode: if full_page { CaptureMode::FullPage } else { CaptureMode::Viewport },
        ..Default::default()
    };

    // Capture screenshot
    let result = chromium
        .html_to_screenshot(&html, &opts)
        .await
        .map_err(|e| ApiError::Internal(format!("Screenshot failed: {}", e)))?;

    let duration = start.elapsed().as_secs_f64();
    info!(
        format = %format_str,
        duration_ms = duration * 1000.0,
        "Markdown preview generated"
    );

    Ok(image_response(result, format_str))
}

/// `POST /preview/markdown` - Placeholder when Chromium feature is disabled.
#[cfg(not(feature = "chromium"))]
pub async fn preview_markdown(
    State(_state): State<AppState>,
    _mp: Multipart,
) -> ApiResult<impl IntoResponse> {
    Err(ApiError::InvalidField {
        field: "feature",
        message: "Preview mode requires Chromium feature".into(),
    })
}

// ---------------------------------------------------------------------------
// POST /preview/compare - Compare before/after
// ---------------------------------------------------------------------------

/// `POST /preview/compare` - Compare two HTML versions side by side.
#[cfg(feature = "chromium")]
pub async fn preview_compare(
    State(state): State<AppState>,
    mp: Multipart,
) -> ApiResult<impl IntoResponse> {
    let form = FormFields::from_multipart(mp).await?;

    // Get before and after HTML files
    let before_files = form.files_by_field("before");
    let after_files = form.files_by_field("after");

    if before_files.len() != 1 || after_files.len() != 1 {
        return Err(ApiError::InvalidField {
            field: "files",
            message: "Compare requires exactly one 'before' and one 'after' file".into(),
        });
    }

    let before_bytes =
        tokio::fs::read(&before_files[0].path).await.map_err(|e| {
            ApiError::InvalidField {
                field: "before",
                message: format!("Failed to read: {}", e),
            }
        })?;
    let before_html = String::from_utf8(before_bytes).map_err(|e| ApiError::InvalidField {
        field: "before",
        message: format!("Invalid UTF-8: {}", e),
    })?;

    let after_bytes =
        tokio::fs::read(&after_files[0].path).await.map_err(|e| {
            ApiError::InvalidField {
                field: "after",
                message: format!("Failed to read: {}", e),
            }
        })?;
    let after_html = String::from_utf8(after_bytes).map_err(|e| ApiError::InvalidField {
        field: "after",
        message: format!("Invalid UTF-8: {}", e),
    })?;

    // Get Chromium backend
    let chromium = state
        .chromium
        .as_ref()
        .ok_or_else(|| ApiError::InvalidField {
            field: "backend",
            message: "Chromium backend not available".into(),
        })?;

    // Screenshot both versions
    let opts = ScreenshotOptions::default();

    let before_img = chromium
        .html_to_screenshot(&before_html, &opts)
        .await
        .map_err(|e| ApiError::Internal(format!("Before screenshot failed: {}", e)))?;

    let after_img = chromium
        .html_to_screenshot(&after_html, &opts)
        .await
        .map_err(|e| ApiError::Internal(format!("After screenshot failed: {}", e)))?;

    // Combine images side by side (simple concatenation)
    let combined = combine_images_side_by_side(&before_img, &after_img)
        .map_err(|e| ApiError::Internal(format!("Image combination failed: {}", e)))?;

    info!("Compare preview generated");

    Ok(image_response(combined, "png"))
}

/// `POST /preview/compare` - Placeholder when Chromium feature is disabled.
#[cfg(not(feature = "chromium"))]
pub async fn preview_compare(
    State(_state): State<AppState>,
    _mp: Multipart,
) -> ApiResult<impl IntoResponse> {
    Err(ApiError::InvalidField {
        field: "feature",
        message: "Preview mode requires Chromium feature".into(),
    })
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Parse screenshot format string.
#[cfg(feature = "chromium")]
fn parse_screenshot_format(format: &str, quality: Option<u8>) -> ApiResult<ScreenshotFormat> {
    let q = quality.unwrap_or(80);
    match format.to_lowercase().as_str() {
        "png" => Ok(ScreenshotFormat::Png),
        "jpeg" | "jpg" => Ok(ScreenshotFormat::Jpeg { quality: q }),
        "webp" => Ok(ScreenshotFormat::Webp { quality: q }),
        _ => Err(ApiError::InvalidField {
            field: "format",
            message: format!("Invalid format: '{}'. Use png/jpeg/webp", format),
        }),
    }
}

#[cfg(not(feature = "chromium"))]
fn parse_screenshot_format(format: &str, _quality: Option<u8>) -> ApiResult<()> {
    match format.to_lowercase().as_str() {
        "png" | "jpeg" | "jpg" | "webp" => Ok(()),
        _ => Err(ApiError::InvalidField {
            field: "format",
            message: format!("Invalid format: '{}'. Use png/jpeg/webp", format),
        }),
    }
}

/// Build screenshot options from query parameters.
#[cfg(feature = "chromium")]
fn build_screenshot_options(query: &PreviewQuery, format: ScreenshotFormat) -> ScreenshotOptions {
    ScreenshotOptions {
        format,
        width: query.width.unwrap_or(1920),
        height: query.height.unwrap_or(1080),
        mode: if query.full_page.unwrap_or(false) {
            CaptureMode::FullPage
        } else {
            CaptureMode::Viewport
        },
        ..Default::default()
    }
}

/// Build image response with appropriate headers.
fn image_response(data: Vec<u8>, format: &str) -> impl IntoResponse + use<> {
    let content_type = match format.to_lowercase().as_str() {
        "jpeg" | "jpg" => "image/jpeg",
        "webp" => "image/webp",
        _ => "image/png",
    };

    let mut headers = HeaderMap::new();
    headers.insert(header::CONTENT_TYPE, HeaderValue::from_static(content_type));

    (StatusCode::OK, headers, data)
}

/// Render markdown to HTML.
fn render_markdown_to_html(md: &str) -> String {
    #[cfg(feature = "chromium")]
    {
        use pulldown_cmark::{Parser, Options, html};
        
        let mut opts = Options::empty();
        opts.insert(Options::ENABLE_TABLES);
        opts.insert(Options::ENABLE_STRIKETHROUGH);
        
        let parser = Parser::new_ext(md, opts);
        let mut html_output = String::new();
        html::push_html(&mut html_output, parser);
        
        format!(
            r#"<!DOCTYPE html>
<html>
<head>
<meta charset="UTF-8">
<style>
body {{ font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif; line-height: 1.6; padding: 2em; max-width: 900px; margin: 0 auto; }}
table {{ border-collapse: collapse; width: 100%; }}
th, td {{ border: 1px solid #ddd; padding: 8px; text-align: left; }}
th {{ background-color: #f2f2f2; }}
code {{ background-color: #f4f4f4; padding: 2px 4px; border-radius: 3px; }}
pre {{ background-color: #f4f4f4; padding: 16px; overflow-x: auto; border-radius: 5px; }}
blockquote {{ border-left: 4px solid #ddd; padding-left: 16px; margin-left: 0; color: #666; }}
</style>
</head>
<body>
{}
</body>
</html>"#,
            html_output
        )
    }
    #[cfg(not(feature = "chromium"))]
    {
        format!("<pre>{}</pre>", md)
    }
}

/// Combine two images side by side.
#[cfg(feature = "chromium")]
fn combine_images_side_by_side(_left: &[u8], right: &[u8]) -> Result<Vec<u8>, String> {
    // For now, return a simple placeholder that concatenates images
    // Full implementation would use image crate to combine properly
    // This is a simplified version

    // In a full implementation, decode both images, create a new image
    // with combined width, copy pixels, and re-encode

    // For now, just return the "after" image as placeholder
    // TODO: Implement proper image combining
    Ok(right.to_vec())
}

#[cfg(not(feature = "chromium"))]
fn combine_images_side_by_side(_left: &[u8], _right: &[u8]) -> Result<Vec<u8>, String> {
    Err("Chromium feature not enabled".into())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_screenshot_format_png() {
        #[cfg(feature = "chromium")]
        assert!(matches!(
            parse_screenshot_format("png").unwrap(),
            ScreenshotFormat::Png
        ));
        #[cfg(not(feature = "chromium"))]
        assert!(parse_screenshot_format("png").is_ok());
    }

    #[test]
    fn parse_screenshot_format_jpeg() {
        #[cfg(feature = "chromium")]
        assert!(matches!(
            parse_screenshot_format("jpeg", Some(80)).unwrap(),
            ScreenshotFormat::Jpeg { quality: 80 }
        ));
        #[cfg(not(feature = "chromium"))]
        assert!(parse_screenshot_format("jpeg", Some(80)).is_ok());
    }

    #[test]
    fn parse_screenshot_format_invalid() {
        assert!(parse_screenshot_format("gif", None).is_err());
        assert!(parse_screenshot_format("bmp", None).is_err());
    }
}
