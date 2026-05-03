//! Detect and download Chrome / Chromium for embedded use.
//!
//! Bindings (`crates/py`, `crates/js`) call [`ensure_chrome`] which
//! returns a path to a usable Chrome executable, downloading a pinned
//! Chrome-for-Testing build into a platform cache directory if no system
//! Chrome is available.
//!
//! See `docs/superpowers/specs/2026-05-01-bindings-design.md` §
//! "Chrome auto-download" for the contract.

#![cfg(feature = "chrome-fetch")]

mod detect;
mod download;
mod cache;

pub use detect::detect_system_chrome;
pub use download::{download_chrome, ChromeFetchError};
pub use cache::{cache_dir, cached_chrome};

use std::path::PathBuf;

/// Pinned Chrome-for-Testing version. Bumped per pdfbro release.
/// Single source of truth: `bindings/CHROME_VERSION` mirrors this string.
pub const CHROME_VERSION: &str = include_str!("../../../../bindings/CHROME_VERSION");

/// Options controlling [`ensure_chrome`].
#[derive(Debug, Clone)]
pub struct EnsureOptions {
    /// Explicit path to a Chrome executable; skips all detection if set.
    pub explicit: Option<PathBuf>,
    /// Override the platform cache directory used for downloaded binaries.
    pub cache_dir: Option<PathBuf>,
    /// When `true`, download Chrome automatically if no system Chrome is found.
    pub auto_download: bool,
}

impl Default for EnsureOptions {
    fn default() -> Self {
        Self { explicit: None, cache_dir: None, auto_download: true }
    }
}

/// Returns a path to a usable Chrome.
pub async fn ensure_chrome(opts: &EnsureOptions) -> Result<PathBuf, ChromeFetchError> {
    if let Some(p) = &opts.explicit {
        return Ok(p.clone());
    }
    if let Some(p) = detect_system_chrome() {
        return Ok(p);
    }
    let cache = opts.cache_dir.clone().unwrap_or_else(cache_dir);
    if let Some(p) = cached_chrome(&cache, CHROME_VERSION.trim()) {
        return Ok(p);
    }
    if !opts.auto_download {
        return Err(ChromeFetchError::NotFoundAndDownloadDisabled);
    }
    download_chrome(&cache, CHROME_VERSION.trim()).await
}
