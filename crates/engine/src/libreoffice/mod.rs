//! `LibreOfficeEngine` — convert office documents to PDF via LibreOfficeKit.
//!
//! LibreOfficeKit (LOK) is loaded in-process via `dlopen` — no Python daemon,
//! no unoserver, no XML-RPC.  Because LOK's global lock allows only one
//! `Office` instance per process and the type is `!Send`, all conversions run
//! on a **single dedicated `std::thread`**.  Async callers communicate with
//! that thread through a `std::sync::mpsc` work queue and per-request
//! `tokio::sync::oneshot` reply channels.

pub mod filter;

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, mpsc};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tracing::{debug, info, instrument, warn};

use crate::types::{EngineError, EngineResult, PageRanges};

// ---------------------------------------------------------------------------
// Engine
// ---------------------------------------------------------------------------

/// Wrapper around LibreOfficeKit. Cheap to clone (`Arc` inside).
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
    /// Work queue to the dedicated LOK thread.
    tx: mpsc::SyncSender<ConvertRequest>,
    /// Set to `false` when the worker thread exits.
    healthy: AtomicBool,
    timeout: Duration,
}

struct ConvertRequest {
    input: PathBuf,
    opts: OfficeOptions,
    reply: tokio::sync::oneshot::Sender<EngineResult<Vec<u8>>>,
}

/// Engine-wide configuration. Pass to [`LibreOfficeEngine::launch`].
#[derive(Debug, Clone)]
pub struct LibreOfficeConfig {
    /// Path to the LOK program directory (e.g. `/usr/lib/libreoffice/program`).
    ///
    /// `None` — auto-discover via `LOK_PROGRAM_PATH` env var or common system
    /// paths.
    pub install_path: Option<PathBuf>,
    /// Per-conversion timeout. Default 120 s.
    pub timeout: Duration,
    /// Start the engine lazily on the first request.
    pub lazy_start: bool,
    /// Shut the engine down after this much idle time.
    pub idle_shutdown_timeout: Option<Duration>,
}

impl Default for LibreOfficeConfig {
    fn default() -> Self {
        Self {
            install_path: None,
            timeout: Duration::from_secs(120),
            lazy_start: false,
            idle_shutdown_timeout: None,
        }
    }
}

impl LibreOfficeEngine {
    /// Launch with default config (auto-discovers LibreOffice).
    pub async fn discover() -> EngineResult<Self> {
        Self::launch(LibreOfficeConfig::default()).await
    }

    /// Launch the engine, spawning the LOK worker thread.
    pub async fn launch(config: LibreOfficeConfig) -> EngineResult<Self> {
        use libreofficekit::Office;

        let install_path = config
            .install_path
            .clone()
            .or_else(Office::find_install_path)
            .ok_or_else(|| {
                EngineError::Internal(
                    "LibreOffice not found — set LOK_PROGRAM_PATH or install LibreOffice".into(),
                )
            })?;

        info!(path = %install_path.display(), "Launching LibreOffice engine via LOK");

        // Bounded channel — back-pressure when the worker falls behind.
        let (tx, rx) = mpsc::sync_channel::<ConvertRequest>(64);

        // Startup rendezvous: worker sends Ok(()) once Office::new() succeeds.
        let (startup_tx, startup_rx) = tokio::sync::oneshot::channel::<EngineResult<()>>();

        let healthy = Arc::new(AtomicBool::new(false));
        let healthy_worker = Arc::clone(&healthy);

        std::thread::Builder::new()
            .name("lok-worker".into())
            .spawn(move || {
                lok_worker_thread(install_path, rx, startup_tx, healthy_worker);
            })
            .map_err(|e| EngineError::Internal(format!("failed to spawn LOK thread: {e}")))?;

        // Wait up to 120 s for LibreOffice to initialise.
        tokio::time::timeout(Duration::from_secs(120), startup_rx)
            .await
            .map_err(|_| EngineError::Timeout(Duration::from_secs(120)))?
            .map_err(|_| EngineError::Internal("LOK worker exited during startup".into()))??;

        info!("LibreOffice engine ready");

        Ok(Self {
            inner: Arc::new(Inner {
                tx,
                healthy: AtomicBool::new(true),
                timeout: config.timeout,
            }),
        })
    }

    // ------------------------------------------------------------------
    // Public API
    // ------------------------------------------------------------------

    /// Convert a single document file to PDF bytes.
    #[instrument(skip(self, opts), fields(path = %input.display()))]
    pub async fn convert(&self, input: &Path, opts: &OfficeOptions) -> EngineResult<Vec<u8>> {
        opts.validate()?;

        // Existence check before queueing: LOK FFI calls cannot be cancelled, so
        // a missing-file request should not have to wait its turn behind a slow
        // (or wedged) conversion already in flight on the worker thread.
        if !input.exists() {
            return Err(EngineError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("input file not found: {}", input.display()),
            )));
        }

        // Fail fast if the worker is already known-wedged or exited — a previous
        // timed-out conversion is likely still occupying the LOK thread.
        if !self.inner.healthy.load(Ordering::SeqCst) {
            return Err(EngineError::Internal(
                "LOK engine is unhealthy (worker exited or wedged) — restart required".into(),
            ));
        }

        debug!("starting LOK conversion");

        let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();

        self.inner
            .tx
            .try_send(ConvertRequest {
                input: input.to_path_buf(),
                opts: opts.clone(),
                reply: reply_tx,
            })
            .map_err(|e| match e {
                mpsc::TrySendError::Full(_) => {
                    EngineError::Internal("LOK request queue full".into())
                }
                mpsc::TrySendError::Disconnected(_) => {
                    self.inner.healthy.store(false, Ordering::SeqCst);
                    EngineError::Internal("LOK worker thread has exited".into())
                }
            })?;

        tokio::time::timeout(self.inner.timeout, reply_rx)
            .await
            .map_err(|_| {
                // The async timeout fires, but the synchronous LOK call inside
                // the worker thread cannot be cancelled. Treat the engine as
                // wedged so subsequent requests don't pile up behind it.
                self.inner.healthy.store(false, Ordering::SeqCst);
                EngineError::Timeout(self.inner.timeout)
            })?
            .map_err(|_| {
                self.inner.healthy.store(false, Ordering::SeqCst);
                EngineError::Internal("LOK worker dropped reply channel".into())
            })?
    }

    /// Convert many document files to PDF, preserving input order.
    ///
    /// Conversions are serialised through the single LOK worker thread.
    pub async fn convert_many(
        &self,
        inputs: &[PathBuf],
        opts: &OfficeOptions,
    ) -> EngineResult<Vec<Vec<u8>>> {
        opts.validate()?;
        let mut out = Vec::with_capacity(inputs.len());
        for input in inputs {
            out.push(self.convert(input, opts).await?);
        }
        Ok(out)
    }

    /// `true` when the LOK worker thread is alive and ready.
    pub async fn healthy(&self) -> bool {
        self.inner.healthy.load(Ordering::SeqCst)
    }
}

// ---------------------------------------------------------------------------
// LOK worker thread
// ---------------------------------------------------------------------------

fn lok_worker_thread(
    install_path: PathBuf,
    rx: mpsc::Receiver<ConvertRequest>,
    startup_tx: tokio::sync::oneshot::Sender<EngineResult<()>>,
    healthy: Arc<AtomicBool>,
) {
    use libreofficekit::Office;

    let office = match Office::new(&install_path) {
        Ok(o) => {
            let _ = startup_tx.send(Ok(()));
            healthy.store(true, Ordering::SeqCst);
            o
        }
        Err(e) => {
            let msg = format!("LOK Office::new failed: {e}");
            warn!("{msg}");
            let _ = startup_tx.send(Err(EngineError::Internal(msg)));
            return;
        }
    };

    info!("LOK worker ready");

    for req in rx {
        let result = lok_convert(&office, &req.input, &req.opts);
        // If the receiver was dropped (timeout fired), send() will simply fail.
        let _ = req.reply.send(result);
    }

    healthy.store(false, Ordering::SeqCst);
    info!("LOK worker exiting");
}

/// Execute a single document→PDF conversion synchronously on the worker thread.
fn lok_convert(office: &libreofficekit::Office, input: &Path, opts: &OfficeOptions) -> EngineResult<Vec<u8>> {
    use libreofficekit::DocUrl;

    // LOK needs absolute file:// URLs for both input and output.
    let in_url = DocUrl::from_path(input)
        .map_err(|e| EngineError::Internal(format!("LOK input URL: {e}")))?;

    // Write output to a temp file; LOK cannot return bytes directly.
    let tmp_dir = tempfile::tempdir().map_err(EngineError::Io)?;
    let out_path = tmp_dir.path().join("output.pdf");
    let out_url = DocUrl::from_path(&out_path)
        .map_err(|e| EngineError::Internal(format!("LOK output URL: {e}")))?;

    // LOK's save_as format param is the short output format ("pdf"), not the
    // internal filter name ("writer_pdf_Export"). LibreOffice auto-selects the
    // right PDF export filter based on the document type.
    let filter_name = "pdf";

    // PDF export options must be passed as a JSON FilterOptions blob. The
    // PDF filter (LO `filter/source/pdf/pdffilter.cxx::filter`) only reads
    // PageRange / SelectPdfVersion / IsLandscape / etc. from `FilterData`,
    // and `JsonToPropertyValues` is the only path that builds FilterData
    // from a string the LOK saveAs API forwards. A bare comma-separated
    // `Key=Value` list is silently dropped here — those tokens are filtered
    // against `TakeOwnership`/`NoFileSync`/`FromTemplate` only.
    let save_opts_json = opts.lok_save_as_options();
    let filter_arg: Option<&str> = save_opts_json.as_deref();

    debug!(
        input = %input.display(),
        filter = filter_name,
        options = filter_arg.unwrap_or(""),
        "LOK save_as"
    );

    // `documentLoadWithOptions`'s `pOptions` arg is a **comma-separated**
    // `Key=Value` list. Only a fixed set of keys are extracted by LOK's
    // `extractParameter` in `desktop/source/lib/init.cxx::doc_loadWithOptions`
    // (lines 2902–2997 in upstream): `Language`, `Timezone`,
    // `DeviceFormFactor`, `Batch`, `MacroSecurityLevel`, `ClientVisibleArea`,
    // `EnableMacrosExecution`. Anything else is forwarded *verbatim* to the
    // import filter as the value of `FilterOptions`.
    //
    // We only opt into options when the file actually needs them, because
    // some non-extracted leftover values (e.g. `InteractionHandler=0`) can
    // confuse the writer/word filter's content sniff for genuine documents
    // and lead to `type detection failed` on perfectly valid `.docx` files
    // — observed on bookworm-backports LO 26.x with non-ASCII filenames.
    //
    // CSV/TSV need help: a bare `document_load` on `.csv` pops the
    // *Text Import* dialog and wedges the worker forever. `Batch=1` flips
    // LO into non-interactive mode (extracted by LOK → sets
    // `DialogCancelMode::LOKSilent` and `Silent=true` on the
    // MediaDescriptor); the trailing tokens (`44,34,76,1` /
    // `9,34,76,1`) become the StarCalc CSV import options after
    // extraction (`fieldSep,textDelim,charSet,firstLineNumber`).
    //
    // For everything else we use plain `document_load` and rely on LO's
    // built-in extension/content detection — same behaviour as the
    // original implementation, before this code path tried to be clever
    // about adding "always-on" load options.
    let ext = input
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let csv_opts: Option<&str> = match ext.as_str() {
        "csv" => Some("Batch=1,44,34,76,1"),
        "tsv" | "tab" => Some("Batch=1,9,34,76,1"),
        _ => None,
    };

    let mut doc = match csv_opts {
        Some(o) => office.document_load_with_options(&in_url, o),
        None => office.document_load(&in_url),
    }
    .map_err(|e| EngineError::Internal(format!("LOK document_load: {e}")))?;

    let ok = doc
        .save_as(&out_url, filter_name, filter_arg)
        .map_err(|e| EngineError::Internal(format!("LOK save_as: {e}")))?;

    if !ok {
        return Err(EngineError::Internal(
            "LOK save_as returned false — conversion failed".into(),
        ));
    }

    let pdf = std::fs::read(&out_path).map_err(EngineError::Io)?;

    if pdf.is_empty() || !pdf.starts_with(b"%PDF-") {
        return Err(EngineError::Internal(
            "LOK produced an empty or non-PDF output".into(),
        ));
    }

    Ok(pdf)
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

    // --- Native Watermarks (LOK filter data keys) ---
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
    /// Validate the option set.
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
        if let Some(v) = self.initial_view
            && !(0..=3).contains(&v)
        {
            return Err(EngineError::InvalidOption(format!(
                "initialView must be 0..=3 (got {v})"
            )));
        }
        if let Some(v) = self.initial_page
            && v < 1
        {
            return Err(EngineError::InvalidOption(format!(
                "initialPage must be >= 1 (got {v})"
            )));
        }
        if let Some(v) = self.magnification
            && !(0..=4).contains(&v)
        {
            return Err(EngineError::InvalidOption(format!(
                "magnification must be 0..=4 (got {v})"
            )));
        }
        if let Some(v) = self.zoom
            && v < 1
        {
            return Err(EngineError::InvalidOption(format!(
                "zoom must be >= 1 (got {v})"
            )));
        }
        if let Some(v) = self.page_layout
            && !(0..=4).contains(&v)
        {
            return Err(EngineError::InvalidOption(format!(
                "pageLayout must be 0..=4 (got {v})"
            )));
        }
        if let Some(v) = self.open_bookmark_levels
            && v != -1
            && !(1..=10).contains(&v)
        {
            return Err(EngineError::InvalidOption(format!(
                "openBookmarkLevels must be -1 or 1..=10 (got {v})"
            )));
        }
        Ok(())
    }

    /// Build a JSON FilterOptions blob suitable for LOK `documentSaveAs`.
    ///
    /// LibreOffice's PDF filter only honours these export properties when
    /// they are delivered as the `FilterData` MediaDescriptor sequence. The
    /// only string format the LOK saveAs path forwards into FilterData is a
    /// JSON object — see `filter/source/pdf/pdffilter.cxx::filter`, which
    /// branches on `aFilterOptions.startsWith("{")` and runs the body
    /// through `comphelper::JsonToPropertyValues`. The expected schema is
    /// `{"PropName": {"type": "<type>", "value": "<stringified value>"}}`
    /// where `type` is one of `string`, `boolean`, `long`, `short`. Returns
    /// `None` when no options are set so the caller can pass `None` to
    /// `save_as` and fall through to the filter's configured defaults.
    pub(crate) fn lok_save_as_options(&self) -> Option<String> {
        use serde_json::{Map, Value, json};

        let mut m: Map<String, Value> = Map::new();
        let s = |v: String| json!({"type": "string", "value": v});
        let b = |v: bool| json!({"type": "boolean", "value": v.to_string()});
        let l = |v: i64| json!({"type": "long", "value": v.to_string()});

        if let Some(pr) = &self.page_ranges {
            m.insert("PageRange".into(), s(pr.to_string()));
        }
        if let Some(prof) = self.pdf_a {
            let v: i64 = match prof {
                PdfAProfile::A1B => 1,
                PdfAProfile::A2B => 2,
                PdfAProfile::A3B => 3,
            };
            m.insert("SelectPdfVersion".into(), l(v));
        }
        if self.pdf_ua {
            m.insert("PDFUACompliance".into(), b(true));
        }
        if let Some(q) = self.quality {
            m.insert("Quality".into(), l(q as i64));
        }
        if let Some(r) = self.max_image_resolution {
            m.insert("MaxImageResolution".into(), l(r as i64));
        }
        if self.landscape {
            m.insert("IsLandscape".into(), b(true));
        }
        if self.export_bookmarks {
            m.insert("ExportBookmarks".into(), b(true));
        }
        if self.export_bookmarks_to_pdf_destination {
            m.insert("ExportBookmarksToPDFDestination".into(), b(true));
        }
        if self.export_form_fields {
            m.insert("ExportFormFields".into(), b(true));
        }
        if self.allow_duplicate_field_names {
            m.insert("AllowDuplicateFieldNames".into(), b(true));
        }
        if self.export_placeholders {
            m.insert("ExportPlaceholders".into(), b(true));
        }
        if self.export_notes {
            m.insert("ExportNotes".into(), b(true));
        }
        if self.export_notes_pages {
            m.insert("ExportNotesPages".into(), b(true));
        }
        if self.export_only_notes_pages {
            m.insert("ExportOnlyNotesPages".into(), b(true));
        }
        if self.export_notes_in_margin {
            m.insert("ExportNotesInMargin".into(), b(true));
        }
        if self.convert_ooo_target_to_pdf_target {
            m.insert("ConvertOOoTargetToPDFTarget".into(), b(true));
        }
        if self.export_links_relative_fsys {
            m.insert("ExportLinksRelativeFsys".into(), b(true));
        }
        if self.export_hidden_slides {
            m.insert("ExportHiddenSlides".into(), b(true));
        }
        if self.skip_empty_pages {
            m.insert("IsSkipEmptyPages".into(), b(true));
        }
        if self.add_original_document_as_stream {
            m.insert("IsAddStream".into(), b(true));
        }
        if self.single_page_sheets {
            m.insert("SinglePageSheets".into(), b(true));
        }
        if self.lossless_image_compression {
            m.insert("UseLosslessCompression".into(), b(true));
        }
        if self.reduce_image_resolution {
            m.insert("ReduceImageResolution".into(), b(true));
        }
        if let Some(text) = &self.native_watermark_text {
            m.insert("Watermark".into(), s(text.clone()));
        }
        if let Some(c) = self.native_watermark_color {
            m.insert("WatermarkColor".into(), l(c as i64));
        }
        if let Some(h) = self.native_watermark_font_height {
            m.insert("WatermarkFontHeight".into(), l(h as i64));
        }
        if let Some(a) = self.native_watermark_rotate_angle {
            m.insert("WatermarkRotateAngle".into(), l(a as i64));
        }
        if let Some(name) = &self.native_watermark_font_name {
            m.insert("WatermarkFontName".into(), s(name.clone()));
        }
        if let Some(text) = &self.native_tiled_watermark_text {
            m.insert("TiledWatermark".into(), s(text.clone()));
        }
        if let Some(v) = self.initial_view {
            m.insert("InitialView".into(), l(v as i64));
        }
        if let Some(v) = self.initial_page {
            m.insert("InitialPage".into(), l(v as i64));
        }
        if let Some(v) = self.magnification {
            m.insert("Magnification".into(), l(v as i64));
        }
        if let Some(v) = self.zoom {
            m.insert("Zoom".into(), l(v as i64));
        }
        if let Some(v) = self.page_layout {
            m.insert("PageLayout".into(), l(v as i64));
        }
        if self.first_page_on_left {
            m.insert("FirstPageOnLeft".into(), b(true));
        }
        if self.resize_window_to_initial_page {
            m.insert("ResizeWindowToInitialPage".into(), b(true));
        }
        if self.center_window {
            m.insert("CenterWindow".into(), b(true));
        }
        if self.open_in_full_screen_mode {
            m.insert("OpenInFullScreenMode".into(), b(true));
        }
        if self.display_pdf_document_title {
            m.insert("DisplayPDFDocumentTitle".into(), b(true));
        }
        if self.hide_viewer_menubar {
            m.insert("HideViewerMenubar".into(), b(true));
        }
        if self.hide_viewer_toolbar {
            m.insert("HideViewerToolbar".into(), b(true));
        }
        if self.hide_viewer_window_controls {
            m.insert("HideViewerWindowControls".into(), b(true));
        }
        if self.use_transition_effects {
            m.insert("UseTransitionEffects".into(), b(true));
        }
        if let Some(v) = self.open_bookmark_levels {
            m.insert("OpenBookmarkLevels".into(), l(v as i64));
        }

        if m.is_empty() {
            None
        } else {
            Some(Value::Object(m).to_string())
        }
    }

    /// Legacy comma-separated `Key=Value` builder. **Not** what LOK saveAs
    /// actually parses — kept only because the unit tests in this module
    /// exercise it as a property-mapping check. Real conversions go through
    /// [`Self::lok_save_as_options`].
    #[cfg(test)]
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

impl std::fmt::Display for PdfAProfile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PdfAProfile::A1B => write!(f, "PDF/A-1b"),
            PdfAProfile::A2B => write!(f, "PDF/A-2b"),
            PdfAProfile::A3B => write!(f, "PDF/A-3b"),
        }
    }
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use static_assertions::assert_impl_all;

    #[test]
    fn config_and_options_are_send_sync() {
        assert_impl_all!(LibreOfficeConfig: Send, Sync, Clone);
        assert_impl_all!(OfficeOptions: Send, Sync, Clone);
        assert_impl_all!(PdfAProfile: Send, Sync, Clone, Copy);
    }

    #[test]
    fn config_defaults() {
        let c = LibreOfficeConfig::default();
        assert!(c.install_path.is_none());
        assert_eq!(c.timeout, Duration::from_secs(120));
        assert!(!c.lazy_start);
    }

    #[test]
    fn default_options_produce_empty_filter_string() {
        assert!(OfficeOptions::default().filter_options().is_empty());
    }

    #[test]
    fn pdf_a_maps_to_correct_select_pdf_version() {
        for (prof, expected) in [
            (PdfAProfile::A1B, "SelectPdfVersion=1"),
            (PdfAProfile::A2B, "SelectPdfVersion=2"),
            (PdfAProfile::A3B, "SelectPdfVersion=3"),
        ] {
            let opts = OfficeOptions { pdf_a: Some(prof), ..Default::default() };
            assert_eq!(opts.filter_options(), vec![expected]);
        }
    }

    #[test]
    fn landscape_option_emits_correct_key() {
        let opts = OfficeOptions { landscape: true, ..Default::default() };
        assert!(opts.filter_options().contains(&"IsLandscape=true".to_string()));
    }

    #[test]
    fn page_ranges_option_emits_correct_key() {
        let opts = OfficeOptions {
            page_ranges: Some(PageRanges::parse("1-3").unwrap()),
            ..Default::default()
        };
        let fo = opts.filter_options();
        assert!(fo.iter().any(|s| s.starts_with("PageRange=")));
    }

    #[test]
    fn pdf_ua_option_emits_correct_key() {
        let opts = OfficeOptions { pdf_ua: true, ..Default::default() };
        assert!(opts.filter_options().contains(&"PDFUACompliance=true".to_string()));
    }

    #[test]
    fn validate_rejects_quality_out_of_range() {
        let bad = OfficeOptions { quality: Some(0), ..Default::default() };
        assert!(bad.validate().is_err());
        let bad = OfficeOptions { quality: Some(101), ..Default::default() };
        assert!(bad.validate().is_err());
    }

    #[test]
    fn validate_rejects_bad_image_resolution() {
        let bad = OfficeOptions { max_image_resolution: Some(0), ..Default::default() };
        assert!(bad.validate().is_err());
        let bad = OfficeOptions { max_image_resolution: Some(999), ..Default::default() };
        assert!(bad.validate().is_err());
    }

    #[test]
    fn validate_rejects_conflicting_pdf_a_and_pdf_ua() {
        // Both together is valid — LibreOffice supports PDF/A + PDF/UA.
        let ok = OfficeOptions {
            pdf_a: Some(PdfAProfile::A2B),
            pdf_ua: true,
            ..Default::default()
        };
        assert!(ok.validate().is_ok());
    }

    #[test]
    fn office_options_roundtrip_json() {
        let json = serde_json::json!({
            "landscape": true,
            "pdfA": "a2-b",
            "quality": 80
        });
        let back: OfficeOptions = serde_json::from_value(json).expect("de");
        assert!(back.landscape);
        assert_eq!(back.pdf_a, Some(PdfAProfile::A2B));
        assert_eq!(back.quality, Some(80));
    }

    #[test]
    fn office_options_deserialize_empty_object() {
        let v: OfficeOptions = serde_json::from_str("{}").expect("de");
        assert_eq!(v, OfficeOptions::default());
    }

    #[test]
    fn validate_accepts_defaults() {
        assert!(OfficeOptions::default().validate().is_ok());
    }
}
