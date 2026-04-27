# Spec 13 — `engine::pdfops`

> Pure-Rust PDF post-processing via `lopdf`. Stateless free functions on
> in-memory PDF byte streams.

## Goal

Provide merge / split / flatten / metadata / watermark operations against
PDF byte streams, with no shell-out to `qpdf`, `pdfcpu`, or `pdftk`, so
the server's `/forms/pdfengines/*` routes mirror Gotenberg using only
Rust dependencies.

## Scope

**In:**

- `merge`, `split`, `flatten`, `read_metadata`, `write_metadata`,
  `watermark`, `rotate`.
- All ops accept and return owned `Vec<u8>`, taking and returning byte
  buffers so they compose with the server's pipeline without filesystem
  round-trips.

**Out:**

- Encryption / decryption (follow-up spec; needs RC4/AES wiring).
- PDF/A or PDF/UA conformance — these require Ghostscript-style passes.
  Requested PDF/A from the LibreOffice path (spec 12) is honored there.
- Bookmarks read/write — follow-up.
- Image / OCR extraction — out of scope.

## Public API

Module path: `engine::pdfops`. All functions are free functions; the
module is stateless.

```rust
use crate::types::{EngineError, EngineResult, PageRanges};
use std::collections::BTreeMap;

/// Concatenate a sequence of PDFs into a single document, preserving order.
/// Empty input slice is an error.
pub fn merge(pdfs: &[&[u8]]) -> EngineResult<Vec<u8>>;

#[derive(Debug, Clone)]
pub enum SplitMode {
    /// One output PDF per `PageRanges` chunk, in order.
    /// Pages absent from any chunk are dropped.
    ByRanges(Vec<PageRanges>),
    /// Split every N pages, in order. Last chunk may be shorter.
    EveryN(u32),
    /// One output PDF per single page.
    OnePagePerFile,
}

pub fn split(pdf: &[u8], mode: &SplitMode) -> EngineResult<Vec<Vec<u8>>>;

/// Flatten interactive form fields and annotations into static page content.
/// Idempotent on already-flat PDFs.
pub fn flatten(pdf: &[u8]) -> EngineResult<Vec<u8>>;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "PascalCase")]
pub struct Metadata {
    pub title: Option<String>,
    pub author: Option<String>,
    pub subject: Option<String>,
    pub keywords: Option<String>,
    pub creator: Option<String>,
    pub producer: Option<String>,
    /// Wire format: "D:YYYYMMDDhhmmss±hh'mm'" (PDF date string).
    pub creation_date: Option<String>,
    pub mod_date: Option<String>,
    /// Custom info-dict entries; keys are PDF Name strings, ASCII only.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub custom: BTreeMap<String, String>,
}

pub fn read_metadata(pdf: &[u8]) -> EngineResult<Metadata>;
/// Merge `meta` into the document's info dict. Fields set to `None` are
/// left untouched; fields set to `Some("")` are removed.
pub fn write_metadata(pdf: &[u8], meta: &Metadata) -> EngineResult<Vec<u8>>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Position {
    Center,
    TopLeft, TopCenter, TopRight,
    MiddleLeft, MiddleRight,
    BottomLeft, BottomCenter, BottomRight,
}

#[derive(Debug, Clone)]
pub struct WatermarkOptions {
    pub kind: WatermarkKind,
    /// 0.0..=1.0; values outside are clamped.
    pub opacity: f32,
    pub rotation_deg: f32,
    pub position: Position,
    /// Apply on every page (true) or only odd pages (false → "stamp first").
    /// Most callers want true.
    pub all_pages: bool,
    /// Tile across the page surface.
    pub tiled: bool,
}

#[derive(Debug, Clone)]
pub enum WatermarkKind {
    Text {
        text: String,
        /// PostScript font name. None = `Helvetica`.
        font: Option<String>,
        /// Point size. Default 48.
        font_size: f32,
        /// RGBA in 0..=1.
        color: [f32; 4],
    },
    ImagePng { bytes: Vec<u8> },
}

pub fn watermark(pdf: &[u8], opts: &WatermarkOptions) -> EngineResult<Vec<u8>>;

/// Rotate pages by 0/90/180/270 degrees (clockwise). Other angles → error.
pub fn rotate(pdf: &[u8], pages: &PageRanges, angle_deg: i32) -> EngineResult<Vec<u8>>;
```

## Behavior

### `merge(pdfs)`

1. Empty slice → `EngineError::InvalidOption("merge requires at least one input")`.
2. Single input → return a clone of the input bytes after a parse round-trip
   (validates input). On parse failure → `EngineError::Internal`.
3. Otherwise:
   1. Load each input via `lopdf::Document::load_mem(bytes)`.
   2. Use `lopdf` page-tree concatenation (the canonical pattern: assemble
      a fresh `Document`, renumber object IDs to avoid collision via
      `Document::renumber_objects()`, then build a unified `/Pages` tree).
   3. Copy `/Outlines` if present from the **first** input only (do not
      attempt to merge bookmarks; out of scope).
   4. Drop `/AcroForm` and `/Names` to avoid name collisions.
   5. Set `/Producer` to `"folio/<version>"`.
   6. Save to `Vec<u8>` via `Document::save_to(&mut Vec<u8>)`.

### `split(pdf, mode)`

1. Parse via `lopdf::Document::load_mem`.
2. Determine `total = doc.get_pages().len() as u32`.
3. For each chunk, build the **inclusive** list of 1-indexed page numbers:
   - `ByRanges(rs)`: `rs.iter().map(|r| pages_for(r, total))`. Empty
     resolved chunk after clamping → skipped (do not produce empty PDFs).
   - `EveryN(n)`: `n == 0` → `EngineError::InvalidOption("EveryN requires N >= 1")`.
     Otherwise produce `ceil(total / n)` chunks of size at most `n`.
   - `OnePagePerFile`: produce `total` chunks, one page each.
4. For each chunk: clone the source `Document`, call
   `Document::delete_pages(&pages_to_remove)`, save to `Vec<u8>`.
5. Return the chunks in the order they were generated.

### `flatten(pdf)`

1. Parse via `lopdf`.
2. Walk the page tree; for each page:
   1. Iterate `/Annots` array. For each annotation:
      - If it's a widget annotation referencing a form field with a
        rendered appearance (`/AP /N`), append the appearance stream as
        a Form XObject and `Do` it from the page's content stream.
      - Other annotation types are dropped (the goal of flattening).
   2. Remove the page's `/Annots` entry.
3. Remove `/AcroForm` from the catalog.
4. Save.

The implementation MUST handle the common case of unfilled forms by
simply removing widgets without crashing. PDFs without forms or
annotations are returned re-serialized but logically identical.

### `read_metadata(pdf)`

1. Parse via `lopdf`.
2. Read `/Info` reference from the trailer; if absent, return
   `Metadata::default()`.
3. Decode each known key (`Title`, `Author`, ...) as PDF text string
   (handles both `()`-literal and `<>`-hex encodings, and the
   UTF-16BE BOM convention).
4. All other entries land in `custom`, with keys as ASCII Names.

### `write_metadata(pdf, meta)`

1. Parse.
2. Get-or-create the `/Info` dictionary.
3. For each `Some` field on `meta`:
   - If the value is `""`, delete the key.
   - Otherwise set it as a PDF text string. Strings with non-ASCII
     characters use the UTF-16BE BOM encoding.
4. Custom keys: same rule. Reject keys not matching `^[A-Za-z][A-Za-z0-9_-]{0,127}$`
   with `EngineError::InvalidOption`.
5. Always update `/ModDate` to "now" in PDF date format unless
   `meta.mod_date` is already set.
6. Save.

### `watermark(pdf, opts)`

1. Validate:
   - `opacity` clamped to `0.0..=1.0`.
   - `rotation_deg` not constrained.
   - `WatermarkKind::Text { font_size, .. }` requires `font_size > 0.0`,
     else `EngineError::InvalidOption`.
   - `WatermarkKind::ImagePng { bytes }`: bytes must start with the PNG
     signature `\x89PNG\r\n\x1a\n`, else `EngineError::InvalidOption`.
2. Parse the input.
3. Build a Form XObject containing the watermark content:
   - Text: a single `BT ... ET` block with `Tf`, `rg/RG`, `cm` (rotation +
     translation), and `Tj` / `TJ`. Use the chosen font (default
     `Helvetica`); embed via `BaseFont`.
   - Image: embed the PNG as an `Image` XObject. Use a transparent
     `Group { S /Transparency }` to support opacity.
4. For each page (or odd pages if `all_pages = false`):
   1. Resolve page MediaBox.
   2. Compute the placement matrix:
      - If `tiled`, repeat the XObject in a grid. Spacing = 1.5 × bbox
        of the watermark XObject.
      - Else, single placement at `Position` with offset 0.
   3. Append a content stream that runs `q ... cm ... gs ... Do Q`.
5. Save.

### `rotate(pdf, pages, angle_deg)`

1. `angle_deg.rem_euclid(360)` must be in `{0, 90, 180, 270}`, else
   `EngineError::InvalidOption("angle must be 0/90/180/270")`.
2. Parse.
3. For each page p in 1..=total: if `pages.contains(p, total)`, set
   `/Rotate` to `(existing + angle_deg).rem_euclid(360)`.
4. Save.

### General

- All ops set `/Producer = "folio/<CARGO_PKG_VERSION>"` (overwrite).
- All ops preserve the input version unless an op fundamentally requires
  bumping (none in MVP).
- All ops compress streams with `FlateDecode` on save.

## Errors

Reuses `EngineError` from spec 10:

| Variant                  | Source                                                                 |
|--------------------------|------------------------------------------------------------------------|
| `InvalidOption(msg)`     | Bad PNG header, invalid angle, empty merge input, EveryN with N=0, etc.|
| `InvalidPageRange(msg)`  | `split(ByRanges)` chunk yields empty page set after parse.             |
| `Internal(msg)`          | `lopdf` parse / save failures, encrypted documents in MVP.             |

Encrypted documents are detected at parse time (`lopdf::Document::is_encrypted`)
and rejected with `EngineError::Internal("encrypted PDFs are not supported in MVP")`.

## Edge cases

| Scenario                                              | Required behavior                                                  |
|-------------------------------------------------------|--------------------------------------------------------------------|
| `merge(&[a])` with valid `a`                          | Returns a parse-resaved copy of `a`.                                |
| `merge` with one corrupted input                      | `EngineError::Internal("merge: input #2: ...")` — never panic.      |
| `split(EveryN(7))` on 3-page doc                      | Returns one chunk with all 3 pages.                                 |
| `split(ByRanges([1-1000]))` on 3-page doc             | Returns one chunk with pages 1..=3 (clamped).                       |
| `split(ByRanges([5-10]))` on 3-page doc               | Empty resolved chunk → skipped; result `vec![]`.                    |
| Repeated `flatten` calls                              | Idempotent. Second call returns identical (modulo timestamps).      |
| `read_metadata` on PDF without `/Info`                | `Metadata::default()`.                                               |
| `write_metadata` with unicode title                   | Stored as UTF-16BE with BOM.                                         |
| `write_metadata { custom: { "bad name!": ... } }`     | `EngineError::InvalidOption`.                                        |
| Watermark on encrypted PDF                            | `EngineError::Internal("encrypted PDFs are not supported in MVP")`. |
| `rotate(pages = "")`                                  | Caught by spec 10's `PageRanges::parse`.                             |
| `rotate(angle_deg = 360)`                             | Treated as 0 — no-op write that re-saves bytes.                     |

## Test plan

All in `crates/engine/src/pdfops/mod.rs` plus
`crates/engine/tests/pdfops.rs`.

### Unit tests (no fixtures required)

- `merge_empty_input_rejected`.
- `merge_invalid_option_message_includes_index`.
- `split_every_n_zero_rejected`.
- `split_every_n_clamps_when_total_smaller_than_n`.
- `split_by_ranges_skips_empty_chunks`.
- `rotate_invalid_angle_rejected`.
- `rotate_normalizes_360_to_0_noop`.
- `metadata_default_when_info_dict_missing`.
- `write_metadata_rejects_invalid_custom_key`.
- `write_metadata_empty_string_removes_key`.
- `watermark_png_header_validation`.
- `watermark_negative_font_size_rejected`.
- `producer_set_after_each_op`.

### Integration tests (`crates/engine/tests/pdfops.rs`)

These use small PDF fixtures committed under
`crates/engine/tests/fixtures/pdf/` (each <50 KB):

- `single_page_a4.pdf`, `three_page_letter.pdf`, `with_form.pdf`,
  `with_annotations.pdf`, `unicode_title.pdf`.

Tests:

- `merge_two_singles_yields_two_pages` — load result, page count == 2.
- `merge_preserves_order` — first page is from input A, second from B.
- `split_every_n_yields_expected_counts` — 3-page doc split N=2 yields
  chunks of 2 + 1.
- `split_by_ranges_extracts_specific_pages`.
- `flatten_removes_form_fields` — input `with_form.pdf` produces output
  whose AcroForm dict is absent.
- `flatten_idempotent` — flatten ∘ flatten = flatten (byte-stable
  modulo `/ModDate`).
- `read_write_metadata_round_trip` — write Title="Hello", read back equal.
- `read_metadata_unicode_title` — `with_unicode_title.pdf` decodes to
  the expected Rust `String`.
- `watermark_text_appears_on_every_page` — flatten then text-extract
  via `lopdf`; assert the watermark string is present per page.
- `watermark_image_png_validates_signature` — corrupt header → error.
- `rotate_only_targeted_pages` — three-page doc, rotate 1,3 by 90°,
  verify `/Rotate` on pages 1 and 3 only.
- `encrypted_input_rejected` — fixture `encrypted.pdf`, every public
  function returns the documented error.

### Property tests (`proptest`)

- `merge_associative_for_two_groupings` — for any 3-element vector of
  small valid PDFs, `merge(merge(a, b), c) == merge(a, merge(b, c))` in
  page count and ordering.
- `split_then_merge_round_trips_page_count` — split EveryN, merge back,
  page count equal.

## Acceptance

- [ ] `crates/engine/src/pdfops/mod.rs` exists and is `pub mod pdfops`
      from `lib.rs`.
- [ ] Public API matches verbatim, including module-level free functions.
- [ ] `lopdf` and `proptest` (dev-only) added via `workspace.dependencies`.
- [ ] All ops are stateless; no `static`s, no `lazy_static`, no global
      mutable state.
- [ ] All ops set `/Producer` to `folio/<crate version>`.
- [ ] Encrypted-input rejection covered by an explicit unit test.
- [ ] All unit tests pass with `cargo test -p engine`.
- [ ] All integration tests pass with `cargo test -p engine`.
- [ ] All property tests pass.
- [ ] `cargo clippy -p engine -- -D warnings` clean.
- [ ] No `unsafe`. No `.unwrap()` outside `#[cfg(test)]` and `#[test]`.

## Out of scope / follow-ups

- Encrypt / decrypt with user/owner passwords.
- Embed missing fonts (would require font subsetting).
- Bookmarks read/write.
- Stamp (similar to watermark but not opacity-blended) — likely a thin
  variant of `watermark` once the latter is solid.
- PDF linearization ("Fast Web View").
- Image extraction.
