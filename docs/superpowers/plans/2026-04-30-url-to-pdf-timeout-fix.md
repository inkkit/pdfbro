# URL-to-PDF Timeout Fix Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix URL-to-PDF timing out on real URLs by removing the default networkIdle wait, adding missing Chrome stability flags, and adding a `network_idle_timeout` opt-in with a fallback race.

**Architecture:** Three coordinated changes: (1) add `--no-zygote` + `--font-render-hinting=none` to Chrome launch flags, (2) remove `skip_network_idle`/`skip_network_almost_idle` from `RequestContext` and replace with `network_idle_timeout: Option<Duration>` in `BrowserConfig`, (3) rewrite `navigate_with_lifecycle` to skip networkIdle by default or race it against the configured timeout.

**Tech Stack:** Rust, chromiumoxide (CDP), tokio (async), axum (test servers)

---

## File Map

| File | Change |
|------|--------|
| `crates/engine/src/chromium/launch.rs` | Add 2 flags to `BASELINE_ARGS` |
| `crates/engine/src/chromium/mod.rs` | Remove `skip_network_idle` + `skip_network_almost_idle` from `RequestContext`; remove 2 unit tests that test those fields |
| `crates/engine/src/types.rs` | Add `network_idle_timeout: Option<Duration>` to `BrowserConfig` |
| `crates/engine/src/chromium/render.rs` | Update `navigate_with_lifecycle` signature + body; update call site in `render_url_on` |
| `crates/server/src/routes/chromium.rs` | Delete `skipNetworkIdle` + `skipNetworkAlmostIdle` header parsing (lines 540–552) |
| `crates/engine/tests/chromium_html.rs` | Remove dead `skip_network_idle`/`skip_network_almost_idle` fields from `cookies_and_headers_round_trip`; add `url_to_pdf_real_network_default` and `url_to_pdf_network_idle_timeout_fallback` tests |

---

## Task 1: Add missing Chrome flags

**Files:**
- Modify: `crates/engine/src/chromium/launch.rs:187-192`

- [ ] **Step 1: Update `BASELINE_ARGS`**

Replace lines 187–192 in `crates/engine/src/chromium/launch.rs`:

```rust
const BASELINE_ARGS: &[&str] = &[
    "--disable-gpu",
    "--hide-scrollbars",
    "--mute-audio",
    "--disable-dev-shm-usage",
    "--no-zygote",
    "--font-render-hinting=none",
];
```

- [ ] **Step 2: Verify it compiles**

```bash
cargo check -p engine
```

Expected: no errors.

- [ ] **Step 3: Commit**

```bash
git add crates/engine/src/chromium/launch.rs
git commit -m "fix: add --no-zygote and --font-render-hinting=none to Chrome baseline flags"
```

---

## Task 2: Add `network_idle_timeout` to `BrowserConfig`

**Files:**
- Modify: `crates/engine/src/types.rs:442-477`

- [ ] **Step 1: Add field to struct and Default**

In `crates/engine/src/types.rs`, add `network_idle_timeout` to `BrowserConfig`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct BrowserConfig {
    pub executable: Option<PathBuf>,
    pub headless: bool,
    pub extra_args: Vec<String>,
    pub no_sandbox: bool,
    #[serde(with = "humantime_serde")]
    pub timeout: Duration,
    pub lazy_start: bool,
    #[serde(with = "humantime_serde")]
    pub idle_shutdown_timeout: Option<Duration>,
    /// When `Some(t)`, after `load` events fire, race `networkIdle` against
    /// this timeout — whichever fires first wins. When `None` (default),
    /// networkIdle is skipped entirely, matching gotenberg's default.
    #[serde(with = "humantime_serde")]
    pub network_idle_timeout: Option<Duration>,
}

impl Default for BrowserConfig {
    fn default() -> Self {
        Self {
            executable: None,
            headless: true,
            extra_args: Vec::new(),
            no_sandbox: cfg!(target_os = "linux"),
            timeout: Duration::from_secs(60),
            lazy_start: false,
            idle_shutdown_timeout: None,
            network_idle_timeout: None,
        }
    }
}
```

- [ ] **Step 2: Verify it compiles**

```bash
cargo check -p engine
```

Expected: no errors.

- [ ] **Step 3: Commit**

```bash
git add crates/engine/src/types.rs
git commit -m "feat: add network_idle_timeout to BrowserConfig"
```

---

## Task 3: Remove `skip_network_idle`/`skip_network_almost_idle` from `RequestContext`

**Files:**
- Modify: `crates/engine/src/chromium/mod.rs:85-110` (struct) and `455-490` (unit tests)

- [ ] **Step 1: Remove the two fields from `RequestContext`**

In `crates/engine/src/chromium/mod.rs`, the struct definition currently has:

```rust
pub struct RequestContext {
    pub user_agent: Option<String>,
    pub extra_headers: HashMap<String, String>,
    pub cookies: Vec<Cookie>,
    pub fail_on_status: Vec<u16>,
    pub fail_on_resource_status: Vec<u16>,
    pub fail_on_console_exceptions: bool,
    pub fail_on_resource_loading_failed: bool,
    pub skip_network_idle: bool,
    pub skip_network_almost_idle: bool,
}
```

Replace it with (remove the last two fields):

```rust
pub struct RequestContext {
    pub user_agent: Option<String>,
    pub extra_headers: HashMap<String, String>,
    pub cookies: Vec<Cookie>,
    pub fail_on_status: Vec<u16>,
    pub fail_on_resource_status: Vec<u16>,
    pub fail_on_console_exceptions: bool,
    pub fail_on_resource_loading_failed: bool,
}
```

Also remove the two assertions in `request_context_default_values` unit test (lines 462–463):
```rust
// Remove these two lines:
assert!(!ctx.skip_network_idle);
assert!(!ctx.skip_network_almost_idle);
```

And delete the entire `request_context_with_skip_options` unit test (lines 481–490):
```rust
// Delete this entire test:
#[test]
fn request_context_with_skip_options() {
    let ctx = RequestContext {
        skip_network_idle: true,
        skip_network_almost_idle: true,
        ..RequestContext::default()
    };
    assert!(ctx.skip_network_idle);
    assert!(ctx.skip_network_almost_idle);
}
```

- [ ] **Step 2: Check compile errors to find all callers**

```bash
cargo check --workspace 2>&1 | grep "error\[" | head -30
```

Expected: errors in `render.rs` (call site), `chromium.rs` (server route), and `chromium_html.rs` (engine test). Fix each in subsequent steps.

- [ ] **Step 3: Commit the struct change**

```bash
git add crates/engine/src/chromium/mod.rs
git commit -m "refactor: remove skip_network_idle/skip_network_almost_idle from RequestContext"
```

---

## Task 4: Rewrite `navigate_with_lifecycle` in render.rs

**Files:**
- Modify: `crates/engine/src/chromium/render.rs:157` (call site) and `451-518` (function + helper)

- [ ] **Step 1: Update the call site in `render_url_on`**

At line 157 in `crates/engine/src/chromium/render.rs`, replace:

```rust
navigate_with_lifecycle(page, url, request.skip_network_idle, request.skip_network_almost_idle)
    .await
    .map_err(|e| navigation_error(url, e))?;
```

With:

```rust
navigate_with_lifecycle(page, url, engine.inner().config.network_idle_timeout)
    .await
    .map_err(|e| navigation_error(url, e))?;
```

- [ ] **Step 2: Rewrite `navigate_with_lifecycle` signature and body**

Replace the entire function at lines 448–498 of `crates/engine/src/chromium/render.rs`:

```rust
/// Navigate to URL and wait for lifecycle events.
///
/// Always waits for `domContentLoaded` and `load`.
/// If `network_idle_timeout` is `Some(t)`, races `networkIdle` against `t`
/// after load — proceeds whichever fires first.
/// If `None`, skips networkIdle entirely (default, matches gotenberg).
async fn navigate_with_lifecycle(
    page: &Page,
    url: &str,
    network_idle_timeout: Option<std::time::Duration>,
) -> Result<(), chromiumoxide::error::CdpError> {
    debug!("navigate_with_lifecycle: registering event listeners");
    let mut dom_content_events = page.event_listener::<EventDomContentEventFired>().await?;
    let mut load_events = page.event_listener::<EventLoadEventFired>().await?;
    debug!("navigate_with_lifecycle: listeners registered");

    debug!("navigate_with_lifecycle: calling page.goto({})", url);
    page.goto(url).await?;
    debug!("navigate_with_lifecycle: page.goto returned");

    debug!("navigate_with_lifecycle: waiting for domContentLoaded and load events");
    let dom_fut = async {
        dom_content_events.next().await;
        debug!("navigate_with_lifecycle: domContentLoaded received");
    };
    let load_fut = async {
        load_events.next().await;
        debug!("navigate_with_lifecycle: load event received");
    };
    tokio::join!(dom_fut, load_fut);
    debug!("navigate_with_lifecycle: domContentLoaded and load done");

    if let Some(timeout) = network_idle_timeout {
        debug!("navigate_with_lifecycle: racing networkIdle against {:?}", timeout);
        tokio::select! {
            _ = wait_lifecycle_event(page, "networkIdle") => {
                debug!("navigate_with_lifecycle: networkIdle fired");
            }
            _ = tokio::time::sleep(timeout) => {
                debug!("navigate_with_lifecycle: networkIdle timeout, proceeding");
            }
        }
    }

    debug!("navigate_with_lifecycle: complete");
    Ok(())
}
```

- [ ] **Step 3: Verify it compiles**

```bash
cargo check -p engine
```

Expected: no errors in the engine crate. (Server/test errors are expected — fixed in later tasks.)

- [ ] **Step 4: Commit**

```bash
git add crates/engine/src/chromium/render.rs
git commit -m "feat: rewrite navigate_with_lifecycle to skip networkIdle by default, add opt-in timeout race"
```

---

## Task 5: Remove dead header parsing from server route

**Files:**
- Modify: `crates/server/src/routes/chromium.rs:540-552`

- [ ] **Step 1: Delete the two `skipNetworkIdle`/`skipNetworkAlmostIdle` parse blocks**

In `crates/server/src/routes/chromium.rs`, remove lines 540–552:

```rust
// Delete this entire block:
if let Some(s) = nonempty(map, "skipNetworkIdle") {
    ctx.skip_network_idle = s.parse::<bool>().map_err(|e| ApiError::InvalidField {
        field: "skipNetworkIdle",
        message: e.to_string(),
    })?;
}

if let Some(s) = nonempty(map, "skipNetworkAlmostIdle") {
    ctx.skip_network_almost_idle = s.parse::<bool>().map_err(|e| ApiError::InvalidField {
        field: "skipNetworkAlmostIdle",
        message: e.to_string(),
    })?;
}
```

- [ ] **Step 2: Verify it compiles**

```bash
cargo check -p server
```

Expected: no errors.

- [ ] **Step 3: Commit**

```bash
git add crates/server/src/routes/chromium.rs
git commit -m "refactor: remove skipNetworkIdle/skipNetworkAlmostIdle HTTP header parsing"
```

---

## Task 6: Fix `cookies_and_headers_round_trip` test

**Files:**
- Modify: `crates/engine/tests/chromium_html.rs:311-328`

- [ ] **Step 1: Remove the dead fields from the `RequestContext` literal**

In `crates/engine/tests/chromium_html.rs`, the `RequestContext` at lines 311–328 currently is:

```rust
let request = RequestContext {
    user_agent: Some("FolioTest/1.0".into()),
    extra_headers,
    cookies: vec![Cookie { ... }],
    fail_on_status: vec![],
    fail_on_resource_status: vec![],
    fail_on_console_exceptions: false,
    fail_on_resource_loading_failed: false,
    skip_network_idle: false,
    skip_network_almost_idle: false,
};
```

Replace with (remove last two lines):

```rust
let request = RequestContext {
    user_agent: Some("FolioTest/1.0".into()),
    extra_headers,
    cookies: vec![Cookie {
        name: "session".into(),
        value: "abc123".into(),
        domain: Some(addr.ip().to_string()),
        path: Some("/".into()),
        secure: false,
        http_only: false,
    }],
    fail_on_status: vec![],
    fail_on_resource_status: vec![],
    fail_on_console_exceptions: false,
    fail_on_resource_loading_failed: false,
};
```

- [ ] **Step 2: Compile check**

```bash
cargo check --tests -p engine
```

Expected: no errors.

- [ ] **Step 3: Commit**

```bash
git add crates/engine/tests/chromium_html.rs
git commit -m "fix: remove deleted skip_network fields from cookies_and_headers_round_trip test"
```

---

## Task 7: Add `url_to_pdf_real_network_default` test

**Files:**
- Modify: `crates/engine/tests/chromium_html.rs` (append after existing `url_to_pdf_against_local_axum` test ~line 200)

- [ ] **Step 1: Add the test**

Add this test after `url_to_pdf_against_local_axum` in `crates/engine/tests/chromium_html.rs`:

```rust
#[tokio::test]
async fn url_to_pdf_real_network_default() {
    // Proves that URL-to-PDF works with default BrowserConfig (no networkIdle wait).
    // This was the main regression: the old default waited for networkIdle, which
    // never fired for most real pages, causing a 60s timeout.
    let router = Router::new().route(
        "/page",
        get(|| async {
            Html("<!doctype html><html><body><h1>real network default</h1></body></html>")
        }),
    );
    let (addr, shutdown) = spawn_server(router).await;

    let Some(engine) = launch_engine().await else { return; };

    let bytes = engine
        .url_to_pdf(
            &format!("http://{addr}/page"),
            &PdfOptions::default(),
            &RequestContext::default(),
        )
        .await
        .expect("url_to_pdf timed out or failed — default must not wait for networkIdle");

    assert!(bytes.starts_with(b"%PDF"), "output is not a PDF");
    assert!(bytes.len() > 1024, "PDF suspiciously small: {} bytes", bytes.len());

    engine.shutdown().await.ok();
    let _ = shutdown.send(());
}
```

- [ ] **Step 2: Compile check**

```bash
cargo check --tests -p engine
```

Expected: no errors.

- [ ] **Step 3: Commit**

```bash
git add crates/engine/tests/chromium_html.rs
git commit -m "test: add url_to_pdf_real_network_default to prove default no longer waits for networkIdle"
```

---

## Task 8: Add `url_to_pdf_network_idle_timeout_fallback` test

**Files:**
- Modify: `crates/engine/tests/chromium_html.rs` (append after previous new test)

- [ ] **Step 1: Add the test**

Add this test after `url_to_pdf_real_network_default`:

```rust
#[tokio::test(flavor = "multi_thread")]
async fn url_to_pdf_network_idle_timeout_fallback() {
    // A page that constantly polls /ping — networkIdle will never fire.
    // With network_idle_timeout: Some(2s), the race should time out and
    // still return a PDF rather than blocking until the 60s render timeout.
    let router = Router::new()
        .route(
            "/busy",
            get(|| async {
                Html(r#"<!doctype html><html><body>
                    <h1>busy page</h1>
                    <script>setInterval(() => fetch('/ping'), 100);</script>
                </body></html>"#)
            }),
        )
        .route("/ping", get(|| async { "pong" }));
    let (addr, shutdown) = spawn_server(router).await;

    let cfg = BrowserConfig {
        network_idle_timeout: Some(Duration::from_secs(2)),
        executable: std::env::var("CHROME_PATH").ok().map(PathBuf::from),
        ..BrowserConfig::default()
    };
    let engine = match ChromiumEngine::launch_with(cfg).await {
        Ok(e) => e,
        Err(e) => {
            eprintln!("skipping: failed to launch Chrome: {e}");
            let _ = shutdown.send(());
            return;
        }
    };

    // The whole conversion must complete well under the 60s render timeout.
    let result = tokio::time::timeout(
        Duration::from_secs(15),
        engine.url_to_pdf(
            &format!("http://{addr}/busy"),
            &PdfOptions::default(),
            &RequestContext::default(),
        ),
    )
    .await;

    let bytes = result
        .expect("timed out after 15s — network_idle_timeout fallback did not fire")
        .expect("render failed");

    assert!(bytes.starts_with(b"%PDF"), "output is not a PDF");

    engine.shutdown().await.ok();
    let _ = shutdown.send(());
}
```

- [ ] **Step 2: Compile check**

```bash
cargo check --tests -p engine
```

Expected: no errors.

- [ ] **Step 3: Compile entire workspace cleanly**

```bash
cargo check --workspace
```

Expected: zero errors, zero warnings (or only pre-existing warnings unrelated to this change).

- [ ] **Step 4: Commit**

```bash
git add crates/engine/tests/chromium_html.rs
git commit -m "test: add url_to_pdf_network_idle_timeout_fallback for opt-in networkIdle race"
```

---

## Task 9: Run tests

- [ ] **Step 1: Run all non-Chrome unit tests**

```bash
cargo test --workspace --lib
```

Expected: all pass.

- [ ] **Step 2: Run Chrome integration tests (requires Chrome)**

```bash
CHROME_PATH=/usr/bin/chromium cargo test -p engine --test chromium_html -- --nocapture 2>&1 | tail -30
```

Expected: `url_to_pdf_against_local_axum`, `url_to_pdf_real_network_default`, `cookies_and_headers_round_trip`, and `url_to_pdf_network_idle_timeout_fallback` all pass. Tests that can't find Chrome print "skipping:" and exit 0.

- [ ] **Step 3: Final commit if any test-only tweaks were needed**

```bash
git add -p
git commit -m "fix: address any issues found during test run"
```
