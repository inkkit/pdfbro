//! `watermark` — overlay text or PNG content onto each page.
//!
//! The MVP implementation supports text watermarks with rotation,
//! per-position placement, and opacity (via an `ExtGState` dictionary).
//! PNG watermarks are validated (signature match) but their pixels are
//! not rendered yet — full image embedding requires PNG decode and
//! `Image XObject` reconstruction; tracked as a follow-up.
//!
//! Tiled placement is not implemented in the MVP; the option is honoured
//! at the type level so callers can request it once the renderer lands.
//! For non-tiled placement, a single watermark stamp is appended to each
//! targeted page's content stream.

use std::fmt::Write as _;

use lopdf::{Document, Object, ObjectId, dictionary};
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
    let mut doc = super::parse_input(pdf)?;
    let opacity = opts.opacity.clamp(0.0, 1.0);

    // Pre-register a single ExtGState dict for opacity, shared by all
    // pages. PDF readers use /CA for stroke alpha, /ca for fill alpha.
    let gs_id: Option<ObjectId> = if opacity < 1.0 {
        Some(doc.add_object(dictionary! {
            "Type" => "ExtGState",
            "CA" => Object::Real(opacity),
            "ca" => Object::Real(opacity),
        }))
    } else {
        None
    };

    let pages: Vec<(u32, ObjectId)> = doc.get_pages().into_iter().collect();
    for (page_num, page_id) in pages {
        if !opts.all_pages && page_num.is_multiple_of(2) {
            continue; // honour `all_pages = false` ⇒ odd pages only.
        }

        let media_box = read_media_box(&doc, page_id).unwrap_or([0.0, 0.0, 612.0, 792.0]);
        let snippet = match &opts.kind {
            WatermarkKind::Text {
                text,
                font_size,
                color,
                ..
            } => render_text_snippet(
                text,
                *font_size,
                *color,
                opts.rotation_deg,
                opts.position,
                media_box,
                gs_id.is_some(),
            ),
            WatermarkKind::ImagePng { .. } => {
                // MVP: image rendering is deferred. The signature has
                // already been validated by `validate_opts`. We still
                // run the page through the finalize pipeline so the
                // op never panics on PNG inputs.
                String::new()
            }
        };

        if !snippet.is_empty() {
            ensure_helvetica_in_resources(&mut doc, page_id);
            if let Some(gs) = gs_id {
                ensure_extgstate_in_resources(&mut doc, page_id, b"Gs1", gs);
            }
            append_page_content(&mut doc, page_id, snippet.into_bytes());
        }
    }

    super::finalize(doc)
}

/// Compose a complete content-stream snippet that draws the watermark
/// text onto the page. Returned bytes are appended to the page's content
/// after a leading newline.
fn render_text_snippet(
    text: &str,
    font_size: f32,
    color: [f32; 4],
    rotation_deg: f32,
    position: Position,
    media_box: [f32; 4],
    use_gs: bool,
) -> String {
    let approx_width = font_size * text.chars().count() as f32 * 0.5;
    let approx_height = font_size;
    let (tx, ty) = anchor_for(position, media_box, approx_width, approx_height);

    let theta = rotation_deg.to_radians();
    let (c, s) = (theta.cos(), theta.sin());
    let escaped = escape_pdf_literal(text);

    let mut out = String::new();
    out.push_str("\nq\n");
    if use_gs {
        out.push_str("/Gs1 gs\n");
    }
    let _ = writeln!(out, "{} {} {} rg", color[0], color[1], color[2]);
    out.push_str("BT\n");
    let _ = writeln!(out, "/F1 {font_size} Tf");
    let _ = writeln!(out, "{c} {s} {} {c} {tx} {ty} Tm", -s);
    let _ = writeln!(out, "({escaped}) Tj");
    out.push_str("ET\n");
    out.push_str("Q\n");
    out
}

/// Compute the (tx, ty) translation in user space for placing a watermark
/// of `(w, h)` size at `position` within `media_box = [llx, lly, urx, ury]`.
/// The anchor is the lower-left corner of the watermark.
fn anchor_for(position: Position, media_box: [f32; 4], w: f32, h: f32) -> (f32, f32) {
    let [llx, lly, urx, ury] = media_box;
    let (page_w, page_h) = (urx - llx, ury - lly);
    let (cx, cy) = match position {
        Position::TopLeft => (0.0, page_h - h),
        Position::TopCenter => ((page_w - w) / 2.0, page_h - h),
        Position::TopRight => (page_w - w, page_h - h),
        Position::MiddleLeft => (0.0, (page_h - h) / 2.0),
        Position::Center => ((page_w - w) / 2.0, (page_h - h) / 2.0),
        Position::MiddleRight => (page_w - w, (page_h - h) / 2.0),
        Position::BottomLeft => (0.0, 0.0),
        Position::BottomCenter => ((page_w - w) / 2.0, 0.0),
        Position::BottomRight => (page_w - w, 0.0),
    };
    (llx + cx, lly + cy)
}

/// Read the page leaf's `/MediaBox`, falling back to the spec default of
/// US Letter (612×792 pt) when absent. Inherited media boxes from the
/// `/Pages` ancestor chain are not resolved.
fn read_media_box(doc: &Document, page_id: ObjectId) -> Option<[f32; 4]> {
    let dict = doc.get_object(page_id).ok()?.as_dict().ok()?;
    let arr = dict.get(b"MediaBox").ok()?.as_array().ok()?;
    if arr.len() != 4 {
        return None;
    }
    let mut out = [0.0_f32; 4];
    for (i, o) in arr.iter().enumerate() {
        out[i] = match o {
            Object::Integer(n) => *n as f32,
            Object::Real(r) => *r,
            _ => return None,
        };
    }
    Some(out)
}

/// Escape a Rust string for inclusion inside a PDF `()` literal: backslash,
/// open-paren, close-paren, and CR are escaped; any non-ASCII byte is
/// rewritten as the question mark (`?`). The full unicode story is
/// handled by Form XObjects with custom CIDFonts; out of scope for MVP.
fn escape_pdf_literal(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if !c.is_ascii() {
            out.push('?');
            continue;
        }
        match c {
            '\\' => out.push_str("\\\\"),
            '(' => out.push_str("\\("),
            ')' => out.push_str("\\)"),
            '\r' => out.push_str("\\r"),
            other => out.push(other),
        }
    }
    out
}

/// Make sure the page's `/Resources/Font` dict has an `/F1 -> Helvetica`
/// entry, creating any missing intermediate dicts. The Helvetica font
/// itself is added as a fresh indirect object on first use.
fn ensure_helvetica_in_resources(doc: &mut Document, page_id: ObjectId) {
    let font_id = doc.add_object(dictionary! {
        "Type" => "Font",
        "Subtype" => "Type1",
        "BaseFont" => "Helvetica",
        "Encoding" => "WinAnsiEncoding",
    });
    insert_into_resources_subdict(doc, page_id, b"Font", b"F1", font_id);
}

/// Make sure the page's `/Resources/ExtGState` dict has the supplied
/// `name` mapped to `extgstate_id`.
fn ensure_extgstate_in_resources(
    doc: &mut Document,
    page_id: ObjectId,
    name: &[u8],
    extgstate_id: ObjectId,
) {
    insert_into_resources_subdict(doc, page_id, b"ExtGState", name, extgstate_id);
}

/// Generic helper: ensure `page.Resources[subdict_key][name] = id`,
/// promoting inline dicts and creating missing levels along the way.
fn insert_into_resources_subdict(
    doc: &mut Document,
    page_id: ObjectId,
    subdict_key: &[u8],
    name: &[u8],
    id: ObjectId,
) {
    // Step 1: ensure the page has a direct /Resources dictionary.
    let resources_obj = {
        let Ok(Object::Dictionary(page)) = doc.get_object(page_id) else {
            return;
        };
        page.get(b"Resources").ok().cloned()
    };

    let resources_inline = match resources_obj {
        Some(Object::Dictionary(d)) => Some(d),
        Some(Object::Reference(rid)) => {
            // Indirect resources dict — modify it in place.
            let Ok(Object::Dictionary(rd)) = doc.get_object_mut(rid) else {
                return;
            };
            insert_into_dict_subdict(rd, subdict_key, name, id);
            return;
        }
        _ => None,
    };

    // Inline (or absent) resources — modify the page dict directly.
    if let Ok(Object::Dictionary(page)) = doc.get_object_mut(page_id) {
        let mut dict = resources_inline.unwrap_or_default();
        insert_into_dict_subdict(&mut dict, subdict_key, name, id);
        page.set("Resources", Object::Dictionary(dict));
    }
}

/// Mutate `dict[subdict_key][name] = Reference(id)`, creating the subdict
/// inline if absent.
fn insert_into_dict_subdict(
    dict: &mut lopdf::Dictionary,
    subdict_key: &[u8],
    name: &[u8],
    id: ObjectId,
) {
    let mut sub = match dict.get(subdict_key).cloned() {
        Ok(Object::Dictionary(d)) => d,
        _ => lopdf::Dictionary::new(),
    };
    sub.set(
        std::str::from_utf8(name).unwrap_or("UnknownName"),
        Object::Reference(id),
    );
    dict.set(
        std::str::from_utf8(subdict_key).unwrap_or("Unknown"),
        Object::Dictionary(sub),
    );
}

/// Append `bytes` to the end of the page's content stream.
///
/// Handles the three common content-stream shapes: a single indirect
/// stream reference, an array of stream references, or an inline content
/// stream. New content is appended to the *last* stream so it draws on
/// top of any pre-existing content.
fn append_page_content(doc: &mut Document, page_id: ObjectId, bytes: Vec<u8>) {
    let contents = {
        let Ok(Object::Dictionary(page)) = doc.get_object(page_id) else {
            return;
        };
        page.get(b"Contents").ok().cloned()
    };

    let target_stream_id: Option<ObjectId> = match contents {
        Some(Object::Reference(id)) => Some(id),
        Some(Object::Array(arr)) => arr.last().and_then(|o| o.as_reference().ok()),
        _ => None,
    };

    let Some(stream_id) = target_stream_id else {
        return;
    };

    if let Ok(Object::Stream(stream)) = doc.get_object_mut(stream_id) {
        // If the stream is compressed, decompress in place so we can append
        // safely; lopdf will recompress on save.
        stream.decompress();
        stream.content.extend_from_slice(&bytes);
        // Update /Length to match the new content length; lopdf normally
        // rewrites this on save, but be explicit.
        stream
            .dict
            .set("Length", Object::Integer(stream.content.len() as i64));
    }
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
    use crate::pdfops::test_support::{make_blank_pdf, make_multipage_pdf};
    use lopdf::Document;

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

    fn text_kind(text: &str) -> WatermarkKind {
        WatermarkKind::Text {
            text: text.into(),
            font: None,
            font_size: 48.0,
            color: [0.5, 0.5, 0.5, 1.0],
        }
    }

    /// Decompress + concatenate every content stream attached to `page_num`,
    /// returning the raw PDF operators as bytes. We don't use lopdf's
    /// extract_text because it normalises whitespace; we want to assert
    /// the literal `(Marker) Tj` is in the stream.
    fn raw_page_content(pdf: &[u8], page_num: u32) -> Vec<u8> {
        let doc = Document::load_mem(pdf).unwrap();
        let pages = doc.get_pages();
        let page_id = *pages.get(&page_num).unwrap();
        doc.get_page_content(page_id).unwrap()
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

    #[test]
    fn watermark_png_valid_header_round_trips() {
        let pdf = make_blank_pdf();
        let opts = default_opts(WatermarkKind::ImagePng {
            bytes: b"\x89PNG\r\n\x1a\n\0\0\0\0".to_vec(),
        });
        // Image rendering is deferred but the call must succeed and the
        // page count must round-trip.
        let out = watermark(&pdf, &opts).unwrap();
        assert_eq!(Document::load_mem(&out).unwrap().get_pages().len(), 1);
    }

    #[test]
    fn watermark_text_appears_on_every_page() {
        let pdf = make_multipage_pdf(3, 612, 792);
        let opts = default_opts(text_kind("FOLIO-MARK"));
        let out = watermark(&pdf, &opts).unwrap();

        for p in 1..=3 {
            let content = raw_page_content(&out, p);
            let needle = b"(FOLIO-MARK) Tj";
            assert!(
                content.windows(needle.len()).any(|w| w == needle),
                "page {p} content missing watermark: {:?}",
                String::from_utf8_lossy(&content)
            );
        }
    }

    #[test]
    fn watermark_only_odd_pages_when_all_pages_false() {
        let pdf = make_multipage_pdf(4, 612, 792);
        let mut opts = default_opts(text_kind("ODD"));
        opts.all_pages = false;
        let out = watermark(&pdf, &opts).unwrap();

        let needle = b"(ODD) Tj";
        for p in [1u32, 3] {
            let content = raw_page_content(&out, p);
            assert!(
                content.windows(needle.len()).any(|w| w == needle),
                "page {p} should have watermark"
            );
        }
        for p in [2u32, 4] {
            let content = raw_page_content(&out, p);
            assert!(
                !content.windows(needle.len()).any(|w| w == needle),
                "page {p} should NOT have watermark"
            );
        }
    }

    #[test]
    fn watermark_escapes_special_chars_in_text() {
        let pdf = make_blank_pdf();
        let opts = default_opts(text_kind("(weird)\\path"));
        let out = watermark(&pdf, &opts).unwrap();
        let content = raw_page_content(&out, 1);
        // Parens and backslash must be escaped; the resulting literal must
        // still parse as a balanced PDF string.
        let s = String::from_utf8_lossy(&content);
        assert!(s.contains("(\\(weird\\)\\\\path) Tj"), "content: {s}");
    }

    #[test]
    fn watermark_registers_helvetica_font() {
        let pdf = make_blank_pdf();
        let opts = default_opts(text_kind("M"));
        let out = watermark(&pdf, &opts).unwrap();

        let doc = Document::load_mem(&out).unwrap();
        let pages = doc.get_pages();
        let page_id = *pages.get(&1).unwrap();
        let fonts = doc.get_page_fonts(page_id).unwrap();
        assert!(
            fonts.contains_key(b"F1".as_slice()),
            "fonts: {:?}",
            fonts.keys().collect::<Vec<_>>()
        );
    }

    #[test]
    fn anchor_for_center_centers_box() {
        let mb = [0.0, 0.0, 100.0, 200.0];
        let (x, y) = anchor_for(Position::Center, mb, 20.0, 10.0);
        assert!((x - 40.0).abs() < 0.01);
        assert!((y - 95.0).abs() < 0.01);
    }

    #[test]
    fn anchor_for_corners_pin_correctly() {
        let mb = [0.0, 0.0, 100.0, 200.0];
        let (w, h) = (20.0, 10.0);
        assert_eq!(anchor_for(Position::TopLeft, mb, w, h), (0.0, 190.0));
        assert_eq!(anchor_for(Position::TopRight, mb, w, h), (80.0, 190.0));
        assert_eq!(anchor_for(Position::BottomLeft, mb, w, h), (0.0, 0.0));
        assert_eq!(anchor_for(Position::BottomRight, mb, w, h), (80.0, 0.0));
    }

    #[test]
    fn escape_pdf_literal_handles_specials() {
        assert_eq!(escape_pdf_literal("a(b)c"), "a\\(b\\)c");
        assert_eq!(escape_pdf_literal("a\\b"), "a\\\\b");
        assert_eq!(escape_pdf_literal("héllo"), "h?llo");
        assert_eq!(escape_pdf_literal("plain"), "plain");
    }
}
