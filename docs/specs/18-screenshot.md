# Spec 18 — Chromium Screenshot API

> Capture web page screenshots as PNG or JPEG images.
> Alternative to PDF generation for image output.

## Goal

Provide screenshot capabilities using Chromium to capture web pages as
PNG or JPEG images. Mirrors Gotenberg's screenshot endpoints while
integrating with our existing Chromium infrastructure.

## Scope

**In:**

- Screenshot from HTML string or URL.
- PNG and JPEG output formats.
- Full page or viewport-only capture.
- Window/clipping size configuration.
- Wait conditions (load, networkidle).
- Custom headers, cookies, authentication.

**Out:**

- PDF screenshots (use convert endpoints).
- Video recording.
- Mobile device emulation (follow-up).
- Element-level screenshots (single element only).

## Public API

Module path: `engine::chromium::screenshot`. Extends existing ChromiumEngine.

```rust
use crate::types::{EngineError, EngineResult, BrowserConfig};

/// Screenshot format.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScreenshotFormat {
    Png,
    Jpeg { quality: u8 }, // 0-100
}

/// Screenshot capture mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptureMode {
    /// Capture visible viewport only.
    Viewport,
    /// Capture full page (scroll and stitch).
    FullPage,
}

/// Screenshot options.
#[derive(Debug, Clone)]
pub struct ScreenshotOptions {
    /// Output format.
    pub format: ScreenshotFormat,
    /// Capture mode.
    pub mode: CaptureMode,
    /// Viewport width in pixels.
    pub width: u32,
    /// Viewport height in pixels.
    pub height: u32,
    /// Device scale factor (1.0 = standard, 2.0 = retina).
    pub device_scale_factor: f32,
    /// Wait condition before capture.
    pub wait_condition: WaitCondition,
    /// Custom HTTP headers.
    pub extra_headers: HashMap<String, String>,
    /// Cookies to set.
    pub cookies: Vec<Cookie>,
    /// Background CSS (e.g., "white" for opaque).
    pub background_color: Option<String>,
}

impl Default for ScreenshotOptions {
    fn default() -> Self {
        Self {
            format: ScreenshotFormat::Png,
            mode: CaptureMode::Viewport,
            width: 1920,
            height: 1080,
            device_scale_factor: 1.0,
            wait_condition: WaitCondition::Load,
            extra_headers: HashMap::new(),
            cookies: Vec::new(),
            background_color: None,
        }
    }
}

/// Screenshot from HTML string.
pub async fn screenshot_html(
    engine: &ChromiumEngine,
    html: &str,
    opts: &ScreenshotOptions,
) -> EngineResult<Vec<u8>>;

/// Screenshot from URL.
pub async fn screenshot_url(
    engine: &ChromiumEngine,
    url: &str,
    opts: &ScreenshotOptions,
) -> EngineResult<Vec<u8>>;

/// Screenshot from Markdown.
pub async fn screenshot_markdown(
    engine: &ChromiumEngine,
    markdown: &str,
    opts: &ScreenshotOptions,
) -> EngineResult<Vec<u8>> {
    let html = render_markdown_to_html(markdown);
    screenshot_html(engine, &html, opts).await
}
```

## Implementation Strategy

### Using `chromiumoxide`

The `chromiumoxide` crate provides CDP (Chrome DevTools Protocol) access.
Screenshot capture uses the `Page.captureScreenshot` CDP command.

For full page screenshots:
1. Get full page dimensions via `Page.getLayoutMetrics()`
2. Set viewport to full page size
3. Capture screenshot
4. Restore viewport

For viewport screenshots:
1. Set requested viewport size
2. Navigate and wait
3. Capture screenshot

### CDP Commands

```rust
// Set viewport
Page::set_viewport(
    width, height, device_scale_factor, mobile, fit_window
).await?;

// Navigate and wait
Page::goto(url).await?;
Page::wait_for(selector_or_condition).await?;

// Capture
let screenshot = Page::capture_screenshot(
    format, // "png" or "jpeg"
    quality, // for jpeg
    clip, // optional viewport clipping
    from_surface, // true
).await?;
```

## Server API

### Endpoints

```
POST /forms/chromium/screenshot/html
POST /forms/chromium/screenshot/url
POST /forms/chromium/screenshot/markdown
```

### Form Fields

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `files` | file | - | HTML/Markdown file (for file endpoints) |
| `url` | string | - | URL to capture |
| `format` | string | "png" | "png" or "jpeg" |
| `quality` | int | 80 | JPEG quality 0-100 |
| `width` | int | 1920 | Viewport width |
| `height` | int | 1080 | Viewport height |
| `fullPage` | bool | false | Capture full scrollable page |
| `scale` | float | 1.0 | Device scale factor |
| `waitFor` | string | "load" | "load", "networkidle", "domcontentloaded" |
| `backgroundColor` | string | - | CSS color for background |

### Headers

Same as convert endpoints:
- `Gotenberg-Trace`
- `Gotenberg-Output-Filename`
- Custom headers via `Gotenberg-*` forwarded to page

### Response

```http
HTTP/1.1 200 OK
Content-Type: image/png (or image/jpeg)
Content-Disposition: attachment; filename="screenshot.png"

<binary image data>
```

## Error Handling

| Error | Condition |
|-------|-----------|
| `EngineError::ChromeLaunch` | Browser connection failed |
| `EngineError::NavigationFailed` | URL unreachable |
| `EngineError::Timeout` | Wait condition not met |
| `EngineError::ScreenshotFailed` | CDP screenshot error |

## Testing

Unit tests:
- Screenshot HTML with various viewport sizes
- Full page vs viewport capture
- PNG and JPEG output
- Wait conditions

Integration tests:
- Gotenberg feature parity: `chromium_screenshot_*.feature`
- Image dimensions verification
- File format validation

## Dependencies

Uses existing `chromiumoxide` dependency.

## References

- Chrome DevTools Protocol: https://chromedevtools.github.io/devtools-protocol/
- Page.captureScreenshot: https://chromedevtools.github.io/devtools-protocol/tot/Page/#method-captureScreenshot
- Gotenberg docs: https://gotenberg.dev/docs/routes#screenshots

## Notes

- Screenshots are handled separately from PDF conversion but share the same Chromium pool
- Consider rate limiting for screenshot endpoints (expensive operation)
- Full page screenshots can be memory-intensive for very long pages
