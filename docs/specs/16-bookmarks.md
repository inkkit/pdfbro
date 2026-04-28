# Spec 16 — PDF Bookmarks (Outlines)

> Read and write PDF document outlines (bookmarks/table of contents).
> Enables navigation structures in PDF documents.

## Goal

Provide read/write access to PDF bookmark hierarchies (Outlines in PDF
terminology). This allows generating tables of contents, extracting
document structure, and adding navigation to merged documents.

## Scope

**In:**

- Read existing bookmark/outline structure from PDF.
- Write new bookmarks to PDF (replacing existing).
- Hierarchical bookmarks with nested children.
- Page number references (0-indexed or 1-indexed configurable).
- JSON serialization for API wire format.

**Out:**

- Partial bookmark updates (merge with existing).
- Text position anchors (only page-level).
- Named destinations (follow-up spec).

## Public API

Module path: `engine::bookmarks`. Stateless free functions.

```rust
use crate::types::{EngineError, EngineResult};

/// A single bookmark entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Bookmark {
    /// Display text for the bookmark.
    pub title: String,
    /// Target page number (1-indexed for user convenience).
    pub page: u32,
    /// Nesting level (1 = top level, 2 = child, etc.).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub level: Option<u32>,
    /// Child bookmarks (nested outline items).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<Bookmark>,
}

/// Read bookmarks from a PDF document.
///
/// Returns empty vector if document has no outline.
pub fn read_bookmarks(pdf: &[u8]) -> EngineResult<Vec<Bookmark>>;

/// Write bookmarks to a PDF document.
///
/// Replaces any existing outline. Bookmarks reference pages by 1-based
/// page numbers. Returns modified PDF with new outline.
pub fn write_bookmarks(pdf: &[u8], bookmarks: &[Bookmark]) -> EngineResult<Vec<u8>>;

/// Flatten nested bookmark structure to a list.
///
/// Useful for linear processing. Level indicates nesting depth.
pub fn flatten_bookmarks(bookmarks: &[Bookmark]) -> Vec<(u32, String, u32)>;
// Returns: (level, title, page)
```

## Bookmark Structure

### JSON Format (API)

```json
[
  {
    "title": "Chapter 1",
    "page": 1,
    "children": [
      {"title": "Section 1.1", "page": 3},
      {"title": "Section 1.2", "page": 5}
    ]
  },
  {
    "title": "Chapter 2",
    "page": 10
  }
]
```

### Flat Format Alternative

For simple lists without nesting:

```json
[
  {"title": "Chapter 1", "page": 1, "level": 1},
  {"title": "Section 1.1", "page": 3, "level": 2},
  {"title": "Chapter 2", "page": 10, "level": 1}
]
```

## Implementation Strategy

### PDF Structure

PDF bookmarks are stored in the `/Outlines` hierarchy:

```
/Outlines (dictionary)
  /First → OutlineItem
  /Last → OutlineItem
  /Count → total count

OutlineItem (dictionary)
  /Title (string)
  /Dest → [page_ref, /Fit]
  /Parent → parent OutlineItem or Outlines
  /First, /Last → child items (if has children)
  /Next, /Prev → sibling items
```

### Using `lopdf`

1. **Read**: Traverse `/Outlines` → `/First` chain, following `/Next` pointers,
   recursively collecting `/Title` and `/Dest` page references.

2. **Write**: Create new outline dictionary, build linked list of items,
   set up parent/child/next/prev references, replace `/Outlines` in catalog.

## Server API

### Read Bookmarks

```
POST /forms/pdfengines/bookmarks/read
```

Form fields:
- `files` - Single PDF file

Response (200 OK):
```json
{
  "filename.pdf": [
    {"title": "Chapter 1", "page": 1, "children": [...]}
  ]
}
```

### Write Bookmarks

```
POST /forms/pdfengines/bookmarks/write
```

Form fields:
- `files` - Single PDF file
- `bookmarks` - JSON array of bookmarks

Response (200 OK):
- PDF file with bookmarks applied
- `Content-Disposition: attachment; filename="result.pdf"`

## Error Handling

| Error | Condition |
|-------|-----------|
| `EngineError::InvalidInput` | PDF has no catalog or is malformed |
| `EngineError::InvalidBookmark` | Bookmark references non-existent page |
| `EngineError::EmptyInput` | Empty bookmark list (valid, clears outline) |

## Testing

Unit tests:
- Read bookmarks from sample PDFs
- Write bookmarks, read back, verify round-trip
- Nested hierarchy preservation
- Page number edge cases (first page, last page)

Integration tests:
- Gotenberg feature parity: `pdfengines_bookmarks.feature`
- Compare with `pdfinfo -meta` output

## Dependencies

Uses existing `lopdf` dependency (already in pdfops).

## Open Questions

1. Should we support named destinations (/Dest as name vs array)?
2. Should we preserve existing bookmarks and merge vs replace?
3. Unicode bookmark titles - any encoding issues?

## References

- ISO 32000-2:2017, Section 12.3.3 (Document Outlines)
- PDF 1.7 spec, Section 8.2.2 (Outline Hierarchy)
