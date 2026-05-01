//! Platform cache directory for downloaded Chrome builds.

use std::path::{Path, PathBuf};

/// Default cache root for Folio's downloaded Chrome.
///
/// - macOS: `~/Library/Caches/folio/chromium`
/// - Linux: `$XDG_CACHE_HOME/folio/chromium` (falls back to `~/.cache`)
/// - Windows: `%LOCALAPPDATA%\folio\chromium`
///
/// Override via `FOLIO_CHROME_CACHE` env var; constructor argument wins
/// over both.
pub fn cache_dir() -> PathBuf {
    if let Ok(env) = std::env::var("FOLIO_CHROME_CACHE") {
        if !env.is_empty() {
            return PathBuf::from(env);
        }
    }
    let base = dirs::cache_dir().unwrap_or_else(|| PathBuf::from("."));
    base.join("folio").join("chromium")
}

/// Returns `Some(path)` if a cached Chrome for `version` exists and the
/// executable is present.
pub fn cached_chrome(cache: &Path, version: &str) -> Option<PathBuf> {
    let exe = chrome_exe_path(&cache.join(version));
    if exe.exists() { Some(exe) } else { None }
}

/// Path to the Chrome executable inside an extracted Chrome-for-Testing
/// distribution rooted at `dist`.
pub(crate) fn chrome_exe_path(dist: &Path) -> PathBuf {
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    {
        dist.join("chrome-mac-arm64").join("Google Chrome for Testing.app")
            .join("Contents/MacOS/Google Chrome for Testing")
    }
    #[cfg(all(target_os = "macos", target_arch = "x86_64"))]
    {
        dist.join("chrome-mac-x64").join("Google Chrome for Testing.app")
            .join("Contents/MacOS/Google Chrome for Testing")
    }
    #[cfg(target_os = "linux")]
    {
        dist.join("chrome-linux64").join("chrome")
    }
    #[cfg(target_os = "windows")]
    {
        dist.join("chrome-win64").join("chrome.exe")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_dir_respects_env_override() {
        // SAFETY: test mutates env in a non-overlapping way.
        unsafe { std::env::set_var("FOLIO_CHROME_CACHE", "/tmp/folio-test-cache"); }
        assert_eq!(cache_dir(), PathBuf::from("/tmp/folio-test-cache"));
        // SAFETY: justified above.
        unsafe { std::env::remove_var("FOLIO_CHROME_CACHE"); }
    }

    #[test]
    fn cached_chrome_none_when_dir_missing() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(cached_chrome(tmp.path(), "999.0.0.0").is_none());
    }
}
