# Spec 36 — Chromium Wait Conditions & Advanced Options

> Advanced Chromium wait conditions and request context options
> that Folio is missing compared to Gotenberg. These fields provide
> finer control over page loading and resource handling.

## Goal

Implement missing Chromium form fields that control wait behavior,
resource validation, and rendering options. These are critical for
production use cases where precise timing and error handling are required.

## Scope

**In:**

- `waitForSelector` - Wait for DOM element visibility
- `skipNetworkIdleEvent` - Skip network idle detection
- `skipNetworkAlmostIdleEvent` - Skip "almost idle" (≤2 connections)
- `waitWindowStatus` - Wait for `window.status` value
- `failOnResourceHttpStatusCodes` - Resource status code validation
- `ignoreResourceHttpStatusDomains` - Exclude domains from checks
- `failOnResourceLoadingFailed` - Fail on resource errors
- `failOnConsoleExceptions` - Fail on JS exceptions
- `omitBackground` - Transparent background rendering

**Out:**

- `failOnHttpStatusCodes` - Already implemented ✅
- `failOnConsoleExceptions` - Future: capture console.error() calls

## Form Fields

### Wait Conditions (Missing in Folio)

| Field | Type | Gotenberg Source | Description |
|-------|------|------------------|-------------|
| `waitForSelector` | string (CSS selector) | `pkg/modules/chromium/formfield.go:WaitForSelector` | Wait for element to be visible before rendering |
| `skipNetworkIdleEvent` | boolean | `pkg/modules/chromium/formfield.go:SkipNetworkIdleEvent` | Skip waiting for network idle (0 connections) |
| `skipNetworkAlmostIdleEvent` | boolean | `pkg/modules/chromium/formfield.go:SkipNetworkAlmostIdleEvent` | Skip "almost idle" (≤2 connections) |
| `waitWindowStatus` | string | `pkg/modules/chromium/formfield.go:WaitWindowStatus` | Wait for `window.status === value` |

### Resource Validation (Missing in Folio)

| Field | Type | Gotenberg Source | Description |
|-------|------|------------------|-------------|
| `failOnResourceHttpStatusCodes` | JSON array | `pkg/modules/chromium/formfield.go:FailOnResourceHttpStatusCodes` | HTTP status codes that fail the conversion |
| `ignoreResourceHttpStatusDomains` | JSON array | `pkg/modules/chromium/formfield.go:IgnoreResourceHttpStatusDomains` | Domains to exclude from status checks |
| `failOnResourceLoadingFailed` | boolean | `pkg/modules/chromium/formfield.go:FailOnResourceLoadingFailed` | Fail when any resource fails to load |
| `failOnConsoleExceptions` | boolean | `pkg/modules/chromium/formfield.go:FailOnConsoleExceptions` | Fail when `console.error()` is called |

### Rendering Options (Missing in Folio)

| Field | Type | Gotenberg Source | Description |
|-------|------|------------------|-------------|
| `omitBackground` | boolean | `pkg/modules/chromium/formfield.go:OmitBackground` | Omit background graphics (transparent background) |

## Implementation

### 1. Extend `PdfOptions` in `crates/engine/src/types.rs`

```rust
pub struct PdfOptions {
    // ... existing fields ...

    // Wait conditions
    pub wait_for_selector: Option<String>,
    pub skip_network_idle: bool,
    pub skip_network_almost_idle: bool,
    pub wait_window_status: Option<String>,

    // Resource validation
    pub fail_on_resource_http_status_codes: Vec<u16>,
    pub ignore_resource_http_status_domains: Vec<String>,
    pub fail_on_resource_loading_failed: bool,
    pub fail_on_console_exceptions: bool,

    // Rendering
    pub omit_background: bool,
}
```

### 2. Update Form Field Parsing in `crates/server/src/routes/chromium.rs`

```rust
// In parse_chromium_form function:
if let Some(selector) = form.get("waitForSelector") {
    opts.wait_for_selector = Some(selector.clone());
}

if let Some(val) = form.get("skipNetworkIdleEvent") {
    opts.skip_network_idle = val == "true";
}

if let Some(val) = form.get("skipNetworkAlmostIdleEvent") {
    opts.skip_network_almost_idle = val == "true";
}

if let Some(status) = form.get("waitWindowStatus") {
    opts.wait_window_status = Some(status.clone());
}

if let Some(codes) = form.get("failOnResourceHttpStatusCodes") {
    // Parse JSON array: [404, 500, 502]
    opts.fail_on_resource_http_status_codes = serde_json::from_str(codes)
        .map_err(|e| EngineError::InvalidOption(...))?;
}

if let Some(domains) = form.get("ignoreResourceHttpStatusDomains") {
    // Parse JSON array: ["cdn.example.com", "*.cloudfront.net"]
    opts.ignore_resource_http_status_domains = serde_json::from_str(domains)
        .map_err(|e| EngineError::InvalidOption(...))?;
}

if let Some(val) = form.get("failOnResourceLoadingFailed") {
    opts.fail_on_resource_loading_failed = val == "true";
}

if let Some(val) = form.get("failOnConsoleExceptions") {
    opts.fail_on_console_exceptions = val == "true";
}

if let Some(val) = form.get("omitBackground") {
    opts.omit_background = val == "true";
}
```

### 3. Implement in `ChromiumEngine` (`crates/engine/src/chromium/render.rs`)

#### Wait for Selector

```rust
use chromiumoxide::page::Page;

async fn wait_for_selector(page: &Page, selector: &str) -> Result<(), EngineError> {
    use chromiumoxide::cdp::browser_protocol::dom::*;

    // Wait for element to be visible
    let cmd = GetElementById {
        node_id: page.find_element(selector).await
            .map_err(|e| EngineError::Navigation(...))?
    };

    // Poll until visible or timeout
    let start = std::time::Instant::now();
    while start.elapsed() < Duration::from_secs(30) {
        if page.is_visible(selector).await.unwrap_or(false) {
            return Ok(());
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    Err(EngineError::Timeout(Duration::from_secs(30)))
}
```

#### Skip Network Idle Events

```rust
// In navigate_and_render:
if !opts.skip_network_idle {
    // Wait for network idle (0 connections)
    page.wait_for_network_idle().await?;
}

if !opts.skip_network_almost_idle {
    // Wait for "almost idle" (≤2 connections)
    wait_for_almost_idle(page).await?;
}
```

#### Wait for Window Status

```rust
async fn wait_for_window_status(page: &Page, status: &str) -> Result<(), EngineError> {
    let start = std::time::Instant::now();
    while start.elapsed() < Duration::from_secs(30) {
        let current: String = page.evaluate("window.status").await?;
        if current == status {
            return Ok(());
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    Err(EngineError::Timeout(Duration::from_secs(30)))
}
```

#### Resource Status Validation

```rust
use chromiumoxide::handler::network::{RequestPaused, ResponseReceived};

struct ResourceValidator {
    fail_codes: Vec<u16>,
    ignore_domains: Vec<String>,
    failed_resources: Vec<String>,
}

impl ResourceValidator {
    fn new(codes: Vec<u16>, domains: Vec<String>) -> Self {
        Self {
            fail_codes: codes,
            ignore_domains: domains,
            failed_resources: Vec::new(),
        }
    }

    fn check_response(&mut self, url: &str, status: u16) {
        if self.fail_codes.contains(&status) {
            if !self.should_ignore(url) {
                self.failed_resources.push(format!("{}: {}", url, status));
            }
        }
    }

    fn should_ignore(&self, url: &str) -> bool {
        self.ignore_domains.iter().any(|domain| url.contains(domain))
    }
}
```

#### Console Exceptions

```rust
use chromiumoxide::cdp::browser_protocol::runtime::ExceptionThrown;

fn enable_console_exception_detection(page: &Page) -> ConsoleExceptionDetector {
    let detector = ConsoleExceptionDetector::new();
    page.enable_runtime().await.unwrap();
    // Listen for ExceptionThrown events
    // If console.error() called, add to exceptions list
    detector
}
```

#### Omit Background

```rust
// In PDF printing options:
let mut print_opts = PrintToPdfParams::builder();

if opts.omit_background {
    print_opts.background_graphics(false);
}
```

## References to Gotenberg Source

| Feature | Gotenberg File | Line Numbers |
|---------|------------------|-------------|
| Form field definitions | `pkg/modules/chromium/formfield.go` | Full file |
| WaitForSelector handling | `pkg/modules/chromium/libreoffice.go` | ~L400-450 |
| Network idle logic | `pkg/modules/chromium/chromium.go` | ~L200-300 |
| Resource validation | `pkg/modules/chromium/chromium.go` | ~L300-400 |
| Window status wait | `pkg/modules/chromium/chromium.go` | ~L400-450 |
| Console exceptions | `pkg/modules/chromium/chromium.go` | ~L450-500 |
| Omit background | `pkg/modules/chromium/formfield.go` | ~L150-200 |

To read Gotenberg source:
```bash
cd /Users/__deesh_reddy__/projects/personal_git/rust_builds/folio/gotenberg
cat pkg/modules/chromium/formfield.go | grep -A5 "WaitForSelector"
```

## Expected Behavior

### `waitForSelector`
- Accept CSS selector string
- Wait until element is visible in DOM
- Timeout after 30s (configurable via `waitDelay`)
- Return error if element not found

### `skipNetworkIdleEvent`
- When `true`, don't wait for network idle (0 connections)
- Speeds up conversion for pages with persistent connections
- Default: `false` (wait for idle)

### `skipNetworkAlmostIdleEvent`
- When `true`, don't wait for "almost idle" (≤2 connections)
- Useful for pages with long-polling or websockets
- Default: `false`

### `waitWindowStatus`
- Wait for `window.status` to equal specified value
- Poll every 100ms with 30s timeout
- Useful for SPA frameworks that set status on render complete

### `failOnResourceHttpStatusCodes`
- Accept JSON array: `[404, 500, 502]`
- Check all subresource requests (images, scripts, XHR)
- Fail conversion if any resource matches
- Ignore domains in `ignoreResourceHttpStatusDomains`

### `ignoreResourceHttpStatusDomains`
- Accept JSON array: `["cdn.example.com", "*.cloudfront.net"]`
- Supports wildcard `*` prefix
- Case-insensitive domain matching

### `failOnResourceLoadingFailed`
- When `true`, fail if any resource fails to load (network error)
- Includes 4xx, 5xx, DNS failure, timeout, etc.
- Default: `false`

### `failOnConsoleExceptions`
- When `true`, fail if `console.error()` is called
- Captures exceptions thrown in `window.onerror`
- Useful for catching JS errors during render
- Default: `false`

### `omitBackground`
- When `true`, render with transparent background
- Sets `background-graphics: false` in print params
- Useful for overlaying PDF on other content
- Default: `false`

## Test Plan

### Unit Tests

- `parse_wait_for_selector_from_form`
- `parse_skip_network_idle_from_form`
- `parse_fail_on_resource_codes_json_array`
- `parse_ignore_domains_wildcard`
- `omit_background_sets_print_param`

### Integration Tests

- `wait_for_selector_success` - Element appears after JS render
- `wait_for_selector_timeout` - Element never appears
- `skip_network_idle_speeds_up_conversion`
- `fail_on_resource_404` - Image 404 fails conversion
- `ignore_domain_cdn` - CDN 404 ignored
- `fail_on_console_error` - JS error fails conversion
- `omit_background_transparent` - PDF has no background

### BDD Scenarios (Port from Gotenberg)

```gherkin
Scenario: Wait for selector before rendering
  Given Chromium is available
  When I POST to "/forms/chromium/convert/url" with:
    | url            | http://example.com/dynamic |
    | waitForSelector | #content               |
    | waitDelay      | 5s                      |
  Then I should receive a PDF
  And the PDF should contain "Dynamic Content"

Scenario: Fail on resource HTTP status
  Given Chromium is available
  When I POST to "/forms/chromium/convert/url" with:
    | url                        | http://example.com/broken |
    | failOnResourceHttpStatusCodes | [404, 500]                  |
  Then the response status code should be 502
  And the error code should be "NAVIGATION"
```

## Acceptance

- [ ] `PdfOptions` extended with all new fields
- [ ] Form field parsing in `chromium.rs` route handler
- [ ] `wait_for_selector` implemented in `ChromiumEngine`
- [ ] Network idle skip options implemented
- [ ] `wait_window_status` implemented
- [ ] Resource validation with domain ignore list
- [ ] Console exception detection
- [ ] `omit_background` sets print parameter
- [ ] Unit tests for all form field parsers
- [ ] Integration tests for each new feature
- [ ] BDD scenarios ported from Gotenberg
- [ ] `cargo clippy -p engine -- -D warnings` clean

## References

- Gotenberg form fields: `/Users/__deesh_reddy__/projects/personal_git/rust_builds/folio/gotenberg/pkg/modules/chromium/formfield.go`
- Gotenberg Chromium module: `/Users/__deesh_reddy__/projects/personal_git/rust_builds/folio/gotenberg/pkg/modules/chromium/`
- Chromiumoxide docs: https://docs.rs/chromiumoxide/
- Chrome DevTools Protocol: https://chromedevtools.github.io/
