//! `flatten` — strip interactive widgets and annotations from a PDF.
//!
//! The MVP behaviour drops the catalog's `/AcroForm` entry and removes the
//! `/Annots` array from every page. Already-flat PDFs are returned through
//! the standard finalize pipeline (producer + compress + save) so the
//! operation is idempotent up to `/ModDate`.
//!
//! Note: spec 13 also calls for baking each widget's `/AP /N` appearance
//! stream into the page's content stream as a `Form XObject` `Do` call.
//! That requires resource-table editing and a `cm` matrix derived from
//! `/Rect` and `/BBox`; it's tracked as a follow-up. Unfilled widgets,
//! which lack `/AP /N`, are correctly handled by the current behaviour.

use lopdf::Object;

use crate::types::EngineResult;

/// Flatten interactive form fields and annotations into static page
/// content. Idempotent on already-flat PDFs.
///
/// # Errors
///
/// [`crate::EngineError::Internal`] if the input fails to parse, is
/// encrypted, or the result fails to save.
pub fn flatten(pdf: &[u8]) -> EngineResult<Vec<u8>> {
    let mut doc = super::parse_input(pdf)?;

    // 1. Remove the catalog's /AcroForm entry. Forms are no longer
    //    interactive.
    if let Ok(root_ref) = doc.trailer.get(b"Root").and_then(|o| o.as_reference())
        && let Ok(Object::Dictionary(catalog)) = doc.get_object_mut(root_ref)
    {
        catalog.remove(b"AcroForm");
    }

    // 2. For each page, drop /Annots.
    let page_ids: Vec<lopdf::ObjectId> = doc.get_pages().into_values().collect();
    for page_id in page_ids {
        if let Ok(Object::Dictionary(page)) = doc.get_object_mut(page_id) {
            page.remove(b"Annots");
        }
    }

    super::finalize(doc)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pdfops::test_support::{
        make_blank_pdf, make_multipage_pdf, make_pdf_with_form_widget,
    };
    use lopdf::Document;

    fn page_count(pdf: &[u8]) -> usize {
        Document::load_mem(pdf).unwrap().get_pages().len()
    }

    fn catalog_has_acroform(pdf: &[u8]) -> bool {
        let doc = Document::load_mem(pdf).unwrap();
        let root = doc
            .trailer
            .get(b"Root")
            .and_then(|o| o.as_reference())
            .expect("Root ref");
        let catalog = doc.get_object(root).unwrap().as_dict().unwrap();
        catalog.has(b"AcroForm")
    }

    fn page_has_annots(pdf: &[u8], page_num: u32) -> bool {
        let doc = Document::load_mem(pdf).unwrap();
        let pages = doc.get_pages();
        let id = *pages.get(&page_num).expect("page exists");
        let dict = doc.get_object(id).unwrap().as_dict().unwrap();
        dict.has(b"Annots")
    }

    #[test]
    fn flatten_blank_pdf_round_trips() {
        let pdf = make_blank_pdf();
        let out = flatten(&pdf).unwrap();
        assert_eq!(page_count(&out), 1);
    }

    #[test]
    fn flatten_multipage_preserves_pages() {
        let pdf = make_multipage_pdf(3, 612, 792);
        let out = flatten(&pdf).unwrap();
        assert_eq!(page_count(&out), 3);
    }

    #[test]
    fn flatten_removes_acroform_and_annots() {
        let pdf = make_pdf_with_form_widget();
        assert!(catalog_has_acroform(&pdf), "fixture should have AcroForm");
        assert!(page_has_annots(&pdf, 1), "fixture page should have Annots");

        let flat = flatten(&pdf).unwrap();
        assert!(!catalog_has_acroform(&flat), "AcroForm must be dropped");
        assert!(!page_has_annots(&flat, 1), "page Annots must be dropped");
        assert_eq!(page_count(&flat), 1);
    }

    #[test]
    fn flatten_idempotent_for_already_flat_pdf() {
        let pdf = make_multipage_pdf(2, 612, 792);
        let once = flatten(&pdf).unwrap();
        let twice = flatten(&once).unwrap();
        // Page count is stable; AcroForm absent both times; second call
        // is functionally identical to the first.
        assert_eq!(page_count(&twice), 2);
        assert!(!catalog_has_acroform(&twice));
    }

    #[test]
    fn flatten_idempotent_for_form_pdf() {
        let pdf = make_pdf_with_form_widget();
        let once = flatten(&pdf).unwrap();
        let twice = flatten(&once).unwrap();
        assert!(!catalog_has_acroform(&twice));
        assert!(!page_has_annots(&twice, 1));
        assert_eq!(page_count(&twice), 1);
    }
}
