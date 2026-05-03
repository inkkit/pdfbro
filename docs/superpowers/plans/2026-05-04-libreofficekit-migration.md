# LibreOfficeKit Migration Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Take `feat/libreofficekit` from "tests pass" to production-ready: real lazy-start / idle-shutdown semantics, graceful shutdown bypassing LO ≥ 6.5's atexit teardown bug, structured error mapping for encrypted / corrupted / unsupported documents, breaking flag rename (`--soffice` → `--lo-program-dir`), one bench validation run, and surrounding doc cleanup.

**Architecture:** The `LibreOfficeEngine` keeps its single dedicated `lok-worker` thread + mpsc/oneshot plumbing (LOK's `GLOBAL_OFFICE_LOCK` forces this). We restructure `Inner` so `tx` and `worker_handle` are `Mutex<Option<…>>`, enabling both lazy first-spawn and a clean `shutdown()` that joins the worker and `mem::forget`s the `Office` instance. Idle-shutdown runs in a sibling watcher thread that calls `libc::_exit(0)`; the orchestrator (Cloud Run / Fly / k8s) is expected to restart on the next request. Errors are classified via LO error-string match plus a content-sniffer ported from `office-convert-server`.

**Tech Stack:** Rust (tokio, libc, libreofficekit 0.5, serde_json), no Python, LibreOffice ≥ 26.x from Debian bookworm-backports.

**Spec:** `docs/superpowers/specs/2026-05-04-libreofficekit-migration-design.md`.

---

## File Map

| Action | File | Responsibility |
|---|---|---|
| Modify | `crates/engine/src/types.rs` | Add 3 new `EngineError` variants |
| Create | `crates/engine/src/libreoffice/error.rs` | Classifier + content sniffer |
| Modify | `crates/engine/src/libreoffice/mod.rs` | `Inner` shape, lazy-start, idle-shutdown, shutdown, classifier wiring |
| Create | `crates/engine/tests/fixtures/office/encrypted.docx` | OOXML encrypted fixture (≤ 8 KB) |
| Create | `crates/engine/tests/fixtures/office/truncated.docx` | Real .docx truncated to 200 bytes |
| Modify | `crates/engine/tests/libreoffice.rs` | New integration tests |
| Modify | `crates/server/src/config.rs` | Flag rename + env rename |
| Modify | `crates/server/src/main.rs` | `lo.shutdown()` wiring, config field rename |
| Modify | `crates/server/src/error.rs` | Map 3 new variants to HTTP responses |
| Create | `crates/server/tests/bdd/features/libreoffice_errors.feature` | Encrypted/corrupted/unsupported scenarios |
| Create | `crates/server/tests/bdd/testdata/encrypted.docx` | BDD fixture |
| Create | `crates/server/tests/bdd/testdata/truncated.docx` | BDD fixture |
| Create | `crates/server/tests/bdd/testdata/unknown.xyz` | BDD fixture |
| Modify | `Dockerfile` | Swap `LIBREOFFICE_PATH` → `LO_PROGRAM_PATH` (already uses LOK_PROGRAM_PATH; add LO_PROGRAM_PATH) |
| Modify | `scripts/test-images.sh` | Drop `(unoserver)` text |
| Modify | `bench/README.md` | Drop unoserver from RSS list |
| Modify | `README.md` | Move LOK feature to present tense; swap `LIBREOFFICE_PATH` references |
| Modify | `docs/implementation-status.md` | Flip LO row to "in-process via LOK" |
| Create | `bench/results/2026-05-04-lok-validation.md` | Bench artefact |

---

## Task 1: Add `EngineError` variants for LibreOffice document failures

**Files:**
- Modify: `crates/engine/src/types.rs`
- Modify: `crates/server/src/error.rs` (only the `engine_status_and_code` exhaustive match — add stub arms returning INTERNAL until Task 10 fills them in properly)

- [ ] **Step 1: Open `crates/engine/src/types.rs`. After the `LibreOfficeTimeout` variant (line 87), insert three new variants:**

```rust
    /// LibreOffice could not load the document because it is encrypted or
    /// password-protected. Surfaced for OOXML files containing an
    /// `EncryptedPackage` stream, encrypted ODF, and PDFs with `/Encrypt`.
    #[error("LibreOffice document is encrypted or password-protected")]
    LibreOfficeEncrypted,

    /// LibreOffice could not load the document because the file is
    /// corrupted or unreadable. Includes truncated uploads, zero-byte
    /// files, and ZIPs/CDFs that fail structural parsing.
    #[error("LibreOffice document is corrupted or unreadable: {0}")]
    LibreOfficeCorrupted(String),

    /// LibreOffice does not recognise the file format. Returned when
    /// content-type detection fails on a file whose bytes match no
    /// supported import filter.
    #[error("LibreOffice does not recognise this file format")]
    LibreOfficeUnsupportedFormat,
```

- [ ] **Step 2: Update `crates/server/src/error.rs::engine_status_and_code` so the match is still exhaustive — add three stub arms returning the existing INTERNAL mapping (real mapping added in Task 10):**

Locate the function (around line 636). Add these arms before the closing brace:

```rust
        EngineError::LibreOfficeEncrypted
        | EngineError::LibreOfficeCorrupted(_)
        | EngineError::LibreOfficeUnsupportedFormat => {
            (StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL")
        }
```

Also locate the `to_response` match (around line 145) — if it has an exhaustive `match self { … }` over `EngineError`, add a stub arm so it compiles:

```rust
            // Refined in Task 10.
            ApiError::Engine(EngineError::LibreOfficeEncrypted)
            | ApiError::Engine(EngineError::LibreOfficeCorrupted(_))
            | ApiError::Engine(EngineError::LibreOfficeUnsupportedFormat) => ApiErrorResponse {
                error: "LibreOffice document error".to_string(),
                code: code.to_string(),
                details: None,
                suggestion: None,
                documentation: None,
            },
```

- [ ] **Step 3: Verify the build still passes**

Run: `cargo check -p engine -p server`
Expected: `Finished` with zero errors. (Warnings about unused new variants are fine — they're consumed in Task 3.)

- [ ] **Step 4: Commit**

```bash
git add crates/engine/src/types.rs crates/server/src/error.rs
git commit -m "feat(engine): add LibreOffice encrypted/corrupted/unsupported error variants"
```

---

## Task 2: Create the classifier + content sniffer

**Files:**
- Create: `crates/engine/src/libreoffice/error.rs`
- Modify: `crates/engine/src/libreoffice/mod.rs` (add `mod error;`)

- [ ] **Step 1: Create the new file `crates/engine/src/libreoffice/error.rs` with the full skeleton:**

```rust
//! Classifier for LibreOfficeKit document-load failures.
//!
//! LOK reports load failures through opaque `OfficeError::OfficeError(msg)`
//! values whose payloads originate in `framework/source/loadenv/loadenv.cxx`.
//! We map the well-known message strings to actionable engine errors and
//! fall back to a content-sniffer for the ambiguous "Unsupported URL" wrapper
//! that LO uses for both encrypted files and outright corruption.
//!
//! The sniffer mirrors the production logic in
//! `office-convert-server/src/encrypted.rs` (same author as the
//! `libreofficekit` crate); we are not inventing a new heuristic.

use crate::types::EngineError;

/// Map a LOK load-time error message + the input file's first bytes to
/// the most informative `EngineError` variant we can derive.
pub(super) fn classify_load_error(msg: &str, file_prefix: &[u8]) -> EngineError {
    if msg.contains("Unsupported URL") {
        return match sniff_file_condition(file_prefix) {
            FileCondition::Encrypted => EngineError::LibreOfficeEncrypted,
            FileCondition::Corrupted => EngineError::LibreOfficeCorrupted(msg.to_string()),
            FileCondition::Unknown => EngineError::LibreOfficeUnsupportedFormat,
        };
    }
    if msg.contains("loadComponentFromURL returned an empty reference") {
        return EngineError::LibreOfficeCorrupted(msg.to_string());
    }
    if msg.contains("type detection failed") {
        return EngineError::LibreOfficeUnsupportedFormat;
    }
    EngineError::Internal(format!("LOK document_load: {msg}"))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum FileCondition {
    Encrypted,
    Corrupted,
    Unknown,
}

/// Inspect the first ~8 KB of a file and return our best guess at why
/// LO might have refused to open it. This is intentionally heuristic —
/// it never aborts a conversion on its own, only refines the error.
pub(super) fn sniff_file_condition(bytes: &[u8]) -> FileCondition {
    if bytes.len() < 4 {
        return FileCondition::Corrupted;
    }

    // PDF
    if bytes.starts_with(b"%PDF-") {
        // PDF has /Encrypt anywhere in the document, but encrypted PDFs
        // typically declare it inside the trailer near the start.
        // Searching the prefix is correct for files we've truncated to
        // ~8 KB; for full files we'd want the trailer.
        if bytes.windows(8).any(|w| w == b"/Encrypt") {
            return FileCondition::Encrypted;
        }
        return FileCondition::Unknown;
    }

    // ZIP (OOXML / ODF / EPUB ... )
    if bytes.starts_with(b"PK\x03\x04") {
        return classify_zip(bytes);
    }

    // OLE compound document (legacy .doc / .xls / .ppt — also encrypted OOXML
    // wrapper in some toolchains)
    if bytes.starts_with(b"\xD0\xCF\x11\xE0\xA1\xB1\x1A\xE1") {
        // We don't walk the FAT here — too much code for a fixed-size prefix.
        // Default to Unknown; LO's own error string usually pins this down.
        return FileCondition::Unknown;
    }

    FileCondition::Unknown
}

/// ZIP-aware sub-classifier. Walks the local-file-header sequence in the
/// prefix and looks for OOXML / ODF marker entries.
fn classify_zip(bytes: &[u8]) -> FileCondition {
    let mut saw_content_types = false;
    let mut saw_mimetype = false;
    let mut saw_encrypted_package = false;

    let mut i = 0usize;
    while i + 30 <= bytes.len() {
        // Local file header signature
        if &bytes[i..i + 4] != b"PK\x03\x04" {
            break;
        }
        // Filename length at offset 26..28 (little-endian u16)
        let name_len = u16::from_le_bytes([bytes[i + 26], bytes[i + 27]]) as usize;
        let extra_len = u16::from_le_bytes([bytes[i + 28], bytes[i + 29]]) as usize;
        let comp_size = u32::from_le_bytes([
            bytes[i + 18], bytes[i + 19], bytes[i + 20], bytes[i + 21],
        ]) as usize;

        let name_start = i + 30;
        let name_end = name_start.saturating_add(name_len);
        if name_end > bytes.len() {
            // Truncated header — treat as corrupted.
            return FileCondition::Corrupted;
        }

        let name = &bytes[name_start..name_end];
        if name == b"[Content_Types].xml" {
            saw_content_types = true;
        }
        if name == b"mimetype" {
            saw_mimetype = true;
        }
        if name == b"EncryptedPackage" {
            saw_encrypted_package = true;
        }

        // Advance past name + extra + compressed payload to the next header.
        i = name_end + extra_len + comp_size;
    }

    if saw_encrypted_package {
        return FileCondition::Encrypted;
    }
    if saw_content_types || saw_mimetype {
        return FileCondition::Unknown; // looks like a normal Office ZIP
    }
    // Valid ZIP magic but no recognisable Office markers in the prefix.
    FileCondition::Corrupted
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unsupported_url_with_encrypted_zip_yields_encrypted() {
        let bytes = build_zip_with_entry(b"EncryptedPackage", &[0u8; 16]);
        let err = classify_load_error("Unsupported URL <file:///x>: \"type detection failed\"", &bytes);
        assert!(matches!(err, EngineError::LibreOfficeEncrypted), "got {err:?}");
    }

    #[test]
    fn loadcomponent_failure_yields_corrupted() {
        let err = classify_load_error(
            "loadComponentFromURL returned an empty reference",
            &[],
        );
        assert!(matches!(err, EngineError::LibreOfficeCorrupted(_)), "got {err:?}");
    }

    #[test]
    fn type_detection_failed_alone_yields_unsupported() {
        let err = classify_load_error("type detection failed", &[]);
        assert!(matches!(err, EngineError::LibreOfficeUnsupportedFormat), "got {err:?}");
    }

    #[test]
    fn unknown_message_falls_through_to_internal() {
        let err = classify_load_error("something completely different", &[]);
        assert!(matches!(err, EngineError::Internal(_)), "got {err:?}");
    }

    #[test]
    fn pdf_with_encrypt_is_encrypted() {
        let mut bytes = b"%PDF-1.4\n%blah\n".to_vec();
        bytes.extend_from_slice(b"1 0 obj\n<< /Encrypt 2 0 R >>\nendobj\n");
        assert_eq!(sniff_file_condition(&bytes), FileCondition::Encrypted);
    }

    #[test]
    fn ooxml_zip_without_content_types_is_corrupted() {
        let bytes = build_zip_with_entry(b"someotherfile.xml", b"hello");
        assert_eq!(sniff_file_condition(&bytes), FileCondition::Corrupted);
    }

    #[test]
    fn ooxml_zip_with_content_types_is_unknown() {
        let bytes = build_zip_with_entry(b"[Content_Types].xml", b"<xml/>");
        assert_eq!(sniff_file_condition(&bytes), FileCondition::Unknown);
    }

    #[test]
    fn random_bytes_are_unknown() {
        let bytes = b"this is not any known magic".to_vec();
        assert_eq!(sniff_file_condition(&bytes), FileCondition::Unknown);
    }

    #[test]
    fn empty_bytes_are_corrupted() {
        assert_eq!(sniff_file_condition(&[]), FileCondition::Corrupted);
    }

    /// Build a minimal ZIP local-file-header followed by a payload, in the
    /// same shape `classify_zip` walks. No central directory needed because
    /// the classifier only walks LFHs.
    fn build_zip_with_entry(name: &[u8], payload: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(b"PK\x03\x04"); // signature
        out.extend_from_slice(&[20, 0]); // version
        out.extend_from_slice(&[0, 0]); // gp flag
        out.extend_from_slice(&[0, 0]); // compression (stored)
        out.extend_from_slice(&[0, 0, 0, 0]); // mod time/date
        out.extend_from_slice(&[0, 0, 0, 0]); // crc32
        let comp = (payload.len() as u32).to_le_bytes();
        out.extend_from_slice(&comp); // compressed size
        out.extend_from_slice(&comp); // uncompressed size
        let nlen = (name.len() as u16).to_le_bytes();
        out.extend_from_slice(&nlen); // filename length
        out.extend_from_slice(&[0, 0]); // extra length
        out.extend_from_slice(name);
        out.extend_from_slice(payload);
        out
    }
}
```

- [ ] **Step 2: Add the module to `crates/engine/src/libreoffice/mod.rs`**

Locate the existing `pub mod filter;` line (around line 10) and add a sibling:

```rust
pub mod filter;
mod error;
```

- [ ] **Step 3: Run the unit tests (no LOK runtime needed)**

Run: `cargo test -p engine --lib libreoffice::error::tests`
Expected: all 9 tests pass.

- [ ] **Step 4: Commit**

```bash
git add crates/engine/src/libreoffice/error.rs crates/engine/src/libreoffice/mod.rs
git commit -m "feat(engine): add LOK error classifier + content sniffer"
```

---

## Task 3: Wire the classifier into `lok_convert`

**Files:**
- Modify: `crates/engine/src/libreoffice/mod.rs`

- [ ] **Step 1: Add an integration-test-style assertion (will pass once the wiring is done) — but first write the test so it fails:**

Open `crates/engine/tests/libreoffice.rs`. Append at the end of the file:

```rust
#[tokio::test]
async fn convert_corrupted_returns_corrupted_error() {
    let Some(lo) = engine().await else { return; };
    let tmp = tempfile::tempdir().expect("tempdir");
    let bad = tmp.path().join("truncated.docx");
    // First 200 bytes of a real OOXML — valid PK header but no central dir.
    let head = b"PK\x03\x04\x14\x00\x06\x00\x08\x00\x00\x00!\x00";
    let mut bytes = head.to_vec();
    bytes.resize(200, 0u8);
    std::fs::write(&bad, &bytes).expect("write");

    let err = lo
        .convert(&bad, &OfficeOptions::default())
        .await
        .expect_err("expected error on truncated docx");

    assert!(
        matches!(
            err,
            EngineError::LibreOfficeCorrupted(_) | EngineError::LibreOfficeUnsupportedFormat
        ),
        "got {err:?}"
    );
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p engine --test libreoffice convert_corrupted_returns_corrupted_error -- --test-threads=1`
Expected: FAIL — the current code returns `EngineError::Internal(...)` because nothing classifies the LOK error.

(If the test is skipped because `engine()` returns `None`, the LOK runtime isn't available locally — run the test inside Docker via `Dockerfile.test`, or trust the integration check that runs in CI.)

- [ ] **Step 3: Wire the classifier into `lok_convert`**

Open `crates/engine/src/libreoffice/mod.rs`. Find the `let mut doc = match csv_opts { … }.map_err(|e| EngineError::Internal(format!("LOK document_load: {e}")))?;` block in `lok_convert` (around line 333) and replace it with classifier-aware error handling:

```rust
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
            // prefix fails we just classify against an empty buffer.
            let prefix = read_prefix(input, 8 * 1024).unwrap_or_default();
            return Err(error::classify_load_error(&e.to_string(), &prefix));
        }
    };
```

Then add the helper at the bottom of `mod.rs` (or under the `lok_convert` function — anywhere that's still in scope):

```rust
fn read_prefix(path: &Path, max_bytes: usize) -> std::io::Result<Vec<u8>> {
    use std::io::Read;
    let mut f = std::fs::File::open(path)?;
    let mut buf = Vec::with_capacity(max_bytes);
    f.take(max_bytes as u64).read_to_end(&mut buf)?;
    Ok(buf)
}
```

- [ ] **Step 4: Run the test again — verify it passes**

Run: `cargo test -p engine --test libreoffice convert_corrupted_returns_corrupted_error -- --test-threads=1`
Expected: PASS (or skipped locally; required to pass under Docker).

- [ ] **Step 5: Run the full LOK integration suite to confirm no regressions**

Run: `cargo test -p engine --test libreoffice -- --test-threads=1`
Expected: 11 tests pass (10 pre-existing + the new one).

- [ ] **Step 6: Commit**

```bash
git add crates/engine/src/libreoffice/mod.rs crates/engine/tests/libreoffice.rs
git commit -m "feat(engine): classify LOK document_load errors via sniffer"
```

---

## Task 4: Restructure `Inner` for shutdown + lazy-start

**Files:**
- Modify: `crates/engine/src/libreoffice/mod.rs`

This is a refactor with no behaviour change. Eager-start path still works exactly as before; we just make `tx` and `worker_handle` nullable and lockable so future tasks can mutate them.

- [ ] **Step 1: Add `parking_lot` to engine deps if not already present**

Run: `grep '^parking_lot' crates/engine/Cargo.toml`
If empty, open `crates/engine/Cargo.toml` and add to `[dependencies]`:

```toml
parking_lot = { workspace = true }
```

If the workspace doesn't define it, use `parking_lot = "0.12"` instead. (We use it because `std::sync::Mutex` would force `.lock().unwrap()` everywhere.)

- [ ] **Step 2: Rewrite the `Inner` struct + the `lok_worker_thread` signature**

In `crates/engine/src/libreoffice/mod.rs`, replace the existing `struct Inner { … }` (around line 48) with:

```rust
struct Inner {
    /// Work queue to the dedicated LOK thread. `None` until `init_worker()`
    /// runs (eager start in `launch()`, or first `convert()` in lazy mode).
    tx: parking_lot::Mutex<Option<mpsc::SyncSender<ConvertRequest>>>,
    /// Join handle for the worker thread. Taken by `shutdown()`.
    worker_handle: parking_lot::Mutex<Option<std::thread::JoinHandle<()>>>,
    /// Set to `true` after the worker has successfully initialised and is
    /// ready to accept requests. Flips to `false` on timeout / wedge / exit.
    healthy: AtomicBool,
    /// Per-conversion timeout.
    timeout: Duration,
    /// Path to the LOK program directory; cached so lazy init doesn't have
    /// to re-discover.
    install_path: PathBuf,
    /// `true` when the engine was constructed with `lazy_start = true`.
    lazy_start: bool,
    /// Single-flight guard for lazy init: prevents two concurrent
    /// first-`convert()` callers from spawning two worker threads.
    init_lock: tokio::sync::Mutex<()>,
}
```

- [ ] **Step 3: Replace `launch()` with eager + lazy paths sharing an `init_worker()` helper**

Replace the body of `LibreOfficeEngine::launch` (around lines 96–142) with:

```rust
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
                healthy: AtomicBool::new(false),
                timeout: config.timeout,
                install_path,
                lazy_start: config.lazy_start,
                init_lock: tokio::sync::Mutex::new(()),
            }),
        };

        if !config.lazy_start {
            engine.init_worker().await?;
        }

        // Idle-shutdown watcher (Task 8 implements the body).
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
        let healthy_arc = Arc::new(AtomicBool::new(false));
        let healthy_worker = Arc::clone(&healthy_arc);

        let handle = std::thread::Builder::new()
            .name("lok-worker".into())
            .spawn(move || {
                lok_worker_thread(install_path, rx, startup_tx, healthy_worker);
            })
            .map_err(|e| EngineError::Internal(format!("failed to spawn LOK thread: {e}")))?;

        // Wait up to 120 s for LOK init.
        tokio::time::timeout(Duration::from_secs(120), startup_rx)
            .await
            .map_err(|_| EngineError::Timeout(Duration::from_secs(120)))?
            .map_err(|_| EngineError::Internal("LOK worker exited during startup".into()))??;

        // Worker is up; commit the tx + handle to Inner and mark healthy.
        *self.inner.tx.lock() = Some(tx);
        *self.inner.worker_handle.lock() = Some(handle);
        self.inner.healthy.store(true, Ordering::SeqCst);

        info!("LibreOffice engine ready");
        Ok(())
    }

    /// Stub — body filled in by Task 8.
    fn spawn_idle_watcher(&self, _timeout: Duration) {
        // Intentionally empty until Task 8.
    }
```

- [ ] **Step 4: Update `convert()` to lazy-init + use the `Mutex<Option<…>>` `tx`**

Replace the `convert()` method (around lines 148–205) with:

```rust
    pub async fn convert(&self, input: &Path, opts: &OfficeOptions) -> EngineResult<Vec<u8>> {
        opts.validate()?;

        if !input.exists() {
            return Err(EngineError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("input file not found: {}", input.display()),
            )));
        }

        // Lazy init on first call.
        if self.inner.lazy_start && self.inner.tx.lock().is_none() {
            self.init_worker().await?;
        }

        if !self.inner.healthy.load(Ordering::SeqCst) {
            return Err(EngineError::Internal(
                "LOK engine is unhealthy (worker exited or wedged) — restart required".into(),
            ));
        }

        debug!("starting LOK conversion");

        let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();

        // Snapshot tx under the lock; release before awaiting.
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
                self.inner.healthy.store(false, Ordering::SeqCst);
                EngineError::Timeout(self.inner.timeout)
            })?
            .map_err(|_| {
                self.inner.healthy.store(false, Ordering::SeqCst);
                EngineError::Internal("LOK worker dropped reply channel".into())
            })?
    }
```

- [ ] **Step 5: Update `lok_worker_thread` to `mem::forget(office)` on exit**

Replace the body (around lines 233–265) with:

```rust
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
        let _ = req.reply.send(result);
    }

    healthy.store(false, Ordering::SeqCst);
    info!("LOK worker exiting; leaking Office to bypass LO ≥ 6.5 atexit teardown bug");

    // Skip Office::Drop -> lok_destroy entirely. LO ≥ 6.5 segfaults during
    // teardown; the process is already exiting (or about to be restarted by
    // shutdown()) so the kernel reclaims memory either way.
    std::mem::forget(office);
}
```

- [ ] **Step 6: Update the `healthy_worker` Arc and `Inner.healthy` reconciliation**

The `healthy` field in `Inner` is now an `AtomicBool` set inside `init_worker` after successful startup, and the worker thread maintains its own `Arc<AtomicBool>` for the wedge/exit signal. Bridge them: in `init_worker`, replace the line `self.inner.healthy.store(true, Ordering::SeqCst);` with logic that mirrors changes from the worker.

The simplest correct approach is to share the same `AtomicBool` between `Inner` and the worker. Change `Inner::healthy` from `AtomicBool` to `Arc<AtomicBool>`:

```rust
struct Inner {
    tx: parking_lot::Mutex<Option<mpsc::SyncSender<ConvertRequest>>>,
    worker_handle: parking_lot::Mutex<Option<std::thread::JoinHandle<()>>>,
    healthy: Arc<AtomicBool>,
    timeout: Duration,
    install_path: PathBuf,
    lazy_start: bool,
    init_lock: tokio::sync::Mutex<()>,
}
```

Update construction in `launch`: `healthy: Arc::new(AtomicBool::new(false))`.
In `init_worker`, pass `Arc::clone(&self.inner.healthy)` to the worker thread instead of allocating a new Arc, and remove the post-startup `self.inner.healthy.store(true, …)` because the worker sets it from inside `Office::new` Ok-arm.

Also update every `self.inner.healthy.load/store/...` call site in `convert()` to dereference correctly (no change needed — `Arc<AtomicBool>` derefs the same way).

- [ ] **Step 7: Compile**

Run: `cargo check -p engine`
Expected: zero errors. Fix any borrow/move issues exposed by the refactor.

- [ ] **Step 8: Run full LOK integration suite — must still pass with eager-start path**

Run: `cargo test -p engine --test libreoffice -- --test-threads=1` (under Docker if no local LO).
Expected: all 11 tests pass.

- [ ] **Step 9: Commit**

```bash
git add crates/engine/Cargo.toml crates/engine/src/libreoffice/mod.rs
git commit -m "refactor(engine): make LOK Inner mutable for shutdown + lazy-start"
```

---

## Task 5: Implement `LibreOfficeEngine::shutdown()` + Drop guard

**Files:**
- Modify: `crates/engine/src/libreoffice/mod.rs`
- Modify: `crates/engine/tests/libreoffice.rs`

- [ ] **Step 1: Write the failing integration test for graceful shutdown**

Append to `crates/engine/tests/libreoffice.rs`:

```rust
#[tokio::test]
async fn shutdown_drains_inflight_then_returns_ok() {
    let Some(lo) = engine().await else { return; };

    // Kick off a real conversion in the background.
    let lo_for_convert = lo.clone();
    let convert_task = tokio::spawn(async move {
        lo_for_convert
            .convert(&writer_fixture(), &OfficeOptions::default())
            .await
    });

    // Give the worker ~50 ms to actually pick up the request.
    tokio::time::sleep(Duration::from_millis(50)).await;

    // Shut down. This MUST drain the in-flight conversion successfully.
    let started = Instant::now();
    lo.shutdown().await.expect("shutdown");
    let shutdown_took = started.elapsed();

    let convert_result = convert_task.await.expect("join");
    assert!(convert_result.is_ok(), "in-flight convert was lost: {convert_result:?}");
    assert!(shutdown_took < Duration::from_secs(10), "shutdown was slow: {shutdown_took:?}");
}

#[tokio::test]
async fn convert_after_shutdown_returns_error() {
    let Some(lo) = engine().await else { return; };
    // Note: this test mutates the SHARED engine, so it MUST run last.
    // Test ordering is alphabetical; "z_" prefix ensures this.
    // (See test naming below.)
    lo.shutdown().await.expect("shutdown");
    let err = lo
        .convert(&writer_fixture(), &OfficeOptions::default())
        .await
        .expect_err("expected failure post-shutdown");
    assert!(matches!(err, EngineError::Internal(_)), "got {err:?}");
}
```

**Important**: rename the second test to `z_convert_after_shutdown_returns_error` (alphabetically last) so it runs after every other test in the binary. Same applies to the first one — name it `z_shutdown_drains_inflight_then_returns_ok` and accept it can only run with the second still pending. Actually, refactor: combine into one test that drains, then asserts post-shutdown convert fails:

```rust
#[tokio::test]
async fn z_shutdown_drains_then_rejects_new_requests() {
    let Some(lo) = engine().await else { return; };

    let lo_for_convert = lo.clone();
    let convert_task = tokio::spawn(async move {
        lo_for_convert
            .convert(&writer_fixture(), &OfficeOptions::default())
            .await
    });
    tokio::time::sleep(Duration::from_millis(50)).await;

    lo.shutdown().await.expect("shutdown");

    let convert_result = convert_task.await.expect("join");
    assert!(convert_result.is_ok(), "in-flight convert was lost: {convert_result:?}");

    let err = lo
        .convert(&writer_fixture(), &OfficeOptions::default())
        .await
        .expect_err("expected failure post-shutdown");
    assert!(matches!(err, EngineError::Internal(_)), "got {err:?}");
}
```

- [ ] **Step 2: Verify the test fails to compile (no `shutdown()` method yet)**

Run: `cargo test -p engine --test libreoffice z_shutdown_drains_then_rejects_new_requests -- --test-threads=1 --no-run`
Expected: compile error: `no method named 'shutdown'`.

- [ ] **Step 3: Implement `shutdown()`**

In `crates/engine/src/libreoffice/mod.rs`, inside the `impl LibreOfficeEngine` block, add:

```rust
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
```

- [ ] **Step 4: Add the `Drop` belt-and-braces fallback**

Append to `crates/engine/src/libreoffice/mod.rs`:

```rust
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
```

- [ ] **Step 5: Run the new test — must pass**

Run: `cargo test -p engine --test libreoffice z_shutdown_drains_then_rejects_new_requests -- --test-threads=1`
Expected: PASS.

- [ ] **Step 6: Run full LOK integration suite — make sure nothing else breaks**

Run: `cargo test -p engine --test libreoffice -- --test-threads=1`
Expected: 12 tests pass.

- [ ] **Step 7: Commit**

```bash
git add crates/engine/src/libreoffice/mod.rs crates/engine/tests/libreoffice.rs
git commit -m "feat(engine): graceful LibreOfficeEngine::shutdown() with mem::forget(office)"
```

---

## Task 6: Wire `shutdown()` into the server's graceful-shutdown path

**Files:**
- Modify: `crates/server/src/main.rs`

- [ ] **Step 1: Locate the post-axum-serve cleanup block**

In `crates/server/src/main.rs` around lines 213–222, the existing code shuts down Chromium with a bounded budget. We add LibreOffice next to it.

- [ ] **Step 2: Add the `lo.shutdown()` call**

Find the block:

```rust
    tracing::info!("server stopped accepting connections; closing engines");

    // Best-effort engine shutdown with a bounded budget.
    #[cfg(feature = "chromium")]
    {
        let shutdown = tokio::time::timeout(shutdown::DEFAULT_DRAIN, chromium.shutdown());
        if let Err(_e) = shutdown.await {
            tracing::warn!("Chromium shutdown exceeded drain budget");
        }
    }
```

Replace with (adding the `#[cfg(feature = "libreoffice")]` block):

```rust
    tracing::info!("server stopped accepting connections; closing engines");

    // Best-effort engine shutdown with a bounded budget.
    #[cfg(feature = "chromium")]
    {
        let shutdown = tokio::time::timeout(shutdown::DEFAULT_DRAIN, chromium.shutdown());
        if let Err(_e) = shutdown.await {
            tracing::warn!("Chromium shutdown exceeded drain budget");
        }
    }

    #[cfg(feature = "libreoffice")]
    {
        let shutdown = tokio::time::timeout(shutdown::DEFAULT_DRAIN, libreoffice.shutdown());
        match shutdown.await {
            Ok(Ok(())) => tracing::info!("LibreOffice shut down cleanly"),
            Ok(Err(e)) => tracing::warn!("LibreOffice shutdown error: {e}"),
            Err(_) => tracing::warn!("LibreOffice shutdown exceeded drain budget"),
        }
    }
```

You may need to verify the local binding name — search for `let lo` / `let libreoffice` near where `lo_cfg` is used (line 60ff) and use whichever name binds the `LibreOfficeEngine`. Adjust accordingly.

- [ ] **Step 3: Build to verify**

Run: `cargo build -p server --features "chromium libreoffice"`
Expected: zero errors.

- [ ] **Step 4: Smoke test (optional, requires LO on the host)**

Run the server in one terminal: `cargo run -p server --features "chromium libreoffice" -- --no-sandbox`
In another: `kill -TERM $(pgrep pdfbro-server)`
Expected: log line "LibreOffice shut down cleanly" before the process exits, no segfault.

- [ ] **Step 5: Commit**

```bash
git add crates/server/src/main.rs
git commit -m "feat(server): call libreoffice.shutdown() on graceful shutdown"
```

---

## Task 7: Lazy-start integration test

The `lazy_start` plumbing is already done in Task 4. We just need a test that proves it.

**Files:**
- Modify: `crates/engine/tests/libreoffice.rs`

- [ ] **Step 1: Write the test**

Append to `crates/engine/tests/libreoffice.rs`:

```rust
#[tokio::test]
async fn lazy_start_defers_lok_init_until_first_convert() {
    // Don't reuse the SHARED engine here — we need a fresh lazy one.
    let lo = match LibreOfficeEngine::launch(LibreOfficeConfig {
        lazy_start: true,
        ..LibreOfficeConfig::default()
    })
    .await
    {
        Ok(e) => e,
        Err(_) => return, // LOK runtime not present — skip
    };

    // Immediately after launch, the worker is NOT initialised.
    assert!(!lo.healthy().await, "lazy launch should leave healthy=false");

    // First convert triggers init.
    let bytes = lo
        .convert(&writer_fixture(), &OfficeOptions::default())
        .await
        .expect("first convert");
    assert!(bytes.starts_with(b"%PDF-"));

    // After init, healthy is true.
    assert!(lo.healthy().await, "engine should be healthy after first convert");

    // Clean up (don't leak a worker thread per test binary run).
    lo.shutdown().await.expect("shutdown");
}
```

Note: this test must NOT come alphabetically after `z_shutdown_drains_then_rejects_new_requests` because that test mutates the shared engine. The natural order (`lazy_start_…`) lands before `z_…`, which is correct.

- [ ] **Step 2: Run the test**

Run: `cargo test -p engine --test libreoffice lazy_start_defers_lok_init_until_first_convert -- --test-threads=1`
Expected: PASS.

- [ ] **Step 3: Commit**

```bash
git add crates/engine/tests/libreoffice.rs
git commit -m "test(engine): assert lazy_start defers LOK init until first convert"
```

---

## Task 8: Implement idle-shutdown

**Files:**
- Modify: `crates/engine/src/libreoffice/mod.rs`

- [ ] **Step 1: Add `last_activity` to `Inner`**

In `crates/engine/src/libreoffice/mod.rs`, extend `Inner`:

```rust
struct Inner {
    tx: parking_lot::Mutex<Option<mpsc::SyncSender<ConvertRequest>>>,
    worker_handle: parking_lot::Mutex<Option<std::thread::JoinHandle<()>>>,
    healthy: Arc<AtomicBool>,
    timeout: Duration,
    install_path: PathBuf,
    lazy_start: bool,
    init_lock: tokio::sync::Mutex<()>,
    /// `Some(t)` after each successful conversion; `None` until the first
    /// completes. The idle-watcher uses this to decide when to exit.
    last_activity: parking_lot::Mutex<Option<std::time::Instant>>,
}
```

Add `last_activity: parking_lot::Mutex::new(None)` to the `Inner { … }` literal in `launch()`.

- [ ] **Step 2: Update the worker thread to stamp `last_activity`**

The worker doesn't have direct access to `Inner`. Instead, pass an `Arc<parking_lot::Mutex<Option<Instant>>>` into `lok_worker_thread`:

In `init_worker`, before `std::thread::Builder::new()`, add:

```rust
        let last_activity = Arc::new(parking_lot::Mutex::new(None::<std::time::Instant>));
        let last_activity_worker = Arc::clone(&last_activity);
```

Pass `last_activity_worker` to the worker:

```rust
        let handle = std::thread::Builder::new()
            .name("lok-worker".into())
            .spawn(move || {
                lok_worker_thread(install_path, rx, startup_tx, healthy_worker, last_activity_worker);
            })
```

After init succeeds, stash the shared Arc into `Inner.last_activity` so the watcher (and tests) can read it. Easiest: change `Inner.last_activity`'s type to `Arc<parking_lot::Mutex<Option<Instant>>>` and assign `*self.inner.last_activity.lock() = …` no — simpler still: store the Arc once.

Actually the cleanest shape: `last_activity: Arc<parking_lot::Mutex<Option<Instant>>>` directly on `Inner`, constructed once in `launch()`, cloned into the worker. Drop the inner `parking_lot::Mutex<Option<…>>` wrapper and just use the Arc-wrapped one. Use a helper:

```rust
struct Inner {
    // …existing fields…
    last_activity: Arc<parking_lot::Mutex<Option<std::time::Instant>>>,
}
```

In `launch`, construct `last_activity: Arc::new(parking_lot::Mutex::new(None))`. In `init_worker`, pass `Arc::clone(&self.inner.last_activity)` into the worker.

Update `lok_worker_thread` signature:

```rust
fn lok_worker_thread(
    install_path: PathBuf,
    rx: mpsc::Receiver<ConvertRequest>,
    startup_tx: tokio::sync::oneshot::Sender<EngineResult<()>>,
    healthy: Arc<AtomicBool>,
    last_activity: Arc<parking_lot::Mutex<Option<std::time::Instant>>>,
) {
    // … existing init …

    for req in rx {
        let result = lok_convert(&office, &req.input, &req.opts);
        // Stamp activity AFTER the conversion completes (success or failure)
        // so a wedged conversion doesn't keep the watcher from triggering.
        *last_activity.lock() = Some(std::time::Instant::now());
        let _ = req.reply.send(result);
    }

    // … existing exit + mem::forget …
}
```

- [ ] **Step 3: Add `should_exit_for_idle` as a pure helper for unit testing**

Append to `crates/engine/src/libreoffice/mod.rs`:

```rust
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
```

- [ ] **Step 4: Implement the watcher thread body**

Replace the empty `spawn_idle_watcher` stub with:

```rust
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
                    // Sleep in increments so we notice an unhealthy engine quickly.
                    std::thread::sleep(idle_timeout.min(Duration::from_secs(15)));
                    if !healthy.load(Ordering::SeqCst) {
                        // Worker died or shutdown was called; don't fire _exit
                        // unnecessarily — the process will be replaced anyway.
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
```

- [ ] **Step 5: Add `libc` to engine deps if not present**

Run: `grep '^libc' crates/engine/Cargo.toml`
If empty, add to `[dependencies]`:

```toml
libc = { workspace = true }
```

(Or `libc = "0.2"` if no workspace entry exists.)

- [ ] **Step 6: Run unit tests**

Run: `cargo test -p engine --lib libreoffice::idle_tests`
Expected: all 3 tests pass.

- [ ] **Step 7: Run the integration suite**

Run: `cargo test -p engine --test libreoffice -- --test-threads=1`
Expected: all tests pass (idle-watcher only fires with `idle_shutdown_timeout = Some(_)` which no integration test sets).

- [ ] **Step 8: Commit**

```bash
git add crates/engine/Cargo.toml crates/engine/src/libreoffice/mod.rs
git commit -m "feat(engine): implement LibreOffice idle-shutdown via _exit(0)"
```

---

## Task 9: Server flag rename — `--soffice` → `--lo-program-dir`, `LIBREOFFICE_PATH` → `LO_PROGRAM_PATH`

**Files:**
- Modify: `crates/server/src/config.rs`
- Modify: `crates/server/src/main.rs`
- Modify: `Dockerfile`
- Modify: `Dockerfile.test`
- Modify: `README.md`

- [ ] **Step 1: Rename the CLI arg**

In `crates/server/src/config.rs`, locate the `Args` struct (around line 89) and replace:

```rust
    /// Override the LibreOffice / `soffice` executable path.
    #[arg(long, value_name = "PATH")]
    pub soffice: Option<PathBuf>,
```

with:

```rust
    /// Override the LibreOffice program directory (the folder containing
    /// `libsofficeapp.so` / `liblibreofficekit.so`, e.g.
    /// `/usr/lib/libreoffice/program`).
    #[arg(long = "lo-program-dir", value_name = "DIR", env = "LO_PROGRAM_PATH")]
    pub lo_program_dir: Option<PathBuf>,
```

- [ ] **Step 2: Rename the `ServerConfig` field**

In the same file, locate (around line 268):

```rust
    /// Override path to `soffice`, if any.
    pub soffice_path: Option<PathBuf>,
```

Replace with:

```rust
    /// Override path to the LibreOffice program directory, if any.
    pub lo_program_dir: Option<PathBuf>,
```

- [ ] **Step 3: Update the construction logic**

Around line 441, replace:

```rust
        let soffice_path = args
            .soffice
            .clone()
            .or_else(|| env.get("LIBREOFFICE_PATH").map(PathBuf::from));
```

with:

```rust
        // Discovery order: --lo-program-dir, LO_PROGRAM_PATH (already wired
        // via clap's env attribute), LOK_PROGRAM_PATH (libreofficekit-rs
        // honours it directly so we accept it as an alias), else None and
        // let the engine auto-discover.
        let lo_program_dir = args.lo_program_dir.clone().or_else(|| {
            env.get("LOK_PROGRAM_PATH").map(PathBuf::from)
        });
```

Around line 635, change `soffice_path,` in the `ServerConfig { … }` literal to `lo_program_dir,`.

- [ ] **Step 4: Update tests in `config.rs`**

Around line 837, replace:

```rust
        assert!(cfg.soffice_path.is_none());
```

with:

```rust
        assert!(cfg.lo_program_dir.is_none());
```

Around line 854, replace:

```rust
                ("LIBREOFFICE_PATH", "/opt/soffice"),
```

with:

```rust
                ("LO_PROGRAM_PATH", "/opt/libreoffice/program"),
```

Around line 871, replace:

```rust
            cfg.soffice_path.as_deref().map(|p| p.to_str().unwrap()),
```

with:

```rust
            cfg.lo_program_dir.as_deref().map(|p| p.to_str().unwrap()),
```

Update the expected value in that assertion to `"/opt/libreoffice/program"`.

- [ ] **Step 5: Update `crates/server/src/main.rs::libreoffice_config_from`**

Around lines 253–264, replace:

```rust
#[cfg(feature = "libreoffice")]
fn libreoffice_config_from(config: &ServerConfig) -> LibreOfficeConfig {
    LibreOfficeConfig {
        install_path: config
            .soffice_path
            .as_deref()
            .and_then(|p| p.parent())
            .map(|p| p.to_path_buf()),
        timeout: config.request_timeout,
        lazy_start: config.libreoffice_lazy_start,
        idle_shutdown_timeout: config.libreoffice_idle_shutdown_timeout,
    }
}
```

with:

```rust
#[cfg(feature = "libreoffice")]
fn libreoffice_config_from(config: &ServerConfig) -> LibreOfficeConfig {
    LibreOfficeConfig {
        // The user-supplied path is now a *directory* (LOK's program path);
        // pass it through verbatim. None falls through to libreofficekit's
        // own discovery (LOK_PROGRAM_PATH + known system locations).
        install_path: config.lo_program_dir.clone(),
        timeout: config.request_timeout,
        lazy_start: config.libreoffice_lazy_start,
        idle_shutdown_timeout: config.libreoffice_idle_shutdown_timeout,
    }
}
```

- [ ] **Step 6: Update Dockerfiles**

In `Dockerfile`, search for `LIBREOFFICE_PATH` — there should be no occurrences. If there are, swap them for `LO_PROGRAM_PATH`. Confirm `LOK_PROGRAM_PATH=/usr/lib/libreoffice/program` is still set on the LO-bearing stages (it already is, lines 198 and 277).

In `Dockerfile.test`, same swap if any `LIBREOFFICE_PATH` mentions exist (none expected after the recent commits).

- [ ] **Step 7: Update README**

Run: `grep -n LIBREOFFICE_PATH README.md`
For every match, replace `LIBREOFFICE_PATH` → `LO_PROGRAM_PATH` and adjust the prose so the example value is a directory (e.g. `/usr/lib/libreoffice/program`) not a binary path.

- [ ] **Step 8: Build and test**

Run: `cargo build -p server --features "chromium libreoffice"`
Expected: zero errors.

Run: `cargo test -p server --lib config -- --test-threads=1`
Expected: all config tests pass.

- [ ] **Step 9: Commit**

```bash
git add crates/server/src/config.rs crates/server/src/main.rs Dockerfile Dockerfile.test README.md
git commit -m "feat(server)!: rename --soffice → --lo-program-dir, LIBREOFFICE_PATH → LO_PROGRAM_PATH

BREAKING: --soffice flag and LIBREOFFICE_PATH env var are removed.
Use --lo-program-dir <DIR> or LO_PROGRAM_PATH=<DIR> with the LOK
program directory (e.g. /usr/lib/libreoffice/program). LOK_PROGRAM_PATH
is still honoured as a fallback because the libreofficekit crate reads
it directly."
```

---

## Task 10: Server-side error mapping for new variants

**Files:**
- Modify: `crates/server/src/error.rs`

- [ ] **Step 1: Replace the stub HTTP-status mapping in `engine_status_and_code`**

Around line 651, find:

```rust
        EngineError::LibreOfficeEncrypted
        | EngineError::LibreOfficeCorrupted(_)
        | EngineError::LibreOfficeUnsupportedFormat => {
            (StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL")
        }
```

Replace with:

```rust
        EngineError::LibreOfficeEncrypted => {
            (StatusCode::UNPROCESSABLE_ENTITY, "DOCUMENT_ENCRYPTED")
        }
        EngineError::LibreOfficeCorrupted(_) => {
            (StatusCode::UNPROCESSABLE_ENTITY, "DOCUMENT_CORRUPTED")
        }
        EngineError::LibreOfficeUnsupportedFormat => {
            (StatusCode::UNSUPPORTED_MEDIA_TYPE, "UNSUPPORTED_FORMAT")
        }
```

- [ ] **Step 2: Replace the stub `to_response` arm with three real arms**

Around line 145–250 there's a giant exhaustive `match self { … }`. Find the temporary stub added in Task 1 and replace it with three arms:

```rust
            ApiError::Engine(EngineError::LibreOfficeEncrypted) => ApiErrorResponse {
                error: "Document is encrypted or password-protected".to_string(),
                code: code.to_string(),
                details: None,
                suggestion: Some(
                    "Decrypt the document and resubmit. pdfbro does not currently accept passwords."
                        .to_string(),
                ),
                documentation: Some(documentation_link("DOCUMENT_ENCRYPTED")),
            },

            ApiError::Engine(EngineError::LibreOfficeCorrupted(msg)) => ApiErrorResponse {
                error: format!("Document is corrupted or unreadable: {msg}"),
                code: code.to_string(),
                details: None,
                suggestion: Some(
                    "Open the file in Office/LibreOffice and resave it; common causes are \
                     truncated uploads or zero-byte files."
                        .to_string(),
                ),
                documentation: Some(documentation_link("DOCUMENT_CORRUPTED")),
            },

            ApiError::Engine(EngineError::LibreOfficeUnsupportedFormat) => ApiErrorResponse {
                error: "Unsupported document format".to_string(),
                code: code.to_string(),
                details: None,
                suggestion: Some(
                    "Verify the file extension matches its content. Supported: \
                     docx/doc/odt/rtf/txt/html, xlsx/xls/ods/csv, pptx/ppt/odp."
                        .to_string(),
                ),
                documentation: Some(documentation_link("UNSUPPORTED_FORMAT")),
            },
```

- [ ] **Step 3: Add documentation links**

Find the `fn documentation_link` (around line 658) and add three new arms inside its match:

```rust
        "DOCUMENT_ENCRYPTED" => "/troubleshooting#document-encrypted",
        "DOCUMENT_CORRUPTED" => "/troubleshooting#document-corrupted",
        "UNSUPPORTED_FORMAT" => "/troubleshooting#unsupported-format",
```

- [ ] **Step 4: Build to verify exhaustive match still works**

Run: `cargo build -p server --features "chromium libreoffice"`
Expected: zero errors.

- [ ] **Step 5: Commit**

```bash
git add crates/server/src/error.rs
git commit -m "feat(server): map LibreOffice encrypted/corrupted/unsupported errors to 422/415"
```

---

## Task 11: BDD scenarios for encrypted / corrupted / unsupported

**Files:**
- Create: `crates/server/tests/bdd/testdata/encrypted.docx`
- Create: `crates/server/tests/bdd/testdata/truncated.docx`
- Create: `crates/server/tests/bdd/testdata/unknown.xyz`
- Create: `crates/server/tests/bdd/features/libreoffice_errors.feature`

- [ ] **Step 1: Create `truncated.docx`** (real .docx truncated to its first 200 bytes — still has the ZIP header but no central directory)

```bash
head -c 200 crates/server/tests/bdd/testdata/Special_Chars_ß.docx \
  > crates/server/tests/bdd/testdata/truncated.docx
ls -la crates/server/tests/bdd/testdata/truncated.docx
```

Expected: file is exactly 200 bytes.

- [ ] **Step 2: Create `encrypted.docx`** (an OOXML file containing an `EncryptedPackage` stream)

The simplest reliable way is to encrypt an existing test fixture in LibreOffice or Word and copy it. If you don't have one to hand, generate one programmatically with Python (one-time only, not committed as a script):

```bash
# One-time manual step — run on a host with LibreOffice or use the recipe below.
# Create a minimal ZIP containing an EncryptedPackage entry. The classifier
# only sniffs for the entry name, so a stub ZIP is enough for our test.
python3 - <<'PY' > crates/server/tests/bdd/testdata/encrypted.docx
import zipfile, io, sys
buf = io.BytesIO()
with zipfile.ZipFile(buf, 'w') as z:
    z.writestr('EncryptedPackage', b'\x00' * 32)
sys.stdout.buffer.write(buf.getvalue())
PY
ls -la crates/server/tests/bdd/testdata/encrypted.docx
```

This is enough to drive the classifier (which sniffs for the marker entry name); LO itself will reject it with the same `Unsupported URL` error any genuinely-encrypted OOXML produces.

- [ ] **Step 3: Create `unknown.xyz`** (random bytes with no recognised magic)

```bash
printf 'this is a totally unknown format with no recognised magic bytes' \
  > crates/server/tests/bdd/testdata/unknown.xyz
```

- [ ] **Step 4: Write the feature file**

Create `crates/server/tests/bdd/features/libreoffice_errors.feature`:

```gherkin
Feature: LibreOffice document-load error mapping

  Scenario: Encrypted DOCX returns 422 with DOCUMENT_ENCRYPTED
    Given I have a default pdfbro container
    When I make a "POST" request to "/forms/libreoffice/convert" with:
      | files | testdata/encrypted.docx | file |
    Then the response status code should be 422
    And the response body should contain "DOCUMENT_ENCRYPTED"
    And the response body should contain "encrypted"

  Scenario: Corrupted DOCX returns 422 with DOCUMENT_CORRUPTED
    Given I have a default pdfbro container
    When I make a "POST" request to "/forms/libreoffice/convert" with:
      | files | testdata/truncated.docx | file |
    Then the response status code should be 422
    And the response body should contain "DOCUMENT_CORRUPTED"

  Scenario: Unknown format returns 415 with UNSUPPORTED_FORMAT
    Given I have a default pdfbro container
    When I make a "POST" request to "/forms/libreoffice/convert" with:
      | files | testdata/unknown.xyz | file |
    Then the response status code should be 415
    And the response body should contain "UNSUPPORTED_FORMAT"
```

(Adjust the route name `/forms/libreoffice/convert` if the BDD step or actual route uses a different path — `grep -rn '/forms/libreoffice' crates/server/tests/bdd/features/ | head -3` will tell you.)

- [ ] **Step 5: Run the BDD suite to verify the scenarios pass**

Run: `cargo test -p server --test bdd -- --test-threads=1` (under Docker via `Dockerfile.test`).
Expected: the three new scenarios pass; pre-existing scenarios (including the special-char and Swedish-filename ones) also pass.

- [ ] **Step 6: Commit**

```bash
git add crates/server/tests/bdd/testdata/encrypted.docx \
        crates/server/tests/bdd/testdata/truncated.docx \
        crates/server/tests/bdd/testdata/unknown.xyz \
        crates/server/tests/bdd/features/libreoffice_errors.feature
git commit -m "test(bdd): add encrypted/corrupted/unsupported LibreOffice scenarios"
```

---

## Task 12: Misc cleanup

**Files:**
- Modify: `scripts/test-images.sh`
- Modify: `bench/README.md`
- Modify: `crates/engine/src/libreoffice/mod.rs` (delete `#[cfg(test)] filter_options()`)
- Modify: `README.md`
- Modify: `docs/implementation-status.md`
- Read-through (no necessary edits): `crates/cli/src/banner.rs`, `crates/server/src/banner.rs`

- [ ] **Step 1: `scripts/test-images.sh`**

Edit line 145. Replace:

```bash
    echo -e "${DIM}    waiting for LibreOffice engine (unoserver)...${NC}"
```

with:

```bash
    echo -e "${DIM}    waiting for LibreOffice engine (LOK)...${NC}"
```

- [ ] **Step 2: `bench/README.md`**

Open and find the line containing "Chrome, LibreOffice, unoserver". Replace with "Chrome, LibreOffice".

- [ ] **Step 3: Delete the dead `#[cfg(test)] filter_options()` builder**

Open `crates/engine/src/libreoffice/mod.rs`. Find the function `pub(crate) fn filter_options(&self) -> Vec<String>` (it's currently gated with `#[cfg(test)]`). Delete the function entirely along with its preceding doc comment.

Then delete the now-orphaned unit tests that called it: in the `#[cfg(test)] mod tests` block at the bottom of the file, remove `default_options_produce_empty_filter_string`, `pdf_a_maps_to_correct_select_pdf_version`, `landscape_option_emits_correct_key`, `page_ranges_option_emits_correct_key`, and `pdf_ua_option_emits_correct_key`. These were placeholder coverage; the new `lok_save_as_options` JSON method has equivalent assertions in the same module.

(Verify by searching: `grep -n 'lok_save_as_options' crates/engine/src/libreoffice/mod.rs` — there should be tests in the same `mod tests` block exercising the JSON output.)

If `lok_save_as_options` doesn't yet have its own unit tests, add four:

```rust
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
```

- [ ] **Step 4: `README.md`**

Locate the future-work section that mentions "Native LibreOffice integration via LibreOfficeKit" (search: `grep -n 'LibreOfficeKit' README.md`). Move that bullet out of the future-work section into the present-tense feature section. Phrasing: "**LibreOffice integration via LibreOfficeKit (LOK)** — in-process Rust bindings, no Python daemon, lower memory footprint."

- [ ] **Step 5: `docs/implementation-status.md`**

Search for the LibreOffice row. Whatever it currently labels the status as, change to something like:

```
| LibreOffice | ✅ in-process via LOK (`libreofficekit` crate, single dedicated worker thread, JSON `FilterData` for export options) |
```

(Match the table's existing column shape.)

- [ ] **Step 6: Banner read-through**

Run: `grep -in 'unoserver\|python\|XML-RPC' crates/cli/src/banner.rs crates/server/src/banner.rs`
If anything matches, edit accordingly. (Expected: nothing to fix.)

- [ ] **Step 7: Verify production Dockerfile already sets `LANG=C.UTF-8`**

Run: `grep -n 'LANG=\|LC_ALL=' Dockerfile`
Expected: lines 82–83 set `LANG=C.UTF-8` and `LC_ALL=C.UTF-8` on the `common` stage. No edit needed.

- [ ] **Step 8: Build and run unit tests**

```
cargo build -p server --features "chromium libreoffice"
cargo test -p engine --lib
```

Expected: all green.

- [ ] **Step 9: Commit**

```bash
git add scripts/test-images.sh bench/README.md README.md docs/implementation-status.md \
        crates/engine/src/libreoffice/mod.rs
git commit -m "chore: remove unoserver references and dead filter_options builder"
```

---

## Task 13: Bench validation

**Files:**
- Create: `bench/results/2026-05-04-lok-validation.md`

- [ ] **Step 1: Build the two images**

```bash
docker build -t pdfbro:lok-validation -f Dockerfile --target pdfbro .
docker pull gotenberg/gotenberg:8
```

Capture the digests:

```bash
docker inspect --format '{{.Id}}' pdfbro:lok-validation
docker inspect --format '{{.Id}}' gotenberg/gotenberg:8
```

- [ ] **Step 2: Start both containers on the bench's expected ports**

```bash
docker run --rm -d --name bench-pdfbro    -p 3001:3000 pdfbro:lok-validation
docker run --rm -d --name bench-gotenberg -p 3002:3000 gotenberg/gotenberg:8
```

Wait ~30 s for both to become healthy:

```bash
until curl -fsS http://localhost:3001/health > /dev/null; do sleep 2; done
until curl -fsS http://localhost:3002/health > /dev/null; do sleep 2; done
echo "both up"
```

- [ ] **Step 3: Run the perf bench, isolated mode, with `pdfengines-merge` skipped**

```bash
cargo run -p bench --release -- perf \
    --concurrency 4 \
    --warmup-secs 30 \
    --duration-secs 60 \
    --repetitions 3 \
    --isolated \
    --skip pdfengines-merge \
    --output-dir bench/results/2026-05-04-lok-run
```

(Adjust `perf` subcommand name if `cargo run -p bench --release -- --help` shows a different verb.)

- [ ] **Step 4: Capture peak RSS for both containers during a re-run**

In a separate shell while the bench is mid-run:

```bash
docker stats --no-stream --format 'table {{.Name}}\t{{.MemUsage}}' bench-pdfbro bench-gotenberg \
  | tee bench/results/2026-05-04-lok-rss.txt
```

(Run this every ~30 s during the bench and keep the highest values.)

- [ ] **Step 5: Stop the containers**

```bash
docker stop bench-pdfbro bench-gotenberg
```

- [ ] **Step 6: Write `bench/results/2026-05-04-lok-validation.md`**

Hand-write a summary referencing the auto-generated report in `bench/results/2026-05-04-lok-run/`. Template:

```markdown
# LOK migration bench validation — 2026-05-04

Compares pdfbro (LOK in-process) against Gotenberg 8.

## Image digests

- pdfbro:lok-validation — `<sha256>`
- gotenberg/gotenberg:8 — `<sha256>`

## Setup

- `bench` crate, perf subcommand, `--isolated --concurrency 4 --warmup-secs 30 --duration-secs 60 --repetitions 3`.
- `--skip pdfengines-merge` (intermittent flake unrelated to LO).

## Latency results

| Workload | pdfbro p50 (ms) | Gotenberg p50 (ms) | Δ p50 | pdfbro p95 | Gotenberg p95 |
|---|---:|---:|---:|---:|---:|
| libreoffice-docx | … | … | … | … | … |
| libreoffice-xlsx | … | … | … | … | … |
| libreoffice-pptx | … | … | … | … | … |

(Numbers copied from `bench/results/2026-05-04-lok-run/perf-report.json`.)

## Memory (peak RSS, informational)

- bench-pdfbro: …
- bench-gotenberg: …

## Commentary

- Wins: …
- Regressions vs target (Gotenberg p50 + 50 ms): …
- Follow-ups: file issue if any workload misses the bound; do not block the migration.
```

Fill in the blanks from the bench output and `docker stats` capture.

- [ ] **Step 7: Commit**

```bash
git add bench/results/2026-05-04-lok-validation.md bench/results/2026-05-04-lok-run/
git commit -m "bench: validate LibreOfficeKit migration against Gotenberg 8 baseline"
```

---

## Task 14: Final integration smoke + push

- [ ] **Step 1: Full Docker test run**

```bash
docker buildx build -f Dockerfile.test --progress=plain .
```

Expected: the test stage's `bash -c '… cargo test … grep …'` pipeline reports `::all libtest sections reported ok`. Build exits 0.

- [ ] **Step 2: Final manual smoke**

In two separate shells:

```bash
# Terminal A: lazy + idle-shutdown
docker run --rm -e LIBREOFFICE_LAZY_START=true -e LIBREOFFICE_IDLE_SHUTDOWN_TIMEOUT=30s \
  -p 3000:3000 pdfbro:lok-validation
# Watch for: no "LibreOffice engine ready" log until first request.
# After 30 s of idle post-first-request, see the WARN line and the container exits.

# Terminal B: send one request, then leave it idle
curl -F "files=@crates/server/tests/bdd/testdata/Special_Chars_ß.docx" \
     http://localhost:3000/forms/libreoffice/convert -o /tmp/out.pdf
ls -la /tmp/out.pdf  # should be a valid PDF
# Wait 35 s, container should self-exit.
```

- [ ] **Step 3: Push the branch**

```bash
git push origin feat/libreofficekit
```

- [ ] **Step 4: Open / update the PR**

If the PR already exists, no action needed beyond pushing. If not:

```bash
gh pr create --title "feat: complete LibreOfficeKit migration" --body "$(cat <<'EOF'
## Summary
- Replaces unoserver-based LibreOffice integration with in-process LibreOfficeKit (LOK) bindings.
- Adds graceful shutdown via mem::forget(office) to bypass LO ≥ 6.5 atexit teardown bug.
- Implements lazy-start + idle-shutdown semantics (idle-shutdown via libc::_exit; orchestrator restarts).
- Adds three new EngineError variants for encrypted/corrupted/unsupported documents with content-sniff classifier.
- BREAKING: --soffice → --lo-program-dir, LIBREOFFICE_PATH → LO_PROGRAM_PATH.

## Bench
See `bench/results/2026-05-04-lok-validation.md`.

## Spec / plan
- Spec: `docs/superpowers/specs/2026-05-04-libreofficekit-migration-design.md`
- Plan: `docs/superpowers/plans/2026-05-04-libreofficekit-migration.md`

## Test plan
- [x] `cargo test -p engine --lib`
- [x] `cargo test -p engine --test libreoffice` (under Docker)
- [x] BDD scenarios pass under Docker (`Dockerfile.test` build green)
- [x] Bench run committed
- [x] Manual smoke for lazy-start / idle-shutdown / SIGTERM

🤖 Generated with [Claude Code](https://claude.com/claude-code)
EOF
)"
```

---

## Self-Review

**Spec coverage (each spec section → tasks that implement it):**

- Architecture (Inner shape, lazy-start, idle-shutdown, error mapping): Tasks 4, 7, 8, 1–3, 10.
- Lazy-start: Tasks 4 (refactor) + 7 (test).
- Idle-shutdown: Task 8.
- Graceful shutdown (`shutdown()` + Drop): Task 5; server wiring Task 6.
- Error taxonomy (3 variants + sniffer): Tasks 1, 2, 3, 10.
- CLI rename: Task 9.
- Bench validation: Task 13.
- Misc cleanup: Task 12.
- Test plan (unit + integration + BDD + manual): Tasks 2, 3, 5, 7, 8, 11, 14.

**Placeholder scan:** No "TBD"/"TODO"/"add appropriate handling" markers. Every code step contains code. Every command has expected output.

**Type consistency:**
- `Inner.healthy: Arc<AtomicBool>` — set in Task 4, cloned into worker in Task 4 + Task 8.
- `Inner.tx: parking_lot::Mutex<Option<SyncSender<ConvertRequest>>>` — used in Tasks 4, 5.
- `Inner.worker_handle: parking_lot::Mutex<Option<JoinHandle<()>>>` — used in Tasks 4, 5.
- `Inner.last_activity: Arc<parking_lot::Mutex<Option<Instant>>>` — used in Task 8.
- `classify_load_error(msg: &str, file_prefix: &[u8]) -> EngineError` — defined Task 2, called Task 3.
- `sniff_file_condition(bytes: &[u8]) -> FileCondition` — defined Task 2, called by `classify_load_error` only.
- `should_exit_for_idle(last: Option<Instant>, now: Instant, timeout: Duration) -> bool` — defined Task 8, called from `spawn_idle_watcher`.
- `LibreOfficeEngine::shutdown(&self) -> EngineResult<()>` — defined Task 5, called Task 6.
- `LibreOfficeEngine::init_worker(&self) -> EngineResult<()>` — defined Task 4, called from `launch` (eager) and `convert` (lazy).
- `LibreOfficeEngine::spawn_idle_watcher(&self, idle_timeout: Duration)` — stub in Task 4, body in Task 8.
- `ServerConfig::lo_program_dir: Option<PathBuf>` — defined in Task 9, consumed by `libreoffice_config_from` in Task 9.
- `EngineError::LibreOfficeEncrypted | LibreOfficeCorrupted(String) | LibreOfficeUnsupportedFormat` — defined Task 1, produced Task 3, consumed Task 10.

All references match.
