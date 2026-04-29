//! Browser-launch and Chrome-executable discovery.
//!
//! Implements steps 1–4 of the *Launch flow* in
//! `docs/specs/11-engine-chromium.md`.

use std::path::{Path, PathBuf};

use chromiumoxide::Browser;
use chromiumoxide::browser::BrowserConfig as CxBrowserConfig;
// HeadlessMode removed in 0.9 - use arg() instead
use futures_util::StreamExt;

use crate::types::{BrowserConfig, EngineError, EngineResult};

use super::ChromiumEngine;

/// Resolve a Chrome / Chromium executable per the spec's discovery rules.
///
/// Order:
/// 1. `explicit` if `Some` — used as-is, even if it does not exist on
///    disk (this lets the caller drive a custom test path).
/// 2. `$BROWSER_PATH` if set and non-empty.
/// 3. `$PATH` lookups for `chromium`, `google-chrome`, `chrome`,
///    `chromium-browser`.
/// 4. Platform-typical install locations.
///
/// On failure returns [`EngineError::ChromeNotFound`] with the list of
/// paths that were searched, in order.
///
/// The `path_lookup` and `exists` closures are injected so unit tests
/// can drive resolution without touching the filesystem.
pub(crate) fn resolve_executable_with(
    explicit: Option<&Path>,
    env_var: Option<&str>,
    path_lookup: &dyn Fn(&str) -> Option<PathBuf>,
    exists: &dyn Fn(&Path) -> bool,
) -> EngineResult<PathBuf> {
    let mut searched: Vec<PathBuf> = Vec::new();

    if let Some(p) = explicit {
        return Ok(p.to_path_buf());
    }

    if let Some(env) = env_var
        && !env.is_empty()
    {
        let p = PathBuf::from(env);
        if exists(&p) {
            return Ok(p);
        }
        searched.push(p);
    }

    for name in PATH_BINARIES {
        if let Some(p) = path_lookup(name) {
            return Ok(p);
        }
        searched.push(PathBuf::from(name));
    }

    for candidate in platform_defaults() {
        let p = PathBuf::from(candidate);
        if exists(&p) {
            return Ok(p);
        }
        searched.push(p);
    }

    Err(EngineError::ChromeNotFound { searched })
}

/// Check Chrome version and warn if it's newer than what chromiumoxide supports.
/// chromiumoxide 0.9 supports Chrome up to ~135. Newer versions emit warnings
/// for unknown CDP events but PDF generation still works.
fn check_chrome_version(executable: &Path) {
    let output = std::process::Command::new(executable)
        .arg("--version")
        .output();

    if let Ok(output) = output {
        if output.status.success() {
            let version_str = String::from_utf8_lossy(&output.stdout);
            // Parse "Google Chrome 147.0.7727.102" -> 147
            if let Some(major) = version_str
                .split_whitespace()
                .last()
                .and_then(|v| v.split('.').next())
                .and_then(|m| m.parse::<u32>().ok())
            {
                const MAX_TESTED: u32 = 135;
                if major > MAX_TESTED {
                    tracing::warn!(
                        chrome_version = %version_str.trim(),
                        max_tested = MAX_TESTED,
                        "Chrome version newer than chromiumoxide supports. \
                         You may see 'WS Invalid message' warnings - PDF generation \
                         still works, but consider downgrading Chrome or waiting \
                         for chromiumoxide update."
                    );
                } else {
                    tracing::info!(chrome_version = %version_str.trim(), "Chrome version OK");
                }
            }
        }
    }
}

/// Public entrypoint used by `ChromiumEngine::launch_with`.
pub(crate) async fn launch_with(config: BrowserConfig) -> EngineResult<ChromiumEngine> {
    let executable = resolve_executable_with(
        config.executable.as_deref(),
        std::env::var("BROWSER_PATH").ok().as_deref(),
        &which_in_path,
        &Path::exists,
    )?;

    // Check Chrome version and warn if potentially incompatible
    check_chrome_version(&executable);

    let cx_config = build_chromiumoxide_config(&config, &executable)?;

    let (mut browser, mut handler) = Browser::launch(cx_config)
        .await
        .map_err(|e| EngineError::ChromeLaunch(e.to_string()))?;

    let handler_task = tokio::spawn(async move {
        // chromiumoxide's handler stream yields Result<(), CdpError>.
        // Many emitted errors are non-fatal (e.g. transient WS frames,
        // unknown event payloads); breaking on the first Err strands
        // every subsequent CDP request as `oneshot canceled`. Drive the
        // stream until it ends and only log errors.
        while let Some(item) = handler.next().await {
            if let Err(e) = item {
                tracing::debug!(error = %e, "chromiumoxide handler event error (ignored)");
            }
        }
    });

    // Capture the child-process PID so [`Inner::Drop`] can synchronously
    // SIGKILL Chrome if the engine is dropped without explicit shutdown
    // (e.g. test panic, early return).
    let chrome_pid = browser
        .get_mut_child()
        .and_then(|child| child.as_mut_inner().id());

    Ok(ChromiumEngine::from_parts(browser, handler_task, config, chrome_pid))
}

/// Translate our [`BrowserConfig`] into a chromiumoxide
/// [`CxBrowserConfig`].
fn build_chromiumoxide_config(
    config: &BrowserConfig,
    executable: &Path,
) -> EngineResult<CxBrowserConfig> {
    let mut builder = CxBrowserConfig::builder()
        .chrome_executable(executable)
        .request_timeout(config.timeout);

    if !config.headless {
        builder = builder.with_head();
    } else {
        // Default is headless; --headless=new set via baseline args
    }

    if config.no_sandbox {
        builder = builder.no_sandbox();
    }

    // Spec-mandated baseline flags. `--headless=new` is set via
    // `headless_mode` above; the rest are appended verbatim.
    for flag in BASELINE_ARGS {
        builder = builder.arg(*flag);
    }

    for extra in &config.extra_args {
        builder = builder.arg(extra.as_str());
    }

    builder
        .build()
        .map_err(|e| EngineError::ChromeLaunch(e.to_string()))
}

/// Default flags every launch passes to Chrome (in addition to
/// `--headless=new`, which is set via `HeadlessMode::New`).
const BASELINE_ARGS: &[&str] = &[
    "--disable-gpu",
    "--hide-scrollbars",
    "--mute-audio",
    "--disable-dev-shm-usage",
];

/// Names looked up on `$PATH`, in order.
const PATH_BINARIES: &[&str] = &[
    "chromium",
    "chromium-browser",
    "google-chrome",
    "google-chrome-stable",
    "chrome",
];

/// Platform-typical install locations consulted last.
fn platform_defaults() -> &'static [&'static str] {
    #[cfg(target_os = "macos")]
    {
        &[
            "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
            "/Applications/Chromium.app/Contents/MacOS/Chromium",
            "/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge",
        ]
    }
    #[cfg(target_os = "linux")]
    {
        &[
            "/usr/bin/google-chrome",
            "/usr/bin/google-chrome-stable",
            "/usr/bin/chromium",
            "/usr/bin/chromium-browser",
            "/snap/bin/chromium",
        ]
    }
    #[cfg(target_os = "windows")]
    {
        &[
            "C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe",
            "C:\\Program Files (x86)\\Google\\Chrome\\Application\\chrome.exe",
            "C:\\Program Files\\Chromium\\Application\\chrome.exe",
        ]
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
        &[]
    }
}

/// Manual `$PATH` walk equivalent to `which`. Returns `None` if not found.
fn which_in_path(binary: &str) -> Option<PathBuf> {
    let path_var = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path_var) {
        let candidate = dir.join(binary);
        if candidate.is_file() {
            return Some(candidate);
        }
        // Windows .exe variant; harmless on other platforms.
        let with_exe = dir.join(format!("{binary}.exe"));
        if with_exe.is_file() {
            return Some(with_exe);
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::path::PathBuf;

    // Helper: build an `exists` closure that returns true for a fixed
    // set of paths.
    fn exists_for(allowed: &'static [&'static str]) -> impl Fn(&Path) -> bool {
        move |p: &Path| allowed.iter().any(|a| Path::new(a) == p)
    }

    fn no_path_lookup(_: &str) -> Option<PathBuf> {
        None
    }

    #[test]
    fn executable_resolution_prefers_explicit() {
        let explicit = PathBuf::from("/custom/chrome");
        let resolved = resolve_executable_with(
            Some(&explicit),
            Some("/should/not/be/used"),
            &no_path_lookup,
            &|_| true,
        )
        .unwrap();
        assert_eq!(resolved, explicit);
    }

    #[test]
    fn executable_resolution_falls_back_to_env_var() {
        let env_path = "/opt/special/chrome";
        let resolved = resolve_executable_with(
            None,
            Some(env_path),
            &no_path_lookup,
            &exists_for(&["/opt/special/chrome"]),
        )
        .unwrap();
        assert_eq!(resolved, PathBuf::from(env_path));
    }

    #[test]
    fn executable_resolution_falls_back_to_path() {
        let calls: RefCell<Vec<String>> = RefCell::new(Vec::new());
        let path_lookup = |name: &str| -> Option<PathBuf> {
            calls.borrow_mut().push(name.to_string());
            if name == "google-chrome" {
                Some(PathBuf::from("/usr/local/bin/google-chrome"))
            } else {
                None
            }
        };
        let resolved = resolve_executable_with(None, None, &path_lookup, &|_| false).unwrap();
        assert_eq!(resolved, PathBuf::from("/usr/local/bin/google-chrome"));
        // We must have consulted at least one earlier name before
        // `google-chrome`.
        let made = calls.into_inner();
        assert!(
            made.iter().any(|n| n == "google-chrome"),
            "did not consult google-chrome: {made:?}"
        );
    }

    #[test]
    fn executable_resolution_emits_searched_list_on_failure() {
        let err = resolve_executable_with(None, Some("/nope/chrome"), &no_path_lookup, &|_| false)
            .unwrap_err();
        match err {
            EngineError::ChromeNotFound { searched } => {
                assert!(
                    searched.iter().any(|p| p == Path::new("/nope/chrome")),
                    "expected /nope/chrome in searched list, got {searched:?}"
                );
                assert!(
                    searched.iter().any(|p| p == Path::new("chromium")),
                    "expected $PATH names in searched list, got {searched:?}"
                );
                assert!(
                    !searched.is_empty(),
                    "searched list must include attempted paths"
                );
            }
            other => panic!("expected ChromeNotFound, got {other:?}"),
        }
    }

    #[test]
    fn executable_resolution_ignores_empty_env_var() {
        let path_lookup = |name: &str| {
            if name == "chromium" {
                Some(PathBuf::from("/usr/bin/chromium"))
            } else {
                None
            }
        };
        let resolved = resolve_executable_with(None, Some(""), &path_lookup, &|_| false).unwrap();
        assert_eq!(resolved, PathBuf::from("/usr/bin/chromium"));
    }
}
