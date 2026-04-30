//! `/debug/*` route handlers for diagnostics (Font Doctor feature).
//!
//! Implementation of `docs/specs/43-font-doctor.md`.

use std::collections::HashSet;
use std::sync::OnceLock;

use axum::extract::{Multipart, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use regex::Regex;
use serde::Serialize;
use tracing::{info, warn};

use crate::error::{ApiError, ApiResult};
use crate::multipart::FormFields;
use crate::state::AppState;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Font information response.
#[derive(Debug, Clone, Serialize)]
pub struct FontInfo {
    /// Font family name.
    pub family: String,
    /// Font file path (if available).
    pub path: Option<String>,
}

/// List of system fonts response.
#[derive(Debug, Clone, Serialize)]
pub struct FontList {
    /// Available system fonts.
    pub fonts: Vec<FontInfo>,
}

/// Font validation result for a single font family.
#[derive(Debug, Clone, Serialize)]
pub struct FontValidation {
    /// Font family name.
    pub family: String,
    /// Whether the font is available on the system.
    pub available: bool,
    /// Installed font path if available.
    pub path: Option<String>,
    /// Suggestion if font is not available.
    pub suggestion: Option<String>,
}

/// Font validation response.
#[derive(Debug, Clone, Serialize)]
pub struct FontValidationResponse {
    /// Validation results for each font family.
    pub fonts: Vec<FontValidation>,
}

/// Font detail in HTML diagnostics.
#[derive(Debug, Clone, Serialize)]
pub struct FontDetail {
    /// Font family name.
    pub family: String,
    /// Whether the font is installed.
    pub installed: bool,
    /// Font file path if installed.
    pub path: Option<String>,
}

/// HTML font diagnostics response.
#[derive(Debug, Clone, Serialize)]
pub struct HtmlDiagnostics {
    /// Fonts found in HTML/CSS.
    pub fonts: Vec<FontDetail>,
    /// Warning messages.
    pub warnings: Vec<String>,
    /// Suggestion messages.
    pub suggestions: Vec<String>,
}

// ---------------------------------------------------------------------------
// GET /debug/fonts - List all system fonts
// ---------------------------------------------------------------------------

/// `GET /debug/fonts` - List all system fonts available to Chromium.
pub async fn debug_list_fonts() -> ApiResult<impl IntoResponse> {
    info!("Listing system fonts");

    let fonts = list_system_fonts().await;

    info!(font_count = fonts.len(), "Found system fonts");

    Ok((StatusCode::OK, Json(FontList { fonts })))
}

/// List system fonts by querying common font directories and fc-list.
async fn list_system_fonts() -> Vec<FontInfo> {
    let mut fonts = Vec::new();
    let mut seen_families = HashSet::new();

    // Try fc-list (fontconfig) first - most reliable
    if let Ok(output) = tokio::process::Command::new("fc-list")
        .args([":family", "-f", "%{family}\n"])
        .output()
        .await
    {
        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            for line in stdout.lines() {
                // fc-list can return multiple comma-separated families
                for family in line.split(',') {
                    let family = family.trim();
                    if !family.is_empty() && seen_families.insert(family.to_string()) {
                        fonts.push(FontInfo {
                            family: family.to_string(),
                            path: None,
                        });
                    }
                }
            }
        }
    }

    // If fc-list didn't work, try common font directories
    if fonts.is_empty() {
        let font_dirs = [
            "/usr/share/fonts",
            "/usr/local/share/fonts",
            "/usr/share/fonts/truetype",
            "/usr/share/fonts/TTF",
            "/usr/share/fonts/opentype",
            "/System/Library/Fonts",       // macOS
            "/Library/Fonts",              // macOS
            "C:\\Windows\\Fonts",          // Windows
        ];

        for dir in &font_dirs {
            if let Ok(entries) = tokio::fs::read_dir(dir).await {
                let mut entries = entries;
                while let Ok(Some(entry)) = entries.next_entry().await {
                    let path = entry.path();
                    if let Some(ext) = path.extension() {
                        let ext = ext.to_string_lossy().to_lowercase();
                        if ext == "ttf" || ext == "otf" || ext == "woff" || ext == "woff2" {
                            // Try to extract family name from filename
                            if let Some(stem) = path.file_stem() {
                                let family = stem.to_string_lossy().to_string();
                                if seen_families.insert(family.clone()) {
                                    fonts.push(FontInfo {
                                        family,
                                        path: Some(path.display().to_string()),
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // Sort by family name
    fonts.sort_by(|a, b| a.family.cmp(&b.family));
    fonts
}

// ---------------------------------------------------------------------------
// POST /debug/validate-fonts - Validate fonts in HTML/CSS
// ---------------------------------------------------------------------------

/// `POST /debug/validate-fonts` - Check if fonts will render correctly.
pub async fn debug_validate_fonts(mp: Multipart) -> ApiResult<impl IntoResponse> {
    let form = FormFields::from_multipart(mp).await?;

    // Extract font families from form fields
    let font_families = extract_font_families_from_form(&form);

    if font_families.is_empty() {
        return Err(ApiError::InvalidField {
            field: "fonts",
            message: "No font families found. Provide 'fonts' as comma-separated list, or 'html'/'css' content".into(),
        });
    }

    info!(family_count = font_families.len(), "Validating fonts");

    let mut results = Vec::new();
    for family in font_families {
        let validation = validate_font_family(&family).await;
        results.push(validation);
    }

    Ok((StatusCode::OK, Json(FontValidationResponse { fonts: results })))
}

/// Extract font families from form data.
fn extract_font_families_from_form(form: &FormFields) -> Vec<String> {
    let mut families = HashSet::new();

    // Direct font list
    if let Some(fonts_str) = form.map.get("fonts") {
        for family in fonts_str.split(',') {
            let family = family.trim();
            if !family.is_empty() {
                families.insert(family.to_string());
            }
        }
    }

    // Extract from HTML content
    if let Some(html) = form.map.get("html") {
        let html_families = extract_font_families_from_html(html);
        families.extend(html_families);
    }

    // Extract from CSS content
    if let Some(css) = form.map.get("css") {
        let css_families = extract_font_families_from_css(css);
        families.extend(css_families);
    }

    families.into_iter().collect()
}

/// Static regex pattern for font-family extraction.
static FONT_FAMILY_RE: OnceLock<Regex> = OnceLock::new();
/// Static regex pattern for @font-face extraction.
static FONT_FACE_RE: OnceLock<Regex> = OnceLock::new();

/// Extract font families from HTML content using regex.
fn extract_font_families_from_html(html: &str) -> Vec<String> {
    let mut families = HashSet::new();

    // Match font-family in style attributes
    let font_family_re = FONT_FAMILY_RE.get_or_init(|| {
        Regex::new(r#"font-family\s*:\s*['"]?([^'";,}]+)['"]?"#).unwrap()
    });

    for cap in font_family_re.captures_iter(html) {
        let family_list = &cap[1];
        for family in family_list.split(',') {
            let family = family.trim().trim_matches('"').trim_matches('\'');
            if !family.is_empty() {
                families.insert(family.to_string());
            }
        }
    }

    // Match @font-face font-family
    let font_face_re = FONT_FACE_RE.get_or_init(|| {
        Regex::new(r#"@font-face\s*\{[^}]*font-family\s*:\s*['"]?([^'";,}]+)['"]?"#).unwrap()
    });

    for cap in font_face_re.captures_iter(html) {
        let family = cap[1].trim().trim_matches('"').trim_matches('\'');
        if !family.is_empty() {
            families.insert(family.to_string());
        }
    }

    families.into_iter().collect()
}

/// Extract font families from CSS content.
fn extract_font_families_from_css(css: &str) -> Vec<String> {
    extract_font_families_from_html(css) // Same logic works for CSS
}

/// Validate a single font family.
async fn validate_font_family(family: &str) -> FontValidation {
    // Check system availability via fc-match
    let available = check_font_available(family).await;

    let (path, suggestion) = if available {
        // Try to get the actual font path
        let font_path = get_font_path(family).await;
        (font_path, None)
    } else {
        (None, Some(generate_font_suggestion(family)))
    };

    FontValidation {
        family: family.to_string(),
        available,
        path,
        suggestion,
    }
}

/// Check if a font family is available using fc-match.
async fn check_font_available(family: &str) -> bool {
    match tokio::process::Command::new("fc-match")
        .arg(family)
        .output()
        .await
    {
        Ok(output) if output.status.success() => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            // fc-match returns the matched font; if it contains the requested
            // family name (case-insensitive), it's available
            let stdout_lower = stdout.to_lowercase();
            let family_lower = family.to_lowercase();

            // Check if the output contains our family or a reasonable fallback
            stdout_lower.contains(&family_lower)
                || !stdout_lower.contains("sans")
                || !stdout_lower.contains("serif")
        }
        _ => {
            // Fallback: check if any system font file name contains the family
            let system_fonts = list_system_fonts().await;
            let family_lower = family.to_lowercase();
            system_fonts.iter().any(|f| f.family.to_lowercase().contains(&family_lower))
        }
    }
}

/// Get the font file path for a family using fc-list.
async fn get_font_path(family: &str) -> Option<String> {
    match tokio::process::Command::new("fc-list")
        .args([family, ":file", "-f", "%{file}\n"])
        .output()
        .await
    {
        Ok(output) if output.status.success() => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            stdout.lines().next().map(|s| s.trim().to_string())
        }
        _ => None,
    }
}

/// Generate a suggestion for installing a missing font.
fn generate_font_suggestion(family: &str) -> String {
    let family_lower = family.to_lowercase();

    // Common font mappings
    let package = if family_lower.contains("arial")
        || family_lower.contains("helvetica")
        || family_lower.contains("times")
        || family_lower.contains("courier")
    {
        "ttf-mscorefonts-installer"
    } else if family_lower.contains("liberation") {
        "fonts-liberation"
    } else if family_lower.contains("noto") {
        "fonts-noto"
    } else if family_lower.contains("dejavu") {
        "fonts-dejavu"
    } else if family_lower.contains("roboto") {
        "fonts-roboto"
    } else if family_lower.contains("ubuntu") {
        "fonts-ubuntu"
    } else if family_lower.contains("open sans") {
        "fonts-open-sans"
    } else {
        "fonts-<package>"
    };

    if package == "fonts-<package>" {
        format!(
            "Font '{}' not found. Install with: apt-get install <font-package> or copy .ttf/.otf files to /usr/share/fonts/",
            family
        )
    } else {
        format!(
            "Font '{}' not found. Install with: apt-get install {}",
            family, package
        )
    }
}

// ---------------------------------------------------------------------------
// POST /debug/diagnose-html - Full HTML font diagnostics
// ---------------------------------------------------------------------------

/// `POST /debug/diagnose-html` - Full font diagnostics for HTML.
pub async fn debug_diagnose_html(
    State(_state): State<AppState>,
    mp: Multipart,
) -> ApiResult<impl IntoResponse> {
    let form = FormFields::from_multipart(mp).await?;

    // Get HTML content
    let html = form
        .map
        .get("html")
        .ok_or_else(|| ApiError::MissingField("html"))?;

    info!("Running font diagnostics on HTML");

    let mut diagnostics = HtmlDiagnostics {
        fonts: Vec::new(),
        warnings: Vec::new(),
        suggestions: Vec::new(),
    };

    // 1. Extract and validate all font families
    let font_families = extract_font_families_from_html(html);
    let system_fonts = list_system_fonts().await;
    let system_family_names: HashSet<String> =
        system_fonts.iter().map(|f| f.family.to_lowercase()).collect();

    for family in font_families {
        let family_lower = family.to_lowercase();
        let installed = system_family_names.iter().any(|sf| sf.contains(&family_lower));

        let path = if installed {
            system_fonts
                .iter()
                .find(|f| f.family.to_lowercase().contains(&family_lower))
                .and_then(|f| f.path.clone())
        } else {
            None
        };

        diagnostics.fonts.push(FontDetail {
            family: family.clone(),
            installed,
            path,
        });

        if !installed {
            diagnostics.warnings.push(format!("Font '{}' not installed", family));
            diagnostics.suggestions.push(generate_font_suggestion(&family));
        }
    }

    // 2. Check for web fonts (will bloat PDF)
    if has_web_fonts(html) {
        diagnostics.warnings.push(
            "HTML uses @font-face web fonts - PDF size may increase significantly".into(),
        );
        diagnostics.suggestions.push(
            "Consider installing fonts locally: apt-get install ttf-mscorefonts-installer".into(),
        );
    }

    // 3. Check for Google Fonts or external font URLs
    if has_external_fonts(html) {
        diagnostics.warnings.push(
            "HTML references external fonts (Google Fonts, etc.) - may cause slow rendering".into(),
        );
        diagnostics.suggestions.push(
            "Download and host fonts locally for better reliability".into(),
        );
    }

    info!(
        font_count = diagnostics.fonts.len(),
        warning_count = diagnostics.warnings.len(),
        "Font diagnostics complete"
    );

    Ok((StatusCode::OK, Json(diagnostics)))
}

/// Check if HTML contains @font-face declarations.
fn has_web_fonts(html: &str) -> bool {
    html.contains("@font-face")
}

/// Check if HTML references external font sources.
fn has_external_fonts(html: &str) -> bool {
    let patterns = [
        "fonts.googleapis.com",
        "fonts.gstatic.com",
        "fontlibrary.org",
        "use.typekit.net",
    ];

    let html_lower = html.to_lowercase();
    patterns.iter().any(|p| html_lower.contains(p))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_font_families_from_html_basic() {
        let html = r#"
            <style>
                body { font-family: 'Helvetica Neue', Helvetica, Arial, sans-serif; }
                h1 { font-family: Georgia, serif; }
                @font-face { font-family: 'Custom Font'; src: url('font.woff2'); }
            </style>
        "#;

        let families = extract_font_families_from_html(html);
        assert!(families.contains(&"Helvetica Neue".to_string()));
        assert!(families.contains(&"Helvetica".to_string()));
        assert!(families.contains(&"Arial".to_string()));
        assert!(families.contains(&"sans-serif".to_string()));
        assert!(families.contains(&"Georgia".to_string()));
        assert!(families.contains(&"serif".to_string()));
        assert!(families.contains(&"Custom Font".to_string()));
    }

    #[test]
    fn extract_font_families_from_css_inline() {
        let css = r#"
            .title { font-family: 'Roboto', sans-serif; }
            .body { font-family: "Open Sans", Arial, sans-serif; }
        "#;

        let families = extract_font_families_from_css(css);
        assert!(families.contains(&"Roboto".to_string()));
        assert!(families.contains(&"Open Sans".to_string()));
        assert!(families.contains(&"Arial".to_string()));
    }

    #[test]
    fn has_web_fonts_detects_at_font_face() {
        let html_with = r#"<style>@font-face { font-family: 'Foo'; }</style>"#;
        let html_without = r#"<style>body { font-family: Arial; }</style>"#;

        assert!(has_web_fonts(html_with));
        assert!(!has_web_fonts(html_without));
    }

    #[test]
    fn has_external_fonts_detects_google_fonts() {
        let html_with = r#"<link href="https://fonts.googleapis.com/css2?family=Roboto">"#;
        let html_without = r#"<style>body { font-family: Arial; }</style>"#;

        assert!(has_external_fonts(html_with));
        assert!(!has_external_fonts(html_without));
    }

    #[test]
    fn generate_font_suggestion_helvetica() {
        let suggestion = generate_font_suggestion("Helvetica Neue");
        assert!(suggestion.contains("ttf-mscorefonts-installer"));
    }

    #[test]
    fn generate_font_suggestion_roboto() {
        let suggestion = generate_font_suggestion("Roboto");
        assert!(suggestion.contains("fonts-roboto"));
    }
}
