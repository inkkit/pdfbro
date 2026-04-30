//! Shared types and error model for the Folio engine.
//!
//! Implementation of `docs/specs/10-engine-types.md`. All other engine
//! submodules and downstream crates build on these types; nothing else
//! should redeclare them.

use std::fmt;
use std::path::PathBuf;
use std::time::Duration;

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

/// All errors produced by the engine layer.
///
/// This is the *only* error type returned from any spec in the `engine::*`
/// family. New variants are added here when downstream specs need them.
#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    /// A user-supplied option failed validation.
    #[error("invalid option: {0}")]
    InvalidOption(String),

    /// A user-supplied page-range string failed to parse.
    #[error("invalid page range: {0}")]
    InvalidPageRange(String),

    /// Chrome / Chromium executable could not be located.
    #[error("chrome executable not found (searched: {searched:?})")]
    ChromeNotFound {
        /// Paths that were searched, in order.
        searched: Vec<PathBuf>,
    },

    /// Chrome failed to launch.
    #[error("chrome failed to launch: {0}")]
    ChromeLaunch(String),

    /// A Chrome DevTools Protocol call failed.
    #[error("CDP error: {0}")]
    Cdp(String),

    /// Page navigation failed (DNS error, refused connection, fail-on-status, ...).
    #[error("navigation failed for {url}: {reason}")]
    Navigation {
        /// URL the engine attempted to navigate to.
        url: String,
        /// Human-readable reason.
        reason: String,
    },

    /// An operation exceeded its allotted timeout.
    #[error("operation timed out after {0:?}")]
    Timeout(Duration),

    /// An I/O error occurred (filesystem, sockets, etc.).
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    /// A PDF parsing/manipulation error.
    #[error("pdf error: {0}")]
    Pdf(String),

    /// A bug or unhandled internal condition.
    #[error("internal error: {0}")]
    Internal(String),
}

impl From<lopdf::Error> for EngineError {
    fn from(e: lopdf::Error) -> Self {
        EngineError::Pdf(e.to_string())
    }
}

/// Convenience alias for results returned by engine operations.
pub type EngineResult<T> = Result<T, EngineError>;

// ---------------------------------------------------------------------------
// Paper size
// ---------------------------------------------------------------------------

/// Paper dimensions in inches.
///
/// Constructors enforce that both dimensions are positive, finite, and
/// at most 200 inches. Use the named constants (`A4`, `LETTER`, ...) or
/// [`PaperSize::new`] to construct values.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PaperSize {
    /// Width in inches.
    pub width_in: f32,
    /// Height in inches.
    pub height_in: f32,
}

impl PaperSize {
    /// ISO A4 — 8.27" × 11.69".
    pub const A4: Self = Self {
        width_in: 8.27,
        height_in: 11.69,
    };
    /// US Letter — 8.5" × 11".
    pub const LETTER: Self = Self {
        width_in: 8.5,
        height_in: 11.0,
    };
    /// US Legal — 8.5" × 14".
    pub const LEGAL: Self = Self {
        width_in: 8.5,
        height_in: 14.0,
    };
    /// ISO A3 — 11.69" × 16.54".
    pub const A3: Self = Self {
        width_in: 11.69,
        height_in: 16.54,
    };
    /// ISO A5 — 5.83" × 8.27".
    pub const A5: Self = Self {
        width_in: 5.83,
        height_in: 8.27,
    };

    /// Construct a custom paper size.
    ///
    /// # Errors
    ///
    /// Returns [`EngineError::InvalidOption`] if either dimension is
    /// non-positive, non-finite, or larger than 200 inches.
    pub fn new(width_in: f32, height_in: f32) -> EngineResult<Self> {
        for (label, v) in [("width", width_in), ("height", height_in)] {
            if !v.is_finite() {
                return Err(EngineError::InvalidOption(format!(
                    "paper {label} must be finite (got {v})"
                )));
            }
            if v <= 0.0 {
                return Err(EngineError::InvalidOption(format!(
                    "paper dimensions must be > 0 (got {label}={v})"
                )));
            }
            if v > 200.0 {
                return Err(EngineError::InvalidOption(format!(
                    "paper dimensions must be <= 200in (got {label}={v})"
                )));
            }
        }
        Ok(Self {
            width_in,
            height_in,
        })
    }
}

// ---------------------------------------------------------------------------
// Margins
// ---------------------------------------------------------------------------

/// Page margins in inches.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Margins {
    /// Top margin in inches.
    pub top: f32,
    /// Right margin in inches.
    pub right: f32,
    /// Bottom margin in inches.
    pub bottom: f32,
    /// Left margin in inches.
    pub left: f32,
}

impl Margins {
    /// Zero margins on all sides.
    pub const ZERO: Self = Self {
        top: 0.0,
        right: 0.0,
        bottom: 0.0,
        left: 0.0,
    };
    /// Default margins (~1cm = 0.39in) on all sides.
    pub const DEFAULT: Self = Self {
        top: 0.39,
        right: 0.39,
        bottom: 0.39,
        left: 0.39,
    };

    /// Set all four margins to the same value.
    pub fn uniform(inches: f32) -> Self {
        Self {
            top: inches,
            right: inches,
            bottom: inches,
            left: inches,
        }
    }
}

// ---------------------------------------------------------------------------
// MediaType
// ---------------------------------------------------------------------------

/// CSS media emulation. Defaults to [`MediaType::Print`] which is what most
/// PDF renderers want.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MediaType {
    /// Emulate `@media print`. Default.
    #[default]
    Print,
    /// Emulate `@media screen`.
    Screen,
}

// ---------------------------------------------------------------------------
// Page ranges
// ---------------------------------------------------------------------------

/// One element of a page-ranges expression.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageRange {
    /// A single page (1-indexed).
    Single(u32),
    /// A closed `start-end` range (1-indexed, inclusive on both ends).
    Closed(u32, u32),
    /// An open-ended `start-` range that continues to the last page.
    OpenEnd(u32),
}

impl fmt::Display for PageRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PageRange::Single(n) => write!(f, "{n}"),
            PageRange::Closed(a, b) => write!(f, "{a}-{b}"),
            PageRange::OpenEnd(a) => write!(f, "{a}-"),
        }
    }
}

/// A parsed list of page ranges, e.g. `"1-3,5,7-"`.
///
/// Round-trips through serde as a single string.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct PageRanges(Vec<PageRange>);

impl PageRanges {
    /// Parse a Gotenberg-compatible page-ranges string.
    ///
    /// Grammar (whitespace ignored around tokens):
    ///
    /// ```text
    /// ranges := range ("," range)*
    /// range  := number | number "-" number | number "-"
    /// number := [1-9][0-9]*
    /// ```
    ///
    /// # Errors
    ///
    /// Returns [`EngineError::InvalidPageRange`] for empty input, empty
    /// segments, leading-zero numbers, or inverted (`end < start`) ranges.
    pub fn parse(s: &str) -> EngineResult<Self> {
        let trimmed = s.trim();
        if trimmed.is_empty() {
            return Err(EngineError::InvalidPageRange("empty".into()));
        }
        let mut out = Vec::new();
        for seg in trimmed.split(',') {
            let seg = seg.trim();
            if seg.is_empty() {
                return Err(EngineError::InvalidPageRange("empty range".into()));
            }
            out.push(parse_range_segment(seg)?);
        }
        Ok(Self(out))
    }

    /// Return `true` iff `page` (1-indexed) is included in any range,
    /// given a document `total` page count.
    pub fn contains(&self, page: u32, total: u32) -> bool {
        if page == 0 || page > total {
            return false;
        }
        self.0.iter().any(|r| match *r {
            PageRange::Single(n) => n == page && n <= total,
            PageRange::Closed(a, b) => page >= a && page <= b.min(total),
            PageRange::OpenEnd(a) => page >= a && page <= total,
        })
    }

    /// View the underlying parsed segments.
    pub fn as_slice(&self) -> &[PageRange] {
        &self.0
    }

    /// Expand this expression into a sorted, deduplicated list of 1-indexed
    /// page numbers that are within `1..=total`.
    pub fn expand(&self, total: u32) -> Vec<u32> {
        use std::collections::BTreeSet;
        let mut pages = BTreeSet::new();
        for r in &self.0 {
            match *r {
                PageRange::Single(n) => {
                    if (1..=total).contains(&n) {
                        pages.insert(n);
                    }
                }
                PageRange::Closed(a, b) => {
                    let lo = a.max(1);
                    let hi = b.min(total);
                    if lo <= hi {
                        for p in lo..=hi {
                            pages.insert(p);
                        }
                    }
                }
                PageRange::OpenEnd(a) => {
                    let lo = a.max(1);
                    for p in lo..=total {
                        pages.insert(p);
                    }
                }
            }
        }
        pages.into_iter().collect()
    }
}

impl fmt::Display for PageRanges {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut first = true;
        for r in &self.0 {
            if !first {
                write!(f, ",")?;
            }
            first = false;
            write!(f, "{r}")?;
        }
        Ok(())
    }
}

impl From<PageRanges> for String {
    fn from(p: PageRanges) -> Self {
        p.to_string()
    }
}

impl TryFrom<String> for PageRanges {
    type Error = EngineError;
    fn try_from(s: String) -> Result<Self, Self::Error> {
        Self::parse(&s)
    }
}

fn parse_range_segment(seg: &str) -> EngineResult<PageRange> {
    if let Some((l, r)) = seg.split_once('-') {
        let lo = parse_page_number(l.trim())?;
        let r = r.trim();
        if r.is_empty() {
            Ok(PageRange::OpenEnd(lo))
        } else {
            let hi = parse_page_number(r)?;
            if hi < lo {
                return Err(EngineError::InvalidPageRange(format!(
                    "end {hi} < start {lo}"
                )));
            }
            Ok(PageRange::Closed(lo, hi))
        }
    } else {
        Ok(PageRange::Single(parse_page_number(seg)?))
    }
}

fn parse_page_number(s: &str) -> EngineResult<u32> {
    if s.is_empty() {
        return Err(EngineError::InvalidPageRange("missing page number".into()));
    }
    if !s.chars().all(|c| c.is_ascii_digit()) {
        return Err(EngineError::InvalidPageRange(format!(
            "not a number: {s:?}"
        )));
    }
    if s.starts_with('0') {
        return Err(EngineError::InvalidPageRange(
            "page numbers are 1-indexed".into(),
        ));
    }
    s.parse::<u32>()
        .map_err(|_| EngineError::InvalidPageRange(format!("not a number: {s:?}")))
}

// ---------------------------------------------------------------------------
// WaitCondition
// ---------------------------------------------------------------------------

/// What to wait for after navigation/setContent before rendering.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum WaitCondition {
    /// Wait until `Page.loadEventFired` (the default).
    #[default]
    Load,
    /// Wait until `Page.domContentEventFired`.
    DomContentLoaded,
    /// Wait for the `networkIdle` lifecycle event.
    NetworkIdle,
    /// Poll the page until `document.querySelector(selector)` returns a node.
    Selector {
        /// CSS selector to poll for.
        selector: String,
    },
    /// Poll the page until `eval(expression)` is truthy.
    Expression {
        /// JavaScript expression to evaluate (must coerce to bool).
        expression: String,
    },
    /// Sleep for a fixed duration after navigation.
    Delay {
        /// How long to sleep.
        #[serde(with = "humantime_serde")]
        duration: Duration,
    },
    /// Wait until `window.status` equals the given value.
    WindowStatus {
        /// The value to wait for in `window.status`.
        status: String,
    },
}
// BrowserConfig
// ---------------------------------------------------------------------------

/// Engine-wide browser configuration.
///
/// Constructed once when launching a [`crate::types::BrowserConfig`]-aware
/// engine; not per-render.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct BrowserConfig {
    /// Path to the browser executable. `None` means autodiscover via
    /// `$PATH` and platform defaults; failure is reported as
    /// [`EngineError::ChromeNotFound`].
    pub executable: Option<PathBuf>,
    /// Run with `--headless=new`. Default: `true`.
    pub headless: bool,
    /// Extra command-line flags appended verbatim.
    pub extra_args: Vec<String>,
    /// Disable Chrome's sandbox. Required inside most Docker images.
    /// Default: `true` on Linux, `false` elsewhere.
    pub no_sandbox: bool,
    /// Per-page navigation/render timeout. Default: 60s.
    #[serde(with = "humantime_serde")]
    pub timeout: Duration,
    /// Use lazy initialization (start on first request).
    /// Default: false (start eagerly at server startup).
    pub lazy_start: bool,
    /// Idle shutdown timeout - browser shuts down after this duration of no requests.
    /// None means no idle shutdown. Default: None.
    #[serde(with = "humantime_serde")]
    pub idle_shutdown_timeout: Option<Duration>,
    /// When `Some(t)`, after `load` events fire, race `networkIdle` against
    /// this timeout — whichever fires first wins. When `None` (default),
    /// networkIdle is skipped entirely, matching gotenberg's default.
    #[serde(with = "humantime_serde")]
    pub network_idle_timeout: Option<Duration>,
}

impl Default for BrowserConfig {
    fn default() -> Self {
        Self {
            executable: None,
            headless: true,
            extra_args: Vec::new(),
            no_sandbox: cfg!(target_os = "linux"),
            timeout: Duration::from_secs(60),
            lazy_start: false,
            idle_shutdown_timeout: None,
            network_idle_timeout: None,
        }
    }
}

// ---------------------------------------------------------------------------
// PdfOptions
// ---------------------------------------------------------------------------

/// All knobs that influence a single PDF render.
///
/// Constructed per-call; values are validated by [`PdfOptions::validate`]
/// before any CDP traffic.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct PdfOptions {
    /// Paper dimensions.
    pub paper: PaperSize,
    /// Page margins.
    pub margin: Margins,
    /// Render in landscape orientation.
    pub landscape: bool,
    /// Multiplier applied to page rendering. Allowed range: `0.1..=2.0`.
    pub scale: f32,
    /// Print background graphics.
    pub print_background: bool,
    /// Honor `@page { size: ... }` CSS rules; overrides [`PdfOptions::paper`]
    /// when present.
    pub prefer_css_page_size: bool,
    /// CSS media to emulate.
    pub emulate_media: MediaType,
    /// Subset of pages to include in the output.
    pub page_ranges: Option<PageRanges>,
    /// HTML template rendered as the page header.
    pub header_template: Option<String>,
    /// HTML template rendered as the page footer.
    pub footer_template: Option<String>,
    /// What to wait for before printing.
    pub wait: WaitCondition,
}

impl Default for PdfOptions {
    fn default() -> Self {
        Self {
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
    }
}

impl PdfOptions {
    /// Validate the option set. Called by engines before any CDP work.
    ///
    /// # Errors
    ///
    /// Returns [`EngineError::InvalidOption`] if `scale` is outside
    /// `0.1..=2.0`, if any margin is non-finite or negative, if a margin
    /// is at least half the corresponding paper dimension, or if a header
    /// or footer template is empty after trimming.
    pub fn validate(&self) -> EngineResult<()> {
        if !self.scale.is_finite() || !(0.1..=2.0).contains(&self.scale) {
            return Err(EngineError::InvalidOption(format!(
                "scale out of range (got {}, allowed 0.1..=2.0)",
                self.scale
            )));
        }

        let m = &self.margin;
        for (label, v) in [
            ("top", m.top),
            ("right", m.right),
            ("bottom", m.bottom),
            ("left", m.left),
        ] {
            if !v.is_finite() {
                return Err(EngineError::InvalidOption(format!(
                    "margin.{label} must be finite (got {v})"
                )));
            }
            if v < 0.0 {
                return Err(EngineError::InvalidOption(format!(
                    "margin.{label} must be >= 0 (got {v})"
                )));
            }
        }

        let half_w = self.paper.width_in / 2.0;
        let half_h = self.paper.height_in / 2.0;
        for (label, v, half) in [
            ("left", m.left, half_w),
            ("right", m.right, half_w),
            ("top", m.top, half_h),
            ("bottom", m.bottom, half_h),
        ] {
            if v >= half {
                return Err(EngineError::InvalidOption(format!(
                    "margin.{label} ({v}) must be < half the paper dimension ({half})"
                )));
            }
        }

        if let Some(t) = &self.header_template
            && t.trim().is_empty()
        {
            return Err(EngineError::InvalidOption("headerTemplate is empty".into()));
        }
        if let Some(t) = &self.footer_template
            && t.trim().is_empty()
        {
            return Err(EngineError::InvalidOption("footerTemplate is empty".into()));
        }

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // --- PaperSize ---------------------------------------------------------

    #[test]
    fn paper_size_constants_match_spec() {
        assert_eq!(PaperSize::A4.width_in, 8.27);
        assert_eq!(PaperSize::A4.height_in, 11.69);
        assert_eq!(PaperSize::LETTER.width_in, 8.5);
        assert_eq!(PaperSize::LETTER.height_in, 11.0);
        assert_eq!(PaperSize::LEGAL.width_in, 8.5);
        assert_eq!(PaperSize::LEGAL.height_in, 14.0);
        assert_eq!(PaperSize::A3.width_in, 11.69);
        assert_eq!(PaperSize::A3.height_in, 16.54);
        assert_eq!(PaperSize::A5.width_in, 5.83);
        assert_eq!(PaperSize::A5.height_in, 8.27);
    }

    #[test]
    fn paper_size_new_rejects_nonpositive() {
        assert!(matches!(
            PaperSize::new(0.0, 11.0),
            Err(EngineError::InvalidOption(_))
        ));
        assert!(matches!(
            PaperSize::new(8.5, -1.0),
            Err(EngineError::InvalidOption(_))
        ));
    }

    #[test]
    fn paper_size_new_rejects_nan_inf() {
        assert!(matches!(
            PaperSize::new(8.5, f32::NAN),
            Err(EngineError::InvalidOption(_))
        ));
        assert!(matches!(
            PaperSize::new(f32::INFINITY, 11.0),
            Err(EngineError::InvalidOption(_))
        ));
        assert!(matches!(
            PaperSize::new(8.5, 201.0),
            Err(EngineError::InvalidOption(_))
        ));
    }

    // --- Margins -----------------------------------------------------------

    #[test]
    fn margins_uniform_sets_all_four() {
        let m = Margins::uniform(0.5);
        assert_eq!(m.top, 0.5);
        assert_eq!(m.right, 0.5);
        assert_eq!(m.bottom, 0.5);
        assert_eq!(m.left, 0.5);
    }

    // --- PageRanges --------------------------------------------------------

    #[test]
    fn page_ranges_parse_single_number() {
        let r = PageRanges::parse("3").unwrap();
        assert_eq!(r.as_slice(), &[PageRange::Single(3)]);
    }

    #[test]
    fn page_ranges_parse_closed_range() {
        let r = PageRanges::parse("2-5").unwrap();
        assert_eq!(r.as_slice(), &[PageRange::Closed(2, 5)]);
    }

    #[test]
    fn page_ranges_parse_open_end() {
        let r = PageRanges::parse("7-").unwrap();
        assert_eq!(r.as_slice(), &[PageRange::OpenEnd(7)]);
    }

    #[test]
    fn page_ranges_parse_mixed_with_whitespace() {
        let r = PageRanges::parse(" 1 - 3 , 7- , 9 ").unwrap();
        assert_eq!(
            r.as_slice(),
            &[
                PageRange::Closed(1, 3),
                PageRange::OpenEnd(7),
                PageRange::Single(9),
            ]
        );
    }

    #[test]
    fn page_ranges_parse_rejects_zero() {
        let err = PageRanges::parse("0-3").unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("1-indexed"), "msg: {msg}");
    }

    #[test]
    fn page_ranges_parse_rejects_inverted() {
        let err = PageRanges::parse("5-3").unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("end 3 < start 5"), "msg: {msg}");
    }

    #[test]
    fn page_ranges_parse_rejects_empty() {
        assert!(matches!(
            PageRanges::parse(""),
            Err(EngineError::InvalidPageRange(_))
        ));
        assert!(matches!(
            PageRanges::parse("   "),
            Err(EngineError::InvalidPageRange(_))
        ));
        assert!(matches!(
            PageRanges::parse(",,"),
            Err(EngineError::InvalidPageRange(_))
        ));
    }

    #[test]
    fn page_ranges_contains_handles_total_clamp() {
        let r = PageRanges::parse("1-3,5,7-").unwrap();
        // Single
        assert!(r.contains(5, 10));
        assert!(!r.contains(5, 4));
        // Closed clamped by total
        assert!(r.contains(3, 10));
        assert!(!r.contains(4, 10));
        // OpenEnd
        assert!(r.contains(8, 10));
        assert!(!r.contains(8, 7));
        // Out of range
        assert!(!r.contains(0, 10));
        assert!(!r.contains(11, 10));
    }

    #[test]
    fn page_ranges_round_trips_via_serde() {
        let original = PageRanges::parse("1-3,5,7-").unwrap();
        let json = serde_json::to_string(&original).unwrap();
        assert_eq!(json, "\"1-3,5,7-\"");
        let parsed: PageRanges = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, original);
    }

    // --- PdfOptions --------------------------------------------------------

    #[test]
    fn pdf_options_default_matches_spec() {
        let d = PdfOptions::default();
        assert_eq!(d.paper, PaperSize::A4);
        assert_eq!(d.margin, Margins::DEFAULT);
        assert!(!d.landscape);
        assert_eq!(d.scale, 1.0);
        assert!(d.print_background);
        assert!(!d.prefer_css_page_size);
        assert_eq!(d.emulate_media, MediaType::Print);
        assert!(d.page_ranges.is_none());
        assert!(d.header_template.is_none());
        assert!(d.footer_template.is_none());
        assert_eq!(d.wait, WaitCondition::Load);
    }

    #[test]
    fn pdf_options_validate_scale_range() {
        let too_small = PdfOptions {
            scale: 0.05,
            ..PdfOptions::default()
        };
        assert!(matches!(
            too_small.validate(),
            Err(EngineError::InvalidOption(_))
        ));

        let too_big = PdfOptions {
            scale: 2.5,
            ..PdfOptions::default()
        };
        assert!(matches!(
            too_big.validate(),
            Err(EngineError::InvalidOption(_))
        ));

        let ok = PdfOptions {
            scale: 1.0,
            ..PdfOptions::default()
        };
        assert!(ok.validate().is_ok());
    }

    #[test]
    fn pdf_options_validate_margin_too_large() {
        // A4 width is 8.27 — half is 4.135. A 5in left margin must fail.
        let oversized = PdfOptions {
            margin: Margins {
                top: 0.5,
                right: 0.5,
                bottom: 0.5,
                left: 5.0,
            },
            ..PdfOptions::default()
        };
        assert!(matches!(
            oversized.validate(),
            Err(EngineError::InvalidOption(_))
        ));

        let negative = PdfOptions {
            margin: Margins {
                top: -0.1,
                ..Margins::DEFAULT
            },
            ..PdfOptions::default()
        };
        assert!(matches!(
            negative.validate(),
            Err(EngineError::InvalidOption(_))
        ));

        let nan = PdfOptions {
            margin: Margins {
                top: f32::NAN,
                ..Margins::DEFAULT
            },
            ..PdfOptions::default()
        };
        assert!(matches!(nan.validate(), Err(EngineError::InvalidOption(_))));
    }

    #[test]
    fn pdf_options_validate_rejects_blank_templates() {
        let blank_header = PdfOptions {
            header_template: Some("   ".into()),
            ..PdfOptions::default()
        };
        assert!(matches!(
            blank_header.validate(),
            Err(EngineError::InvalidOption(_))
        ));

        let blank_footer = PdfOptions {
            footer_template: Some(String::new()),
            ..PdfOptions::default()
        };
        assert!(matches!(
            blank_footer.validate(),
            Err(EngineError::InvalidOption(_))
        ));
    }

    #[test]
    fn pdf_options_serde_camel_case_roundtrip() {
        let opts = PdfOptions {
            paper: PaperSize::A4,
            margin: Margins::DEFAULT,
            landscape: true,
            scale: 1.5,
            print_background: false,
            prefer_css_page_size: true,
            emulate_media: MediaType::Screen,
            page_ranges: Some(PageRanges::parse("1-3").unwrap()),
            header_template: None,
            footer_template: None,
            wait: WaitCondition::NetworkIdle,
        };
        let json = serde_json::to_value(&opts).unwrap();
        // Spot-check camelCase field naming on the public surface
        assert!(json.get("printBackground").is_some());
        assert!(json.get("preferCssPageSize").is_some());
        assert!(json.get("emulateMedia").is_some());
        assert!(json.get("pageRanges").is_some());
        // f32 widens to f64 on serialization; compare with tolerance.
        let w = json["paper"]["widthIn"].as_f64().unwrap();
        let h = json["paper"]["heightIn"].as_f64().unwrap();
        assert!((w - 8.27).abs() < 1e-3, "widthIn was {w}");
        assert!((h - 11.69).abs() < 1e-3, "heightIn was {h}");
        assert_eq!(json["emulateMedia"], "screen");

        let back: PdfOptions = serde_json::from_value(json).unwrap();
        assert_eq!(back, opts);
    }

    #[test]
    fn pdf_options_deserializes_with_missing_fields_via_default() {
        // Every field optional via #[serde(default)] on the struct.
        let v: PdfOptions = serde_json::from_str("{}").unwrap();
        assert_eq!(v, PdfOptions::default());
    }

    // --- WaitCondition -----------------------------------------------------

    #[test]
    fn wait_condition_default_is_load() {
        assert_eq!(WaitCondition::default(), WaitCondition::Load);
    }

    #[test]
    fn wait_condition_serde_tag_kind() {
        // Unit variant
        let v = serde_json::to_value(&WaitCondition::Load).unwrap();
        assert_eq!(v, serde_json::json!({"kind": "load"}));

        let v = serde_json::to_value(&WaitCondition::DomContentLoaded).unwrap();
        assert_eq!(v, serde_json::json!({"kind": "domContentLoaded"}));

        // Struct variant
        let v = serde_json::to_value(&WaitCondition::Selector {
            selector: "#main".into(),
        })
        .unwrap();
        assert_eq!(
            v,
            serde_json::json!({"kind": "selector", "selector": "#main"})
        );

        // Delay with humantime
        let v = serde_json::to_value(&WaitCondition::Delay {
            duration: Duration::from_millis(500),
        })
        .unwrap();
        assert_eq!(v["kind"], "delay");
        // humantime serialises as "500ms" or similar; just round-trip
        let back: WaitCondition = serde_json::from_value(v).unwrap();
        assert_eq!(
            back,
            WaitCondition::Delay {
                duration: Duration::from_millis(500),
            }
        );

        // WindowStatus variant
        let v = serde_json::to_value(&WaitCondition::WindowStatus {
            status: "ready".into(),
        })
        .unwrap();
        assert_eq!(
            v,
            serde_json::json!({"kind": "windowStatus", "status": "ready"})
        );
        let back: WaitCondition = serde_json::from_value(v).unwrap();
        assert_eq!(
            back,
            WaitCondition::WindowStatus {
                status: "ready".into(),
            }
        );
    }

    // --- BrowserConfig -----------------------------------------------------

    #[test]
    fn browser_config_default_no_sandbox_on_linux_only() {
        let c = BrowserConfig::default();
        assert_eq!(c.no_sandbox, cfg!(target_os = "linux"));
        assert_eq!(c.timeout, Duration::from_secs(60));
        assert!(c.headless);
        assert!(c.executable.is_none());
        assert!(c.extra_args.is_empty());
    }

    #[test]
    fn browser_config_serde_round_trip() {
        let c = BrowserConfig {
            executable: Some(PathBuf::from("/usr/bin/chromium")),
            headless: true,
            extra_args: vec!["--mute-audio".into()],
            no_sandbox: true,
            timeout: Duration::from_secs(30),
            lazy_start: false,
            idle_shutdown_timeout: None,
            network_idle_timeout: None,
        };
        let json = serde_json::to_string(&c).unwrap();
        let back: BrowserConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(back.executable, c.executable);
        assert_eq!(back.headless, c.headless);
        assert_eq!(back.extra_args, c.extra_args);
        assert_eq!(back.no_sandbox, c.no_sandbox);
        assert_eq!(back.timeout, c.timeout);
        assert_eq!(back.lazy_start, c.lazy_start);
        assert_eq!(back.idle_shutdown_timeout, c.idle_shutdown_timeout);
        assert_eq!(back.network_idle_timeout, c.network_idle_timeout);
    }

    // --- Sanity: types are Send + Sync where expected ---------------------

    #[test]
    fn types_are_send_sync() {
        use static_assertions::assert_impl_all;
        assert_impl_all!(EngineError: Send, Sync);
        assert_impl_all!(PdfOptions: Send, Sync, Clone);
        assert_impl_all!(BrowserConfig: Send, Sync, Clone);
        assert_impl_all!(PageRanges: Send, Sync, Clone);
    }
}
