//! `LibreOfficeEngine` — convert office documents to PDF via LibreOfficeKit.
//!
//! LibreOfficeKit (LOK) is loaded in-process via `dlopen` — no Python daemon,
//! no unoserver, no XML-RPC.  Because LOK's global lock allows only one
//! `Office` instance per process and the type is `!Send`, all conversions run
//! on a **single dedicated `std::thread`**.  Async callers communicate with
//! that thread through a `std::sync::mpsc` work queue and per-request
//! `tokio::sync::oneshot` reply channels.

pub mod filter;
mod error;

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
    /// Work queue to the dedicated LOK thread. `None` until `init_worker()`
    /// runs (eager start in `launch()`, or first `convert()` in lazy mode).
    tx: parking_lot::Mutex<Option<mpsc::SyncSender<ConvertRequest>>>,
    /// Join handle for the worker thread. Taken by `shutdown()` (Task 5).
    worker_handle: parking_lot::Mutex<Option<std::thread::JoinHandle<()>>>,
    /// Set to `true` after the worker has successfully initialised and is
    /// ready to accept requests. Flips to `false` on timeout / wedge / exit.
    healthy: Arc<AtomicBool>,
    /// Per-conversion timeout.
    timeout: Duration,
    /// Cached LOK program directory; lazy init reuses it on first `convert()`.
    install_path: PathBuf,
    /// `true` when the engine was constructed with `lazy_start = true`.
    lazy_start: bool,
    /// Single-flight guard: prevents two concurrent first-`convert()` callers
    /// from spawning two worker threads.
    init_lock: tokio::sync::Mutex<()>,
    /// `Some(t)` after each successful conversion; `None` until the first
    /// completes. The idle-watcher uses this to decide when to exit.
    last_activity: Arc<parking_lot::Mutex<Option<std::time::Instant>>>,
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

    /// Launch the engine. With `lazy_start = false` (default) the LOK worker
    /// thread is spawned and LOK initialised before this returns. With
    /// `lazy_start = true` the worker is deferred until the first `convert()`.
    pub async fn launch(config: LibreOfficeConfig) -> EngineResult<Self> {
        use libreofficekit::Office;

        let install_path = config
            .install_path
            .clone()
            .or_else(Office::find_install_path)
            .ok_or_else(|| {
                EngineError::Internal(
                    "LibreOffice not found — set LO_PROGRAM_PATH or install LibreOffice".into(),
                )
            })?;

        info!(
            path = %install_path.display(),
            lazy = config.lazy_start,
            "Configuring LibreOffice engine via LOK"
        );

        let engine = Self {
            inner: Arc::new(Inner {
                tx: parking_lot::Mutex::new(None),
                worker_handle: parking_lot::Mutex::new(None),
                healthy: Arc::new(AtomicBool::new(false)),
                timeout: config.timeout,
                install_path,
                lazy_start: config.lazy_start,
                init_lock: tokio::sync::Mutex::new(()),
                last_activity: Arc::new(parking_lot::Mutex::new(None)),
            }),
        };

        if !config.lazy_start {
            engine.init_worker().await?;
        }

        // Idle-shutdown watcher (Task 8 fills in the body).
        if let Some(d) = config.idle_shutdown_timeout {
            engine.spawn_idle_watcher(d);
        }

        Ok(engine)
    }

    /// Single-flight worker spawn. Idempotent — concurrent callers serialise
    /// on `init_lock` and the second one observes `tx.is_some()` and
    /// short-circuits.
    async fn init_worker(&self) -> EngineResult<()> {
        let _guard = self.inner.init_lock.lock().await;
        if self.inner.tx.lock().is_some() {
            return Ok(()); // already initialised
        }

        let (tx, rx) = mpsc::sync_channel::<ConvertRequest>(64);
        let (startup_tx, startup_rx) = tokio::sync::oneshot::channel::<EngineResult<()>>();

        let install_path = self.inner.install_path.clone();
        let healthy_worker = Arc::clone(&self.inner.healthy);
        let last_activity_worker = Arc::clone(&self.inner.last_activity);

        let handle = std::thread::Builder::new()
            .name("lok-worker".into())
            .spawn(move || {
                lok_worker_thread(install_path, rx, startup_tx, healthy_worker, last_activity_worker);
            })
            .map_err(|e| EngineError::Internal(format!("failed to spawn LOK thread: {e}")))?;

        // Wait up to 120 s for LOK init.
        tokio::time::timeout(Duration::from_secs(120), startup_rx)
            .await
            .map_err(|_| EngineError::Timeout(Duration::from_secs(120)))?
            .map_err(|_| EngineError::Internal("LOK worker exited during startup".into()))??;

        // Worker is up; commit the tx + handle to Inner. Healthy is already
        // set to true inside the worker thread before sending the startup ack.
        *self.inner.tx.lock() = Some(tx);
        *self.inner.worker_handle.lock() = Some(handle);

        info!("LibreOffice engine ready");
        Ok(())
    }

    /// Spawn a sibling thread that polls `last_activity`. When the worker
    /// has been idle for at least `idle_timeout`, the watcher logs and
    /// calls `libc::_exit(0)` to terminate the process. The orchestrator
    /// (Cloud Run / Fly / k8s with restartPolicy) is expected to restart
    /// on the next request. This is process-level exit, not engine-level
    /// — there is no in-process recovery path because LOK enforces one
    /// `Office` instance per process for the lifetime of that process.
    fn spawn_idle_watcher(&self, idle_timeout: Duration) {
        let last_activity = Arc::clone(&self.inner.last_activity);
        let healthy = Arc::clone(&self.inner.healthy);
        let lazy = self.inner.lazy_start;

        std::thread::Builder::new()
            .name("lok-idle-watch".into())
            .spawn(move || {
                if !lazy {
                    info!(
                        ?idle_timeout,
                        "idle-shutdown configured without lazy-start; first request after idle-exit will pay full cold-start"
                    );
                }
                loop {
                    // Sleep in increments (cap at 15 s) so the watcher
                    // notices an unhealthy engine quickly even with long
                    // idle timeouts.
                    std::thread::sleep(idle_timeout.min(Duration::from_secs(15)));
                    if !healthy.load(Ordering::SeqCst) {
                        // Worker died or shutdown was called; don't fire
                        // _exit unnecessarily — the process will be
                        // replaced anyway.
                        continue;
                    }
                    let snapshot = *last_activity.lock();
                    let now = std::time::Instant::now();
                    if should_exit_for_idle(snapshot, now, idle_timeout) {
                        warn!(
                            ?idle_timeout,
                            "LibreOffice idle shutdown triggered; process exiting — \
                             orchestrator will restart on next request"
                        );
                        unsafe {
                            libc::_exit(0);
                        }
                    }
                }
            })
            .expect("spawn lok-idle-watch");
    }

    // ------------------------------------------------------------------
    // Public API
    // ------------------------------------------------------------------

    /// Convert a single document file to PDF bytes.
    #[instrument(skip(self, opts), fields(path = %input.display()))]
    pub async fn convert(&self, input: &Path, opts: &OfficeOptions) -> EngineResult<Vec<u8>> {
        opts.validate()?;

        // Existence check before queueing: LOK FFI calls cannot be
        // cancelled, so a missing-file request should not have to wait
        // its turn behind a slow (or wedged) conversion already in
        // flight on the worker thread.
        if !input.exists() {
            return Err(EngineError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("input file not found: {}", input.display()),
            )));
        }

        // Lazy init on first call. Concurrent first-callers serialise on
        // `init_lock` inside `init_worker`, so we never double-spawn.
        if self.inner.lazy_start && self.inner.tx.lock().is_none() {
            self.init_worker().await?;
        }

        // Fail fast if the worker is already known-wedged or exited — a
        // previous timed-out conversion may still be occupying the LOK
        // thread inside an uncancellable FFI call. Letting new requests
        // queue here would just multiply the wait.
        if !self.inner.healthy.load(Ordering::SeqCst) {
            return Err(EngineError::Internal(
                "LOK engine is unhealthy (worker exited or wedged) — restart required".into(),
            ));
        }

        debug!("starting LOK conversion");

        let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();

        // Hold the parking_lot guard only across the synchronous
        // `try_send`. Never across `.await` — parking_lot is not
        // async-aware, and shutdown() needs to be able to acquire this
        // same mutex to drop tx.
        let send_result = {
            let guard = self.inner.tx.lock();
            match guard.as_ref() {
                None => {
                    return Err(EngineError::Internal(
                        "LOK engine has been shut down".into(),
                    ));
                }
                Some(tx) => tx.try_send(ConvertRequest {
                    input: input.to_path_buf(),
                    opts: opts.clone(),
                    reply: reply_tx,
                }),
            }
        };

        send_result.map_err(|e| match e {
            mpsc::TrySendError::Full(_) => EngineError::Internal("LOK request queue full".into()),
            mpsc::TrySendError::Disconnected(_) => {
                self.inner.healthy.store(false, Ordering::SeqCst);
                EngineError::Internal("LOK worker thread has exited".into())
            }
        })?;

        tokio::time::timeout(self.inner.timeout, reply_rx)
            .await
            .map_err(|_| {
                // The async timeout fires, but the synchronous LOK call
                // inside the worker thread cannot be cancelled. Treat
                // the engine as wedged so subsequent requests don't
                // pile up behind it.
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

    /// Drain in-flight conversions, join the worker thread, and skip LOK's
    /// destroy() to bypass the LO ≥ 6.5 atexit teardown bug. Idempotent.
    ///
    /// Bounded to 5 s — if a conversion is wedged inside an uncancellable
    /// FFI call, we give up and return Ok anyway; the process is exiting.
    pub async fn shutdown(&self) -> EngineResult<()> {
        // Mark unhealthy first so concurrent convert() calls fail fast
        // instead of racing the channel close.
        self.inner.healthy.store(false, Ordering::SeqCst);

        // Take and drop the tx so the worker's `for req in rx` exits.
        let _dropped_tx = self.inner.tx.lock().take();

        // Take the join handle and wait for the worker to exit, capped.
        let handle = self.inner.worker_handle.lock().take();
        if let Some(handle) = handle {
            let join_result = tokio::time::timeout(
                Duration::from_secs(5),
                tokio::task::spawn_blocking(move || handle.join()),
            )
            .await;
            match join_result {
                Ok(Ok(Ok(()))) => {}
                Ok(Ok(Err(_))) => {
                    warn!("LOK worker thread panicked during shutdown");
                }
                Ok(Err(e)) => {
                    warn!("LOK worker join task failed: {e}");
                }
                Err(_) => {
                    warn!(
                        "LOK worker did not exit within 5s — likely wedged in FFI call; \
                         giving up and proceeding (process is shutting down anyway)"
                    );
                }
            }
        }
        Ok(())
    }
}

impl Drop for LibreOfficeEngine {
    fn drop(&mut self) {
        // Only the LAST clone needs to do anything; an Arc::strong_count of 2
        // means another Arc still holds the engine (this Drop is for a clone).
        if Arc::strong_count(&self.inner) > 1 {
            return;
        }

        // If we still have a worker handle, the user never called shutdown().
        // Take a best-effort path: drop the tx, but don't block on join —
        // there might not be a runtime, and we can't await here anyway.
        let had_tx = self.inner.tx.lock().take().is_some();
        let had_handle = self.inner.worker_handle.lock().take().is_some();
        if had_tx || had_handle {
            warn!(
                "LibreOfficeEngine dropped without explicit shutdown(); \
                 worker tx dropped, handle leaked. Call shutdown() in your \
                 graceful-shutdown path to clean up."
            );
        }
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
    last_activity: Arc<parking_lot::Mutex<Option<std::time::Instant>>>,
) {
    use libreofficekit::Office;

    let office = match Office::new(&install_path) {
        Ok(o) => {
            healthy.store(true, Ordering::SeqCst);
            let _ = startup_tx.send(Ok(()));
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
        // Stamp activity after the conversion completes (success OR failure)
        // so the watcher only fires when the worker truly idles, not during
        // a slow conversion.
        *last_activity.lock() = Some(std::time::Instant::now());
        let _ = req.reply.send(result);
    }

    healthy.store(false, Ordering::SeqCst);
    info!("LOK worker exiting; leaking Office to bypass LO ≥ 6.5 atexit teardown bug");

    // Skip Office::Drop -> lok_destroy entirely. LO ≥ 6.5 segfaults during
    // teardown; the process is already exiting (or about to be replaced
    // via shutdown()) so the kernel reclaims memory either way.
    std::mem::forget(office);
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

    let load_result = match csv_opts {
        Some(o) => office.document_load_with_options(&in_url, o),
        None => office.document_load(&in_url),
    };

    let mut doc = match load_result {
        Ok(doc) => doc,
        Err(e) => {
            // Read up to 8 KB of the input so the classifier can sniff
            // ZIP / PDF magic and disambiguate "Unsupported URL" between
            // encrypted and corrupted files. Best-effort — if reading the
            // prefix fails (file deleted between LOK's failed load and
            // our re-open, permission flap, etc.) we fall back to an
            // empty buffer and the classifier degrades to
            // `LibreOfficeUnsupportedFormat`. Log so the downgrade is
            // visible in operator dashboards.
            let prefix = read_prefix(input, 8 * 1024).unwrap_or_else(|err| {
                warn!(
                    path = %input.display(),
                    %err,
                    "failed to re-read input for error classification; classifying with empty prefix"
                );
                Vec::new()
            });
            return Err(error::classify_load_error(&e.to_string(), &prefix));
        }
    };

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

/// Read up to `max_bytes` from the beginning of a file.
fn read_prefix(path: &Path, max_bytes: usize) -> std::io::Result<Vec<u8>> {
    use std::io::Read;
    let f = std::fs::File::open(path)?;
    let mut buf = Vec::with_capacity(max_bytes);
    f.take(max_bytes as u64).read_to_end(&mut buf)?;
    Ok(buf)
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

/// Decide whether the idle-watcher should fire `_exit(0)`. Pure function
/// for testability — accepts synthetic `Instant`s.
fn should_exit_for_idle(
    last_activity: Option<std::time::Instant>,
    now: std::time::Instant,
    idle_timeout: Duration,
) -> bool {
    match last_activity {
        // Watcher only arms after the first successful conversion.
        None => false,
        Some(t) => now.saturating_duration_since(t) > idle_timeout,
    }
}

#[cfg(test)]
mod idle_tests {
    use super::*;
    use std::time::{Duration, Instant};

    #[test]
    fn idle_does_not_fire_before_first_activity() {
        let now = Instant::now();
        assert!(!should_exit_for_idle(None, now, Duration::from_secs(30)));
    }

    #[test]
    fn idle_does_not_fire_within_window() {
        let t = Instant::now();
        let now = t + Duration::from_secs(10);
        assert!(!should_exit_for_idle(Some(t), now, Duration::from_secs(30)));
    }

    #[test]
    fn idle_fires_past_window() {
        let t = Instant::now();
        let now = t + Duration::from_secs(31);
        assert!(should_exit_for_idle(Some(t), now, Duration::from_secs(30)));
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
    fn lok_save_as_options_default_returns_none() {
        assert_eq!(OfficeOptions::default().lok_save_as_options(), None);
    }

    #[test]
    fn lok_save_as_options_pdf_a_emits_select_pdf_version() {
        let opts = OfficeOptions { pdf_a: Some(PdfAProfile::A2B), ..Default::default() };
        let json = opts.lok_save_as_options().expect("Some");
        assert!(json.contains("\"SelectPdfVersion\""));
        assert!(json.contains("\"value\":\"2\""));
    }

    #[test]
    fn lok_save_as_options_page_ranges_emits_string_value() {
        let opts = OfficeOptions {
            page_ranges: Some(PageRanges::parse("1-3").unwrap()),
            ..Default::default()
        };
        let json = opts.lok_save_as_options().expect("Some");
        assert!(json.contains("\"PageRange\""));
        assert!(json.contains("\"type\":\"string\""));
        assert!(json.contains("\"value\":\"1-3\""));
    }

    #[test]
    fn lok_save_as_options_landscape_emits_boolean_value() {
        let opts = OfficeOptions { landscape: true, ..Default::default() };
        let json = opts.lok_save_as_options().expect("Some");
        assert!(json.contains("\"IsLandscape\""));
        assert!(json.contains("\"type\":\"boolean\""));
        assert!(json.contains("\"value\":\"true\""));
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
