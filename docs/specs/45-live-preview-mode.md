# Spec 45 — Live Preview Mode

> Provide lightweight preview of HTML/URL before full PDF
> generation. Helps debug rendering issues - a unique Folio
> feature that Gotenberg cannot easily replicate.

## Goal

Create a live preview system that renders HTML/URL to
lightweight images for quick debugging. Solves the "why does
my PDF look bad?" problem (Gotenberg issues #921, #861).

## Problem Analysis#

### User Complaints (Gotenberg Discussions)

> "Every so often a PDF generated with Gotenberg 8 will
> lack all fonts loaded with CSS @font-face... Tryed
> implementing waitForExpression as 'document.readyState ===
> \"complete\"'... No idea what's going on"
> — Discussion #861

> "Numbers 6 and 8 get a bigger font size than other
> numbers after conversion... I suppose a workaround could
> be to rebuild the Docker container"
> — Issue #921

### Root Cause

Users have no way to see what the browser is rendering BEFORE
generating the full PDF. They're flying blind.

## Scope#

**In:**

- `GET /preview/html?url=...` - Preview URL as image
- `POST /preview/html` - Preview HTML as image  
- `GET /preview/markdown?url=...` - Preview Markdown
- Multiple preview formats: png, jpeg, webp
- Preview dimensions: viewport size, clip region
- Auto-refresh for iterative debugging
- Compare mode: before/after changes

**Out:**

- Full PDF preview (too heavy)
- Interactive browser session (complex)
- Screenshot comparison (separate tool)

## Implementation#

### 1. Preview Endpoints#

```rust
// crates/server/src/routes/preview.rs

use axum::extract::Query;

#[derive(Deserialize)]
struct PreviewQuery {
    url: String,
    format: Option<String>,    // png, jpeg, webp
    width: Option<u32>,       // viewport width
    height: Option<u32>,     // viewport height
    clip_x: Option<f64>,
    clip_y: Option<f64>,
    clip_width: Option<f64>,
    clip_height: Option<f64>,
}

/// Preview URL as image.
pub async fn preview_url(
    State(state): State<AppState>,
    Query(query): Query<PreviewQuery>,
) -> ApiResult<impl IntoResponse> {
    let start = Instant::now();

    // Validate format
    let format = query.format.as_deref().unwrap_or("png");
    if !["png", "jpeg", "webp"].contains(&format) {
        return Err(ApiError::InvalidOption(
            format!("Invalid format: '{}'. Use png/jpeg/webp", format)
        ));
    }

    // Build screenshot options
    let mut opts = ScreenshotOptions::default();
    if let Some(w) = query.width {
        opts.viewport_width = w;
    }
    if let Some(h) = query.height {
        opts.viewport_height = h;
    }

    // Capture screenshot
    let result = state
        .chromium
        .as_ref()
        .unwrap()
        .screenshot_url(&query.url, &opts)
        .await
        .map_err(|e| ApiError::from(e))?;

    let duration = start.elapsed().as_secs_f64();
    tracing::info!(
        url = %query.url,
        format = %format,
        duration_ms = duration * 1000.0,
        "Preview generated"
    );

    // Return image
    let content_type = match format {
        "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        _ => "image/png",
    };

    Ok((
        [(header::CONTENT_TYPE, HeaderValue::from_static(content_type))],
        result,
    ))
}
```

### 2. HTML Preview with Form#

```rust
/// Preview HTML file as image.
pub async fn preview_html(
    State(state): State<AppState>,
    mp: Multipart,
) -> ApiResult<impl IntoResponse> {
    let form = parse_multipart(mp).await?;

    let html = form.get("files")
        .ok_or_else(|| ApiError::InvalidOption("HTML file required".into()))?;

    let mut opts = ScreenshotOptions::default();
    if let Some(format) = form.get("format") {
        opts.format = format.clone();
    }

    let result = state
        .chromium
        .as_ref()
        .unwrap()
        .screenshot_html(html, None, &opts)
        .await
        .map_err(|e| ApiError::from(e))?;

    image_response(result, &opts.format)
}
```

### 3. Preview Options#

```rust
// crates/engine/src/chromium/screenshot.rs

pub struct ScreenshotOptions {
    pub format: String,            // png, jpeg, webp
    pub quality: u8,              // 1-100 for jpeg/webp
    pub viewport_width: u32,       // Default 1920
    pub viewport_height: u32,      // Default 1080
    pub clip: Option<ClipRect>,
    pub full_page: bool,           // Screenshot full scrollable page
}

pub struct ClipRect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}
```

### 4. Compare Mode (Advanced)#

```rust
/// Compare two versions side by side.
pub async fn preview_compare(
    State(state): State<AppState>,
    mp: Multipart,
) -> ApiResult<impl IntoResponse> {
    let form = parse_multipart(mp).await?;

    let before = form.get("before")
        .ok_or_else(|| ApiError::InvalidOption("'before' required".into()))?;
    let after = form.get("after")
        .ok_or_else(|| ApiError::InvalidOption("'after' required".into()))?;

    // Screenshot both
    let img1 = state.chromium.as_ref().unwrap()
        .screenshot_html(before, None, &Default::default())
        .await?;
    let img2 = state.chromium.as_ref().unwrap()
        .screenshot_html(after, None, &Default::default())
        .await?;

    // Create side-by-side comparison image
    let comparison = create_comparison_image(&img1, &img2)?;

    image_response(comparison, "png")
}
```

## Form Fields#

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `url` | string | required | URL to preview |
| `files` | file | required | HTML file to preview |
| `format` | string | "png" | Output format: png/jpeg/webp |
| `quality` | int | 90 | JPEG/WebP quality (1-100) |
| `width` | int | 1920 | Viewport width |
| `height` | int | 1080 | Viewport height |
| `fullPage` | bool | false | Capture full scrollable page |
| `clip.x` | float | 0 | Clip rectangle X |
| `clip.y` | float | 0 | Clip rectangle Y |
| `clip.width` | float | viewport | Clip width |
| `clip.height` | float | viewport | Clip height |

## Expected Behaviour#

### Preview URL

```bash
# Quick preview
curl "http://localhost:3000/preview/url?url=https://example.com" -o preview.png

# High-quality JPEG
curl "http://localhost:3000/preview/url?url=https://example.com&format=jpeg&quality=95" -o preview.jpg

# Custom viewport
curl "http://localhost:3000/preview/url?url=https://example.com&width=375&height=667" -o mobile.png
```

### Preview HTML

```bash
curl -X POST http://localhost:3000/preview/html \
  --form files=@index.html \
  --form format=png \
  -o preview.png
```

### Compare Mode

```bash
curl -X POST http://localhost:3000/preview/compare \
  --form before=@old.html \
  --form after=@new.html \
  -o comparison.png
```

## Test Plan#

### Unit Tests

- `preview_url_returns_png_by_default`
- `preview_html_with_jpeg_format`
- `invalid_format_returns_400`
- `viewport_dimensions_applied`

### Integration Tests#

- `preview_url_returns_valid_image`
- `preview_html_screenshot_matches_viewport`
- `compare_mode_creates_side_by_side`
- `full_page_captures_scrollable_content`

## Acceptance#

- [ ] `GET /preview/url` endpoint
- [ ] `POST /preview/html` endpoint
- [ ] `GET /preview/markdown` endpoint
- [ ] Format selection: png/jpeg/webp
- [ ] Viewport dimensions applied
- [ ] Clip rectangle support
- [ ] Compare mode for debugging
- [ ] Unit tests for all endpoints
- [ ] Integration tests with real browser
- [ ] `cargo clippy -p server -- -D warnings` clean

## References#

- Gotenberg discussion #861: https://github.com/gotenberg/gotenberg/discussions/861
- Gotenberg issue #921: https://github.com/gotenberg/gotenberg/issues/921
- Chromium screenshot API: https://chromedevtools.github.io/devtools-protocol/1-3/Page/#method-captureScreenshot
- axum response handling: https://docs.rs/axum/latest/axum/response/
