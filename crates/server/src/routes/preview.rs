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
    let format = parse_screenshot_format(format_str)?;

    // Get Chromium backend
    let chromium = state
        .chromium
        .as_ref()
        .ok_or_else(|| ApiError::InvalidOption("Chromium backend not available".into()))?;

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
    Err(ApiError::InvalidOption(
        "Preview mode requires Chromium feature".into(),
    ))
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

    let html_bytes = crate::routes::util::read_file_to_vec(&files[0].path).await.map_err(|e| {
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
    let format = parse_screenshot_format(format_str)?;

    // Get Chromium backend
    let chromium = state
        .chromium
        .as_ref()
        .ok_or_else(|| ApiError::InvalidOption("Chromium backend not available".into()))?;

    // Build options from form
    let mut opts = ScreenshotOptions::for_pdf(&Default::default());
    opts.format = format;
    opts.capture_mode = if form.map.get("full_page").map(|v| v == "true").unwrap_or(false) {
        CaptureMode::FullPage
    } else {
        CaptureMode::Viewport
    };

    // Viewport sizing
    if let Some(w) = form.map.get("width").and_then(|v| v.parse::<u32>().ok()) {
        opts.viewport.width = w;
    }
    if let Some(h) = form.map.get("height").and_then(|v| v.parse::<u32>().ok()) {
        opts.viewport.height = h;
    }

    // Capture screenshot
    let result = chromium
        .html_to_screenshot(&html, None, &opts)
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
    Err(ApiError::InvalidOption(
        "Preview mode requires Chromium feature".into(),
    ))
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

    let md_bytes = crate::routes::util::read_file_to_vec(&files[0].path).await.map_err(|e| {
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
    let html = crate::routes::chromium::render_markdown_to_html(&markdown);

    // Parse format
    let format_str = form.map.get("format").map(|s| s.as_str()).unwrap_or("png");
    let format = parse_screenshot_format(format_str)?;

    // Get Chromium backend
    let chromium = state
        .chromium
        .as_ref()
        .ok_or_else(|| ApiError::InvalidOption("Chromium backend not available".into()))?;

    // Build options
    let mut opts = ScreenshotOptions::for_pdf(&Default::default());
    opts.format = format;
    opts.capture_mode = if form.map.get("full_page").map(|v| v == "true").unwrap_or(false) {
        CaptureMode::FullPage
    } else {
        CaptureMode::Viewport
    };

    // Capture screenshot
    let result = chromium
        .html_to_screenshot(&html, None, &opts)
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
    Err(ApiError::InvalidOption(
        "Preview mode requires Chromium feature".into(),
    ))
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
        crate::routes::util::read_file_to_vec(&before_files[0].path).await.map_err(|e| {
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
        crate::routes::util::read_file_to_vec(&after_files[0].path).await.map_err(|e| {
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
        .ok_or_else(|| ApiError::InvalidOption("Chromium backend not available".into()))?;

    // Screenshot both versions
    let opts = ScreenshotOptions::for_pdf(&Default::default());

    let before_img = chromium
        .html_to_screenshot(&before_html, None, &opts)
        .await
        .map_err(|e| ApiError::Internal(format!("Before screenshot failed: {}", e)))?;

    let after_img = chromium
        .html_to_screenshot(&after_html, None, &opts)
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
    Err(ApiError::InvalidOption(
        "Preview mode requires Chromium feature".into(),
    ))
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Parse screenshot format string.
#[cfg(feature = "chromium")]
fn parse_screenshot_format(format: &str) -> ApiResult<ScreenshotFormat> {
    match format.to_lowercase().as_str() {
        "png" => Ok(ScreenshotFormat::Png),
        "jpeg" | "jpg" => Ok(ScreenshotFormat::Jpeg),
        "webp" => Ok(ScreenshotFormat::Webp),
        _ => Err(ApiError::InvalidOption(format!(
            "Invalid format: '{}'. Use png/jpeg/webp",
            format
        ))),
    }
}

#[cfg(not(feature = "chromium"))]
fn parse_screenshot_format(format: &str) -> ApiResult<()> {
    match format.to_lowercase().as_str() {
        "png" | "jpeg" | "jpg" | "webp" => Ok(()),
        _ => Err(ApiError::InvalidOption(format!(
            "Invalid format: '{}'. Use png/jpeg/webp",
            format
        ))),
    }
}

/// Build screenshot options from query parameters.
#[cfg(feature = "chromium")]
fn build_screenshot_options(query: &PreviewQuery, format: ScreenshotFormat) -> ScreenshotOptions {
    let mut opts = ScreenshotOptions::for_pdf(&Default::default());
    opts.format = format;

    // Viewport size
    if let Some(w) = query.width {
        opts.viewport.width = w;
    }
    if let Some(h) = query.height {
        opts.viewport.height = h;
    }

    // Full page capture
    opts.capture_mode = if query.full_page.unwrap_or(false) {
        CaptureMode::FullPage
    } else {
        CaptureMode::Viewport
    };

    // Clip region (if specified)
    if let (Some(x), Some(y), Some(w), Some(h)) =
        (query.clip_x, query.clip_y, query.clip_width, query.clip_height)
    {
        opts.clip_rect = Some(engine::ClipRect {
            x,
            y,
            width: w,
            height: h,
        });
    }

    opts
}

/// Build image response with appropriate headers.
fn image_response(data: Vec<u8>, format: &str) -> impl IntoResponse {
    let content_type = match format.to_lowercase().as_str() {
        "jpeg" | "jpg" => "image/jpeg",
        "webp" => "image/webp",
        _ => "image/png",
    };

    let mut headers = HeaderMap::new();
    headers.insert(header::CONTENT_TYPE, HeaderValue::from_static(content_type));

    (StatusCode::OK, headers, data)
}

/// Combine two images side by side.
#[cfg(feature = "chromium")]
fn combine_images_side_by_side(left: &[u8], right: &[u8]) -> Result<Vec<u8>, String> {
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
            parse_screenshot_format("jpeg").unwrap(),
            ScreenshotFormat::Jpeg
        ));
        #[cfg(not(feature = "chromium"))]
        assert!(parse_screenshot_format("jpeg").is_ok());
    }

    #[test]
    fn parse_screenshot_format_invalid() {
        assert!(parse_screenshot_format("gif").is_err());
        assert!(parse_screenshot_format("bmp").is_err());
    }

    #[test]
    fn image_response_content_types() {
        let data = vec![0u8; 10];

        let (status, headers, _) = image_response(data.clone(), "png");
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            headers.get(header::CONTENT_TYPE).unwrap(),
            "image/png"
        );

        let (_, headers, _) = image_response(data.clone(), "jpeg");
        assert_eq!(
            headers.get(header::CONTENT_TYPE).unwrap(),
            "image/jpeg"
        );

        let (_, headers, _) = image_response(data.clone(), "webp");
        assert_eq!(
            headers.get(header::CONTENT_TYPE).unwrap(),
            "image/webp"
        );
    }
}
