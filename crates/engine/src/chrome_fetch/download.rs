// Minimal placeholder so chrome_fetch compiles. Real impl in Task 4.
// TODO(task-4): replace with real download implementation
use std::path::{Path, PathBuf};
use thiserror::Error;

/// Errors that can occur when fetching Chrome.
#[derive(Debug, Error)]
pub enum ChromeFetchError {
    /// System Chrome was not found and `auto_download` was disabled.
    #[error("system Chrome not found and auto_download disabled")]
    NotFoundAndDownloadDisabled,
}

/// Downloads the pinned Chrome-for-Testing build into `cache_root`.
pub async fn download_chrome(_cache_root: &Path, _version: &str) -> Result<PathBuf, ChromeFetchError> {
    Err(ChromeFetchError::NotFoundAndDownloadDisabled)
}
