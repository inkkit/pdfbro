#![deny(rust_2018_idioms, missing_docs)]
//! Folio engine — Chrome / LibreOffice / PDF backends behind a single Rust API.
//!
//! This crate hosts the engine layer used by the `cli`, `server`, `py`, and
//! `js` crates. See `docs/specs/` for per-module specifications.

pub mod bookmarks;
pub mod chromium;
pub mod encrypt;
pub mod libreoffice;
pub mod logging;
pub mod pdfa;
pub mod pdfops;
pub mod types;

pub use bookmarks::{Bookmark, read_bookmarks, write_bookmarks, flatten_bookmarks};
pub use chromium::{ChromiumEngine, Cookie, RequestContext};
pub use chromium::screenshot::{ScreenshotFormat, CaptureMode, ScreenshotOptions};
pub use libreoffice::{LibreOfficeConfig, LibreOfficeEngine, OfficeOptions};
pub use encrypt::{EncryptionAlgorithm, Permissions, encrypt_pdf, decrypt_pdf, is_encrypted, qpdf_available as encrypt_qpdf_available};
pub use pdfa::{PdfAProfile, convert_to_pdfa, ghostscript_available, qpdf_available};
pub use pdfops::{
    Metadata, Position, SplitMode, WatermarkKind, WatermarkOptions, flatten, merge, read_metadata,
    rotate, split, watermark, write_metadata,
};
pub use types::{
    BrowserConfig, EngineError, EngineResult, Margins, MediaType, PageRange, PageRanges, PaperSize,
    PdfOptions, WaitCondition,
};
