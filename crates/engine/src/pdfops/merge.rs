//! `merge` — concatenate a sequence of PDFs preserving order.
//!
//! Implements the page-tree concatenation pattern: each input's object IDs
//! are shifted into a disjoint range via `renumber_objects_with`, their
//! page objects are collected in order, and a fresh `/Pages` tree plus
//! `/Catalog` are assembled in a new output document. The original
//! `/Outlines` from the first input is preserved; `/AcroForm` and `/Names`
//! are dropped to avoid name collisions (per spec 13).

use lopdf::{Document, Object, ObjectId, dictionary};

use crate::types::{EngineError, EngineResult};

/// Concatenate a sequence of PDFs into a single document, preserving order.
///
/// A single input is parsed, validated, and re-serialized through the
/// common `finalize` pipeline (producer stamp, FlateDecode, save).
///
/// # Errors
///
/// - [`EngineError::InvalidOption`] if `pdfs` is empty.
/// - [`EngineError::Internal`] if any input fails to parse, is encrypted,
///   or the merged document fails to save. Parse errors are annotated
///   with the offending input's 1-indexed position (`merge: input #N:`).
pub fn merge(pdfs: &[&[u8]]) -> EngineResult<Vec<u8>> {
    if pdfs.is_empty() {
        return Err(EngineError::InvalidOption(
            "merge requires at least one input".into(),
        ));
    }

    // Parse (and encrypted-gate) every input, attributing errors to their
    // 1-indexed position in the input slice.
    let mut docs: Vec<Document> = Vec::with_capacity(pdfs.len());
    for (idx, bytes) in pdfs.iter().enumerate() {
        let doc = super::parse_input(bytes).map_err(|e| annotate_input_err(idx, e))?;
        docs.push(doc);
    }

    // Single-input shortcut: round-trip through finalize so /Producer is
    // stamped uniformly.
    if docs.len() == 1 {
        return super::finalize(docs.into_iter().next().unwrap());
    }

    // Multi-input path.
    let mut merged = Document::with_version("1.7");
    let mut pages_in_order: Vec<ObjectId> = Vec::new();
    let mut max_id: u32 = 1;
    let mut outlines_from_first: Option<ObjectId> = None;

    for (idx, mut doc) in docs.into_iter().enumerate() {
        // Shift all object IDs in this input into a disjoint range so
        // nothing collides with previously-absorbed inputs.
        doc.renumber_objects_with(max_id);
        max_id = doc.max_id + 1;

        // Collect this doc's page object IDs in page order.
        pages_in_order.extend(doc.get_pages().into_values());

        // On the first input only, remember the /Outlines id so we can
        // wire it into the merged /Catalog. Subsequent inputs drop theirs.
        if idx == 0 {
            outlines_from_first = first_outlines_id(&doc);
        }

        // Copy every non-Catalog, non-root-Pages object into the merged
        // document. The fresh /Pages tree and /Catalog are built below;
        // carrying the old ones would create dangling roots.
        for (id, obj) in doc.objects {
            if is_catalog_or_pages(&obj) {
                continue;
            }
            merged.objects.insert(id, obj);
        }
    }

    // Build a fresh /Pages tree that references all collected page objects.
    let pages_id = merged.new_object_id();
    for page_id in &pages_in_order {
        if let Some(Object::Dictionary(d)) = merged.objects.get_mut(page_id) {
            d.set("Parent", Object::Reference(pages_id));
        }
    }
    let kids: Vec<Object> = pages_in_order
        .iter()
        .map(|id| Object::Reference(*id))
        .collect();
    let page_count = pages_in_order.len() as u32;
    merged.objects.insert(
        pages_id,
        Object::Dictionary(dictionary! {
            "Type" => "Pages",
            "Kids" => kids,
            "Count" => page_count,
        }),
    );

    // Build a fresh /Catalog. /AcroForm and /Names are intentionally
    // omitted. /Outlines, if present on the first input, is preserved.
    let mut catalog = dictionary! {
        "Type" => "Catalog",
        "Pages" => pages_id,
    };
    if let Some(outlines_id) = outlines_from_first {
        catalog.set("Outlines", Object::Reference(outlines_id));
    }
    let catalog_id = merged.add_object(catalog);
    merged.trailer.set("Root", catalog_id);

    // Compact IDs and stamp producer in the common finalize step.
    merged.renumber_objects();
    super::finalize(merged)
}

fn annotate_input_err(idx: usize, err: EngineError) -> EngineError {
    match err {
        EngineError::Internal(msg) => {
            EngineError::Internal(format!("merge: input #{}: {msg}", idx + 1))
        }
        other => other,
    }
}

fn first_outlines_id(doc: &Document) -> Option<ObjectId> {
    let root_ref = doc.trailer.get(b"Root").ok()?.as_reference().ok()?;
    let catalog = doc.get_object(root_ref).ok()?.as_dict().ok()?;
    catalog.get(b"Outlines").ok()?.as_reference().ok()
}

fn is_catalog_or_pages(obj: &Object) -> bool {
    let Object::Dictionary(d) = obj else {
        return false;
    };
    let Ok(name) = d.get(b"Type").and_then(|o| o.as_name()) else {
        return false;
    };
    name == b"Catalog" || name == b"Pages"
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pdfops::test_support::{make_blank_pdf, make_multipage_pdf};

    fn page_count(pdf: &[u8]) -> usize {
        Document::load_mem(pdf).unwrap().get_pages().len()
    }

    fn producer_of(pdf: &[u8]) -> String {
        let doc = Document::load_mem(pdf).unwrap();
        let info_id = doc.trailer.get(b"Info").unwrap().as_reference().unwrap();
        let info = doc.get_object(info_id).unwrap().as_dict().unwrap();
        let bytes = match info.get(b"Producer").unwrap() {
            Object::String(b, _) => b.clone(),
            other => panic!("unexpected producer: {other:?}"),
        };
        String::from_utf8(bytes).unwrap()
    }

    #[test]
    fn merge_empty_input_rejected() {
        let err = merge(&[]).unwrap_err();
        assert!(matches!(err, EngineError::InvalidOption(_)));
    }

    #[test]
    fn merge_single_input_round_trips() {
        let pdf = make_blank_pdf();
        let out = merge(&[&pdf]).unwrap();
        assert_eq!(page_count(&out), 1);
        assert!(producer_of(&out).starts_with("folio/"));
    }

    #[test]
    fn merge_two_blanks_yields_two_pages() {
        let a = make_blank_pdf();
        let b = make_blank_pdf();
        let out = merge(&[&a, &b]).unwrap();
        assert_eq!(page_count(&out), 2);
    }

    #[test]
    fn merge_preserves_page_count_across_mixed_inputs() {
        let one = make_blank_pdf();
        let three = make_multipage_pdf(3, 612, 792);
        let two = make_multipage_pdf(2, 595, 842); // A4 in points.
        let out = merge(&[&one, &three, &two]).unwrap();
        assert_eq!(page_count(&out), 6);
    }

    #[test]
    fn merge_invalid_option_message_includes_index() {
        let good = make_blank_pdf();
        let err = merge(&[&good, b"not a pdf"]).unwrap_err();
        match err {
            EngineError::Internal(msg) => {
                assert!(msg.contains("input #2"), "msg: {msg}");
            }
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn merge_producer_set_to_folio_version() {
        let a = make_blank_pdf();
        let b = make_blank_pdf();
        let out = merge(&[&a, &b]).unwrap();
        assert_eq!(
            producer_of(&out),
            format!("folio/{}", env!("CARGO_PKG_VERSION"))
        );
    }
}
