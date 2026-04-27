#![deny(rust_2018_idioms, missing_docs)]
//! Folio engine — Chrome / LibreOffice / PDF backends behind a single Rust API.
//!
//! This crate hosts the engine layer used by the `cli`, `server`, `py`, and
//! `js` crates. See `docs/specs/` for per-module specifications.

pub mod chromium;
pub mod libreoffice;
pub mod pdfops;
pub mod types;

pub use chromium::{ChromiumEngine, Cookie, RequestContext};
pub use libreoffice::{LibreOfficeConfig, LibreOfficeEngine, OfficeOptions, PdfAProfile};
pub use pdfops::{
    Metadata, Position, SplitMode, WatermarkKind, WatermarkOptions, flatten, merge, read_metadata,
    rotate, split, watermark, write_metadata,
};
pub use types::{
    BrowserConfig, EngineError, EngineResult, Margins, MediaType, PageRange, PageRanges, PaperSize,
    PdfOptions, WaitCondition,
};
