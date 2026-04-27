//! `watermark` — overlay text or PNG content onto each page.

use serde::{Deserialize, Serialize};

use crate::types::{EngineError, EngineResult};

/// Where to place a watermark on each page.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Position {
    /// Geometric centre of the page.
    Center,
    /// Top-left corner.
    TopLeft,
    /// Top edge, horizontally centred.
    TopCenter,
    /// Top-right corner.
    TopRight,
    /// Middle of the left edge.
    MiddleLeft,
    /// Middle of the right edge.
    MiddleRight,
    /// Bottom-left corner.
    BottomLeft,
    /// Bottom edge, horizontally centred.
    BottomCenter,
    /// Bottom-right corner.
    BottomRight,
}

/// Watermark configuration shared by both text and image variants.
#[derive(Debug, Clone)]
pub struct WatermarkOptions {
    /// What to stamp.
    pub kind: WatermarkKind,
    /// Opacity, `0.0..=1.0`. Values outside the range are clamped.
    pub opacity: f32,
    /// Rotation in degrees (counter-clockwise).
    pub rotation_deg: f32,
    /// Position on the page.
    pub position: Position,
    /// Stamp every page (`true`) or only odd pages (`false`).
    pub all_pages: bool,
    /// Tile the watermark across the page surface.
    pub tiled: bool,
}

/// Watermark content variant: text glyphs or a PNG image.
#[derive(Debug, Clone)]
pub enum WatermarkKind {
    /// Vector text watermark.
    Text {
        /// The text string to draw.
        text: String,
        /// PostScript font name; `None` defaults to `Helvetica`.
        font: Option<String>,
        /// Point size; must be `> 0.0`.
        font_size: f32,
        /// RGBA colour, each channel in `0.0..=1.0`.
        color: [f32; 4],
    },
    /// PNG image watermark. The bytes must be a complete PNG file.
    ImagePng {
        /// PNG file bytes.
        bytes: Vec<u8>,
    },
}

/// Apply a watermark to every page (or odd pages) of `pdf`.
///
/// # Errors
///
/// - [`EngineError::InvalidOption`] for malformed watermark options
///   (non-positive font size, malformed PNG header).
/// - [`EngineError::Internal`] if the input fails to parse, is encrypted,
///   or the result fails to save.
pub fn watermark(pdf: &[u8], opts: &WatermarkOptions) -> EngineResult<Vec<u8>> {
    validate_opts(opts)?;
    super::parse_input(pdf)?;
    Err(EngineError::Internal(
        "watermark: not yet implemented".into(),
    ))
}

fn validate_opts(opts: &WatermarkOptions) -> EngineResult<()> {
    match &opts.kind {
        WatermarkKind::Text { font_size, .. } => {
            if !(font_size.is_finite() && *font_size > 0.0) {
                return Err(EngineError::InvalidOption(format!(
                    "watermark font_size must be > 0 (got {font_size})"
                )));
            }
        }
        WatermarkKind::ImagePng { bytes } => {
            const PNG_MAGIC: [u8; 8] = [0x89, b'P', b'N', b'G', b'\r', b'\n', 0x1A, b'\n'];
            if bytes.len() < PNG_MAGIC.len() || bytes[..PNG_MAGIC.len()] != PNG_MAGIC {
                return Err(EngineError::InvalidOption(
                    "watermark image bytes are not a PNG (signature mismatch)".into(),
                ));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pdfops::test_support::make_blank_pdf;

    fn default_opts(kind: WatermarkKind) -> WatermarkOptions {
        WatermarkOptions {
            kind,
            opacity: 0.5,
            rotation_deg: 0.0,
            position: Position::Center,
            all_pages: true,
            tiled: false,
        }
    }

    #[test]
    fn watermark_negative_font_size_rejected() {
        let pdf = make_blank_pdf();
        let opts = default_opts(WatermarkKind::Text {
            text: "X".into(),
            font: None,
            font_size: -1.0,
            color: [0.0; 4],
        });
        let err = watermark(&pdf, &opts).unwrap_err();
        assert!(matches!(err, EngineError::InvalidOption(_)));
    }

    #[test]
    fn watermark_zero_font_size_rejected() {
        let pdf = make_blank_pdf();
        let opts = default_opts(WatermarkKind::Text {
            text: "X".into(),
            font: None,
            font_size: 0.0,
            color: [0.0; 4],
        });
        assert!(matches!(
            watermark(&pdf, &opts),
            Err(EngineError::InvalidOption(_))
        ));
    }

    #[test]
    fn watermark_png_header_validation() {
        let pdf = make_blank_pdf();
        let opts = default_opts(WatermarkKind::ImagePng {
            bytes: b"definitely not a PNG".to_vec(),
        });
        let err = watermark(&pdf, &opts).unwrap_err();
        assert!(matches!(err, EngineError::InvalidOption(_)));
    }
}
