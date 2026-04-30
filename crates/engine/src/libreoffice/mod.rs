//! `LibreOfficeEngine` — convert office documents to PDF via the `soffice`
//! subprocess.
//!
//! Implementation of `docs/specs/12-engine-libreoffice.md`. Each call spawns a
//! short-lived `soffice --headless` child with its own isolated
//! `UserInstallation` profile, making concurrent invocations safe.

pub mod filter;

mod convert;
mod discover;

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tokio::sync::Semaphore;

use tracing::{debug, info, instrument};

use crate::types::{EngineError, EngineResult, PageRanges};

// ---------------------------------------------------------------------------
// Engine
// ---------------------------------------------------------------------------

/// Wrapper around the `soffice` binary. Cheap to clone (`Arc` inside).
///
/// # Example
///
/// ```ignore
/// use std::path::Path;
/// use engine::{LibreOfficeEngine, OfficeOptions};
///
/// # async fn doc() -> engine::EngineResult<()> {
/// let lo = LibreOfficeEngine::discover().await?;
/// let pdf = lo
///     .convert(Path::new("doc.docx"), &OfficeOptions::default())
///     .await?;
/// # let _ = pdf;
/// # Ok(())
/// # }
/// ```
#[derive(Clone)]
pub struct LibreOfficeEngine {
    inner: Arc<Inner>,
}

struct Inner {
    exe: PathBuf,
    timeout: Duration,
    semaphore: Semaphore,
}

/// Engine-wide configuration. Pass to [`LibreOfficeEngine::launch`].
#[derive(Debug, Clone)]
pub struct LibreOfficeConfig {
    /// Path to `soffice` (or `libreoffice`). `None` = autodiscover via
    /// `$LIBREOFFICE_PATH`, `$PATH`, and platform defaults.
    pub executable: Option<PathBuf>,
    /// Per-conversion timeout. Default 120s.
    pub timeout: Duration,
    /// Maximum concurrent subprocess invocations. Default
    /// [`std::thread::available_parallelism`].
    pub max_concurrency: usize,
    /// Use lazy initialization (start on first request).
    /// Default: false (start eagerly at server startup).
    pub lazy_start: bool,
    /// Idle shutdown timeout - engine shuts down after this duration of no requests.
    /// None means no idle shutdown. Default: None.
    pub idle_shutdown_timeout: Option<Duration>,
}

impl Default for LibreOfficeConfig {
    fn default() -> Self {
        Self {
            executable: None,
            timeout: Duration::from_secs(120),
            max_concurrency: std::thread::available_parallelism()
                .map(|n| n.get())
                .unwrap_or(4),
            lazy_start: false,
            idle_shutdown_timeout: None,
        }
    }
}

impl LibreOfficeEngine {
    /// Return a tracing span for this engine instance, tagged with
    /// `engine="libreoffice"`.
    pub fn logger(&self) -> tracing::Span {
        tracing::info_span!(
            "engine",
            engine = "libreoffice",
        )
    }

    /// Discover `soffice` on `$PATH` and platform defaults using
    /// [`LibreOfficeConfig::default`].
    pub async fn discover() -> EngineResult<Self> {
        Self::launch(LibreOfficeConfig::default()).await
    }

    /// Construct an engine with explicit configuration.
    ///
    /// If `config.executable` is `Some`, the path is required to exist;
    /// otherwise auto-discovery is performed. The chosen executable is then
    /// probed (`--headless --version`) before the engine is returned.
    #[instrument(skip(config), fields(executable = ?config.executable))]
    pub async fn launch(config: LibreOfficeConfig) -> EngineResult<Self> {
        info!("Launching LibreOffice engine");
        let exe = match config.executable.as_ref() {
            Some(p) => {
                if !p.exists() {
                    return Err(EngineError::Internal(format!(
                        "LibreOffice not found: {}",
                        p.display()
                    )));
                }
                p.clone()
            }
            None => discover::find_soffice()?,
        };

        discover::probe(&exe, config.timeout).await?;

        let max = config.max_concurrency.max(1);
        info!(executable = %exe.display(), timeout = ?config.timeout, max_concurrency = max, "LibreOffice engine launched");
        Ok(Self {
            inner: Arc::new(Inner {
                exe,
                timeout: config.timeout,
                semaphore: Semaphore::new(max),
            }),
        })
    }

    /// Convert one input file to PDF bytes.
    ///
    /// The input may be any LibreOffice-supported format; see
    /// [`filter::for_extension`] for the dispatch table. Concurrent calls
    /// are gated by `max_concurrency` and each gets a fresh
    /// `UserInstallation` directory.
    #[instrument(skip_all, fields(input = %input.display()))]
    pub async fn convert(&self, input: &Path, opts: &OfficeOptions) -> EngineResult<Vec<u8>> {
        let _span = self.logger();
        opts.validate()?;
        if !input.exists() {
            return Err(EngineError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("input not found: {}", input.display()),
            )));
        }
        let _permit = self
            .inner
            .semaphore
            .acquire()
            .await
            .map_err(|e| EngineError::Internal(format!("semaphore closed: {e}")))?;
        debug!("Starting LibreOffice conversion");
        let start = std::time::Instant::now();
        let result = convert::run_convert(&self.inner.exe, self.inner.timeout, input, opts).await;
        let duration = start.elapsed();
        match &result {
            Ok(_) => info!(
                duration_ms = duration.as_millis() as u64,
                "LibreOffice conversion completed"
            ),
            Err(e) => tracing::error!(
                duration_ms = duration.as_millis() as u64,
                error = %e,
                "LibreOffice conversion failed"
            ),
        }
        result
    }

    /// Convert many inputs in parallel (bounded by `max_concurrency`),
    /// returning one `Vec<u8>` per input in the same order.
    ///
    /// Merging into a single PDF is **not** part of this API — call
    /// `engine::pdfops::merge` (spec 13) on the result if needed.
    #[instrument(skip_all)]
    pub async fn convert_many(
        &self,
        inputs: &[PathBuf],
        opts: &OfficeOptions,
    ) -> EngineResult<Vec<Vec<u8>>> {
        opts.validate()?;
        if inputs.is_empty() {
            return Ok(Vec::new());
        }

        let mut set = tokio::task::JoinSet::new();
        for (i, p) in inputs.iter().enumerate() {
            let engine = self.clone();
            let path = p.clone();
            let opts = opts.clone();
            set.spawn(async move {
                let res = engine.convert(&path, &opts).await;
                (i, res)
            });
        }

        let mut slots: Vec<Option<EngineResult<Vec<u8>>>> =
            (0..inputs.len()).map(|_| None).collect();
        while let Some(joined) = set.join_next().await {
            let (i, res) = joined.map_err(|e| EngineError::Internal(format!("join error: {e}")))?;
            slots[i] = Some(res);
        }

        let mut out = Vec::with_capacity(inputs.len());
        for slot in slots {
            match slot {
                Some(Ok(v)) => out.push(v),
                Some(Err(e)) => return Err(e),
                None => {
                    return Err(EngineError::Internal(
                        "convert_many: missing result slot".into(),
                    ));
                }
            }
        }
        info!(count = inputs.len(), "convert_many completed");
        Ok(out)
    }

    /// Returns `true` iff `soffice --version` succeeds within a 30-second
    /// timeout (regardless of the engine's `config.timeout`).
    pub async fn healthy(&self) -> bool {
        discover::probe(&self.inner.exe, Duration::from_secs(30))
            .await
            .is_ok()
    }
}

impl std::fmt::Debug for LibreOfficeEngine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LibreOfficeEngine")
            .field("exe", &self.inner.exe)
            .field("timeout", &self.inner.timeout)
            .finish()
    }
}

// ---------------------------------------------------------------------------
// Options
// ---------------------------------------------------------------------------

/// Per-call conversion options. All fields are optional; defaults match
/// LibreOffice's own export defaults.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default, rename_all = "camelCase")]
pub struct OfficeOptions {
    /// Render in landscape orientation.
    pub landscape: bool,
    /// Subset of pages to include in the output.
    pub page_ranges: Option<PageRanges>,
    /// PDF/A profile, if any.
    pub pdf_a: Option<PdfAProfile>,
    /// PDF/UA accessibility tagging.
    pub pdf_ua: bool,
    /// JPEG quality knob for embedded raster images. `1..=100`. `None` =
    /// LibreOffice default.
    pub quality: Option<u8>,
    /// Reduce image resolution (DPI). `None` = LibreOffice default.
    pub max_image_resolution: Option<u32>,

    // --- Bookmarks & Index ---
    /// Export bookmarks to PDF outline.
    pub export_bookmarks: bool,
    /// Export bookmarks as Named Destinations.
    pub export_bookmarks_to_pdf_destination: bool,
    /// Update document indexes before conversion.
    pub update_indexes: bool,

    // --- Form Fields & Placeholders ---
    /// Export form fields as interactive widgets.
    pub export_form_fields: bool,
    /// Allow duplicate field names in exported forms.
    pub allow_duplicate_field_names: bool,
    /// Export placeholder field visual markings.
    pub export_placeholders: bool,

    // --- Notes & Margins ---
    /// Export notes to PDF.
    pub export_notes: bool,
    /// Export notes pages (Impress only).
    pub export_notes_pages: bool,
    /// Export only notes pages.
    pub export_only_notes_pages: bool,
    /// Export notes in margin.
    pub export_notes_in_margin: bool,

    // --- Advanced Options ---
    /// Convert .od* link targets to .pdf.
    pub convert_ooo_target_to_pdf_target: bool,
    /// Export file:// links as relative paths.
    pub export_links_relative_fsys: bool,
    /// Export hidden slides (Impress only).
    pub export_hidden_slides: bool,
    /// Suppress automatically inserted empty pages.
    pub skip_empty_pages: bool,
    /// Embed original document as a stream for archiving.
    pub add_original_document_as_stream: bool,
    /// Put every spreadsheet sheet on exactly one page.
    pub single_page_sheets: bool,
    /// Use lossless (PNG) instead of JPEG for images.
    pub lossless_image_compression: bool,
    /// Reduce image resolution before embedding.
    pub reduce_image_resolution: bool,

    // --- Native Watermarks ---
    /// Watermark text drawn on every page.
    pub native_watermark_text: Option<String>,
    /// Watermark color as decimal RGB long (default 8388223 = light green).
    pub native_watermark_color: Option<u32>,
    /// Watermark font height in points.
    pub native_watermark_font_height: Option<u32>,
    /// Watermark rotation angle in degrees.
    pub native_watermark_rotate_angle: Option<i32>,
    /// Watermark font name (default "Helvetica").
    pub native_watermark_font_name: Option<String>,
    /// Tiled watermark text.
    pub native_tiled_watermark_text: Option<String>,

    // --- PDF Viewer Preferences ---
    /// Initial view mode (0=default, 1=bookmarks, 2=thumbnails, 3=layers).
    pub initial_view: Option<i32>,
    /// Page to open on (1-indexed).
    pub initial_page: Option<i32>,
    /// Magnification action (0=default, 1=fit width, 2=fit page, 3=fit visible, 4=use zoom).
    pub magnification: Option<i32>,
    /// Zoom percentage (only when magnification=4).
    pub zoom: Option<i32>,
    /// Page layout (0=default, 1=single page, 2=continuous, 3=facing, 4=continuous facing).
    pub page_layout: Option<i32>,
    /// First page on left side (used with facing layout).
    pub first_page_on_left: bool,
    /// Resize viewer window to fit initial page.
    pub resize_window_to_initial_page: bool,
    /// Center viewer window on screen.
    pub center_window: bool,
    /// Open in full-screen mode.
    pub open_in_full_screen_mode: bool,
    /// Display document title in viewer title bar.
    pub display_pdf_document_title: bool,
    /// Hide viewer menubar.
    pub hide_viewer_menubar: bool,
    /// Hide viewer toolbar.
    pub hide_viewer_toolbar: bool,
    /// Hide viewer window controls.
    pub hide_viewer_window_controls: bool,
    /// Export slide transition effects (Impress only).
    pub use_transition_effects: bool,
    /// How many bookmark levels to auto-open (-1=all, 0=none, 1-10=specific).
    pub open_bookmark_levels: Option<i32>,
}

impl OfficeOptions {
    /// Validate the option set. Called at the top of [`LibreOfficeEngine::convert`]
    /// and [`LibreOfficeEngine::convert_many`].
    ///
    /// # Errors
    ///
    /// Returns [`EngineError::InvalidOption`] if `quality` is outside
    /// `1..=100`, if `max_image_resolution` is `Some(0)`, or if
    /// `page_ranges` is somehow empty.
    pub fn validate(&self) -> EngineResult<()> {
        if let Some(q) = self.quality
            && !(1..=100).contains(&q)
        {
            return Err(EngineError::InvalidOption(format!(
                "quality must be 1..=100 (got {q})"
            )));
        }
        if let Some(r) = self.max_image_resolution {
            if r == 0 {
                return Err(EngineError::InvalidOption(
                    "maxImageResolution must be > 0".into(),
                ));
            }
            if ![75, 150, 300, 600, 1200].contains(&r) {
                return Err(EngineError::InvalidOption(format!(
                    "maxImageResolution must be one of 75, 150, 300, 600, 1200 (got {r})"
                )));
            }
        }
        if let Some(pr) = &self.page_ranges
            && pr.as_slice().is_empty()
        {
            return Err(EngineError::InvalidOption("pageRanges is empty".into()));
        }
        if let Some(v) = self.initial_view {
            if !(0..=3).contains(&v) {
                return Err(EngineError::InvalidOption(format!(
                    "initialView must be 0..=3 (got {v})"
                )));
            }
        }
        if let Some(v) = self.initial_page {
            if v < 1 {
                return Err(EngineError::InvalidOption(format!(
                    "initialPage must be >= 1 (got {v})"
                )));
            }
        }
        if let Some(v) = self.magnification {
            if !(0..=4).contains(&v) {
                return Err(EngineError::InvalidOption(format!(
                    "magnification must be 0..=4 (got {v})"
                )));
            }
        }
        if let Some(v) = self.zoom {
            if v < 1 {
                return Err(EngineError::InvalidOption(format!(
                    "zoom must be >= 1 (got {v})"
                )));
            }
        }
        if let Some(v) = self.page_layout {
            if !(0..=4).contains(&v) {
                return Err(EngineError::InvalidOption(format!(
                    "pageLayout must be 0..=4 (got {v})"
                )));
            }
        }
        if let Some(v) = self.open_bookmark_levels {
            if v != -1 && !(1..=10).contains(&v) {
                return Err(EngineError::InvalidOption(format!(
                    "openBookmarkLevels must be -1 or 1..=10 (got {v})"
                )));
            }
        }
        Ok(())
    }

    /// Build the LibreOffice filter-options blob (the `:{...}` suffix on
    /// `--convert-to`). Returns `None` if no fields are set, in which case
    /// the bare exporter (e.g. `pdf:writer_pdf_Export`) is used unmodified.
    pub(crate) fn filter_blob(&self) -> Option<String> {
        let mut map = serde_json::Map::new();

        if let Some(pr) = &self.page_ranges {
            map.insert("PageRange".into(), entry_str(&pr.to_string()));
        }
        if let Some(prof) = self.pdf_a {
            let v: i64 = match prof {
                PdfAProfile::A1B => 1,
                PdfAProfile::A2B => 2,
                PdfAProfile::A3B => 3,
            };
            map.insert("SelectPdfVersion".into(), entry_long(v));
        }
        if self.pdf_ua {
            map.insert("PDFUACompliance".into(), entry_bool(true));
        }
        if let Some(q) = self.quality {
            map.insert("Quality".into(), entry_long(i64::from(q)));
        }
        if let Some(r) = self.max_image_resolution {
            map.insert("MaxImageResolution".into(), entry_long(i64::from(r)));
        }
        if self.landscape {
            map.insert("IsLandscape".into(), entry_bool(true));
        }

        // Bookmarks
        if self.export_bookmarks {
            map.insert("ExportBookmarks".into(), entry_bool(true));
        }
        if self.export_bookmarks_to_pdf_destination {
            map.insert("ExportBookmarksToPDFDestination".into(), entry_bool(true));
        }

        // Form Fields
        if self.export_form_fields {
            map.insert("ExportFormFields".into(), entry_bool(true));
        }
        if self.allow_duplicate_field_names {
            map.insert("AllowDuplicateFieldNames".into(), entry_bool(true));
        }
        if self.export_placeholders {
            map.insert("ExportPlaceholders".into(), entry_bool(true));
        }

        // Notes
        if self.export_notes {
            map.insert("ExportNotes".into(), entry_bool(true));
        }
        if self.export_notes_pages {
            map.insert("ExportNotesPages".into(), entry_bool(true));
        }
        if self.export_only_notes_pages {
            map.insert("ExportOnlyNotesPages".into(), entry_bool(true));
        }
        if self.export_notes_in_margin {
            map.insert("ExportNotesInMargin".into(), entry_bool(true));
        }

        // Advanced
        if self.convert_ooo_target_to_pdf_target {
            map.insert("ConvertOOoTargetToPDFTarget".into(), entry_bool(true));
        }
        if self.export_links_relative_fsys {
            map.insert("ExportLinksRelativeFsys".into(), entry_bool(true));
        }
        if self.export_hidden_slides {
            map.insert("ExportHiddenSlides".into(), entry_bool(true));
        }
        if self.skip_empty_pages {
            map.insert("IsSkipEmptyPages".into(), entry_bool(true));
        }
        if self.add_original_document_as_stream {
            map.insert("IsAddStream".into(), entry_bool(true));
        }
        if self.single_page_sheets {
            map.insert("SinglePageSheets".into(), entry_bool(true));
        }
        if self.lossless_image_compression {
            map.insert("UseLosslessCompression".into(), entry_bool(true));
        }
        if self.reduce_image_resolution {
            map.insert("ReduceImageResolution".into(), entry_bool(true));
        }

        // Native Watermarks
        if let Some(ref text) = self.native_watermark_text {
            map.insert("Watermark".into(), entry_str(text));
        }
        if let Some(color) = self.native_watermark_color {
            map.insert("WatermarkColor".into(), entry_long(i64::from(color)));
        }
        if let Some(h) = self.native_watermark_font_height {
            map.insert("WatermarkFontHeight".into(), entry_long(i64::from(h)));
        }
        if let Some(angle) = self.native_watermark_rotate_angle {
            map.insert("WatermarkRotateAngle".into(), entry_long(i64::from(angle)));
        }
        if let Some(ref name) = self.native_watermark_font_name {
            map.insert("WatermarkFontName".into(), entry_str(name));
        }
        if let Some(ref text) = self.native_tiled_watermark_text {
            map.insert("TiledWatermark".into(), entry_str(text));
        }

        // Viewer Preferences
        if let Some(v) = self.initial_view {
            map.insert("InitialView".into(), entry_long(i64::from(v)));
        }
        if let Some(v) = self.initial_page {
            map.insert("InitialPage".into(), entry_long(i64::from(v)));
        }
        if let Some(v) = self.magnification {
            map.insert("Magnification".into(), entry_long(i64::from(v)));
        }
        if let Some(v) = self.zoom {
            map.insert("Zoom".into(), entry_long(i64::from(v)));
        }
        if let Some(v) = self.page_layout {
            map.insert("PageLayout".into(), entry_long(i64::from(v)));
        }
        if self.first_page_on_left {
            map.insert("FirstPageOnLeft".into(), entry_bool(true));
        }
        if self.resize_window_to_initial_page {
            map.insert("ResizeWindowToInitialPage".into(), entry_bool(true));
        }
        if self.center_window {
            map.insert("CenterWindow".into(), entry_bool(true));
        }
        if self.open_in_full_screen_mode {
            map.insert("OpenInFullScreenMode".into(), entry_bool(true));
        }
        if self.display_pdf_document_title {
            map.insert("DisplayPDFDocumentTitle".into(), entry_bool(true));
        }
        if self.hide_viewer_menubar {
            map.insert("HideViewerMenubar".into(), entry_bool(true));
        }
        if self.hide_viewer_toolbar {
            map.insert("HideViewerToolbar".into(), entry_bool(true));
        }
        if self.hide_viewer_window_controls {
            map.insert("HideViewerWindowControls".into(), entry_bool(true));
        }
        if self.use_transition_effects {
            map.insert("UseTransitionEffects".into(), entry_bool(true));
        }
        if let Some(v) = self.open_bookmark_levels {
            map.insert("OpenBookmarkLevels".into(), entry_long(i64::from(v)));
        }

        if map.is_empty() {
            None
        } else {
            Some(serde_json::Value::Object(map).to_string())
        }
    }
}

fn entry_str(v: &str) -> serde_json::Value {
    serde_json::json!({ "type": "string", "value": v })
}

fn entry_long(v: i64) -> serde_json::Value {
    serde_json::json!({ "type": "long", "value": v })
}

fn entry_bool(v: bool) -> serde_json::Value {
    serde_json::json!({ "type": "boolean", "value": v })
}

/// PDF/A export profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PdfAProfile {
    /// PDF/A-1b — the most conservative subset (PDF 1.4-based).
    A1B,
    /// PDF/A-2b — based on PDF 1.7; supports JPEG2000 and transparency.
    A2B,
    /// PDF/A-3b — like 2b plus permits embedded arbitrary file attachments.
    A3B,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn engine_is_send_sync_clone() {
        use static_assertions::assert_impl_all;
        assert_impl_all!(LibreOfficeEngine: Send, Sync, Clone);
        assert_impl_all!(LibreOfficeConfig: Send, Sync, Clone);
        assert_impl_all!(OfficeOptions: Send, Sync, Clone);
        assert_impl_all!(PdfAProfile: Send, Sync, Clone, Copy);
    }

    #[test]
    fn libreoffice_config_default_matches_spec() {
        let c = LibreOfficeConfig::default();
        assert!(c.executable.is_none());
        assert_eq!(c.timeout, Duration::from_secs(120));
        assert!(c.max_concurrency >= 1);
    }

    #[test]
    fn office_options_default_emits_no_filter_blob() {
        assert!(OfficeOptions::default().filter_blob().is_none());
    }

    #[test]
    fn office_options_with_page_ranges_emits_pagerange_key() {
        let opts = OfficeOptions {
            page_ranges: Some(PageRanges::parse("1-3,5").expect("parse")),
            ..Default::default()
        };
        let blob = opts.filter_blob().expect("blob");
        let v: serde_json::Value = serde_json::from_str(&blob).expect("json");
        assert_eq!(v["PageRange"]["type"], "string");
        assert_eq!(v["PageRange"]["value"], "1-3,5");
    }

    #[test]
    fn office_options_with_pdf_a_maps_select_pdf_version_long() {
        let cases = [
            (PdfAProfile::A1B, 1),
            (PdfAProfile::A2B, 2),
            (PdfAProfile::A3B, 3),
        ];
        for (prof, expected) in cases {
            let opts = OfficeOptions {
                pdf_a: Some(prof),
                ..Default::default()
            };
            let blob = opts.filter_blob().expect("blob");
            let v: serde_json::Value = serde_json::from_str(&blob).expect("json");
            assert_eq!(v["SelectPdfVersion"]["type"], "long");
            assert_eq!(v["SelectPdfVersion"]["value"], expected);
        }
    }

    #[test]
    fn office_options_landscape_and_pdfua_blob_keys() {
        let opts = OfficeOptions {
            landscape: true,
            pdf_ua: true,
            ..Default::default()
        };
        let blob = opts.filter_blob().expect("blob");
        let v: serde_json::Value = serde_json::from_str(&blob).expect("json");
        assert_eq!(v["IsLandscape"]["type"], "boolean");
        assert_eq!(v["IsLandscape"]["value"], true);
        assert_eq!(v["PDFUACompliance"]["type"], "boolean");
        assert_eq!(v["PDFUACompliance"]["value"], true);
    }

    #[test]
    fn office_options_quality_and_resolution_blob_long() {
        let opts = OfficeOptions {
            quality: Some(75),
            max_image_resolution: Some(150),
            ..Default::default()
        };
        let blob = opts.filter_blob().expect("blob");
        let v: serde_json::Value = serde_json::from_str(&blob).expect("json");
        assert_eq!(v["Quality"]["type"], "long");
        assert_eq!(v["Quality"]["value"], 75);
        assert_eq!(v["MaxImageResolution"]["type"], "long");
        assert_eq!(v["MaxImageResolution"]["value"], 150);
    }

    #[test]
    fn office_options_quality_zero_rejected() {
        let opts = OfficeOptions {
            quality: Some(0),
            ..Default::default()
        };
        assert!(matches!(
            opts.validate(),
            Err(EngineError::InvalidOption(_))
        ));
    }

    #[test]
    fn office_options_quality_above_100_rejected() {
        let opts = OfficeOptions {
            quality: Some(101),
            ..Default::default()
        };
        assert!(matches!(
            opts.validate(),
            Err(EngineError::InvalidOption(_))
        ));
    }

    #[test]
    fn office_options_max_image_resolution_zero_rejected() {
        let opts = OfficeOptions {
            max_image_resolution: Some(0),
            ..Default::default()
        };
        assert!(matches!(
            opts.validate(),
            Err(EngineError::InvalidOption(_))
        ));
    }

    #[test]
    fn office_options_default_validates_ok() {
        assert!(OfficeOptions::default().validate().is_ok());
    }

    #[test]
    fn office_options_serde_camel_case_roundtrip() {
        let opts = OfficeOptions {
            landscape: true,
            page_ranges: Some(PageRanges::parse("1-3").expect("parse")),
            pdf_a: Some(PdfAProfile::A2B),
            pdf_ua: true,
            quality: Some(80),
            max_image_resolution: Some(200),
            ..Default::default()
        };
        let json = serde_json::to_value(&opts).expect("ser");
        assert_eq!(json["pageRanges"], "1-3");
        assert_eq!(json["pdfA"], "a2-b");
        assert_eq!(json["pdfUa"], true);
        assert_eq!(json["maxImageResolution"], 200);
        let back: OfficeOptions = serde_json::from_value(json).expect("de");
        assert_eq!(back, opts);
    }

    #[test]
    fn office_options_deserialise_with_missing_fields() {
        let v: OfficeOptions = serde_json::from_str("{}").expect("de");
        assert_eq!(v, OfficeOptions::default());
    }

    #[test]
    fn office_options_filter_blob_all_new_fields() {
        let opts = OfficeOptions {
            landscape: true,
            export_bookmarks: true,
            export_bookmarks_to_pdf_destination: true,
            export_form_fields: true,
            allow_duplicate_field_names: true,
            export_placeholders: true,
            export_notes: true,
            export_notes_pages: true,
            export_only_notes_pages: true,
            export_notes_in_margin: true,
            convert_ooo_target_to_pdf_target: true,
            export_links_relative_fsys: true,
            export_hidden_slides: true,
            skip_empty_pages: true,
            add_original_document_as_stream: true,
            single_page_sheets: true,
            lossless_image_compression: true,
            reduce_image_resolution: true,
            native_watermark_text: Some("SECRET".into()),
            native_watermark_color: Some(16711680),
            native_watermark_font_height: Some(20),
            native_watermark_rotate_angle: Some(30),
            native_watermark_font_name: Some("Times".into()),
            native_tiled_watermark_text: Some("DRAFT".into()),
            initial_view: Some(1),
            initial_page: Some(5),
            magnification: Some(2),
            zoom: Some(150),
            page_layout: Some(3),
            first_page_on_left: true,
            resize_window_to_initial_page: true,
            center_window: true,
            open_in_full_screen_mode: true,
            display_pdf_document_title: true,
            hide_viewer_menubar: true,
            hide_viewer_toolbar: true,
            hide_viewer_window_controls: true,
            use_transition_effects: true,
            open_bookmark_levels: Some(-1),
            ..Default::default()
        };
        let blob = opts.filter_blob().expect("blob");
        let v: serde_json::Value = serde_json::from_str(&blob).expect("json");
        assert_eq!(v["ExportBookmarks"]["type"], "boolean");
        assert_eq!(v["ExportBookmarks"]["value"], true);
        assert_eq!(v["ExportBookmarksToPDFDestination"]["value"], true);
        assert_eq!(v["ExportFormFields"]["value"], true);
        assert_eq!(v["AllowDuplicateFieldNames"]["value"], true);
        assert_eq!(v["ExportPlaceholders"]["value"], true);
        assert_eq!(v["ExportNotes"]["value"], true);
        assert_eq!(v["ExportNotesPages"]["value"], true);
        assert_eq!(v["ExportOnlyNotesPages"]["value"], true);
        assert_eq!(v["ExportNotesInMargin"]["value"], true);
        assert_eq!(v["ConvertOOoTargetToPDFTarget"]["value"], true);
        assert_eq!(v["ExportLinksRelativeFsys"]["value"], true);
        assert_eq!(v["ExportHiddenSlides"]["value"], true);
        assert_eq!(v["IsSkipEmptyPages"]["value"], true);
        assert_eq!(v["IsAddStream"]["value"], true);
        assert_eq!(v["SinglePageSheets"]["value"], true);
        assert_eq!(v["UseLosslessCompression"]["value"], true);
        assert_eq!(v["ReduceImageResolution"]["value"], true);
        assert_eq!(v["Watermark"]["type"], "string");
        assert_eq!(v["Watermark"]["value"], "SECRET");
        assert_eq!(v["WatermarkColor"]["type"], "long");
        assert_eq!(v["WatermarkColor"]["value"], 16711680);
        assert_eq!(v["WatermarkFontHeight"]["value"], 20);
        assert_eq!(v["WatermarkRotateAngle"]["value"], 30);
        assert_eq!(v["WatermarkFontName"]["value"], "Times");
        assert_eq!(v["TiledWatermark"]["value"], "DRAFT");
        assert_eq!(v["InitialView"]["type"], "long");
        assert_eq!(v["InitialView"]["value"], 1);
        assert_eq!(v["InitialPage"]["value"], 5);
        assert_eq!(v["Magnification"]["value"], 2);
        assert_eq!(v["Zoom"]["value"], 150);
        assert_eq!(v["PageLayout"]["value"], 3);
        assert_eq!(v["FirstPageOnLeft"]["value"], true);
        assert_eq!(v["ResizeWindowToInitialPage"]["value"], true);
        assert_eq!(v["CenterWindow"]["value"], true);
        assert_eq!(v["OpenInFullScreenMode"]["value"], true);
        assert_eq!(v["DisplayPDFDocumentTitle"]["value"], true);
        assert_eq!(v["HideViewerMenubar"]["value"], true);
        assert_eq!(v["HideViewerToolbar"]["value"], true);
        assert_eq!(v["HideViewerWindowControls"]["value"], true);
        assert_eq!(v["UseTransitionEffects"]["value"], true);
        assert_eq!(v["OpenBookmarkLevels"]["value"], -1);
    }

    #[test]
    fn office_options_validation_new_ranges() {
        let bad = OfficeOptions {
            initial_view: Some(5),
            ..Default::default()
        };
        assert!(matches!(bad.validate(), Err(EngineError::InvalidOption(_))));

        let bad = OfficeOptions {
            magnification: Some(10),
            ..Default::default()
        };
        assert!(matches!(bad.validate(), Err(EngineError::InvalidOption(_))));

        let bad = OfficeOptions {
            max_image_resolution: Some(200),
            ..Default::default()
        };
        assert!(matches!(bad.validate(), Err(EngineError::InvalidOption(_))));

        let ok = OfficeOptions {
            initial_view: Some(2),
            magnification: Some(3),
            zoom: Some(200),
            page_layout: Some(1),
            open_bookmark_levels: Some(5),
            max_image_resolution: Some(300),
            ..Default::default()
        };
        assert!(ok.validate().is_ok());
    }

    #[tokio::test]
    async fn launch_with_missing_executable_path_errors() {
        let cfg = LibreOfficeConfig {
            executable: Some(PathBuf::from("/nonexistent/__folio_no_soffice")),
            ..LibreOfficeConfig::default()
        };
        let err = LibreOfficeEngine::launch(cfg)
            .await
            .expect_err("should fail");
        assert!(matches!(err, EngineError::Internal(_)));
    }
}
