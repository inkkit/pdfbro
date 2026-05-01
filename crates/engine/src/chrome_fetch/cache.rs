// Minimal placeholder so chrome_fetch compiles. Real impl in Task 3.
// TODO(task-3): replace with real cache implementation
use std::path::{Path, PathBuf};

/// Returns the platform cache directory for Chrome-for-Testing binaries.
pub fn cache_dir() -> PathBuf { PathBuf::new() }

/// Returns the cached Chrome binary path for the given version, or `None` if not cached.
pub fn cached_chrome(_cache: &Path, _version: &str) -> Option<PathBuf> { None }
