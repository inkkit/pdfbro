#![deny(rust_2018_idioms, missing_docs)]
//! Folio engine — Chrome / LibreOffice / PDF backends behind a single Rust API.
//!
//! This crate hosts the engine layer used by the `cli`, `server`, `py`, and
//! `js` crates. See `docs/specs/` for per-module specifications.

pub mod bookmarks;
pub mod encrypt;
pub mod logging;
pub mod pdfa;
pub mod pdfops;
pub mod types;

#[cfg(feature = "chromium")]
pub mod chromium;
#[cfg(feature = "libreoffice")]
pub mod libreoffice;

pub use bookmarks::{Bookmark, read_bookmarks, write_bookmarks, flatten_bookmarks};
pub use encrypt::{EncryptionAlgorithm, Permissions, encrypt_pdf, decrypt_pdf, is_encrypted, qpdf_available as encrypt_qpdf_available};
pub use pdfa::{PdfAProfile, convert_to_pdfa, ghostscript_available, qpdf_available};
pub use pdfops::{
    Metadata, OptimiseBackend, OptimisePreset, OptimiseResult, Position, SplitMode, WatermarkKind,
    WatermarkOptions, flatten, merge, optimise_pdf, read_metadata, rotate, split, watermark,
    write_metadata,
};
pub use types::{
    EngineError, EngineResult, Margins, MediaType, PageRange, PageRanges, PaperSize,
    PdfOptions, WaitCondition,
};

#[cfg(feature = "chromium")]
pub use types::BrowserConfig;
#[cfg(feature = "chromium")]
pub use chromium::{ChromiumEngine, Cookie, RequestContext};
#[cfg(feature = "chromium")]
pub use chromium::screenshot::{ScreenshotFormat, CaptureMode, ScreenshotOptions};
#[cfg(feature = "libreoffice")]
pub use libreoffice::{LibreOfficeConfig, LibreOfficeEngine, OfficeOptions};
