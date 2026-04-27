# Spec 12 — `engine::libreoffice::LibreOfficeEngine`

> Office document → PDF via the `soffice --headless` subprocess.

## Goal

Convert files in any LibreOffice-supported format (Word, Excel, PowerPoint,
ODF, RTF, CSV, etc.) to PDF bytes by orchestrating short-lived `soffice`
subprocesses, with isolated user profiles for safe concurrency, so the
server's `/forms/libreoffice/convert` route mirrors Gotenberg.

## Scope

**In:**

- Discovery / configuration of the `soffice` binary.
- Single-file and multi-file conversion (with optional merge to one PDF).
- Per-call isolated `UserInstallation` profile.
- PDF/A-1b / A-2b / A-3b export via LibreOffice's filter options.
- Hard timeouts, structured error mapping.

**Out:**

- PDF post-processing (delegated to spec 13 for `merge`).
- Long-running `soffice` daemon mode — every call is a fresh subprocess.
  (A pool may come later as a follow-up if benchmarks justify it.)
- Per-format quirks beyond what LibreOffice's CLI flags expose
  (e.g., specific Excel range selection — out of MVP).

## Public API

Module path: `engine::libreoffice`, re-exported as
`engine::LibreOfficeEngine`.

```rust
use crate::types::{EngineError, EngineResult, PageRanges};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

/// Wrapper around the `soffice` binary. Cheap to clone (`Arc` inside).
#[derive(Clone)]
pub struct LibreOfficeEngine {
    inner: Arc<Inner>, // private: { exe, timeout, semaphore }
}

#[derive(Debug, Clone)]
pub struct LibreOfficeConfig {
    /// Path to `soffice` (or `libreoffice`). `None` = autodiscover.
    pub executable: Option<PathBuf>,
    /// Per-conversion timeout. Default 120s.
    pub timeout: Duration,
    /// Maximum concurrent subprocess invocations. Default `num_cpus::get()`.
    pub max_concurrency: usize,
}

impl Default for LibreOfficeConfig {
    /* see Behavior */
}

impl LibreOfficeEngine {
    /// Discover `soffice` on PATH and platform defaults.
    pub async fn discover() -> EngineResult<Self>;

    pub async fn launch(config: LibreOfficeConfig) -> EngineResult<Self>;

    /// Convert one input file to PDF bytes.
    pub async fn convert(
        &self,
        input: &Path,
        opts: &OfficeOptions,
    ) -> EngineResult<Vec<u8>>;

    /// Convert many inputs, optionally merging into a single PDF.
    /// Inputs are converted in parallel up to `max_concurrency`. Output
    /// order, when merging, follows input order.
    pub async fn convert_many(
        &self,
        inputs: &[PathBuf],
        opts: &OfficeOptions,
    ) -> EngineResult<Vec<Vec<u8>>>;

    /// Returns true iff `soffice --version` succeeds within `timeout`.
    pub async fn healthy(&self) -> bool;
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct OfficeOptions {
    pub landscape: bool,
    pub page_ranges: Option<PageRanges>,
    /// PDF/A profile, if any.
    pub pdf_a: Option<PdfAProfile>,
    /// PDF/UA accessibility tagging.
    pub pdf_ua: bool,
    /// Quality knob for embedded raster images. 1..=100. None = LO default.
    pub quality: Option<u8>,
    /// Reduce image resolution (DPI). None = LO default.
    pub max_image_resolution: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PdfAProfile { A1B, A2B, A3B }
```

## Behavior

### `LibreOfficeConfig::default()`

```rust
LibreOfficeConfig {
    executable: None,
    timeout: Duration::from_secs(120),
    max_concurrency: std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4),
}
```

### Executable discovery (`discover` / `launch` with `executable = None`)

Search order, first hit wins; record the full searched list for
diagnostics:

1. `$LIBREOFFICE_PATH` (env var).
2. `which soffice` then `which libreoffice`.
3. macOS: `/Applications/LibreOffice.app/Contents/MacOS/soffice`.
4. Linux: `/usr/bin/soffice`, `/usr/bin/libreoffice`,
   `/usr/lib/libreoffice/program/soffice`,
   `/snap/bin/libreoffice`, `/var/lib/flatpak/exports/bin/org.libreoffice.LibreOffice`.
5. Windows: `C:\Program Files\LibreOffice\program\soffice.exe`,
   `C:\Program Files (x86)\LibreOffice\program\soffice.exe`.

If none found → `EngineError::Internal("LibreOffice not found: searched [...]")`.
(Reuses `EngineError::Internal` since spec 10 owns the enum; the message
is the discriminator.)

After discovery, the engine probes with `soffice --headless --version`
under `config.timeout`. Probe failure → `EngineError::Internal("LibreOffice probe failed: ...")`.

### `convert(input, opts)`

1. `input.exists()`; else `EngineError::Io(io::ErrorKind::NotFound)`.
2. Acquire one permit from the engine's `Semaphore(max_concurrency)`.
3. Create `tmp = tempfile::tempdir()` (auto-cleanup via Drop).
4. Create `user_dir = tmp.path().join("uipfx")` (LibreOffice
   `UserInstallation`). Build `file://` URL.
5. Build `outdir = tmp.path().join("out")` and create it.
6. Build CLI args:

   ```
   --headless
   --norestore --nologo --nodefault --nofirststartwizard
   --convert-to <export-target>
   --outdir <outdir>
   "-env:UserInstallation=file:///<user_dir>"
   <input absolute path>
   ```

   `<export-target>` is built per [filter rules](#export-filter):

   - Default: `pdf:writer_pdf_Export` (or the appropriate exporter — see
     filter table) with options expressed as a JSON-ish blob:
     `pdf:writer_pdf_Export:{"PageRange":{"type":"string","value":"1-3,5"},...}`.

7. Spawn via `tokio::process::Command`, capture stdout/stderr,
   wait under `tokio::time::timeout(config.timeout, child.wait_with_output())`.
8. On exit code 0:
   - Locate the produced `<basename>.pdf` in `outdir`.
   - Read and return the bytes; `tmp` drops, cleaning everything.
9. Non-zero exit:
   - Try to extract the LibreOffice error message from stderr; map to
     `EngineError::Internal(format!("soffice exit {code}: {stderr}"))`.
10. Timeout: kill child, return `EngineError::Timeout(config.timeout)`.

### `convert_many(inputs, opts)`

1. Empty input slice → `Ok(vec![])`.
2. For each input, spawn a `tokio::task` calling `self.convert(input, opts)`.
3. `tokio::task::JoinSet::join_all` with the same global semaphore
   gating concurrency.
4. Return `Vec<Vec<u8>>` in input order.

`merge = true` is **not** part of `OfficeOptions`. Server / CLI layers
that want a single merged PDF must call `convert_many` and then
`engine::pdfops::merge` (spec 13). This keeps responsibilities clean and
avoids a circular dep between the libreoffice and pdfops modules.

### Export filter

| Input extension(s)                | Exporter (CLI suffix)            |
|-----------------------------------|----------------------------------|
| .doc .docx .odt .rtf .txt .html   | `pdf:writer_pdf_Export`          |
| .xls .xlsx .ods .csv              | `pdf:calc_pdf_Export`            |
| .ppt .pptx .odp                   | `pdf:impress_pdf_Export`         |
| .odg .vsd .vsdx                   | `pdf:draw_pdf_Export`            |
| (anything else)                   | `pdf` (let LO infer)             |

Detection is by lowercased extension only. The full table is kept inside
`engine::libreoffice::filter::for_extension(&str) -> &'static str`.

### Filter parameters → CLI options blob

For `pdf:writer_pdf_Export` (and equivalents), append a `:{...}` JSON-ish
blob containing only the fields set by `OfficeOptions`. The serializer
produces LibreOffice's expected `{"Key":{"type":"...","value":...}}`
shape. Mapping:

| `OfficeOptions` field                    | LO key                | LO type        |
|------------------------------------------|-----------------------|----------------|
| `page_ranges` (formatted as range string)| `PageRange`           | `string`       |
| `pdf_a = A1B` → `1`, `A2B` → `2`, `A3B`=`3` | `SelectPdfVersion` | `long`         |
| `pdf_ua = true`                          | `PDFUACompliance`     | `boolean`      |
| `quality`                                | `Quality`             | `long`         |
| `max_image_resolution`                   | `MaxImageResolution`  | `long`         |
| `landscape = true`                       | `IsLandscape`         | `boolean`      |

If no fields are set, the blob is omitted entirely (`pdf:writer_pdf_Export`
without the `:` suffix).

### Concurrency / safety

Concurrent `soffice` invocations are safe **only** if each uses a
distinct `UserInstallation` directory. The implementation guarantees
this by always allocating a fresh `tempdir` per call.

The `Semaphore` is a backstop against fork-bombing the host when many
calls land at once; it does not affect correctness.

### `healthy()`

Run `soffice --headless --version` with a small (5s) timeout regardless
of `config.timeout`. Returns `true` on exit code 0 with non-empty stdout.

## Errors

Reuses `EngineError` from spec 10. Operative variants:

| Variant                     | Source                                                                              |
|-----------------------------|-------------------------------------------------------------------------------------|
| `Io`                        | Input file missing, tempdir creation failed.                                        |
| `Timeout(timeout)`          | `soffice` exceeded `config.timeout`. Child is force-killed.                         |
| `Internal(msg)`             | Discovery / probe failed, soffice exited non-zero, or output PDF missing.           |
| `InvalidOption(msg)`        | `quality` outside 1..=100, `max_image_resolution` 0, or `page_ranges` empty string. |

## Edge cases

| Scenario                                              | Required behavior                                                       |
|-------------------------------------------------------|-------------------------------------------------------------------------|
| Input path with non-UTF-8 chars                       | Pass through as `OsStr` to `Command::arg`; do not re-encode.            |
| Input file is itself a `.pdf`                         | Allowed — LO will rewrite it. Useful for PDF/A retrofitting.            |
| Filename collides with an existing file in `outdir`   | Cannot happen: `outdir` is a fresh tempdir per call.                    |
| LibreOffice produces an empty PDF                     | Treated as success; bytes returned as-is. Validation is the caller's job. |
| `OfficeOptions::quality = 0`                          | `EngineError::InvalidOption("quality must be 1..=100")`.                |
| `pdf_a = A1B` + `landscape = true`                    | Allowed; LO honors both.                                                 |
| Concurrent calls on slow machines                     | `Semaphore` queues them; total wall time is bounded by oldest pending.   |
| Killed by SIGINT                                      | Tempdir Drop runs; child receives SIGKILL via `Command::kill_on_drop`.   |

## Test plan

### Unit tests (`crates/engine/src/libreoffice/mod.rs`)

No subprocess required.

- `discover_returns_searched_list_when_missing` — point env to a bogus
  path, assert `EngineError::Internal` message contains every searched path.
- `for_extension_maps_writer_calc_impress_draw`.
- `for_extension_is_case_insensitive`.
- `for_extension_unknown_returns_pdf_fallback`.
- `office_options_default_emits_no_filter_blob`.
- `office_options_with_page_ranges_emits_pagerange_key`.
- `office_options_with_pdf_a_maps_select_pdf_version_long`.
- `office_options_quality_zero_rejected`.
- `office_options_quality_above_100_rejected`.
- `office_options_max_image_resolution_zero_rejected`.

### Integration tests (`crates/engine/tests/libreoffice.rs`)

`#[ignore]`d; require `soffice` on PATH or `LIBREOFFICE_PATH`.

- `convert_docx_produces_valid_pdf` — fixture `tests/fixtures/office/sample.docx`,
  assert bytes start with `%PDF-` and `lopdf::Document::load_mem` succeeds.
- `convert_xlsx_landscape_orientation` — when `landscape = true`,
  rendered MediaBox is wider than tall.
- `convert_pptx_page_ranges` — `page_ranges = "1-1"` produces 1 page,
  full doc produces N pages.
- `convert_with_pdf_a_2b_writes_pdfa_metadata` — rendered file's metadata
  contains `pdfaid` namespace.
- `convert_many_preserves_order` — three inputs, timestamps ensure
  parallel execution, output order matches input order.
- `convert_timeout_kills_child` — set `timeout = 100ms`; convert a heavy
  fixture; assert `EngineError::Timeout` and verify no zombie soffice
  process left behind (best-effort assertion via `pgrep`).
- `convert_missing_input_io_error` — non-existent path → `EngineError::Io`.
- `convert_unsupported_format_falls_back_to_generic_filter` — give it a
  weird extension; assert success.
- `concurrent_calls_use_distinct_user_dirs` — instrument by setting
  `UserInstallation` to a captured path via a wrapper script; assert
  paths differ across two parallel invocations.

### Doc tests

Compile-only example mirroring the Server's expected usage:

```ignore
let lo = LibreOfficeEngine::discover().await?;
let pdf = lo.convert(Path::new("doc.docx"), &OfficeOptions::default()).await?;
```

## Acceptance

- [ ] `crates/engine/src/libreoffice/mod.rs` exists and is `pub mod libreoffice`
      from `lib.rs`.
- [ ] All public items in *Public API* compile and match signatures verbatim.
- [ ] `tempfile`, `tokio` (with `process` feature) added via
      `workspace.dependencies`.
- [ ] `OfficeOptions::validate()` exists with the constraints noted under
      *Errors*; called at the top of `convert` and `convert_many`.
- [ ] Filter table covered by exhaustive unit test
      `for_extension_covers_table`.
- [ ] All unit tests pass with `cargo test -p engine`.
- [ ] All `#[ignore]` integration tests pass locally with a system `soffice`.
- [ ] `cargo clippy -p engine -- -D warnings` clean.
- [ ] No global mutable state. No `unsafe`. No leaked tempdirs.
- [ ] `LibreOfficeEngine` is `Send + Sync + Clone` (asserted via
      `static_assertions`).

## Out of scope / follow-ups

- A long-running `soffice --headless --accept` daemon mode with UNO
  socket multiplexing — separate spec when warranted by benchmarks.
- Bulk format conversion routes (e.g. `.docx → .odt`); this engine is
  PDF-only.
- Encrypted document passwords (`--password`-style flags).
- Custom UNO macros executed pre/post export.
- Page count reporting without parsing the produced PDF.
