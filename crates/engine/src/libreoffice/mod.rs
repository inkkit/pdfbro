//! `LibreOfficeEngine` — convert office documents to PDF via unoserver.
//!
//! Implementation of `docs/specs/12-engine-libreoffice.md`. A persistent
//! `unoserver` process is managed as a child, eliminating per-request
//! soffice startup cost (~200–400ms).

pub mod filter;

mod convert;
mod unoserver;

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tokio::sync::{Mutex, Semaphore};

use tracing::{debug, info, instrument};

use crate::types::{EngineError, EngineResult, PageRanges};

// ---------------------------------------------------------------------------
// Engine
// ---------------------------------------------------------------------------

/// Wrapper around a persistent `unoserver` process. Cheap to clone (`Arc` inside).
///
/// # Example
///
/// ```no_run
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
    unoserver: Mutex<unoserver::UnoserverProcess>,
    port: u16,
    unoserver_ready_timeout: Duration,
    executable: Option<PathBuf>,
    client: reqwest::Client,
    timeout: Duration,
    semaphore: Semaphore,
}

/// Engine-wide configuration. Pass to [`LibreOfficeEngine::launch`].
#[derive(Debug, Clone)]
pub struct LibreOfficeConfig {
    /// Path to `soffice` passed to unoserver via `--executable`. `None` = unoserver
    /// auto-discovers soffice.
    pub executable: Option<PathBuf>,
    /// Per-conversion timeout. Default 120s.
    pub timeout: Duration,
    /// Maximum concurrent conversions. Default [`std::thread::available_parallelism`].
    pub max_concurrency: usize,
    /// Use lazy initialization (start on first request).
    /// Default: false (start eagerly at server startup).
    pub lazy_start: bool,
    /// Idle shutdown timeout - engine shuts down after this duration of no requests.
    /// None means no idle shutdown. Default: None.
    pub idle_shutdown_timeout: Option<Duration>,
    /// Port unoserver listens on. Default: 2003.
    pub unoserver_port: u16,
    /// Maximum time to wait for unoserver to be ready at startup. Default: 60s.
    pub unoserver_ready_timeout: Duration,
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
            unoserver_port: 2003,
            unoserver_ready_timeout: Duration::from_secs(60),
        }
    }
}

impl LibreOfficeEngine {
    /// Discover `soffice` on `$PATH` and platform defaults using
    /// [`LibreOfficeConfig::default`].
    pub async fn discover() -> EngineResult<Self> {
        Self::launch(LibreOfficeConfig::default()).await
    }

    /// Construct an engine with explicit configuration.
    ///
    /// Spawns a persistent `unoserver` process. The engine is returned once
    /// unoserver is ready to accept connections.
    pub async fn launch(config: LibreOfficeConfig) -> EngineResult<Self> {
        info!(port = config.unoserver_port, "Launching LibreOffice engine");

        let unoserver = unoserver::UnoserverProcess::spawn(
            config.unoserver_port,
            config.unoserver_ready_timeout,
            config.executable.as_deref(),
        )
        .await?;
        // If config requested port 0, the spawn helper picked a free one.
        let actual_port = unoserver.port();

        let max = config.max_concurrency.max(1);
        let client = reqwest::Client::builder()
            .tcp_keepalive(Some(Duration::from_secs(60)))
            .pool_max_idle_per_host(1)
            .build()
            .map_err(|e| EngineError::Internal(format!("failed to build HTTP client: {e}")))?;

        let inner = Arc::new(Inner {
            unoserver: Mutex::new(unoserver),
            port: actual_port,
            unoserver_ready_timeout: config.unoserver_ready_timeout,
            executable: config.executable,
            client,
            timeout: config.timeout,
            semaphore: Semaphore::new(max),
        });

        // Background task: detect unoserver crashes and restart.
        let inner_weak = Arc::downgrade(&inner);
        tokio::spawn(async move {
            'monitor: loop {
                tokio::time::sleep(Duration::from_secs(5)).await;
                let Some(inner) = inner_weak.upgrade() else { break };

                // try_wait() is non-blocking — it does not yield the lock for long.
                let exited = inner.unoserver.lock().await.try_wait().ok().flatten();

                if exited.is_some() {
                    tracing::warn!("unoserver exited unexpectedly, attempting restart");
                    for attempt in 0..3u32 {
                        tokio::time::sleep(Duration::from_secs(1 << attempt)).await;
                        let Some(inner) = inner_weak.upgrade() else { break 'monitor };
                        match unoserver::UnoserverProcess::spawn(
                            inner.port,
                            inner.unoserver_ready_timeout,
                            inner.executable.as_deref(),
                        )
                        .await
                        {
                            Ok(new_proc) => {
                                *inner.unoserver.lock().await = new_proc;
                                tracing::info!("unoserver restarted");
                                break;
                            }
                            Err(e) if attempt < 2 => {
                                tracing::warn!(attempt = attempt + 1, error = %e, "unoserver restart failed");
                            }
                            Err(_) => {
                                tracing::error!("unoserver failed to restart after 3 attempts, conversions will fail");
                                break 'monitor;
                            }
                        }
                    }
                }
            }
        });

        info!(port = actual_port, timeout = ?config.timeout, max_concurrency = max, "LibreOffice engine launched");
        Ok(Self { inner })
    }

    /// Convert one input file to PDF bytes.
    ///
    /// The input may be any LibreOffice-supported format; see
    /// [`filter::for_extension`] for the dispatch table. Concurrent calls
    /// are gated by `max_concurrency` and each gets a fresh
    /// `UserInstallation` directory.
    #[instrument(skip_all, fields(input = %input.display()))]
    pub async fn convert(&self, input: &Path, opts: &OfficeOptions) -> EngineResult<Vec<u8>> {
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
        let result = convert::run_convert(&self.inner.client, self.inner.port, self.inner.timeout, input, opts).await;
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

    /// Returns `true` iff unoserver responds to an HTTP GET within 5 seconds.
    pub async fn healthy(&self) -> bool {
        let url = format!("http://127.0.0.1:{}/", self.inner.port);
        tokio::time::timeout(
            Duration::from_secs(5),
            self.inner.client.get(&url).send(),
        )
        .await
        .map(|r| r.is_ok())
        .unwrap_or(false)
    }
}

impl std::fmt::Debug for LibreOfficeEngine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LibreOfficeEngine")
            .field("port", &self.inner.port)
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
    /// Note: not currently passed to unoserver (unoserver has no direct API for this).
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

    /// Build the unoserver `filter_options` array. Each entry is a
    /// `Name=Value` string; unoserver parses these with `split('=', 1)`
    /// and infers the value type (`true`/`false` → bool, digits → int,
    /// everything else → string).
    pub(crate) fn filter_options(&self) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();

        if let Some(pr) = &self.page_ranges {
            out.push(format!("PageRange={}", pr));
        }
        if let Some(prof) = self.pdf_a {
            let v: i64 = match prof {
                PdfAProfile::A1B => 1,
                PdfAProfile::A2B => 2,
                PdfAProfile::A3B => 3,
            };
            out.push(format!("SelectPdfVersion={v}"));
        }
        if self.pdf_ua {
            out.push("PDFUACompliance=true".into());
        }
        if let Some(q) = self.quality {
            out.push(format!("Quality={q}"));
        }
        if let Some(r) = self.max_image_resolution {
            out.push(format!("MaxImageResolution={r}"));
        }
        if self.landscape {
            out.push("IsLandscape=true".into());
        }

        if self.export_bookmarks {
            out.push("ExportBookmarks=true".into());
        }
        if self.export_bookmarks_to_pdf_destination {
            out.push("ExportBookmarksToPDFDestination=true".into());
        }

        if self.export_form_fields {
            out.push("ExportFormFields=true".into());
        }
        if self.allow_duplicate_field_names {
            out.push("AllowDuplicateFieldNames=true".into());
        }
        if self.export_placeholders {
            out.push("ExportPlaceholders=true".into());
        }

        if self.export_notes {
            out.push("ExportNotes=true".into());
        }
        if self.export_notes_pages {
            out.push("ExportNotesPages=true".into());
        }
        if self.export_only_notes_pages {
            out.push("ExportOnlyNotesPages=true".into());
        }
        if self.export_notes_in_margin {
            out.push("ExportNotesInMargin=true".into());
        }

        if self.convert_ooo_target_to_pdf_target {
            out.push("ConvertOOoTargetToPDFTarget=true".into());
        }
        if self.export_links_relative_fsys {
            out.push("ExportLinksRelativeFsys=true".into());
        }
        if self.export_hidden_slides {
            out.push("ExportHiddenSlides=true".into());
        }
        if self.skip_empty_pages {
            out.push("IsSkipEmptyPages=true".into());
        }
        if self.add_original_document_as_stream {
            out.push("IsAddStream=true".into());
        }
        if self.single_page_sheets {
            out.push("SinglePageSheets=true".into());
        }
        if self.lossless_image_compression {
            out.push("UseLosslessCompression=true".into());
        }
        if self.reduce_image_resolution {
            out.push("ReduceImageResolution=true".into());
        }

        if let Some(ref text) = self.native_watermark_text {
            out.push(format!("Watermark={text}"));
        }
        if let Some(color) = self.native_watermark_color {
            out.push(format!("WatermarkColor={color}"));
        }
        if let Some(h) = self.native_watermark_font_height {
            out.push(format!("WatermarkFontHeight={h}"));
        }
        if let Some(angle) = self.native_watermark_rotate_angle {
            out.push(format!("WatermarkRotateAngle={angle}"));
        }
        if let Some(ref name) = self.native_watermark_font_name {
            out.push(format!("WatermarkFontName={name}"));
        }
        if let Some(ref text) = self.native_tiled_watermark_text {
            out.push(format!("TiledWatermark={text}"));
        }

        if let Some(v) = self.initial_view {
            out.push(format!("InitialView={v}"));
        }
        if let Some(v) = self.initial_page {
            out.push(format!("InitialPage={v}"));
        }
        if let Some(v) = self.magnification {
            out.push(format!("Magnification={v}"));
        }
        if let Some(v) = self.zoom {
            out.push(format!("Zoom={v}"));
        }
        if let Some(v) = self.page_layout {
            out.push(format!("PageLayout={v}"));
        }
        if self.first_page_on_left {
            out.push("FirstPageOnLeft=true".into());
        }
        if self.resize_window_to_initial_page {
            out.push("ResizeWindowToInitialPage=true".into());
        }
        if self.center_window {
            out.push("CenterWindow=true".into());
        }
        if self.open_in_full_screen_mode {
            out.push("OpenInFullScreenMode=true".into());
        }
        if self.display_pdf_document_title {
            out.push("DisplayPDFDocumentTitle=true".into());
        }
        if self.hide_viewer_menubar {
            out.push("HideViewerMenubar=true".into());
        }
        if self.hide_viewer_toolbar {
            out.push("HideViewerToolbar=true".into());
        }
        if self.hide_viewer_window_controls {
            out.push("HideViewerWindowControls=true".into());
        }
        if self.use_transition_effects {
            out.push("UseTransitionEffects=true".into());
        }
        if let Some(v) = self.open_bookmark_levels {
            out.push(format!("OpenBookmarkLevels={v}"));
        }

        out
    }
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
        assert_eq!(c.unoserver_port, 2003);
        assert_eq!(c.unoserver_ready_timeout, Duration::from_secs(60));
    }

    #[test]
    fn office_options_default_emits_no_filter_options() {
        assert!(OfficeOptions::default().filter_options().is_empty());
    }

    #[test]
    fn office_options_with_page_ranges_emits_pagerange_entry() {
        let opts = OfficeOptions {
            page_ranges: Some(PageRanges::parse("1-3,5").expect("parse")),
            ..Default::default()
        };
        let opts = opts.filter_options();
        assert!(opts.contains(&"PageRange=1-3,5".to_string()), "{opts:?}");
    }

    #[test]
    fn office_options_with_pdf_a_maps_select_pdf_version() {
        let cases = [
            (PdfAProfile::A1B, "SelectPdfVersion=1"),
            (PdfAProfile::A2B, "SelectPdfVersion=2"),
            (PdfAProfile::A3B, "SelectPdfVersion=3"),
        ];
        for (prof, expected) in cases {
            let opts = OfficeOptions {
                pdf_a: Some(prof),
                ..Default::default()
            };
            let opts = opts.filter_options();
            assert!(opts.contains(&expected.to_string()), "{opts:?}");
        }
    }

    #[test]
    fn office_options_landscape_and_pdfua_entries() {
        let opts = OfficeOptions {
            landscape: true,
            pdf_ua: true,
            ..Default::default()
        };
        let opts = opts.filter_options();
        assert!(opts.contains(&"IsLandscape=true".to_string()), "{opts:?}");
        assert!(opts.contains(&"PDFUACompliance=true".to_string()), "{opts:?}");
    }

    #[test]
    fn office_options_quality_and_resolution_entries() {
        let opts = OfficeOptions {
            quality: Some(75),
            max_image_resolution: Some(150),
            ..Default::default()
        };
        let opts = opts.filter_options();
        assert!(opts.contains(&"Quality=75".to_string()), "{opts:?}");
        assert!(opts.contains(&"MaxImageResolution=150".to_string()), "{opts:?}");
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
    fn office_options_filter_options_all_new_fields() {
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
        let entries = opts.filter_options();
        let expected = [
            "ExportBookmarks=true",
            "ExportBookmarksToPDFDestination=true",
            "ExportFormFields=true",
            "AllowDuplicateFieldNames=true",
            "ExportPlaceholders=true",
            "ExportNotes=true",
            "ExportNotesPages=true",
            "ExportOnlyNotesPages=true",
            "ExportNotesInMargin=true",
            "ConvertOOoTargetToPDFTarget=true",
            "ExportLinksRelativeFsys=true",
            "ExportHiddenSlides=true",
            "IsSkipEmptyPages=true",
            "IsAddStream=true",
            "SinglePageSheets=true",
            "UseLosslessCompression=true",
            "ReduceImageResolution=true",
            "Watermark=SECRET",
            "WatermarkColor=16711680",
            "WatermarkFontHeight=20",
            "WatermarkRotateAngle=30",
            "WatermarkFontName=Times",
            "TiledWatermark=DRAFT",
            "InitialView=1",
            "InitialPage=5",
            "Magnification=2",
            "Zoom=150",
            "PageLayout=3",
            "FirstPageOnLeft=true",
            "ResizeWindowToInitialPage=true",
            "CenterWindow=true",
            "OpenInFullScreenMode=true",
            "DisplayPDFDocumentTitle=true",
            "HideViewerMenubar=true",
            "HideViewerToolbar=true",
            "HideViewerWindowControls=true",
            "UseTransitionEffects=true",
            "OpenBookmarkLevels=-1",
        ];
        for e in expected {
            assert!(
                entries.iter().any(|s| s == e),
                "missing {e}; got {entries:?}"
            );
        }
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

}
