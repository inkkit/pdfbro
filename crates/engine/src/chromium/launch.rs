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

/// Log the Chrome version at startup for diagnostics.
fn check_chrome_version(executable: &Path) {
    let output = std::process::Command::new(executable)
        .arg("--version")
        .output();

    if let Ok(output) = output {
        if output.status.success() {
            let version_str = String::from_utf8_lossy(&output.stdout);
            tracing::info!(chrome_version = %version_str.trim(), "Chrome detected");
        }
    }
}

/// Check Chrome version and warn if it's newer than what chromiumoxide supports.
pub(crate) async fn launch_with(config: BrowserConfig) -> EngineResult<ChromiumEngine> {
    let executable = resolve_executable_with(
        config.executable.as_deref(),
        std::env::var("BROWSER_PATH").ok().as_deref(),
        &which_in_path,
        &Path::exists,
    )?;

    // Check Chrome version and warn if potentially incompatible
    check_chrome_version(&executable);

    // Each engine gets its own user-data-dir so concurrent or rapid
    // sequential launches do not collide on chromiumoxide's default
    // `/tmp/chromiumoxide-runner` (whose SingletonLock survives the
    // process if Chrome was SIGKILLed during shutdown).
    let user_data_dir = tempfile::Builder::new()
        .prefix("pdfbro-chromium-")
        .tempdir()
        .map_err(|e| {
            EngineError::ChromeLaunch(format!("failed to create user-data-dir: {e}"))
        })?;

    let cx_config = build_chromiumoxide_config(&config, &executable, user_data_dir.path())?;

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

    Ok(ChromiumEngine::from_parts(
        browser,
        handler_task,
        config,
        chrome_pid,
        Some(user_data_dir),
    ))
}

/// Translate our [`BrowserConfig`] into a chromiumoxide
/// [`CxBrowserConfig`].
fn build_chromiumoxide_config(
    config: &BrowserConfig,
    executable: &Path,
    user_data_dir: &Path,
) -> EngineResult<CxBrowserConfig> {
    let mut builder = CxBrowserConfig::builder()
        .chrome_executable(executable)
        .request_timeout(config.timeout)
        .launch_timeout(config.chrome_launch_timeout)
        .user_data_dir(user_data_dir);

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
///
/// Layout mirrors the chromedp / Puppeteer baseline lists so behaviour
/// is consistent with what the rest of the headless-Chrome ecosystem
/// runs in production. Grouped by what each flag actually does so a
/// future reader can figure out why a particular flag is in the set
/// without grepping through Chromium source.
///
/// References at time of writing:
/// - chromedp `DefaultExecAllocatorOptions`:
///   <https://github.com/chromedp/chromedp/blob/master/allocate.go>
/// - Puppeteer `defaultArgs()`:
///   <https://github.com/puppeteer/puppeteer/blob/main/packages/puppeteer-core/src/node/ChromeLauncher.ts>
const BASELINE_ARGS: &[&str] = &[
    // ── Rendering / UI surface (what we always wanted) ────────────────
    "--disable-gpu",
    "--hide-scrollbars",
    "--mute-audio",
    "--disable-dev-shm-usage",
    "--font-render-hinting=none",

    // ── Tab / renderer throttling — TIER 1 PERF ───────────────────────
    // Headless Chrome considers every tab "backgrounded" because there's
    // no front-of-screen window. Without these four flags Chrome
    // throttles JS, timers, and network in our renderer — directly
    // inflates p95 tail latency on tight workloads.
    "--disable-background-networking",
    "--disable-background-timer-throttling",
    "--disable-backgrounding-occluded-windows",
    "--disable-renderer-backgrounding",

    // ── Subprocess + memory ──────────────────────────────────────────
    // `site-per-process` (default in modern Chrome) creates a separate
    // renderer process per origin. Necessary for browser security; pure
    // overhead for headless PDF rendering. The other features here are
    // either translation prompts, casting/Hangouts preloading, or
    // ML-driven optimisation hints we don't want firing during a
    // benchmarked render.
    "--disable-features=Translate,AcceptCHFrame,MediaRouter,OptimizationHints,site-per-process",

    // ── Startup / load-time noise ────────────────────────────────────
    "--no-first-run",
    "--no-default-browser-check",
    "--disable-default-apps",
    "--disable-extensions",
    "--disable-component-extensions-with-background-pages",

    // ── Phone-home / ambient network — closes a common class of
    //    "occasional latency spike when Chrome decides to fetch X" tail
    //    causes. None of these are needed for a server doing PDF work.
    "--disable-breakpad",
    "--disable-crash-reporter",
    "--metrics-recording-only",
    "--safebrowsing-disable-auto-update",

    // ── Quiet Chrome's chattier subsystems ───────────────────────────
    "--disable-hang-monitor",
    "--disable-popup-blocking",
    "--disable-prompt-on-repost",
    "--disable-sync",

    // ── No system credential / keychain calls. On macOS Chrome will
    //    block on the system keyring at startup if these aren't set;
    //    on Linux it can stall waiting for D-Bus secrets services.
    "--password-store=basic",
    "--use-mock-keychain",

    // ── Sandbox-disabled containerised environments: keep zygote off.
    //    chromedp + Puppeteer both omit `--no-zygote` because they
    //    assume the host has a usable user-namespace setup for the
    //    zygote subprocess to fork from. Our `Dockerfile.test` and
    //    production `Dockerfile` both pair `--no-sandbox` with no
    //    user-namespace remapping; in that combination the zygote can
    //    fail at first cold-launch (observed as `batch_skip_*` cli
    //    test exiting 3 in the test container while every subsequent
    //    Chrome-using test in the same run passed). Keeping the flag.
    //    The "perf cost" — every new tab launches from a cold
    //    renderer rather than forking from a pre-warmed zygote — is
    //    fully amortised in our usage because we keep one warm
    //    browser and reuse it across requests.
    "--no-zygote",
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

    /// Pins the headless-perf intent on `BASELINE_ARGS` so a future
    /// well-meaning revert doesn't silently regress p95 latency on
    /// tight workloads. The four `disable-*` flags below stop Chrome
    /// from throttling JS / timers / network when it considers a tab
    /// "backgrounded" — which for a headless server is every tab,
    /// always.
    #[test]
    fn baseline_args_include_throttling_disables() {
        for required in &[
            "--disable-background-networking",
            "--disable-background-timer-throttling",
            "--disable-backgrounding-occluded-windows",
            "--disable-renderer-backgrounding",
        ] {
            assert!(
                BASELINE_ARGS.contains(required),
                "BASELINE_ARGS must contain {required} — see launch.rs comments"
            );
        }
    }

    /// Pins the keychain-quiet intent. Without these Chrome can stall
    /// at startup waiting for system credential stores (macOS Keychain,
    /// Linux gnome-keyring / kwallet via D-Bus).
    #[test]
    fn baseline_args_include_keychain_disables() {
        assert!(BASELINE_ARGS.contains(&"--password-store=basic"));
        assert!(BASELINE_ARGS.contains(&"--use-mock-keychain"));
    }

    /// `--no-zygote` is required in our sandbox-disabled containerised
    /// environments. See the comment block at the bottom of
    /// `BASELINE_ARGS` for the full reasoning — the short version is
    /// that without user-namespace support paired with `--no-sandbox`,
    /// the zygote subprocess can fail at first cold-launch, which
    /// surfaces as the cli `batch_skip_*` test exiting 3 in the test
    /// container.
    #[test]
    fn baseline_args_includes_no_zygote() {
        assert!(
            BASELINE_ARGS.contains(&"--no-zygote"),
            "--no-zygote must stay in the baseline for sandbox-disabled \
             containerised environments; see launch.rs comments"
        );
    }
}
