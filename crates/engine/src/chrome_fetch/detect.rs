//! Detect a usable system Chrome / Chromium executable.

use std::path::PathBuf;

/// Returns the first existing Chrome executable found via env vars,
/// `$PATH`, and platform-default install paths. Returns `None` if none
/// of the candidates resolve.
pub fn detect_system_chrome() -> Option<PathBuf> {
    detect_with(
        std::env::var("BROWSER_PATH").ok().as_deref(),
        std::env::var("CHROME_PATH").ok().as_deref(),
        &|name| which::which(name).ok(),
        &|p: &std::path::Path| p.exists(),
    )
}

pub(crate) fn detect_with(
    browser_path_env: Option<&str>,
    chrome_path_env: Option<&str>,
    path_lookup: &dyn Fn(&str) -> Option<PathBuf>,
    exists: &dyn Fn(&std::path::Path) -> bool,
) -> Option<PathBuf> {
    for env in [browser_path_env, chrome_path_env].into_iter().flatten() {
        if !env.is_empty() {
            let p = PathBuf::from(env);
            if exists(&p) {
                return Some(p);
            }
        }
    }
    for name in ["chromium-browser", "chromium", "google-chrome", "chrome"] {
        if let Some(p) = path_lookup(name) {
            return Some(p);
        }
    }
    for candidate in platform_defaults() {
        let p = PathBuf::from(candidate);
        if exists(&p) {
            return Some(p);
        }
    }
    None
}

#[cfg(target_os = "macos")]
fn platform_defaults() -> &'static [&'static str] {
    &[
        "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
        "/Applications/Chromium.app/Contents/MacOS/Chromium",
    ]
}

#[cfg(target_os = "linux")]
fn platform_defaults() -> &'static [&'static str] {
    &[
        "/usr/bin/google-chrome",
        "/usr/bin/chromium",
        "/usr/bin/chromium-browser",
        "/snap/bin/chromium",
    ]
}

#[cfg(target_os = "windows")]
fn platform_defaults() -> &'static [&'static str] {
    &[
        r"C:\Program Files\Google\Chrome\Application\chrome.exe",
        r"C:\Program Files (x86)\Google\Chrome\Application\chrome.exe",
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn explicit_browser_path_wins_when_exists() {
        let result = detect_with(
            Some("/fake/chrome"),
            None,
            &|_| None,
            &|p: &Path| p == Path::new("/fake/chrome"),
        );
        assert_eq!(result, Some(PathBuf::from("/fake/chrome")));
    }

    #[test]
    fn falls_back_to_path_lookup() {
        let result = detect_with(
            None,
            None,
            &|name| if name == "chromium" { Some(PathBuf::from("/usr/bin/chromium")) } else { None },
            &|_| false,
        );
        assert_eq!(result, Some(PathBuf::from("/usr/bin/chromium")));
    }

    #[test]
    fn returns_none_when_nothing_found() {
        let result = detect_with(None, None, &|_| None, &|_| false);
        assert_eq!(result, None);
    }

    #[test]
    fn empty_env_var_is_skipped() {
        let result = detect_with(Some(""), None, &|_| None, &|_| false);
        assert_eq!(result, None);
    }
}
