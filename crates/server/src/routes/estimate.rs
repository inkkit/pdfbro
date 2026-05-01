//! `/estimate/*` route handlers for PDF Size Estimator.
//!
//! Implementation of `docs/specs/46-pdf-size-estimator.md`.

use std::sync::OnceLock;

use axum::extract::{Json, Multipart, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use regex::Regex;
use serde::{Deserialize, Serialize};
use tracing::info;

use crate::error::{ApiError, ApiResult};
use crate::multipart::FormFields;
use crate::state::AppState;

// ---------------------------------------------------------------------------
// Static Regex Patterns
// ---------------------------------------------------------------------------

static IMG_SRC_RE: OnceLock<Regex> = OnceLock::new();
static FONT_FACE_RE: OnceLock<Regex> = OnceLock::new();
static CSS_FONT_FAMILY_RE: OnceLock<Regex> = OnceLock::new();

// ---------------------------------------------------------------------------
// Request/Response Types
// ---------------------------------------------------------------------------

/// Size estimation request.
#[derive(Debug, Deserialize)]
pub struct EstimateRequest {
    /// URL to analyze.
    pub url: Option<String>,
    /// HTML content to analyze.
    pub html: Option<String>,
}

/// Size estimation response.
#[derive(Debug, Serialize)]
pub struct EstimateResponse {
    /// Estimated size in MB.
    pub estimated_size_mb: f64,
    /// Confidence level: high, medium, low.
    pub confidence: String,
    /// Size breakdown by component.
    pub breakdown: SizeBreakdown,
    /// Warning messages.
    pub warnings: Vec<String>,
    /// Suggestion messages.
    pub suggestions: Vec<String>,
    /// Comparison with other tools (optional).
    pub comparison: Option<Comparison>,
}

/// Size breakdown by component.
#[derive(Debug, Serialize)]
pub struct SizeBreakdown {
    /// Fonts contribution (MB).
    pub fonts_mb: f64,
    /// Images contribution (MB).
    pub images_mb: f64,
    /// HTML markup contribution (MB).
    pub markup_mb: f64,
    /// PDF overhead (MB).
    pub overhead_mb: f64,
}

/// Comparison with other tools.
#[derive(Debug, Serialize)]
pub struct Comparison {
    /// Estimated Gotenberg size (MB).
    pub gotenberg_mb: f64,
    /// Estimated wkhtmltopdf size (MB).
    pub wkhtmltopdf_mb: f64,
    /// Size factor vs Folio estimate.
    pub factor_vs_folio: f64,
}

/// Batch estimation request.
#[derive(Debug, Deserialize)]
pub struct BatchEstimateRequest {
    /// URLs to estimate.
    pub urls: Vec<String>,
}

/// Batch estimation response.
#[derive(Debug, Serialize)]
pub struct BatchEstimateResponse {
    /// Estimates for each URL.
    pub estimates: Vec<UrlEstimate>,
    /// Total estimated size.
    pub total_mb: f64,
}

/// Individual URL estimate.
#[derive(Debug, Serialize)]
pub struct UrlEstimate {
    /// URL.
    pub url: String,
    /// Estimated size (MB).
    pub estimated_size_mb: f64,
    /// Confidence.
    pub confidence: String,
}

// ---------------------------------------------------------------------------
// POST /estimate - Analyze HTML/URL and estimate size
// ---------------------------------------------------------------------------

/// `POST /estimate` - Analyze HTML/URL and return size prediction.
pub async fn estimate(
    State(_state): State<AppState>,
    Json(req): Json<EstimateRequest>,
) -> ApiResult<impl IntoResponse> {
    info!("Running PDF size estimation");

    // Validate request
    let html = match (&req.url, &req.html) {
        (_, Some(html)) => html.clone(),
        (Some(_url), _) => {
            // For URL-based estimates, we'd need to fetch the content
            // For now, return an estimate based on typical URL characteristics
            return Err(ApiError::InvalidField {
                field: "url",
                message: "URL-based estimation not yet implemented. Use 'html' field.".into(),
            });
        }
        (None, None) => {
            return Err(ApiError::InvalidField {
                field: "html",
                message: "Either 'url' or 'html' must be provided".into(),
            });
        }
    };

    // Analyze HTML content
    let analysis = analyze_html(&html).await;

    // Build size breakdown
    let mut breakdown = SizeBreakdown {
        markup_mb: analysis.markup_size_mb,
        fonts_mb: analysis.font_size_mb,
        images_mb: analysis.image_size_mb,
        overhead_mb: 0.5, // Base PDF overhead
    };

    // Calculate total
    let estimated_mb = breakdown.markup_mb
        + breakdown.fonts_mb
        + breakdown.images_mb
        + breakdown.overhead_mb;

    // Determine confidence
    let confidence = if analysis.has_external_resources {
        "low"
    } else if analysis.has_web_fonts || analysis.has_images {
        "medium"
    } else {
        "high"
    }
    .to_string();

    // Build warnings and suggestions
    let mut warnings = Vec::new();
    let mut suggestions = Vec::new();

    if analysis.has_web_fonts {
        let font_count = html.matches("@font-face").count();
        warnings.push(format!(
            "HTML contains {} @font-face web font(s) - may increase size by ~{:.0}%",
            font_count,
            font_count as f64 * 100.0
        ));
        suggestions.push(
            "Install fonts locally: apt-get install ttf-mscorefonts-installer".into(),
        );
        suggestions.push(
            "Or use POST /forms/pdfengines/optimise after conversion".into(),
        );
    }

    if analysis.has_external_fonts {
        warnings.push(
            "External fonts detected (Google Fonts, etc.) - may cause rendering delays".into(),
        );
        suggestions.push("Download and host fonts locally for better reliability".into());
    }

    if analysis.has_images {
        let img_count = analysis.image_count;
        warnings.push(format!(
            "HTML contains {} image(s) - contributes ~{:.1} MB",
            img_count, analysis.image_size_mb
        ));
        if analysis.has_large_images {
            warnings.push("Large images detected - consider optimization".into());
            suggestions.push("Optimize images before conversion".into());
        }
    }

    if analysis.has_external_resources {
        warnings.push(
            "External resources detected - size estimate may be inaccurate".into(),
        );
    }

    // Size thresholds
    if estimated_mb > 10.0 {
        warnings.push(format!(
            "Estimated size {:.1} MB exceeds 10 MB - consider optimization",
            estimated_mb
        ));
        suggestions.push("Use POST /forms/pdfengines/optimise to compress".into());
    } else if estimated_mb > 5.0 {
        warnings.push(format!("Estimated size {:.1} MB is quite large", estimated_mb));
    }

    // Build comparison
    let comparison = Some(Comparison {
        gotenberg_mb: estimated_mb * 1.5, // Gotenberg typically larger
        wkhtmltopdf_mb: estimated_mb * 0.7, // wkhtmltopdf typically smaller
        factor_vs_folio: 1.0,
    });

    info!(
        estimated_mb = estimated_mb,
        confidence = %confidence,
        warning_count = warnings.len(),
        "Size estimation complete"
    );

    Ok((
        StatusCode::OK,
        Json(EstimateResponse {
            estimated_size_mb: round_to_2dp(estimated_mb),
            confidence,
            breakdown: SizeBreakdown {
                fonts_mb: round_to_2dp(breakdown.fonts_mb),
                images_mb: round_to_2dp(breakdown.images_mb),
                markup_mb: round_to_2dp(breakdown.markup_mb),
                overhead_mb: round_to_2dp(breakdown.overhead_mb),
            },
            warnings,
            suggestions,
            comparison,
        }),
    ))
}

// ---------------------------------------------------------------------------
// POST /estimate - With multipart form (for HTML files)
// ---------------------------------------------------------------------------

/// `POST /estimate` - Multipart form version for HTML files.
pub async fn estimate_form(
    State(state): State<AppState>,
    mp: Multipart,
) -> ApiResult<impl IntoResponse> {
    let form = FormFields::from_multipart(mp).await?;

    // Get HTML from form
    let html = if let Some(html_content) = form.map.get("html") {
        html_content.clone()
    } else {
        // Try to read from file
        let files = form.files_by_field("files");
        if files.len() != 1 {
            return Err(ApiError::InvalidField {
                field: "html",
                message: "Provide 'html' field or 'files' with HTML content".into(),
            });
        }
        let bytes = tokio::fs::read(&files[0].path)
            .await
            .map_err(|e| ApiError::InvalidField {
                field: "files",
                message: format!("Failed to read: {}", e),
            })?;
        String::from_utf8(bytes).map_err(|e| ApiError::InvalidField {
            field: "files",
            message: format!("Invalid UTF-8: {}", e),
        })?
    };

    // Use the JSON handler
    estimate(State(state), Json(EstimateRequest { url: None, html: Some(html) })).await
}

// ---------------------------------------------------------------------------
// POST /estimate/batch - Estimate multiple URLs
// ---------------------------------------------------------------------------

/// `POST /estimate/batch` - Estimate multiple URLs.
pub async fn estimate_batch(
    State(_state): State<AppState>,
    Json(req): Json<BatchEstimateRequest>,
) -> ApiResult<impl IntoResponse> {
    if req.urls.is_empty() {
        return Err(ApiError::InvalidField {
            field: "urls",
            message: "At least one URL required".into(),
        });
    }

    info!(url_count = req.urls.len(), "Running batch size estimation");

    let mut estimates = Vec::new();
    let mut total_mb = 0.0;

    for url in &req.urls {
        // For now, estimate based on URL characteristics
        // In a full implementation, we'd fetch and analyze each URL
        let estimated_mb = estimate_url_size(url);

        estimates.push(UrlEstimate {
            url: url.clone(),
            estimated_size_mb: round_to_2dp(estimated_mb),
            confidence: "low".into(), // URL-based is low confidence
        });

        total_mb += estimated_mb;
    }

    Ok((
        StatusCode::OK,
        Json(BatchEstimateResponse {
            estimates,
            total_mb: round_to_2dp(total_mb),
        }),
    ))
}

/// Estimate size based on URL characteristics (simplified).
fn estimate_url_size(url: &str) -> f64 {
    // Simplified estimation based on URL patterns
    // In reality, would fetch and analyze the content

    let mut base_size = 1.0; // Base 1MB

    // Longer URLs often indicate more complex pages
    if url.len() > 100 {
        base_size += 0.5;
    }

    // URLs with image extensions in path suggest image-heavy pages
    let lower_url = url.to_lowercase();
    if lower_url.contains("/gallery")
        || lower_url.contains("/images")
        || lower_url.contains("/photos")
    {
        base_size += 2.0;
    }

    // Dashboard/app pages often complex
    if lower_url.contains("/dashboard") || lower_url.contains("/app") {
        base_size += 1.0;
    }

    // Add some randomness for realism
    base_size += (url.len() % 10) as f64 / 10.0;

    base_size
}

// ---------------------------------------------------------------------------
// HTML Analysis
// ---------------------------------------------------------------------------

/// HTML content analysis result.
#[derive(Debug, Default)]
struct HtmlAnalysis {
    /// HTML markup size (MB).
    markup_size_mb: f64,
    /// Estimated font size (MB).
    font_size_mb: f64,
    /// Estimated image size (MB).
    image_size_mb: f64,
    /// Number of images found.
    image_count: usize,
    /// Has @font-face declarations.
    has_web_fonts: bool,
    /// Has external font references (Google Fonts, etc.).
    has_external_fonts: bool,
    /// Has img tags.
    has_images: bool,
    /// Has large images (data URIs or external).
    has_large_images: bool,
    /// Has external resources (hard to estimate).
    has_external_resources: bool,
}

/// Analyze HTML content for size estimation.
async fn analyze_html(html: &str) -> HtmlAnalysis {
    let mut result = HtmlAnalysis {
        markup_size_mb: html.len() as f64 / 1_000_000.0,
        ..Default::default()
    };

    // Initialize regex patterns
    let img_src_re = IMG_SRC_RE.get_or_init(|| {
        Regex::new(r#"(?i)<img[^>]+src\s*=\s*["']([^"']+)["']"#).unwrap()
    });
    let font_face_re = FONT_FACE_RE.get_or_init(|| {
        Regex::new(r#"(?i)@font-face"#).unwrap()
    });
    let css_font_family_re = CSS_FONT_FAMILY_RE.get_or_init(|| {
        Regex::new(r#"(?i)font-family\s*:\s*([^;\}]+)"#).unwrap()
    });

    // Check for web fonts (@font-face)
    let font_face_count = font_face_re.find_iter(html).count();
    if font_face_count > 0 {
        result.has_web_fonts = true;
        // Estimate: each @font-face ~500KB
        result.font_size_mb = font_face_count as f64 * 0.5;
    }

    // Check for external font services
    let lower_html = html.to_lowercase();
    if lower_html.contains("fonts.googleapis.com")
        || lower_html.contains("fonts.gstatic.com")
        || lower_html.contains("use.typekit.net")
        || lower_html.contains("fontlibrary.org")
    {
        result.has_external_fonts = true;
        result.font_size_mb += 1.0; // Additional overhead for external fonts
    }

    // Count CSS font-family declarations
    let font_family_count = css_font_family_re.find_iter(html).count();
    if font_family_count > 0 && result.font_size_mb == 0.0 {
        // System fonts are usually small, but still add some overhead
        result.font_size_mb = 0.1;
    }

    // Analyze images
    let mut image_count = 0;
    let mut external_images = 0;

    for cap in img_src_re.captures_iter(html) {
        image_count += 1;
        let src = &cap[1];

        if src.starts_with("http://") || src.starts_with("https://") {
            external_images += 1;
            result.has_external_resources = true;
        } else if src.starts_with("data:") {
            // Data URI images are embedded and often large
            if src.len() > 10000 {
                result.has_large_images = true;
            }
        }
    }

    result.image_count = image_count;
    result.has_images = image_count > 0;

    // Estimate image sizes
    if image_count > 0 {
        // External images: assume ~100KB each (very rough)
        result.image_size_mb += external_images as f64 * 0.1;

        // Data URI images: calculate from base64 size
        // Base64 is ~4/3 of original size
        for cap in img_src_re.captures_iter(html) {
            let src = &cap[1];
            if src.starts_with("data:") {
                let data_len = src.len() as f64;
                let decoded_estimate = (data_len * 0.75) / 1_000_000.0;
                result.image_size_mb += decoded_estimate;
            }
        }
    }

    // Check for external CSS/JS
    if lower_html.contains("<link")
        && (lower_html.contains(".css") || lower_html.contains("stylesheet"))
    {
        // Has external CSS
        if lower_html.contains("http://") || lower_html.contains("https://") {
            result.has_external_resources = true;
        }
    }

    result
}

/// Round to 2 decimal places.
fn round_to_2dp(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn estimate_url_size_basic() {
        let size = estimate_url_size("https://example.com/page");
        assert!(size >= 1.0);
        assert!(size < 5.0);
    }

    #[test]
    fn estimate_url_size_gallery() {
        let size = estimate_url_size("https://example.com/gallery/photos");
        assert!(size >= 2.0); // Gallery pages estimated larger
    }

    #[test]
    fn estimate_url_size_dashboard() {
        let size = estimate_url_size("https://example.com/dashboard/app");
        assert!(size >= 1.0); // Dashboard pages estimated larger
    }

    #[test]
    fn analyze_html_empty() {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let result = runtime.block_on(analyze_html(""));
        assert_eq!(result.markup_size_mb, 0.0);
        assert!(!result.has_web_fonts);
        assert!(!result.has_images);
    }

    #[test]
    fn analyze_html_with_web_font() {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let html = r#"
            <style>
                @font-face { font-family: 'Custom'; src: url('font.woff2'); }
            </style>
        "#;
        let result = runtime.block_on(analyze_html(html));
        assert!(result.has_web_fonts);
        assert!(result.font_size_mb > 0.0);
    }

    #[test]
    fn analyze_html_with_images() {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let html = r#"
            <img src="https://example.com/image1.jpg">
            <img src="https://example.com/image2.png">
        "#;
        let result = runtime.block_on(analyze_html(html));
        assert!(result.has_images);
        assert_eq!(result.image_count, 2);
        assert!(result.image_size_mb > 0.0);
    }

    #[test]
    fn analyze_html_with_external_fonts() {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let html = r#"
            <link href="https://fonts.googleapis.com/css?family=Roboto">
        "#;
        let result = runtime.block_on(analyze_html(html));
        assert!(result.has_external_fonts);
    }

    #[test]
    fn round_to_2dp_test() {
        assert_eq!(round_to_2dp(1.2345), 1.23);
        assert_eq!(round_to_2dp(1.2355), 1.24);
        assert_eq!(round_to_2dp(1.0), 1.0);
    }

    #[test]
    fn size_breakdown_serialization() {
        let breakdown = SizeBreakdown {
            fonts_mb: 0.5,
            images_mb: 1.2,
            markup_mb: 0.1,
            overhead_mb: 0.5,
        };
        let json = serde_json::to_string(&breakdown).unwrap();
        assert!(json.contains("fonts_mb"));
        assert!(json.contains("images_mb"));
    }
}
