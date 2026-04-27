//! PDF post-processing operations: merge, split, flatten, metadata,
//! watermark, rotate.
//!
//! Implementation of `docs/specs/13-engine-pdfops.md`. All operations are
//! stateless free functions that take and return owned PDF byte streams.
//! Encrypted inputs are uniformly rejected at parse time.

use lopdf::{Dictionary, Document, Object, ObjectId, StringFormat};

use crate::types::{EngineError, EngineResult};

mod flatten;
mod merge;
mod metadata;
mod rotate;
mod split;
mod watermark;

pub use flatten::flatten;
pub use merge::merge;
pub use metadata::{Metadata, read_metadata, write_metadata};
pub use rotate::rotate;
pub use split::{SplitMode, split};
pub use watermark::{Position, WatermarkKind, WatermarkOptions, watermark};

// ---------------------------------------------------------------------------
// Crate-private helpers shared by all ops.
// ---------------------------------------------------------------------------
//
// The `allow(dead_code)` attributes carry their weight only during the
// spec-13 scaffold commit. Each helper is wired up by the implementation
// commits that follow (merge, split, flatten, metadata, watermark, rotate)
// and the attributes are removed as they become live.

/// Producer string written to every output PDF.
#[allow(dead_code)]
pub(crate) fn producer_string() -> String {
    format!("folio/{}", env!("CARGO_PKG_VERSION"))
}

/// Parse an input byte stream and uniformly reject encrypted documents.
///
/// Maps lopdf parse failures to [`EngineError::Internal`].
pub(crate) fn parse_input(bytes: &[u8]) -> EngineResult<Document> {
    let doc = Document::load_mem(bytes)
        .map_err(|e| EngineError::Internal(format!("failed to parse PDF: {e}")))?;
    if doc.is_encrypted() {
        return Err(EngineError::Internal(
            "encrypted PDFs are not supported in MVP".into(),
        ));
    }
    Ok(doc)
}

/// Finalize a document for output: stamp `/Producer`, compress streams,
/// and serialize to bytes.
#[allow(dead_code)]
pub(crate) fn finalize(mut doc: Document) -> EngineResult<Vec<u8>> {
    set_producer(&mut doc);
    doc.compress();
    let mut out = Vec::new();
    doc.save_to(&mut out)
        .map_err(|e| EngineError::Internal(format!("failed to save PDF: {e}")))?;
    Ok(out)
}

/// Write `/Producer` into the document's `/Info` dict, creating the dict if
/// it doesn't exist.
#[allow(dead_code)]
fn set_producer(doc: &mut Document) {
    let producer = producer_string();
    let info_id = ensure_info_dict(doc);
    if let Ok(Object::Dictionary(dict)) = doc.get_object_mut(info_id) {
        dict.set("Producer", encode_pdf_text_string(&producer));
    }
}

/// Return the `ObjectId` of the document's `/Info` dictionary, creating an
/// empty one if absent.
#[allow(dead_code)]
pub(crate) fn ensure_info_dict(doc: &mut Document) -> ObjectId {
    if let Ok(id) = doc.trailer.get(b"Info").and_then(|o| o.as_reference()) {
        return id;
    }
    let id = doc.add_object(Dictionary::new());
    doc.trailer.set("Info", Object::Reference(id));
    id
}

/// Encode a Rust string as a PDF text string. ASCII strings use a literal
/// `()` form; non-ASCII strings use UTF-16BE with a leading BOM (per the
/// PDF 1.7 spec, §7.9.2.2).
#[allow(dead_code)]
pub(crate) fn encode_pdf_text_string(s: &str) -> Object {
    if s.is_ascii() {
        Object::String(s.as_bytes().to_vec(), StringFormat::Literal)
    } else {
        let mut bytes = vec![0xFE, 0xFF];
        for u in s.encode_utf16() {
            bytes.extend_from_slice(&u.to_be_bytes());
        }
        Object::String(bytes, StringFormat::Literal)
    }
}

/// Decode a PDF text-string byte slice. Recognises the UTF-16BE BOM and
/// falls back to lossy UTF-8 for ASCII / PDFDocEncoding strings.
#[allow(dead_code)]
pub(crate) fn decode_pdf_text_string(bytes: &[u8]) -> String {
    if bytes.starts_with(&[0xFE, 0xFF]) {
        let units: Vec<u16> = bytes[2..]
            .chunks_exact(2)
            .map(|c| u16::from_be_bytes([c[0], c[1]]))
            .collect();
        String::from_utf16_lossy(&units)
    } else {
        String::from_utf8_lossy(bytes).into_owned()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
pub(crate) mod test_support {
    //! Test-only helpers: build minimal valid PDFs in memory.
    use super::*;
    use lopdf::dictionary;

    /// Build a minimal valid 1-page PDF document and serialize to bytes.
    /// Page size is US Letter (612×792 pt); content stream is empty.
    pub fn make_blank_pdf() -> Vec<u8> {
        let mut doc = Document::with_version("1.5");

        let pages_id = doc.new_object_id();

        let content_id = doc.add_object(lopdf::Stream::new(dictionary! {}, Vec::new()));
        let resources_id = doc.add_object(dictionary! {});
        let page_id = doc.add_object(dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
            "Resources" => resources_id,
            "Contents" => content_id,
        });

        doc.objects.insert(
            pages_id,
            Object::Dictionary(dictionary! {
                "Type" => "Pages",
                "Kids" => vec![page_id.into()],
                "Count" => 1,
            }),
        );

        let catalog_id = doc.add_object(dictionary! {
            "Type" => "Catalog",
            "Pages" => pages_id,
        });
        doc.trailer.set("Root", catalog_id);

        let mut bytes = Vec::new();
        doc.save_to(&mut bytes).expect("save blank pdf");
        bytes
    }

    /// Build an N-page PDF with the given paper size (in points). Used by
    /// integration tests for fixtures.
    #[allow(dead_code)]
    pub fn make_multipage_pdf(num_pages: u32, width_pt: i64, height_pt: i64) -> Vec<u8> {
        let mut doc = Document::with_version("1.5");
        let pages_id = doc.new_object_id();
        let resources_id = doc.add_object(dictionary! {});

        let mut kids = Vec::with_capacity(num_pages as usize);
        for _ in 0..num_pages {
            let content_id = doc.add_object(lopdf::Stream::new(dictionary! {}, Vec::new()));
            let page_id = doc.add_object(dictionary! {
                "Type" => "Page",
                "Parent" => pages_id,
                "MediaBox" => vec![
                    0.into(), 0.into(),
                    width_pt.into(), height_pt.into(),
                ],
                "Resources" => resources_id,
                "Contents" => content_id,
            });
            kids.push(Object::Reference(page_id));
        }

        doc.objects.insert(
            pages_id,
            Object::Dictionary(dictionary! {
                "Type" => "Pages",
                "Kids" => kids,
                "Count" => num_pages,
            }),
        );

        let catalog_id = doc.add_object(dictionary! {
            "Type" => "Catalog",
            "Pages" => pages_id,
        });
        doc.trailer.set("Root", catalog_id);

        let mut bytes = Vec::new();
        doc.save_to(&mut bytes).expect("save multipage pdf");
        bytes
    }

    /// Build a 1-page PDF and add a fake `/Encrypt` reference to its
    /// trailer so `Document::is_encrypted()` returns true on reload.
    /// The Encrypt dict is a stub (`/Filter /Standard`); we never attempt
    /// to decrypt it — we just verify our gate rejects it.
    pub fn make_fake_encrypted_pdf() -> Vec<u8> {
        let mut doc = Document::with_version("1.5");

        let pages_id = doc.new_object_id();
        let content_id = doc.add_object(lopdf::Stream::new(dictionary! {}, Vec::new()));
        let resources_id = doc.add_object(dictionary! {});
        let page_id = doc.add_object(dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
            "Resources" => resources_id,
            "Contents" => content_id,
        });
        doc.objects.insert(
            pages_id,
            Object::Dictionary(dictionary! {
                "Type" => "Pages",
                "Kids" => vec![page_id.into()],
                "Count" => 1,
            }),
        );
        let catalog_id = doc.add_object(dictionary! {
            "Type" => "Catalog",
            "Pages" => pages_id,
        });

        // Stub Encrypt dict — V=1, R=2 is RC4; we never attempt to decrypt.
        let encrypt_id = doc.add_object(dictionary! {
            "Filter" => "Standard",
            "V" => 1,
            "R" => 2,
            "Length" => 40,
            "P" => -1,
            "O" => Object::String(vec![0u8; 32], StringFormat::Hexadecimal),
            "U" => Object::String(vec![0u8; 32], StringFormat::Hexadecimal),
        });

        doc.trailer.set("Root", catalog_id);
        doc.trailer.set("Encrypt", Object::Reference(encrypt_id));
        // /ID is required for encrypted docs.
        let id_bytes = Object::String(vec![0u8; 16], StringFormat::Hexadecimal);
        doc.trailer
            .set("ID", Object::Array(vec![id_bytes.clone(), id_bytes]));

        let mut bytes = Vec::new();
        doc.save_to(&mut bytes).expect("save fake encrypted pdf");
        bytes
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use test_support::*;

    #[test]
    fn parse_input_accepts_blank_pdf() {
        let bytes = make_blank_pdf();
        let doc = parse_input(&bytes).expect("blank pdf should parse");
        assert_eq!(doc.get_pages().len(), 1);
    }

    #[test]
    fn parse_input_rejects_encrypted() {
        let bytes = make_fake_encrypted_pdf();
        let err = parse_input(&bytes).expect_err("encrypted input must be rejected");
        match err {
            EngineError::Internal(msg) => {
                assert!(msg.contains("encrypted"), "msg: {msg}");
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn finalize_sets_producer() {
        let bytes = make_blank_pdf();
        let doc = parse_input(&bytes).unwrap();
        let out = finalize(doc).unwrap();

        let reread = Document::load_mem(&out).expect("reload");
        let info_id = reread
            .trailer
            .get(b"Info")
            .and_then(|o| o.as_reference())
            .expect("Info ref");
        let info = reread
            .get_object(info_id)
            .and_then(|o| o.as_dict())
            .expect("Info dict");
        let producer_bytes = match info.get(b"Producer").expect("Producer entry") {
            Object::String(b, _) => b.clone(),
            other => panic!("unexpected producer object: {other:?}"),
        };
        let expected = producer_string();
        assert_eq!(String::from_utf8(producer_bytes).unwrap(), expected);
    }

    #[test]
    fn encode_text_string_ascii_uses_literal() {
        let obj = encode_pdf_text_string("Hello");
        match obj {
            Object::String(bytes, StringFormat::Literal) => {
                assert_eq!(bytes, b"Hello");
            }
            other => panic!("unexpected encoding: {other:?}"),
        }
    }

    #[test]
    fn encode_text_string_unicode_uses_utf16be_bom() {
        let obj = encode_pdf_text_string("héllo");
        match obj {
            Object::String(bytes, StringFormat::Literal) => {
                assert_eq!(&bytes[..2], &[0xFE, 0xFF]);
                let decoded = decode_pdf_text_string(&bytes);
                assert_eq!(decoded, "héllo");
            }
            other => panic!("unexpected encoding: {other:?}"),
        }
    }

    #[test]
    fn decode_text_string_round_trips_ascii() {
        let obj = encode_pdf_text_string("plain ASCII");
        if let Object::String(bytes, _) = obj {
            assert_eq!(decode_pdf_text_string(&bytes), "plain ASCII");
        } else {
            panic!("unexpected object");
        }
    }
}
