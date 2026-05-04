# LibreOfficeKit Migration — Design Spec

**Date:** 2026-05-04
**Branch:** `feat/libreofficekit`
**Supersedes:** `docs/superpowers/plans/2026-05-01-libreoffice-performance.md` (unoserver design)

## Goal

Replace the unoserver-based LibreOffice integration with in-process LibreOfficeKit (LOK) bindings via the `libreofficekit` crate, eliminating the Python runtime dependency, lowering the resident memory footprint, and giving us a single Rust toolchain end-to-end.

The motivation is **memory usage** (no second Python interpreter, no `unoconvert` per-request fork) and **modern maintenance** (one language, type-checked at the FFI boundary, no XML-RPC marshalling).

## Non-Goals

- Multi-process LOK pool. LOK enforces `GLOBAL_OFFICE_LOCK` (one Office instance per process); a pool would mean spawning sibling processes, which directly contradicts the memory motivation.
- Drop-in API parity with unoserver-era flags. Pre-launch software, no users — breaking renames are accepted where they buy clarity.
- A persistent CI bench gate. The bench run is a one-shot validation, committed as an artefact, not enforced on every PR.

## Status of Current Branch

Already done on `feat/libreofficekit`:

- `unoserver.rs` and the unoserver-era `convert.rs` deleted.
- `LibreOfficeEngine` rewritten around a single dedicated `lok-worker` thread, an `mpsc::SyncSender<ConvertRequest>` queue, and per-request `tokio::sync::oneshot` reply channels.
- CSV/TSV load options use `Batch=1` to suppress the Calc *Text Import* dialog (the wedge source on the original test run).
- PDF export options serialised as the JSON `FilterData` blob the LOK PDF filter expects (per `filter/source/pdf/pdffilter.cxx`).
- Existence-check hoisted out of the worker into `convert()`.
- On per-call timeout, `Inner::healthy` flips to `false` so subsequent calls fail fast instead of queueing onto a wedged worker.
- `Dockerfile` and `Dockerfile.test` install LO ≥ 26.x from `bookworm-backports`, set `LANG=C.UTF-8`/`LC_ALL=C.UTF-8`, and rely on `libsofficeapp.so` for LOK loading.
- All 10 LOK integration tests pass under Docker.

What this spec adds is the **finishing work** that turns the migration from "tests pass" into "production-ready and clean": real lazy-start / idle-shutdown semantics, graceful shutdown, structured error mapping, breaking flag rename, bench validation, and the surrounding doc cleanup.

## Architecture

### What stays

- The `LibreOfficeEngine` public API (`launch`, `discover`, `convert`, `convert_many`, `healthy`).
- The single `lok-worker` thread and its `mpsc + oneshot` plumbing.
- The 120 s per-call timeout.
- The CSV/TSV `Batch=1`-prefixed load options and the JSON `lok_save_as_options` builder.
- The unhealthy-on-timeout fast-fail.

### What changes

- `LibreOfficeConfig::idle_shutdown_timeout` and `LibreOfficeConfig::lazy_start` get **real implementations** (currently dead fields).
- New public `LibreOfficeEngine::shutdown(&self) -> EngineResult<()>` that drains in-flight work, joins the worker, and `mem::forget`s the `Office` instance to bypass LO's broken atexit teardown.
- New `EngineError` variants: `LibreOfficeEncrypted`, `LibreOfficeCorrupted`, `LibreOfficeUnsupportedFormat`. `lok_convert` classifies LOK error strings into them, falling back to a content-sniff for ambiguous cases.

### What is removed

- `--soffice <PATH>` flag.
- `LIBREOFFICE_PATH` env var.
- The `parent()`-strip workaround that made the binary-path config accidentally work as a directory.

Replaced by `--lo-program-dir <DIR>` and `LO_PROGRAM_PATH`. The `libreofficekit` crate's existing `LOK_PROGRAM_PATH` discovery is honoured as a fallback.

## Lazy-start

When `LibreOfficeConfig::lazy_start = true`:

- `LibreOfficeEngine::launch()` returns immediately with an `Inner` whose request queue and worker handle are `Mutex<Option<…>>` (None until lazy init runs); LOK is not yet initialised. (Same shape as the `tx`/`worker_handle` mutation needed for graceful shutdown — single field type, both states use it.)
- The first `convert()` call spawns the worker thread under a single-flight `Mutex` so concurrent first-callers don't double-spawn, then awaits LOK init via the existing startup-rendezvous oneshot channel.
- `healthy()` returns `false` until init completes.
- Implementation: extract today's body of `launch()` into an `init_worker()` helper called from both eager and lazy paths.

Eager (`lazy_start = false`) behaviour is unchanged: `launch()` still blocks until LOK is initialised, returning `EngineError::Timeout` on failure.

## Idle-shutdown

When `LibreOfficeConfig::idle_shutdown_timeout = Some(d)`:

- The worker thread maintains a `last_activity: Instant` updated after every conversion completes (success or failure).
- A second background thread (`lok-idle-watch`) sleeps `d` between checks. When `now - last_activity > d`, it logs `WARN: LibreOffice idle shutdown triggered after {d}; process exiting — orchestrator will restart on next request` and calls `libc::_exit(0)`.
- The watcher only arms after the **first successful conversion**, so an idle-on-startup container with `lazy_start = false` doesn't immediately exit.
- This is process-level exit, not engine-level. The orchestrator (Cloud Run / Fly machines / k8s with a restartPolicy) is expected to restart the container on the next request. This trade-off is documented in the README under "deployment".

If `idle_shutdown_timeout` is set without an orchestrator-style restart in front of the process, subsequent requests will fail (the container is gone). We log a startup `WARN` ("idle-shutdown configured without lazy-start; subsequent requests will fail until process is restarted") to surface the misconfiguration but proceed — it's the user's call.

## Graceful shutdown

`LibreOfficeEngine::shutdown(&self) -> EngineResult<()>`:

1. Set `Inner::healthy = false` so any in-flight `convert()` calls reaching the queue check abort early.
2. Take the `Inner::tx` (now `Mutex<Option<SyncSender<…>>>`); dropping it closes the channel.
3. Join the worker thread under a bounded 5 s timeout via `tokio::task::spawn_blocking`. The worker, on observing a closed channel, finishes its current conversion (if any), exits its `for req in rx` loop, and calls `std::mem::forget(office)` before returning.
4. If the join times out (a wedged conversion the per-call timer didn't catch), give up and return `Ok(())` anyway — the process is exiting, the kernel reclaims memory.

Wired into `crates/server/src/main.rs::shutdown_signal` after axum's `with_graceful_shutdown` future returns: drain in-flight HTTP first (axum does this), then call `lo.shutdown()`, then Chromium shutdown stays as-is.

`impl Drop for LibreOfficeEngine` provides a belt-and-braces fallback: if a runtime is current it `block_on`s `shutdown()`; otherwise it logs a `warn!` and `mem::forget`s the inner `Office` so a panicked drop still doesn't trigger LO's broken atexit.

Structural changes to support this:

- `Inner::tx`: `SyncSender<ConvertRequest>` → `Mutex<Option<SyncSender<ConvertRequest>>>`.
- `Inner::worker_handle`: new field, `Mutex<Option<JoinHandle<()>>>`.
- `lok_worker_thread`: ends with `std::mem::forget(office)` after the request-loop returns.

## Error taxonomy

### New `EngineError` variants

```rust
#[error("LibreOffice document is encrypted or password-protected")]
LibreOfficeEncrypted,

#[error("LibreOffice document is corrupted or unreadable: {0}")]
LibreOfficeCorrupted(String),

#[error("LibreOffice does not recognise this file format")]
LibreOfficeUnsupportedFormat,
```

### Classifier

`crates/engine/src/libreoffice/error.rs` (new module):

```rust
pub(super) fn classify_load_error(msg: &str, file_bytes: &[u8]) -> EngineError {
    if msg.contains("Unsupported URL") {
        return match sniff_file_condition(file_bytes) {
            FileCondition::Encrypted => EngineError::LibreOfficeEncrypted,
            FileCondition::Corrupted => EngineError::LibreOfficeCorrupted(msg.into()),
            FileCondition::Unknown   => EngineError::LibreOfficeUnsupportedFormat,
        };
    }
    if msg.contains("loadComponentFromURL returned an empty reference") {
        return EngineError::LibreOfficeCorrupted(msg.into());
    }
    if msg.contains("type detection failed") {
        return EngineError::LibreOfficeUnsupportedFormat;
    }
    EngineError::Internal(format!("LOK document_load: {msg}"))
}
```

### Content sniffer

`sniff_file_condition` reads the first 8 KB of the input and returns one of `Encrypted | Corrupted | Unknown`:

- **PDF** (`%PDF-` magic): if `/Encrypt` token appears in the prefix → `Encrypted`.
- **ZIP** (`PK\x03\x04` magic):
  - Walk the central directory. Any entry named `EncryptedPackage` (OOXML) → `Encrypted`.
  - For ODF (mimetype entry present): if `meta.xml` is missing AND a manifest entry references `urn:oasis:names:tc:opendocument:xmlns:manifest:1.0:encryption` → `Encrypted`.
  - Cleanly-parsed ZIP without `[Content_Types].xml` (OOXML) and without `mimetype` (ODF) → `Corrupted`.
- **OLE compound document** (`\xD0\xCF\x11\xE0`): walk the FAT, look for an `EncryptedPackage` stream. Best-effort; default to `Unknown` on parse failure.
- **Otherwise** → `Unknown`.

The 8 KB prefix is read from disk only on the error path, after `document_load` has already returned `Err` — no extra I/O on the happy path.

This logic mirrors the production `office-convert-server/src/encrypted.rs` written by the `libreofficekit` crate's author; we're not inventing a new sniffer.

### Server-side mapping

`crates/server/src/error.rs`:

| `EngineError` variant | HTTP | error code | suggestion |
|---|---|---|---|
| `LibreOfficeEncrypted` | 422 | `DOCUMENT_ENCRYPTED` | "Decrypt the document and resubmit. pdfbro does not currently accept passwords." |
| `LibreOfficeCorrupted(_)` | 422 | `DOCUMENT_CORRUPTED` | "Open the file in Office/LibreOffice and resave it; common causes are truncated uploads or zero-byte files." |
| `LibreOfficeUnsupportedFormat` | 415 | `UNSUPPORTED_FORMAT` | "Verify the file extension matches its content. Supported: docx/doc/odt/rtf/txt/html, xlsx/xls/ods/csv, pptx/ppt/odp." |

## Bench validation

A single bench run captured as an artefact, **not** a CI gate.

```
cargo run -p bench --release -- \
    --target pdfbro \
    --target gotenberg \
    --workloads libreoffice-docx,libreoffice-xlsx,libreoffice-pptx \
    --report bench/results/2026-05-04-lok-validation.md
```

(Exact CLI subject to whatever the bench crate already accepts; the implementation task verifies first.)

### Acceptance criteria (informational)

- Per `libreoffice-*` workload, `pdfbro` p50 ≤ Gotenberg p50 + 50 ms.
- `docker stats --no-stream` peak RSS for both containers captured during the run, recorded as a footnote. No specific MB target — just visible numbers.

If a workload misses the p50 bound, we file a follow-up issue and proceed; the migration is not gated on bench numbers.

### Artefact

`bench/results/2026-05-04-lok-validation.md` includes:

- Table of p50/p95/p99 per workload per target.
- Peak RSS per container.
- Image digests / build SHAs for reproducibility.
- One paragraph of commentary calling out wins, regressions, and follow-ups.

Committed in the same PR as the migration code so the perf claim ships alongside it.

## CLI & env breaking changes

| Removed | Replacement |
|---|---|
| `--soffice <PATH>` | `--lo-program-dir <DIR>` |
| `LIBREOFFICE_PATH` env | `LO_PROGRAM_PATH` env |

### Discovery order in `crates/server/src/main.rs::libreoffice_config_from`

1. `--lo-program-dir` CLI flag.
2. `LO_PROGRAM_PATH` env.
3. `LOK_PROGRAM_PATH` env (already honoured by `libreofficekit-rs::Office::find_install_path`; accepted as an alias).
4. Auto-discovery via `Office::find_install_path()`.

The previous `parent()`-strip is deleted; we pass the directory through verbatim.

`ServerConfig::soffice_path` field renamed to `lo_program_dir`. The `("LIBREOFFICE_PATH", "/opt/soffice")` test case in `config.rs` becomes `("LO_PROGRAM_PATH", "/opt/libreoffice/program")`.

`--libreoffice-lazy-start` and `--libreoffice-idle-shutdown-timeout` (and their env equivalents) keep their names; only their *implementation* changes.

## Misc cleanup

- `scripts/test-images.sh:145` — drop `(unoserver)` from the LibreOffice wait message.
- `bench/README.md:98` — remove unoserver from the listed RSS contributors.
- `crates/engine/src/libreoffice/mod.rs` — delete the `#[cfg(test)] filter_options()` legacy Vec\<String\> builder once the new `lok_save_as_options` JSON tests cover the same property mappings.
- `README.md` — move the "Native LibreOffice via LibreOfficeKit" claim from the future-work list into the present-tense feature list. Swap every `LIBREOFFICE_PATH` mention for `LO_PROGRAM_PATH`.
- `docs/implementation-status.md` — flip the LibreOffice row to "in-process via LOK".
- `crates/cli/src/banner.rs` and `crates/server/src/banner.rs` — read through to confirm no stale unoserver / Python references in the startup banner.
- Confirm production `Dockerfile` already sets `LANG=C.UTF-8`/`LC_ALL=C.UTF-8` (it does, lines 82–83); no change needed there, just verified by the implementation task.

## Test plan

### Unit (engine crate, no LOK runtime)

- `lok_save_as_options` — keep the existing JSON-property-shape tests; replace the legacy `filter_options()` Vec\<String\> tests with equivalent assertions on the JSON method.
- `classify_load_error` — table-driven: known LO error strings → correct variant; unknown string falls through to `Internal`.
- `sniff_file_condition` — minimal byte fixtures for encrypted PDF, encrypted OOXML, encrypted ODT, corrupted ZIP, random bytes. No real Office files committed.
- `LibreOfficeConfig::idle_shutdown_timeout` parsing accepts `"10m"`, `"0"` (disabled), rejects garbage.

### Integration (`crates/engine/tests/libreoffice.rs`, real LOK)

- All 10 existing tests stay green.
- `convert_encrypted_docx_returns_encrypted_error` — fixture: `tests/fixtures/office/encrypted.docx` (OOXML with `EncryptedPackage`, ≤ 8 KB). Asserts `EngineError::LibreOfficeEncrypted`.
- `convert_corrupted_docx_returns_corrupted_error` — fixture: `tests/fixtures/office/truncated.docx` (real .docx truncated to 200 bytes). Asserts `EngineError::LibreOfficeCorrupted`.
- `shutdown_drains_inflight_then_exits` — kicks off a `convert()` on the writer fixture, calls `engine.shutdown()` mid-conversion, asserts the conversion completes successfully and `shutdown` returns `Ok(())` within the bounded timeout.
- `convert_after_shutdown_returns_internal_error` — calls `shutdown()`, then `convert()`; expects fast-fail with the existing "engine is unhealthy" message.
- `lazy_start_defers_lok_init_until_first_convert` — `launch(LibreOfficeConfig{ lazy_start: true, .. })`; `healthy()` is `false` immediately; first `convert()` triggers init and succeeds; `healthy()` is `true` after.

Idle-shutdown is unit-tested in isolation, not integration-tested: we extract the "is it time to exit" decision into a pure function and feed it synthetic `Instant`s. Calling `_exit(0)` from an integration test would terminate the test binary.

### BDD (`crates/server/tests/bdd/`)

- The two existing failing scenarios (`Special_Chars_ß.docx`, `Longitudinell_jämförelse_…docx`) must pass after the locale fix and error mapping land. Verified in the same Docker run.
- New scenario: encrypted-docx upload → 422 with `DOCUMENT_ENCRYPTED`. Fixture in `crates/server/tests/bdd/testdata/`.
- New scenario: corrupted-docx upload → 422 with `DOCUMENT_CORRUPTED`.
- New scenario: unknown-format upload (e.g. `.xyz`) → 415 with `UNSUPPORTED_FORMAT`.

### Bench

One run, results committed to `bench/results/2026-05-04-lok-validation.md`.

### Manual smoke

- `LIBREOFFICE_LAZY_START=true` — engine doesn't init at startup (visible in startup logs); first request triggers init.
- `LIBREOFFICE_IDLE_SHUTDOWN_TIMEOUT=30s` — after 30 s of inactivity post-first-request, container exits with the documented WARN line.
- `docker stop pdfbro` (SIGTERM) — graceful shutdown completes within 5 s, no segfault in logs.
