# Spec 12 — `engine::libreoffice::LibreOfficeEngine`

> Office document → PDF via the `soffice --headless` subprocess.
> Outline only — to be expanded before implementation begins.

## Goal

Convert Word / Excel / PowerPoint / OpenDocument files (and the ~100
formats LibreOffice supports) to PDF via a sandboxed subprocess invocation,
so that the server's `/forms/libreoffice/convert` route can mirror
Gotenberg.

## Public API (sketch)

```rust
pub struct LibreOfficeEngine { /* soffice path, isolated user dir */ }

#[derive(Debug, Clone, Default)]
pub struct OfficeOptions {
    pub landscape: bool,
    pub page_ranges: Option<PageRanges>,
    pub merge: bool,            // multiple inputs → single PDF
    pub native_pdf_a: Option<PdfA>,
}

#[derive(Debug, Clone, Copy)]
pub enum PdfA { A1a, A2a, A3a, A3b }

impl LibreOfficeEngine {
    pub async fn discover() -> EngineResult<Self>;
    pub async fn with_executable(path: PathBuf) -> EngineResult<Self>;

    /// Convert one input file to PDF bytes.
    pub async fn convert(
        &self,
        input: &Path,
        opts: &OfficeOptions,
    ) -> EngineResult<Vec<u8>>;

    /// Convert many inputs and (optionally) merge into one PDF.
    pub async fn convert_many(
        &self,
        inputs: &[PathBuf],
        opts: &OfficeOptions,
    ) -> EngineResult<Vec<u8>>;
}
```

## Behavior (high level)

- Each `convert*` call creates a **per-call temp dir** (used as
  `-env:UserInstallation=file:///...`) so concurrent calls don't share
  LibreOffice state.
- Build args: `--headless --convert-to pdf:writer_pdf_Export
  --outdir <tmp> <input>` (params adjusted for PDF/A when requested).
- Spawn via `tokio::process::Command`; enforce `BrowserConfig::timeout`-style
  bound via `tokio::time::timeout`.
- Read produced PDF, return bytes, drop temp dir.
- `convert_many` with `merge = true` delegates to `engine::pdfops::merge`
  (spec 13) after individual conversions.

## To expand before implementation

- [ ] Format detection + allowed input extensions table.
- [ ] Native PDF/A export flags (`SelectPdfVersion`, etc.).
- [ ] Concurrency limits — single soffice process is not safe for parallel
      conversions; must serialise per `LibreOfficeEngine` instance with a
      `tokio::sync::Mutex`.
- [ ] Test plan with fixture documents under `crates/engine/tests/fixtures/office/`.
- [ ] Acceptance checklist mirroring spec 11's pattern.
