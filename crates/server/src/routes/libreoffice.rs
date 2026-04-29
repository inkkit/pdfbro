//! `/forms/libreoffice/convert` route handler.

use std::collections::HashMap;
use std::path::PathBuf;

use axum::extract::{Multipart, State};
use axum::http::HeaderMap;
use axum::response::Response;
use engine::{OfficeOptions, PageRanges};
use engine::libreoffice::PdfAProfile;

use crate::error::{ApiError, ApiResult};
use crate::multipart::FormFields;
use crate::routes::util::{pdf_response, zip_response, build_zip};
use crate::state::AppState;

/// `POST /forms/libreoffice/convert`.
pub async fn libreoffice_convert(
    State(state): State<AppState>,
    headers: HeaderMap,
    mp: Multipart,
) -> ApiResult<Response> {
    let _permit = state
        .sem
        .clone()
        .acquire_owned()
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    let form = FormFields::from_multipart(mp).await?;
    let lo = state
        .libreoffice
        .as_ref()
        .ok_or_else(|| ApiError::Internal("LibreOffice engine unavailable".to_string()))?;

    let opts = parse_office_options(&form.map)?;
    opts.validate()?;
    let merge = parse_merge_flag(&form.map)?;

    let inputs: Vec<PathBuf> = form
        .files_by_field("files")
        .iter()
        .map(|f| f.path.clone())
        .collect();
    if inputs.is_empty() {
        return Err(ApiError::MissingFile("files".to_string()));
    }

    // Read file bytes for potential webhook job.
    let mut file_bytes = Vec::new();
    for path in &inputs {
        let bytes = tokio::fs::read(path).await
            .map_err(|e| ApiError::Internal(e.to_string()))?;
        file_bytes.push(bytes);
    }

    if let Some(resp) = crate::webhook::maybe_spawn_webhook(
        &headers,
        &state,
        crate::webhook::WebhookOperation::LibreOfficeConvert,
        crate::webhook::JobData::LibreOffice {
            files: file_bytes,
            options: opts.clone(),
            merge,
        },
    ).await? {
        return Ok(resp);
    }

    let mut outputs = lo.convert_many(&inputs, &opts).await?;

    if merge && outputs.len() > 1 {
        let merged =
            tokio::task::spawn_blocking(move || engine::pdfops::merge(&materialise(&outputs)))
                .await
                .map_err(|e| ApiError::Internal(e.to_string()))??;
        return Ok(pdf_response(merged, "result.pdf"));
    }

    if outputs.len() == 1
        && let Some(only) = outputs.pop()
    {
        return Ok(pdf_response(only, "result.pdf"));
    }

    // Multiple outputs: zip with names mirroring the inputs.
    let filenames: Vec<String> = form
        .files_by_field("files")
        .iter()
        .map(|f| pdf_filename_for(&f.filename))
        .collect();
    let zip_bytes = build_zip(&filenames, &outputs)?;
    Ok(zip_response(zip_bytes, "result.zip"))
}

// `convert_many` consumes its outputs into a single `Vec<Vec<u8>>`, but the
// merge call wants `&[&[u8]]`. We materialise references into a fresh vec
// inside the blocking task because the lifetimes of borrows from the moved
// vec cannot be expressed here.
fn materialise(outputs: &[Vec<u8>]) -> Vec<&[u8]> {
    outputs.iter().map(|v| v.as_slice()).collect()
}

fn pdf_filename_for(input_name: &str) -> String {
    match input_name.rsplit_once('.') {
        Some((stem, _)) => format!("{stem}.pdf"),
        None => format!("{input_name}.pdf"),
    }
}

fn parse_merge_flag(map: &HashMap<String, String>) -> ApiResult<bool> {
    match map.get("merge").map(String::as_str) {
        None | Some("") => Ok(false),
        Some(s) => match s.trim().to_ascii_lowercase().as_str() {
            "1" | "true" | "yes" | "on" => Ok(true),
            "0" | "false" | "no" | "off" => Ok(false),
            other => Err(ApiError::InvalidField {
                field: "merge",
                message: format!("expected boolean, got `{other}`"),
            }),
        },
    }
}

/// Build [`OfficeOptions`] from a form map.
pub fn parse_office_options(map: &HashMap<String, String>) -> ApiResult<OfficeOptions> {
    let mut opts = OfficeOptions::default();

    // Basic
    if let Some(s) = nonempty(map, "landscape") {
        opts.landscape = parse_bool(&s, "landscape")?;
    }
    let range_raw = nonempty(map, "nativePageRanges").or_else(|| nonempty(map, "pageRanges"));
    if let Some(s) = range_raw {
        opts.page_ranges = Some(PageRanges::parse(&s)?);
    }
    if let Some(s) = nonempty(map, "pdfa") {
        opts.pdf_a = Some(parse_pdf_a(&s)?);
    }
    if let Some(s) = nonempty(map, "pdfua") {
        opts.pdf_ua = parse_bool(&s, "pdfua")?;
    }
    if let Some(s) = nonempty(map, "quality") {
        let v: u8 = s
            .parse()
            .map_err(|e: std::num::ParseIntError| ApiError::InvalidField {
                field: "quality",
                message: e.to_string(),
            })?;
        opts.quality = Some(v);
    }
    if let Some(s) = nonempty(map, "maxImageResolution") {
        let v: u32 = s
            .parse()
            .map_err(|e: std::num::ParseIntError| ApiError::InvalidField {
                field: "maxImageResolution",
                message: e.to_string(),
            })?;
        opts.max_image_resolution = Some(v);
    }

    // Bookmarks
    if let Some(s) = nonempty(map, "exportBookmarks") {
        opts.export_bookmarks = parse_bool(&s, "exportBookmarks")?;
    }
    if let Some(s) = nonempty(map, "exportBookmarksToPdfDestination") {
        opts.export_bookmarks_to_pdf_destination = parse_bool(&s, "exportBookmarksToPdfDestination")?;
    }
    if let Some(s) = nonempty(map, "updateIndexes") {
        opts.update_indexes = parse_bool(&s, "updateIndexes")?;
    }

    // Form Fields
    if let Some(s) = nonempty(map, "exportFormFields") {
        opts.export_form_fields = parse_bool(&s, "exportFormFields")?;
    }
    if let Some(s) = nonempty(map, "allowDuplicateFieldNames") {
        opts.allow_duplicate_field_names = parse_bool(&s, "allowDuplicateFieldNames")?;
    }
    if let Some(s) = nonempty(map, "exportPlaceholders") {
        opts.export_placeholders = parse_bool(&s, "exportPlaceholders")?;
    }

    // Notes
    if let Some(s) = nonempty(map, "exportNotes") {
        opts.export_notes = parse_bool(&s, "exportNotes")?;
    }
    if let Some(s) = nonempty(map, "exportNotesPages") {
        opts.export_notes_pages = parse_bool(&s, "exportNotesPages")?;
    }
    if let Some(s) = nonempty(map, "exportOnlyNotesPages") {
        opts.export_only_notes_pages = parse_bool(&s, "exportOnlyNotesPages")?;
    }
    if let Some(s) = nonempty(map, "exportNotesInMargin") {
        opts.export_notes_in_margin = parse_bool(&s, "exportNotesInMargin")?;
    }

    // Advanced
    if let Some(s) = nonempty(map, "convertOooTargetToPdfTarget") {
        opts.convert_ooo_target_to_pdf_target = parse_bool(&s, "convertOooTargetToPdfTarget")?;
    }
    if let Some(s) = nonempty(map, "exportLinksRelativeFsys") {
        opts.export_links_relative_fsys = parse_bool(&s, "exportLinksRelativeFsys")?;
    }
    if let Some(s) = nonempty(map, "exportHiddenSlides") {
        opts.export_hidden_slides = parse_bool(&s, "exportHiddenSlides")?;
    }
    if let Some(s) = nonempty(map, "skipEmptyPages") {
        opts.skip_empty_pages = parse_bool(&s, "skipEmptyPages")?;
    }
    if let Some(s) = nonempty(map, "addOriginalDocumentAsStream") {
        opts.add_original_document_as_stream = parse_bool(&s, "addOriginalDocumentAsStream")?;
    }
    if let Some(s) = nonempty(map, "singlePageSheets") {
        opts.single_page_sheets = parse_bool(&s, "singlePageSheets")?;
    }
    if let Some(s) = nonempty(map, "losslessImageCompression") {
        opts.lossless_image_compression = parse_bool(&s, "losslessImageCompression")?;
    }
    if let Some(s) = nonempty(map, "reduceImageResolution") {
        opts.reduce_image_resolution = parse_bool(&s, "reduceImageResolution")?;
    }

    // Native Watermarks
    if let Some(s) = nonempty(map, "nativeWatermarkText") {
        opts.native_watermark_text = Some(s);
    }
    if let Some(s) = nonempty(map, "nativeWatermarkColor") {
        let v: u32 = s.parse().map_err(|e: std::num::ParseIntError| ApiError::InvalidField {
            field: "nativeWatermarkColor",
            message: e.to_string(),
        })?;
        opts.native_watermark_color = Some(v);
    }
    if let Some(s) = nonempty(map, "nativeWatermarkFontHeight") {
        let v: u32 = s.parse().map_err(|e: std::num::ParseIntError| ApiError::InvalidField {
            field: "nativeWatermarkFontHeight",
            message: e.to_string(),
        })?;
        opts.native_watermark_font_height = Some(v);
    }
    if let Some(s) = nonempty(map, "nativeWatermarkRotateAngle") {
        let v: i32 = s.parse().map_err(|e: std::num::ParseIntError| ApiError::InvalidField {
            field: "nativeWatermarkRotateAngle",
            message: e.to_string(),
        })?;
        opts.native_watermark_rotate_angle = Some(v);
    }
    if let Some(s) = nonempty(map, "nativeWatermarkFontName") {
        opts.native_watermark_font_name = Some(s);
    }
    if let Some(s) = nonempty(map, "nativeTiledWatermarkText") {
        opts.native_tiled_watermark_text = Some(s);
    }

    // Viewer Preferences
    if let Some(s) = nonempty(map, "initialView") {
        let v: i32 = s.parse().map_err(|e: std::num::ParseIntError| ApiError::InvalidField {
            field: "initialView",
            message: e.to_string(),
        })?;
        opts.initial_view = Some(v);
    }
    if let Some(s) = nonempty(map, "initialPage") {
        let v: i32 = s.parse().map_err(|e: std::num::ParseIntError| ApiError::InvalidField {
            field: "initialPage",
            message: e.to_string(),
        })?;
        opts.initial_page = Some(v);
    }
    if let Some(s) = nonempty(map, "magnification") {
        let v: i32 = s.parse().map_err(|e: std::num::ParseIntError| ApiError::InvalidField {
            field: "magnification",
            message: e.to_string(),
        })?;
        opts.magnification = Some(v);
    }
    if let Some(s) = nonempty(map, "zoom") {
        let v: i32 = s.parse().map_err(|e: std::num::ParseIntError| ApiError::InvalidField {
            field: "zoom",
            message: e.to_string(),
        })?;
        opts.zoom = Some(v);
    }
    if let Some(s) = nonempty(map, "pageLayout") {
        let v: i32 = s.parse().map_err(|e: std::num::ParseIntError| ApiError::InvalidField {
            field: "pageLayout",
            message: e.to_string(),
        })?;
        opts.page_layout = Some(v);
    }
    if let Some(s) = nonempty(map, "firstPageOnLeft") {
        opts.first_page_on_left = parse_bool(&s, "firstPageOnLeft")?;
    }
    if let Some(s) = nonempty(map, "resizeWindowToInitialPage") {
        opts.resize_window_to_initial_page = parse_bool(&s, "resizeWindowToInitialPage")?;
    }
    if let Some(s) = nonempty(map, "centerWindow") {
        opts.center_window = parse_bool(&s, "centerWindow")?;
    }
    if let Some(s) = nonempty(map, "openInFullScreenMode") {
        opts.open_in_full_screen_mode = parse_bool(&s, "openInFullScreenMode")?;
    }
    if let Some(s) = nonempty(map, "displayPDFDocumentTitle") {
        opts.display_pdf_document_title = parse_bool(&s, "displayPDFDocumentTitle")?;
    }
    if let Some(s) = nonempty(map, "hideViewerMenubar") {
        opts.hide_viewer_menubar = parse_bool(&s, "hideViewerMenubar")?;
    }
    if let Some(s) = nonempty(map, "hideViewerToolbar") {
        opts.hide_viewer_toolbar = parse_bool(&s, "hideViewerToolbar")?;
    }
    if let Some(s) = nonempty(map, "hideViewerWindowControls") {
        opts.hide_viewer_window_controls = parse_bool(&s, "hideViewerWindowControls")?;
    }
    if let Some(s) = nonempty(map, "useTransitionEffects") {
        opts.use_transition_effects = parse_bool(&s, "useTransitionEffects")?;
    }
    if let Some(s) = nonempty(map, "openBookmarkLevels") {
        let v: i32 = s.parse().map_err(|e: std::num::ParseIntError| ApiError::InvalidField {
            field: "openBookmarkLevels",
            message: e.to_string(),
        })?;
        opts.open_bookmark_levels = Some(v);
    }

    Ok(opts)
}

fn parse_pdf_a(s: &str) -> ApiResult<PdfAProfile> {
    let normalised: String = s
        .trim()
        .to_ascii_lowercase()
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .collect();
    match normalised.as_str() {
        "a1b" | "pdfa1b" => Ok(PdfAProfile::A1B),
        "a2b" | "pdfa2b" => Ok(PdfAProfile::A2B),
        "a3b" | "pdfa3b" => Ok(PdfAProfile::A3B),
        other => Err(ApiError::InvalidField {
            field: "pdfa",
            message: format!("expected one of A-1b/A-2b/A-3b, got `{other}`"),
        }),
    }
}

fn parse_bool(s: &str, field: &'static str) -> ApiResult<bool> {
    match s.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Ok(true),
        "0" | "false" | "no" | "off" => Ok(false),
        other => Err(ApiError::InvalidField {
            field,
            message: format!("expected boolean, got `{other}`"),
        }),
    }
}

fn nonempty(map: &HashMap<String, String>, key: &str) -> Option<String> {
    map.get(key).filter(|s| !s.is_empty()).cloned()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fm(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect()
    }

    #[test]
    fn merge_flag_parses() {
        assert!(!parse_merge_flag(&fm(&[])).unwrap());
        assert!(parse_merge_flag(&fm(&[("merge", "true")])).unwrap());
        assert!(parse_merge_flag(&fm(&[("merge", "1")])).unwrap());
        assert!(!parse_merge_flag(&fm(&[("merge", "false")])).unwrap());
        assert!(parse_merge_flag(&fm(&[("merge", "maybe")])).is_err());
    }

    #[test]
    fn office_options_parse_basic() {
        let map = fm(&[
            ("landscape", "true"),
            ("pageRanges", "1-3"),
            ("pdfa", "PDF/A-2b"),
            ("pdfua", "true"),
            ("quality", "75"),
            ("maxImageResolution", "300"),
        ]);
        let opts = parse_office_options(&map).unwrap();
        assert!(opts.landscape);
        assert!(opts.page_ranges.is_some());
        assert_eq!(opts.pdf_a, Some(PdfAProfile::A2B));
        assert!(opts.pdf_ua);
        assert_eq!(opts.quality, Some(75));
        assert_eq!(opts.max_image_resolution, Some(300));
    }

    #[test]
    fn office_options_parse_all_new_fields() {
        let map = fm(&[
            // Bookmarks
            ("exportBookmarks", "true"),
            ("exportBookmarksToPdfDestination", "true"),
            ("updateIndexes", "false"),
            // Form Fields
            ("exportFormFields", "true"),
            ("allowDuplicateFieldNames", "true"),
            ("exportPlaceholders", "true"),
            // Notes
            ("exportNotes", "true"),
            ("exportNotesPages", "true"),
            ("exportOnlyNotesPages", "false"),
            ("exportNotesInMargin", "true"),
            // Advanced
            ("convertOooTargetToPdfTarget", "true"),
            ("exportLinksRelativeFsys", "true"),
            ("exportHiddenSlides", "true"),
            ("skipEmptyPages", "true"),
            ("addOriginalDocumentAsStream", "true"),
            ("singlePageSheets", "true"),
            ("losslessImageCompression", "true"),
            ("reduceImageResolution", "true"),
            // Native Watermarks
            ("nativeWatermarkText", "CONFIDENTIAL"),
            ("nativeWatermarkColor", "16711680"),
            ("nativeWatermarkFontHeight", "24"),
            ("nativeWatermarkRotateAngle", "45"),
            ("nativeWatermarkFontName", "Arial"),
            ("nativeTiledWatermarkText", "DRAFT"),
            // Viewer Preferences
            ("initialView", "1"),
            ("initialPage", "3"),
            ("magnification", "2"),
            ("zoom", "150"),
            ("pageLayout", "2"),
            ("firstPageOnLeft", "true"),
            ("resizeWindowToInitialPage", "true"),
            ("centerWindow", "true"),
            ("openInFullScreenMode", "true"),
            ("displayPDFDocumentTitle", "false"),
            ("hideViewerMenubar", "true"),
            ("hideViewerToolbar", "true"),
            ("hideViewerWindowControls", "true"),
            ("useTransitionEffects", "true"),
            ("openBookmarkLevels", "-1"),
        ]);
        let opts = parse_office_options(&map).unwrap();
        assert!(opts.export_bookmarks);
        assert!(opts.export_bookmarks_to_pdf_destination);
        assert!(!opts.update_indexes);
        assert!(opts.export_form_fields);
        assert!(opts.allow_duplicate_field_names);
        assert!(opts.export_placeholders);
        assert!(opts.export_notes);
        assert!(opts.export_notes_pages);
        assert!(!opts.export_only_notes_pages);
        assert!(opts.export_notes_in_margin);
        assert!(opts.convert_ooo_target_to_pdf_target);
        assert!(opts.export_links_relative_fsys);
        assert!(opts.export_hidden_slides);
        assert!(opts.skip_empty_pages);
        assert!(opts.add_original_document_as_stream);
        assert!(opts.single_page_sheets);
        assert!(opts.lossless_image_compression);
        assert!(opts.reduce_image_resolution);
        assert_eq!(opts.native_watermark_text, Some("CONFIDENTIAL".into()));
        assert_eq!(opts.native_watermark_color, Some(16711680));
        assert_eq!(opts.native_watermark_font_height, Some(24));
        assert_eq!(opts.native_watermark_rotate_angle, Some(45));
        assert_eq!(opts.native_watermark_font_name, Some("Arial".into()));
        assert_eq!(opts.native_tiled_watermark_text, Some("DRAFT".into()));
        assert_eq!(opts.initial_view, Some(1));
        assert_eq!(opts.initial_page, Some(3));
        assert_eq!(opts.magnification, Some(2));
        assert_eq!(opts.zoom, Some(150));
        assert_eq!(opts.page_layout, Some(2));
        assert!(opts.first_page_on_left);
        assert!(opts.resize_window_to_initial_page);
        assert!(opts.center_window);
        assert!(opts.open_in_full_screen_mode);
        assert!(!opts.display_pdf_document_title);
        assert!(opts.hide_viewer_menubar);
        assert!(opts.hide_viewer_toolbar);
        assert!(opts.hide_viewer_window_controls);
        assert!(opts.use_transition_effects);
        assert_eq!(opts.open_bookmark_levels, Some(-1));
    }

    #[test]
    fn native_page_ranges_alias_overrides_page_ranges() {
        let map = fm(&[("pageRanges", "1-3"), ("nativePageRanges", "5-7")]);
        let opts = parse_office_options(&map).unwrap();
        let ranges = opts.page_ranges.unwrap();
        // The string repr should reflect the native override.
        let as_str: String = ranges.into();
        assert_eq!(as_str, "5-7");
    }

    #[test]
    fn pdf_filename_for_strips_extension() {
        assert_eq!(pdf_filename_for("doc.docx"), "doc.pdf");
        assert_eq!(pdf_filename_for("noext"), "noext.pdf");
        assert_eq!(pdf_filename_for("a.b.c"), "a.b.pdf");
    }
}
