# Spec 13 — `engine::pdfops`

> Pure-Rust PDF post-processing via `lopdf`. Outline only.

## Goal

Provide merge, split, flatten, metadata, and watermark operations on PDF
byte streams without spawning any external tooling, so the server's
`/forms/pdfengines/*` routes can mirror Gotenberg without `qpdf` /
`pdfcpu` / `pdftk` dependencies.

## Public API (sketch)

```rust
/// All ops are stateless free functions in `engine::pdfops`.

pub fn merge(pdfs: &[&[u8]]) -> EngineResult<Vec<u8>>;

pub enum SplitMode {
    /// Split into one file per `PageRanges` chunk.
    ByRanges(Vec<PageRanges>),
    /// Split every N pages.
    EveryN(u32),
}
pub fn split(pdf: &[u8], mode: SplitMode) -> EngineResult<Vec<Vec<u8>>>;

pub fn flatten(pdf: &[u8]) -> EngineResult<Vec<u8>>;

pub fn read_metadata(pdf: &[u8]) -> EngineResult<Metadata>;
pub fn write_metadata(pdf: &[u8], meta: &Metadata) -> EngineResult<Vec<u8>>;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Metadata {
    pub title: Option<String>,
    pub author: Option<String>,
    pub subject: Option<String>,
    pub keywords: Option<String>,
    pub creator: Option<String>,
    pub producer: Option<String>,
    pub creation_date: Option<chrono::DateTime<chrono::Utc>>,
    pub mod_date: Option<chrono::DateTime<chrono::Utc>>,
    pub custom: BTreeMap<String, String>,
}

pub struct WatermarkOptions {
    pub text: Option<String>,
    pub image_png: Option<Vec<u8>>,
    pub opacity: f32,           // 0..=1
    pub rotation_deg: f32,
    pub position: Position,     // Center, TopLeft, ...
    pub tiled: bool,
}
pub fn watermark(pdf: &[u8], opts: &WatermarkOptions) -> EngineResult<Vec<u8>>;
```

## Behavior (high level)

- All ops accept and return owned `Vec<u8>` so callers can chain via the
  server pipeline without filesystem round-trips.
- `merge` validates each input parses; concatenates page trees; rebuilds
  cross-reference table.
- `split(ByRanges)` reuses `PageRanges::contains` from spec 10.
- `flatten` collapses AcroForm widgets to page content streams.
- `watermark` uses `lopdf` content stream operators; PNG watermarks
  embedded as `XObject`.

## To expand before implementation

- [ ] Map every operation to specific `lopdf` API calls.
- [ ] Define behavior for password-encrypted inputs (likely
      `EngineError::InvalidOption("encrypted PDF not supported in MVP")`).
- [ ] PDF/A conformance pass — out of MVP scope; flagged as follow-up.
- [ ] Encryption / decryption helpers — follow-up spec.
- [ ] Test plan: fixture PDFs under `crates/engine/tests/fixtures/pdf/`,
      property tests via `proptest` on merge order.
