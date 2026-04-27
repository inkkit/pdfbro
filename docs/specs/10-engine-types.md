# Spec 10 — `engine::types`

> Shared types and error model for the Folio engine. All other specs build on
> this; nothing else should redeclare these types.

## Goal

Provide the canonical, serde-aware Rust types that describe a PDF generation
request and the engine's error surface, without taking any dependency on
`chromiumoxide`, `lopdf`, or HTTP frameworks.

## Scope

**In:** `PdfOptions`, `PaperSize`, `Margins`, `WaitCondition`, `MediaType`,
`PageRanges`, `BrowserConfig`, `EngineError`, `EngineResult<T>`.

**Out:** Anything Chromium-, LibreOffice-, or HTTP-specific. Those live in
their own specs and may *use* these types.

## Public API

Module path: `engine::types` (re-exported from `engine`'s crate root).

```rust
use std::path::PathBuf;
use std::time::Duration;
use serde::{Deserialize, Serialize};

/// All knobs that influence a single PDF render.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct PdfOptions {
    pub paper: PaperSize,
    pub margin: Margins,
    pub landscape: bool,
    /// Multiplier applied to page rendering. 0.1..=2.0.
    pub scale: f32,
    pub print_background: bool,
    pub prefer_css_page_size: bool,
    pub emulate_media: MediaType,
    pub page_ranges: Option<PageRanges>,
    pub header_template: Option<String>,
    pub footer_template: Option<String>,
    pub wait: WaitCondition,
}

impl Default for PdfOptions { /* see Behavior */ }

/// Paper dimensions in inches. Constructors enforce > 0.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PaperSize {
    pub width_in: f32,
    pub height_in: f32,
}

impl PaperSize {
    pub const A4:     Self = Self { width_in: 8.27,  height_in: 11.69 };
    pub const LETTER: Self = Self { width_in: 8.5,   height_in: 11.0  };
    pub const LEGAL:  Self = Self { width_in: 8.5,   height_in: 14.0  };
    pub const A3:     Self = Self { width_in: 11.69, height_in: 16.54 };
    pub const A5:     Self = Self { width_in: 5.83,  height_in: 8.27  };

    pub fn new(width_in: f32, height_in: f32) -> Result<Self, EngineError>;
}

/// Margins in inches.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Margins {
    pub top: f32, pub right: f32, pub bottom: f32, pub left: f32,
}

impl Margins {
    pub const ZERO:    Self = Self { top: 0.0,  right: 0.0,  bottom: 0.0,  left: 0.0  };
    pub const DEFAULT: Self = Self { top: 0.39, right: 0.39, bottom: 0.39, left: 0.39 }; // ~1cm

    pub fn uniform(inches: f32) -> Self;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MediaType { #[default] Print, Screen }

/// Page ranges parsed from the Gotenberg-compatible string form, e.g. "1-3,5,7-".
/// `to_string` round-trips canonical form.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct PageRanges(Vec<PageRange>);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageRange { Single(u32), Closed(u32, u32), OpenEnd(u32) /* "7-" */ }

impl PageRanges {
    pub fn parse(s: &str) -> Result<Self, EngineError>;
    pub fn contains(&self, page: u32, total: u32) -> bool;
}

/// What to wait for after navigation/setContent before rendering.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum WaitCondition {
    #[default]
    Load,
    DomContentLoaded,
    NetworkIdle,
    Selector { selector: String },
    Expression { expression: String },
    Delay { #[serde(with = "humantime_serde")] duration: Duration },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct BrowserConfig {
    /// Path to chrome/chromium. If `None`, autodiscover via $PATH then
    /// platform-typical locations; finally fall through to `EngineError::ChromeNotFound`.
    pub executable: Option<PathBuf>,
    /// Run with --headless=new. Default true.
    pub headless: bool,
    /// Extra command line flags appended verbatim.
    pub extra_args: Vec<String>,
    /// Disable Chrome's sandbox. Required inside most Docker images.
    /// Default: true on Linux, false elsewhere.
    pub no_sandbox: bool,
    /// Per-page navigation/render timeout.
    #[serde(with = "humantime_serde")]
    pub timeout: Duration,
}

impl Default for BrowserConfig { /* see Behavior */ }

#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    #[error("invalid option: {0}")]
    InvalidOption(String),

    #[error("invalid page range: {0}")]
    InvalidPageRange(String),

    #[error("chrome executable not found (searched: {searched:?})")]
    ChromeNotFound { searched: Vec<PathBuf> },

    #[error("chrome failed to launch: {0}")]
    ChromeLaunch(String),

    #[error("CDP error: {0}")]
    Cdp(String),

    #[error("navigation failed for {url}: {reason}")]
    Navigation { url: String, reason: String },

    #[error("operation timed out after {0:?}")]
    Timeout(Duration),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("internal error: {0}")]
    Internal(String),
}

pub type EngineResult<T> = Result<T, EngineError>;
```

## Behavior

### `PdfOptions::default()`

```
PdfOptions {
    paper: PaperSize::A4,
    margin: Margins::DEFAULT,
    landscape: false,
    scale: 1.0,
    print_background: true,
    prefer_css_page_size: false,
    emulate_media: MediaType::Print,
    page_ranges: None,
    header_template: None,
    footer_template: None,
    wait: WaitCondition::Load,
}
```

### `BrowserConfig::default()`

```
BrowserConfig {
    executable: None,
    headless: true,
    extra_args: vec![],
    no_sandbox: cfg!(target_os = "linux"),
    timeout: Duration::from_secs(60),
}
```

### `PaperSize::new(w, h)`

- If `w <= 0.0` or `h <= 0.0` → `EngineError::InvalidOption("paper dimensions must be > 0")`.
- If `w > 200.0` or `h > 200.0` → `EngineError::InvalidOption("paper dimensions must be <= 200in")`.
- Else `Ok(Self { width_in: w, height_in: h })`.

### `PageRanges::parse(s)`

Grammar (whitespace ignored):

```
ranges  := range ("," range)*
range   := number | number "-" number | number "-"
number  := [1-9][0-9]*
```

- Empty input or only commas → `EngineError::InvalidPageRange`.
- A range `a-b` requires `a <= b`, else error.
- Result preserves input order. Caller is responsible for de-duplication.

### `PageRanges::contains(page, total)`

- `Single(n)` → `page == n && n <= total`.
- `Closed(a, b)` → `a <= page && page <= b.min(total)`.
- `OpenEnd(a)` → `a <= page && page <= total`.

### Validation (used by `ChromiumEngine` before invoking CDP)

`PdfOptions::validate(&self) -> EngineResult<()>` checks:

- `0.1 <= scale <= 2.0`,
- `paper.width_in > 0 && paper.height_in > 0` (already by constructor),
- All margins are finite and `>= 0` and each `< paper.width_in / 2` (left/right) or `< paper.height_in / 2` (top/bottom),
- Header/footer templates, if `Some`, are non-empty after trimming.

This function MUST be exposed publicly; binaries call it before queueing a render.

## Errors

The full `EngineError` enum is the *only* error type returned from any spec
in the `engine::*` family. Each downstream spec adds variants by editing
this spec rather than introducing parallel error enums.

## Edge cases

| Input                             | Required behavior                                         |
|-----------------------------------|-----------------------------------------------------------|
| `PageRanges::parse("")`           | `Err(InvalidPageRange("empty"))`                          |
| `PageRanges::parse(",,")`         | `Err(InvalidPageRange)`                                   |
| `PageRanges::parse("0-3")`        | `Err(InvalidPageRange("page numbers are 1-indexed"))`     |
| `PageRanges::parse("5-3")`        | `Err(InvalidPageRange("end < start"))`                    |
| `PageRanges::parse(" 1 - 3 , 7-")`| `Ok([Closed(1,3), OpenEnd(7)])`                           |
| `PaperSize::new(0.0, 11.0)`       | `Err(InvalidOption(..))`                                  |
| `PaperSize::new(8.5, f32::NAN)`   | `Err(InvalidOption(..))`                                  |
| `PdfOptions { scale: 3.0, .. }`   | `validate()` → `Err(InvalidOption("scale out of range"))` |
| Missing fields in JSON deserialise| Treated as defaults via `#[serde(default)]`               |

## Test plan

All in `crates/engine/src/types.rs` under `#[cfg(test)] mod tests`.

- `paper_size_constants_match_spec` — all five preset constants.
- `paper_size_new_rejects_nonpositive`.
- `paper_size_new_rejects_nan_inf`.
- `margins_uniform_sets_all_four`.
- `page_ranges_parse_single_number`.
- `page_ranges_parse_closed_range`.
- `page_ranges_parse_open_end`.
- `page_ranges_parse_mixed_with_whitespace`.
- `page_ranges_parse_rejects_zero`.
- `page_ranges_parse_rejects_inverted`.
- `page_ranges_parse_rejects_empty`.
- `page_ranges_contains_handles_total_clamp`.
- `page_ranges_round_trips_via_serde`.
- `pdf_options_default_matches_spec`.
- `pdf_options_validate_scale_range`.
- `pdf_options_validate_margin_too_large`.
- `pdf_options_serde_camel_case_roundtrip` — JSON `{"paper":{"widthIn":...},...}`.
- `wait_condition_default_is_load`.
- `wait_condition_serde_tag_kind`.
- `browser_config_default_no_sandbox_on_linux_only`.

## Acceptance

- [ ] `crates/engine/src/types.rs` exists and is `pub mod types` from `lib.rs`.
- [ ] All public items in *Public API* compile and match signatures verbatim.
- [ ] Workspace deps added: `serde`, `serde_json` (dev), `thiserror`, `humantime-serde`.
- [ ] `cargo test -p engine` passes with all tests in *Test plan*.
- [ ] `cargo doc -p engine --no-deps` produces no warnings.
- [ ] No `unwrap`/`expect` on user-supplied input paths.
- [ ] `lib.rs` carries `#![deny(rust_2018_idioms, missing_docs)]`.

## Out of scope / follow-ups

- ScreenshotOptions (separate spec when we tackle `/screenshot/*`).
- PDF/A and PDF/UA flags (added when spec 13 lands).
- Cookies / extra HTTP headers (added by spec 11; types live there).
