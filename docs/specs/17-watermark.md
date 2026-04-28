# Spec 17 — PDF Watermark & Stamp

> Overlay images or text onto PDF pages.
> Watermark appears behind content, Stamp appears in front.

## Goal

Provide watermark and stamp functionality for PDF documents, allowing
users to overlay images (PNG, JPEG) or text on pages at configurable
positions with opacity control.

## Scope

**In:**

- Image watermark/stamp (PNG, JPEG support via image crate).
- Text watermark/stamp (with font selection).
- Position control: center, corners, edges, custom coordinates.
- Opacity/transparency (0.0 to 1.0).
- Rotation (degrees).
- Page range selection (all pages, odd, even, specific pages).
- Watermark (behind content) vs Stamp (in front of content).

**Out:**

- SVG watermarks (rasterize first).
- Multi-page watermark documents.
- Animated watermarks.
- Pattern fills.

## Public API

Module path: `engine::watermark`. Stateless free functions.

```rust
use crate::types::{EngineError, EngineResult};

/// Type of overlay.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverlayType {
    /// Watermark appears behind page content.
    Watermark,
    /// Stamp appears in front of page content.
    Stamp,
}

/// Content to overlay.
#[derive(Debug, Clone)]
pub enum OverlayContent {
    /// Image file bytes (PNG or JPEG).
    Image { data: Vec<u8>, format: ImageFormat },
    /// Text with font specification.
    Text { text: String, font: FontSpec },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageFormat {
    Png,
    Jpeg,
}

#[derive(Debug, Clone)]
pub struct FontSpec {
    /// Font family name.
    pub family: String,
    /// Font size in points.
    pub size: f32,
    /// RGB color (0-255 each).
    pub color: (u8, u8, u8),
    /// Bold, italic, etc.
    pub style: FontStyle,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct FontStyle {
    pub bold: bool,
    pub italic: bool,
}

/// Position on page for overlay.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Position {
    Center,
    TopLeft, TopCenter, TopRight,
    MiddleLeft, MiddleRight,
    BottomLeft, BottomCenter, BottomRight,
    /// Custom position in PDF points from bottom-left.
    Custom { x: f32, y: f32 },
}

/// Scale mode for image overlays.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScaleMode {
    /// Original size in pixels.
    Original,
    /// Fit within page maintaining aspect ratio.
    FitPage,
    /// Fill page maintaining aspect ratio (may crop).
    FillPage,
    /// Custom width/height in points.
    Custom { width: f32, height: f32 },
}

/// Watermark/stamp options.
#[derive(Debug, Clone)]
pub struct WatermarkOptions {
    /// Watermark or stamp.
    pub overlay_type: OverlayType,
    /// Content to overlay.
    pub content: OverlayContent,
    /// Position on page.
    pub position: Position,
    /// Opacity 0.0-1.0.
    pub opacity: f32,
    /// Rotation in degrees (0 = no rotation).
    pub rotation: f32,
    /// Page range to apply.
    pub pages: PageSelection,
    /// Scale mode for images.
    pub scale: ScaleMode,
}

/// Page selection for watermark application.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageSelection {
    All,
    First,
    Last,
    Odd,
    Even,
    Range(u32, u32), // start, end (1-indexed, inclusive)
}

/// Apply watermark or stamp to PDF.
///
/// Returns new PDF with overlay applied.
pub fn apply_watermark(
    pdf: &[u8],
    opts: &WatermarkOptions,
) -> EngineResult<Vec<u8>>;

/// Convenience: apply image watermark.
pub fn apply_image_watermark(
    pdf: &[u8],
    image: &[u8],
    format: ImageFormat,
    position: Position,
    opacity: f32,
) -> EngineResult<Vec<u8>> {
    let opts = WatermarkOptions {
        overlay_type: OverlayType::Watermark,
        content: OverlayContent::Image { data: image.to_vec(), format },
        position,
        opacity,
        rotation: 0.0,
        pages: PageSelection::All,
        scale: ScaleMode::FitPage,
    };
    apply_watermark(pdf, &opts)
}

/// Convenience: apply text stamp.
pub fn apply_text_stamp(
    pdf: &[u8],
    text: &str,
    position: Position,
    opacity: f32,
) -> EngineResult<Vec<u8>> {
    let opts = WatermarkOptions {
        overlay_type: OverlayType::Stamp,
        content: OverlayContent::Text {
            text: text.to_string(),
            font: FontSpec {
                family: "Helvetica".into(),
                size: 48.0,
                color: (128, 128, 128),
                style: FontStyle::default(),
            },
        },
        position,
        opacity,
        rotation: 0.0,
        pages: PageSelection::All,
        scale: ScaleMode::Original,
    };
    apply_watermark(pdf, &opts)
}
```

## Implementation Strategy

### Using `lopdf` + `image`

1. **Load PDF** with `lopdf::Document::load_mem()`.
2. **Load image** with `image` crate, convert to PDF XObject.
3. **For each target page**:
   - Get page content stream
   - Create overlay XObject (Form XObject containing image or text)
   - Insert into page resources
   - Modify content stream to draw overlay:
     - Watermark: Add before existing content (gsave/q/qx/q.../grestore)
     - Stamp: Add after existing content
4. **Save modified PDF**.

### Text Rendering

For text watermarks:
- Use built-in PDF fonts (Helvetica, Times, Courier) for simplicity
- Or embed TrueType font subset
- Create text object with:
  - BT (Begin Text)
  - Tf (Set Font)
  - Td (Move Text Position)
  - Tj (Show Text)
  - ET (End Text)

## Server API

### Watermark Endpoint

```
POST /forms/pdfengines/watermark
```

Form fields:
- `files` - Single PDF file
- `watermark` - Image file (PNG/JPEG) or text string
- `mode` - `"watermark"` (behind) or `"stamp"` (front)
- `position` - `"center"`, `"top-left"`, etc.
- `opacity` - 0.0 to 1.0
- `rotation` - Degrees (optional)
- `pages` - Page range (optional, default "all")

Response:
- PDF with watermark applied
- `Content-Disposition: attachment; filename="result.pdf"`

### Stamp Endpoint

```
POST /forms/pdfengines/stamp
```

Same as watermark, defaults to mode="stamp".

## Error Handling

| Error | Condition |
|-------|-----------|
| `EngineError::InvalidInput` | Invalid image format |
| `EngineError::InvalidPage` | Page range out of bounds |
| `EngineError::FontNotFound` | Requested font unavailable |

## Testing

Unit tests:
- Image watermark on single page
- Text stamp on all pages
- Opacity verification (PDF structure)
- Position accuracy
- Page range selection

Integration tests:
- Gotenberg feature parity
- Visual verification (manual or screenshot)
- File size not exploded

## Dependencies

```toml
[dependencies]
# Image processing
image = { version = "0.25", default-features = false, features = ["png", "jpeg"] }
# PDF manipulation (already have lopdf)
```

## References

- PDF Spec ISO 32000-2: Section 8.10 (External Objects), 9 (Text)
- Gotenberg docs: https://gotenberg.dev/docs/routes#watermark
