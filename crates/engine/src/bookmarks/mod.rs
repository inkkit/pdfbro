//! PDF bookmarks (document outlines) read/write operations.
//!
//! Implements spec 16 — PDF Bookmarks.
//! Updated for lopdf 0.34+ API.

use std::collections::HashMap;

use lopdf::{Dictionary, Document, Object, ObjectId};
use serde::{Deserialize, Serialize};

use crate::types::{EngineError, EngineResult};

/// A single bookmark entry with optional children.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Bookmark {
    /// Display text for the bookmark.
    pub title: String,
    /// Target page number (1-indexed for user convenience).
    pub page: u32,
    /// Child bookmarks (nested outline items).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<Bookmark>,
}

/// Read bookmarks from a PDF document.
///
/// Returns empty vector if document has no outline.
/// Page numbers are 1-indexed (first page = 1).
pub fn read_bookmarks(pdf: &[u8]) -> EngineResult<Vec<Bookmark>> {
    let doc = Document::load_mem(pdf)?;

    // Get catalog and check for Outlines
    let catalog_id = doc
        .trailer
        .get(b"Root")
        .ok()
        .and_then(|o| o.as_reference().ok())
        .ok_or_else(|| EngineError::InvalidOption("PDF has no catalog".into()))?;

    let catalog = doc
        .get_object(catalog_id)
        .ok()
        .and_then(|o| o.as_dict().ok())
        .ok_or_else(|| EngineError::InvalidOption("PDF catalog not found".into()))?;

    let outlines_ref = match catalog.get(b"Outlines") {
        Ok(Object::Reference(id)) => *id,
        Ok(_) => return Err(EngineError::InvalidOption("Invalid Outlines entry".into())),
        Err(_) => return Ok(Vec::new()), // No outline
    };

    let outlines = doc
        .get_object(outlines_ref)
        .ok()
        .and_then(|o| o.as_dict().ok())
        .ok_or_else(|| EngineError::InvalidOption("Outlines dictionary not found".into()))?;

    // Build page number mapping (ObjectId -> page number)
    let page_map = build_page_map(&doc)?;

    // Get first outline item
    let first_ref = match outlines.get(b"First") {
        Ok(Object::Reference(id)) => *id,
        _ => return Ok(Vec::new()), // Empty outline
    };

    // Traverse outline tree
    let bookmarks = traverse_outlines(&doc, first_ref, &page_map)?;

    Ok(bookmarks)
}

/// Write bookmarks to a PDF document.
///
/// Replaces any existing outline. Bookmarks reference pages by 1-based page numbers.
/// Returns modified PDF with new outline.
pub fn write_bookmarks(pdf: &[u8], bookmarks: &[Bookmark]) -> EngineResult<Vec<u8>> {
    let mut doc = Document::load_mem(pdf)?;

    // Build page mapping (page number -> ObjectId)
    let page_map = build_page_map(&doc)?;
    let page_count = doc.get_pages().len() as u32;

    // Validate all bookmark pages exist
    validate_bookmark_pages(bookmarks, page_count)?;

    // Get catalog
    let catalog_id = doc
        .trailer
        .get(b"Root")
        .ok()
        .and_then(|o| o.as_reference().ok())
        .ok_or_else(|| EngineError::InvalidOption("PDF has no catalog".into()))?;

    // Create outline structure
    let (outlines_id, count) = create_outlines(&mut doc, bookmarks, &page_map)?;

    // Update catalog
    if let Ok(Object::Dictionary(catalog)) = doc.get_object_mut(catalog_id) {
        catalog.set("Outlines", Object::Reference(outlines_id));
        // Mark document as having bookmarks
        catalog.set("PageMode", Object::String(b"UseOutlines".to_vec(), lopdf::StringFormat::Literal));
    }

    // Update outlines count
    if let Ok(Object::Dictionary(outlines)) = doc.get_object_mut(outlines_id) {
        outlines.set("Count", Object::Integer(count as i64));
    }

    // Save to bytes
    let mut output = Vec::new();
    doc.save_to(&mut output)?;

    Ok(output)
}

/// Flatten nested bookmark structure to a list.
///
/// Returns: (level, title, page) where level starts at 1.
pub fn flatten_bookmarks(bookmarks: &[Bookmark]) -> Vec<(u32, String, u32)> {
    let mut result = Vec::new();
    flatten_recursive(bookmarks, 1, &mut result);
    result
}

fn flatten_recursive(bookmarks: &[Bookmark], level: u32, result: &mut Vec<(u32, String, u32)>) {
    for bookmark in bookmarks {
        result.push((level, bookmark.title.clone(), bookmark.page));
        if !bookmark.children.is_empty() {
            flatten_recursive(&bookmark.children, level + 1, result);
        }
    }
}

// -----------------------------------------------------------------------------
// Internal helpers
// -----------------------------------------------------------------------------

/// Build mapping from page ObjectId to 1-indexed page number.
fn build_page_map(doc: &Document) -> EngineResult<HashMap<ObjectId, u32>> {
    let pages = doc.get_pages();
    let mut map = HashMap::with_capacity(pages.len());

    for (page_num, (page_id, _)) in pages.iter().enumerate() {
        // page_id is u32 in newer lopdf, convert to ObjectId tuple
        let object_id = (*page_id, 0u16);
        map.insert(object_id, (page_num + 1) as u32);
    }

    Ok(map)
}

/// Traverse outline linked list and build bookmark tree.
fn traverse_outlines(
    doc: &Document,
    first_id: ObjectId,
    page_map: &HashMap<ObjectId, u32>,
) -> EngineResult<Vec<Bookmark>> {
    let mut bookmarks = Vec::new();
    let mut current_id = Some(first_id);

    while let Some(id) = current_id {
        // Parse current item
        if let Ok(item) = doc.get_object(id).and_then(Object::as_dict) {
            if let Some(bookmark) = parse_outline_item(doc, item, page_map).ok() {
                bookmarks.push(bookmark);
            }
        }

        // Get next sibling
        current_id = doc
            .get_object(id)
            .ok()
            .and_then(|o| o.as_dict().ok())
            .and_then(|d| d.get(b"Next").ok())
            .and_then(|o| o.as_reference().ok());
    }

    Ok(bookmarks)
}

/// Parse a single outline item into Bookmark.
fn parse_outline_item(
    doc: &Document,
    item: &Dictionary,
    page_map: &HashMap<ObjectId, u32>,
) -> EngineResult<Bookmark> {
    // Get title — PDF strings may be UTF-16BE-with-BOM; use the same
    // decoder used by the metadata module so we handle all encodings.
    let title = item
        .get(b"Title")
        .ok()
        .and_then(|o| crate::pdfops::decode_pdf_text_string(o))
        .unwrap_or_default();

    // Get destination page
    let page = match item.get(b"Dest") {
        Ok(Object::Array(dest)) if !dest.is_empty() => {
            // First element is page reference
            dest[0]
                .as_reference()
                .ok()
                .and_then(|page_id| page_map.get(&page_id).copied())
                .unwrap_or(1)
        }
        Ok(Object::Reference(page_id)) => {
            page_map.get(&page_id).copied().unwrap_or(1)
        }
        _ => 1,
    };

    // Get children
    let children = if let Ok(Object::Reference(first_child)) = item.get(b"First") {
        traverse_outlines(doc, *first_child, page_map)?
    } else {
        Vec::new()
    };

    Ok(Bookmark { title, page, children })
}

/// Validate that all bookmark pages exist in the document.
fn validate_bookmark_pages(bookmarks: &[Bookmark], page_count: u32) -> EngineResult<()> {
    fn check(bookmarks: &[Bookmark], page_count: u32) -> EngineResult<()> {
        for bookmark in bookmarks {
            if bookmark.page == 0 || bookmark.page > page_count {
                return Err(EngineError::InvalidOption(format!(
                    "Bookmark '{}' references invalid page {} (document has {} pages)",
                    bookmark.title, bookmark.page, page_count
                )));
            }
            check(&bookmark.children, page_count)?;
        }
        Ok(())
    }
    check(bookmarks, page_count)
}

/// Create outline structure from bookmarks.
fn create_outlines(
    doc: &mut Document,
    bookmarks: &[Bookmark],
    page_map: &HashMap<ObjectId, u32>,
) -> EngineResult<(ObjectId, usize)> {
    // Create outlines dictionary
    let outlines_id = doc.new_object_id();

    // Create outline items
    let (first_id, last_id, count) = create_outline_items(doc, bookmarks, page_map, None)?;

    let mut outlines = Dictionary::new();
    if let Some(first) = first_id {
        outlines.set("First", Object::Reference(first));
        outlines.set("Type", Object::String(b"Outlines".to_vec(), lopdf::StringFormat::Literal));
    }
    if let Some(last) = last_id {
        outlines.set("Last", Object::Reference(last));
    }

    doc.objects.insert(outlines_id, Object::Dictionary(outlines));

    Ok((outlines_id, count))
}

/// Recursively create outline items.
fn create_outline_items(
    doc: &mut Document,
    bookmarks: &[Bookmark],
    page_map: &HashMap<ObjectId, u32>,
    parent_id: Option<ObjectId>,
) -> EngineResult<(Option<ObjectId>, Option<ObjectId>, usize)> {
    if bookmarks.is_empty() {
        return Ok((None, None, 0));
    }

    let mut first_id: Option<ObjectId> = None;
    let mut prev_id: Option<ObjectId> = None;
    let mut total_count = 0;

    for (i, bookmark) in bookmarks.iter().enumerate() {
        let item_id = doc.new_object_id();

        if i == 0 {
            first_id = Some(item_id);
        }

        // Get page ObjectId from page number
        let page_id = page_map
            .iter()
            .find(|(_, page)| **page == bookmark.page)
            .map(|(id, _)| *id)
            .ok_or_else(|| EngineError::InvalidOption(format!(
                "Cannot find page {} for bookmark '{}'",
                bookmark.page, bookmark.title
            )))?;

        // Create children first
        let (child_first, child_last, child_count) =
            create_outline_items(doc, &bookmark.children, page_map, Some(item_id))?;

        // Build item dictionary
        let mut item = Dictionary::new();
        item.set("Title", Object::String(
            bookmark.title.clone().into_bytes(),
            lopdf::StringFormat::Literal
        ));
        item.set("Parent", parent_id.map(Object::Reference).unwrap_or_else(|| {
            // Will be set to outlines reference later
            Object::Null
        }));
        item.set("Dest", Object::Array(vec![
            Object::Reference(page_id),
            Object::Name(b"Fit".to_vec()),
        ]));

        if let Some(prev) = prev_id {
            item.set("Prev", Object::Reference(prev));
        }

        if let Some(first_child) = child_first {
            item.set("First", Object::Reference(first_child));
            item.set("Last", Object::Reference(child_last.unwrap()));
            item.set("Count", Object::Integer(child_count as i64));
            total_count += child_count;
        }

        doc.objects.insert(item_id, Object::Dictionary(item));

        // Link previous item to this one
        if let Some(prev) = prev_id {
            if let Ok(Object::Dictionary(prev_item)) = doc.get_object_mut(prev) {
                prev_item.set("Next", Object::Reference(item_id));
            }
        }

        prev_id = Some(item_id);
        total_count += 1;
    }

    Ok((first_id, prev_id, total_count))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bookmark_serialization() {
        let bookmark = Bookmark {
            title: "Chapter 1".into(),
            page: 1,
            children: vec![Bookmark {
                title: "Section 1.1".into(),
                page: 3,
                children: vec![],
            }],
        };

        let json = serde_json::to_string(&bookmark).unwrap();
        assert!(json.contains("Chapter 1"));
        assert!(json.contains("Section 1.1"));

        let parsed: Bookmark = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.title, "Chapter 1");
        assert_eq!(parsed.children.len(), 1);
    }

    #[test]
    fn flatten_bookmarks_test() {
        let bookmarks = vec![
            Bookmark {
                title: "Chapter 1".into(),
                page: 1,
                children: vec![
                    Bookmark { title: "Section 1.1".into(), page: 3, children: vec![] },
                    Bookmark { title: "Section 1.2".into(), page: 5, children: vec![] },
                ],
            },
            Bookmark {
                title: "Chapter 2".into(),
                page: 10,
                children: vec![],
            },
        ];

        let flat = flatten_bookmarks(&bookmarks);
        assert_eq!(flat.len(), 4);
        assert_eq!(flat[0], (1, "Chapter 1".into(), 1));
        assert_eq!(flat[1], (2, "Section 1.1".into(), 3));
        assert_eq!(flat[2], (2, "Section 1.2".into(), 5));
        assert_eq!(flat[3], (1, "Chapter 2".into(), 10));
    }
}
