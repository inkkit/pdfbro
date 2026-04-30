# URL-to-PDF Timeout Fix + Reliable Test

**Date:** 2026-04-30  
**Branch:** chrome

---

## Problem

`url_to_pdf` times out on real URLs because `navigate_with_lifecycle` waits for the `networkIdle` CDP lifecycle event by default. Real pages (analytics scripts, polling, websockets) never reach network idle, so every render hangs until the 60 s render timeout.

Setting `skip_network_idle: true` per-request fixes the hang, confirming the root cause. Chrome version was a red herring.

Gotenberg (Go reference implementation) skips networkIdle by default and instead waits for `DomContentLoaded + LoadEventFired`. Folio inverts this — a bad default.

---

## Design

### 1. Chrome flags (`crates/engine/src/chromium/launch.rs`)

Add two flags missing vs gotenberg to `BASELINE_ARGS`:

```rust
"--no-zygote",                // prevents zygote deadlock in Docker (gotenberg issue #1177)
"--font-render-hinting=none", // consistent PDF text rendering cross-platform
```

### 2. Remove `skip_network_idle` / `skip_network_almost_idle` from `RequestContext`

Both fields are removed entirely from `crates/engine/src/chromium/mod.rs`. No backward compat shim. All callers updated.

The server route parser in `crates/server/src/routes/chromium.rs` (lines 541–548) that reads these from HTTP headers is deleted.

### 3. Add `network_idle_timeout: Option<Duration>` to `BrowserConfig`

In `crates/engine/src/types.rs`:

```rust
pub struct BrowserConfig {
    // ...existing fields...
    /// When Some(t), race networkIdle against this timeout after load events fire.
    /// When None (default), skip networkIdle entirely — matches gotenberg default.
    #[serde(default, with = "humantime_serde")]
    pub network_idle_timeout: Option<Duration>,
}
```

Default: `None`.

### 4. Update `navigate_with_lifecycle` (`crates/engine/src/chromium/render.rs`)

Signature changes from `(skip_network_idle: bool, skip_network_almost_idle: bool)` to `(network_idle_timeout: Option<Duration>)`.

Behaviour:
- Always waits for `DomContentLoaded` + `LoadEventFired` (unchanged).
- `network_idle_timeout: None` → stops here. No networkIdle wait.
- `network_idle_timeout: Some(t)` → races `wait_lifecycle_event("networkIdle")` against `tokio::time::sleep(t)`. Whichever fires first wins. No error on timeout — just proceeds.

`networkAlmostIdle` is dropped entirely (it was redundant with networkIdle).

Call site in `render_url_on` passes `engine.inner().config.network_idle_timeout`.

### 5. Tests (`crates/engine/tests/chromium_html.rs`)

**Modified:** `cookies_and_headers_round_trip` — remove explicit `skip_network_idle: false` / `skip_network_almost_idle: false` from the `RequestContext` literal. Use struct update or field-by-field construction without those fields.

**New: `url_to_pdf_real_network_default`**
- Spawns local axum server serving a static HTML page (no background JS/polling).
- Calls `url_to_pdf` with default `RequestContext` and default `BrowserConfig` (no `network_idle_timeout`).
- Asserts PDF bytes returned, non-empty, valid PDF header (`%PDF`).
- Proves real URL conversion works without any skip flags.

**New: `url_to_pdf_network_idle_timeout_fallback`**
- Spawns local axum server serving a page with `setInterval(() => fetch('/ping'), 300)` — permanently "not idle".
- Uses `BrowserConfig { network_idle_timeout: Some(Duration::from_secs(2)), ..Default::default() }`.
- Asserts PDF returned within well under the 60s global timeout (test has its own 15s tokio timeout).
- Proves the fallback fires and doesn't block.

Both tests skip gracefully if Chrome not found (consistent with existing pattern).

---

## Files Changed

| File | Change |
|------|--------|
| `crates/engine/src/chromium/launch.rs` | Add 2 flags to `BASELINE_ARGS` |
| `crates/engine/src/chromium/mod.rs` | Remove 2 fields from `RequestContext`, update unit tests |
| `crates/engine/src/types.rs` | Add `network_idle_timeout` to `BrowserConfig` |
| `crates/engine/src/chromium/render.rs` | Rewrite `navigate_with_lifecycle` signature + body |
| `crates/server/src/routes/chromium.rs` | Delete skip_network header parsing |
| `crates/engine/tests/chromium_html.rs` | Fix existing test, add 2 new tests |
