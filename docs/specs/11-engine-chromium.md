# Spec 11 — `engine::chromium::ChromiumEngine`

> The Phase-1 MVP. Converts HTML / URL / Markdown to PDF via real Chrome
> through the Chrome DevTools Protocol.

## Goal

Provide a single `ChromiumEngine` type that reliably produces a PDF byte
stream from HTML strings, remote URLs, or Markdown — usable from binaries
(CLI, server) and bindings without any wrapper layer.

## Scope

**In:**

- Browser lifecycle (launch, reuse, shutdown).
- `html_to_pdf`, `url_to_pdf`, `markdown_to_pdf`.
- Wait conditions (load / domcontentloaded / networkidle / selector / expression / delay).
- All `PdfOptions` knobs from spec 10 mapped onto CDP `Page.printToPDF`.
- Cookies, extra HTTP headers, custom user agent (per-call).

**Out:**

- Connection pooling for HTTP server (spec 30 wraps this engine in a pool).
- Auto-download of Chrome (deferred — first cut requires a chrome on `$PATH`
  or in `BrowserConfig::executable`).
- Screenshot capture (separate follow-up spec).
- PDF/A / PDF/UA conformance (spec 13).

## Public API

Module path: `engine::chromium`, re-exported as `engine::ChromiumEngine`.

```rust
use crate::types::{BrowserConfig, EngineResult, PdfOptions};
use std::collections::HashMap;
use std::sync::Arc;

/// One Chromium browser instance shared across many concurrent renders.
/// Cheap to clone (`Arc` inside).
#[derive(Clone)]
pub struct ChromiumEngine {
    inner: Arc<Inner>, // private
}

impl ChromiumEngine {
    /// Launch a new browser with default config.
    pub async fn launch() -> EngineResult<Self>;

    /// Launch with explicit config (executable path, sandbox, timeout, ...).
    pub async fn launch_with(config: BrowserConfig) -> EngineResult<Self>;

    /// Render an HTML string to PDF bytes.
    /// `base_url`, when `Some`, is used as the document's base URL so that
    /// relative `<img>`, `<link>` etc. resolve against it.
    pub async fn html_to_pdf(
        &self,
        html: &str,
        base_url: Option<&str>,
        opts: &PdfOptions,
        request: &RequestContext,
    ) -> EngineResult<Vec<u8>>;

    /// Navigate to `url` and render to PDF bytes.
    pub async fn url_to_pdf(
        &self,
        url: &str,
        opts: &PdfOptions,
        request: &RequestContext,
    ) -> EngineResult<Vec<u8>>;

    /// Render Markdown to PDF. Implementation: render to HTML internally
    /// (CommonMark + tables + strikethrough + task lists) wrapped in a small
    /// stylesheet, then call `html_to_pdf`.
    pub async fn markdown_to_pdf(
        &self,
        markdown: &str,
        opts: &PdfOptions,
        request: &RequestContext,
    ) -> EngineResult<Vec<u8>>;

    /// Best-effort liveness probe — `true` iff the browser process responds
    /// to `Browser.getVersion` within `BrowserConfig::timeout`.
    pub async fn healthy(&self) -> bool;

    /// Close the browser. Idempotent. Future calls return
    /// `EngineError::Internal("engine shut down")`.
    pub async fn shutdown(self) -> EngineResult<()>;
}

/// Per-render request context. Always passed even when empty.
#[derive(Debug, Clone, Default)]
pub struct RequestContext {
    pub user_agent: Option<String>,
    pub extra_headers: HashMap<String, String>,
    pub cookies: Vec<Cookie>,
    /// HTTP statuses that should fail the render. Empty means no statuses fail.
    pub fail_on_status: Vec<u16>,
}

#[derive(Debug, Clone)]
pub struct Cookie {
    pub name: String,
    pub value: String,
    pub domain: Option<String>,
    pub path: Option<String>,
    pub secure: bool,
    pub http_only: bool,
}
```

## Behavior

### Launch flow

1. Resolve `BrowserConfig::executable`:
   1. If `Some(p)`, use it.
   2. Else, in order, check `$BROWSER_PATH`, `which chromium`, `which chrome`,
      and platform-typical defaults
      (`/Applications/Google Chrome.app/Contents/MacOS/Google Chrome`,
      `/usr/bin/google-chrome`, `/usr/bin/chromium`, etc.).
   3. If none → `EngineError::ChromeNotFound { searched }`.
2. Spawn Chrome with: `--headless=new`, `--disable-gpu`,
   `--hide-scrollbars`, `--mute-audio`, plus `--no-sandbox` iff
   `config.no_sandbox`, plus `config.extra_args`.
3. Connect via WebSocket using `chromiumoxide::Browser::launch`. On error
   → `EngineError::ChromeLaunch(msg)`.
4. Spawn a background task to drive the chromiumoxide handler future. Store
   its `JoinHandle` in `Inner` so `shutdown` can abort it.

### `html_to_pdf`

1. `opts.validate()?` (from spec 10).
2. Open a new page (`browser.new_page("about:blank")`).
3. Apply `RequestContext`:
   - If `user_agent.is_some()`, send `Network.setUserAgentOverride`.
   - If `!extra_headers.is_empty()`, send `Network.setExtraHTTPHeaders`.
   - For each cookie, send `Network.setCookie`.
4. If `base_url.is_some()`, navigate first to that URL with `wait = Load`,
   then call `Page.setDocumentContent` on the main frame to inject `html`.
   Otherwise, set the page content directly via `page.set_content(html)`.
5. Run `Emulation.setEmulatedMedia` with `"print"` or `"screen"` per
   `opts.emulate_media`.
6. Wait per `opts.wait` (see Wait Conditions).
7. Build CDP `Page.printToPDF` params from `opts` and call. The engine MUST
   handle paginated streaming responses (chromiumoxide returns a base64
   string by default; decode to `Vec<u8>`).
8. Close the page (best-effort; log errors but do not fail the render).
9. Return PDF bytes.

If any CDP call returns an error, map to:

- Network/connection close → `EngineError::Cdp(msg)`.
- Navigation failures (`net::ERR_*`) → `EngineError::Navigation`.
- A `tokio::time::timeout` of `BrowserConfig::timeout` wraps the entire
  render; on elapse → `EngineError::Timeout`.

### `url_to_pdf`

Same as `html_to_pdf` but step 4 becomes `page.goto(url)` and the
`base_url` parameter does not apply.

If `RequestContext::fail_on_status` is non-empty, listen for
`Network.responseReceived`; if the main frame's response status is in the
list → cancel and return `EngineError::Navigation`.

### `markdown_to_pdf`

1. Convert via `pulldown-cmark` with `Options::all()`.
2. Wrap in a built-in HTML template (`<html><head><meta charset>...
   <style>{default-css}</style></head><body>{rendered}</body></html>`).
3. Delegate to `html_to_pdf` with `base_url = None`.

The default stylesheet lives in `crates/engine/src/chromium/markdown.css`
and is `include_str!`'d. Minimum: readable typography, code-block
monospace, table borders.

### Wait conditions

| `WaitCondition`       | Implementation                                                                                 |
|-----------------------|------------------------------------------------------------------------------------------------|
| `Load`                | Already implicit after `set_content` / `goto`. No extra wait.                                  |
| `DomContentLoaded`    | Subscribe to `Page.domContentEventFired`. Resolve on first event.                              |
| `NetworkIdle`         | Subscribe to `Page.lifecycleEvent` and resolve on `name == "networkIdle"`.                     |
| `Selector { s }`      | Poll `Runtime.evaluate("!!document.querySelector(s)")` every 50ms until `true` or timeout.     |
| `Expression { e }`    | Same polling pattern but evaluating the user expression. Must coerce result to bool.           |
| `Delay { duration }`  | `tokio::time::sleep(duration)`.                                                                |

All wait paths are bounded by `BrowserConfig::timeout`.

### `Page.printToPDF` parameter mapping

```
landscape            <- opts.landscape
displayHeaderFooter  <- opts.header_template.is_some() || opts.footer_template.is_some()
headerTemplate       <- opts.header_template
footerTemplate       <- opts.footer_template
printBackground      <- opts.print_background
scale                <- opts.scale
paperWidth           <- opts.paper.width_in
paperHeight          <- opts.paper.height_in
marginTop            <- opts.margin.top
marginBottom         <- opts.margin.bottom
marginLeft           <- opts.margin.left
marginRight          <- opts.margin.right
pageRanges           <- opts.page_ranges.map(|r| r.to_string())
preferCSSPageSize    <- opts.prefer_css_page_size
transferMode         <- "ReturnAsBase64"
```

### Concurrency

`html_to_pdf` / `url_to_pdf` / `markdown_to_pdf` are safe to invoke from
many concurrent tasks against a single `ChromiumEngine`. Each call opens
its own page — there is no implicit serialization. Callers wanting
back-pressure should impose a `tokio::sync::Semaphore` upstream (the
server crate, spec 30, will).

## Errors

Reuses `EngineError` from spec 10. New error sources documented above:
`ChromeNotFound`, `ChromeLaunch`, `Cdp`, `Navigation`, `Timeout`. No new
variants needed.

## Edge cases

| Scenario                                            | Required behavior                                                              |
|-----------------------------------------------------|--------------------------------------------------------------------------------|
| HTML body empty string                              | Produce a single blank page; not an error.                                     |
| URL returns 5xx, `fail_on_status = [500..=599]`     | `EngineError::Navigation { reason: "status 503" }`.                            |
| URL is not http/https (e.g. `file://`)              | Allowed if Chrome accepts it; we do not pre-validate scheme.                   |
| `opts.scale = 3.0`                                  | Caught by `opts.validate()` → `EngineError::InvalidOption` before any CDP call.|
| `Selector` never matches before timeout             | `EngineError::Timeout`.                                                        |
| Engine cloned then dropped                          | Browser stays alive while *any* clone exists.                                  |
| `shutdown()` called while another render is running | Render returns `EngineError::Internal("engine shut down")`; shutdown succeeds. |
| Markdown contains raw `<script>`                    | Tag stripped by `pulldown-cmark` defaults; not executed.                       |
| Header template references `{date}` etc.            | Pass through verbatim; Chrome substitutes.                                     |

## Test plan

### Unit tests (`crates/engine/src/chromium/mod.rs`)

These do not need Chrome.

- `executable_resolution_prefers_explicit`.
- `executable_resolution_falls_back_to_path`.
- `executable_resolution_emits_searched_list_on_failure`.
- `printtopdf_params_built_from_pdfoptions` — assert exact CDP param map.
- `markdown_template_wraps_with_charset_meta`.

### Integration tests (`crates/engine/tests/chromium_html.rs`)

Marked `#[ignore]`; require `CHROME_PATH` env or system Chrome. Run via
`cargo test -p engine -- --ignored`.

- `html_to_pdf_returns_valid_pdf_bytes` — bytes start with `%PDF-` and
  load via `lopdf::Document::load_mem`.
- `html_to_pdf_respects_paper_size` — render 1in×1in page; check
  `MediaBox` in lopdf.
- `url_to_pdf_against_local_axum` — spin up a tiny axum server with
  `/index.html`, render, assert page count == 1.
- `wait_selector_completes_when_element_appears` — page injects element
  after 100ms via setTimeout; assert success.
- `wait_selector_times_out_when_missing` — assert `EngineError::Timeout`.
- `cookies_and_headers_round_trip` — local server echoes them back into
  the rendered HTML; assert echoes appear in PDF text (via lopdf text
  extraction).
- `concurrent_renders_do_not_deadlock` — spawn 8 tasks, all complete.
- `markdown_to_pdf_renders_table` — assert table cells appear in
  extracted text.
- `shutdown_cancels_in_flight_render` — assert in-flight render returns
  the documented internal error.

### Doc tests (`engine/src/chromium/mod.rs`)

Compile-only example showing the canonical usage from `@README.md:85-97`,
behind `#[cfg(doctest)]` `no_run`.

## Acceptance

- [ ] `crates/engine/src/chromium/mod.rs` exists with the full Public API.
- [ ] `chromiumoxide` and `pulldown-cmark` added to `crates/engine/Cargo.toml`
      via `workspace.dependencies`.
- [ ] All unit tests in *Test plan* pass with `cargo test -p engine`.
- [ ] All ignored integration tests pass locally with a system Chrome.
- [ ] No `unsafe`. No `panic!` outside test code.
- [ ] `cargo clippy -p engine -- -D warnings` clean.
- [ ] `ChromiumEngine` is `Send + Sync + Clone` (assert via `static_assertions`).
- [ ] `shutdown` is idempotent (test).

## Out of scope / follow-ups

- Screenshot routes (`/screenshot/*`) — separate spec.
- Auto-download of Chrome — feature flag `auto-download` once stable.
- PDF/A and PDF/UA — picked up in spec 13 + a Ghostscript-style post-pass.
- Browser pool (multiple Chrome processes) — picked up in spec 30 once
  benchmarks indicate need.
