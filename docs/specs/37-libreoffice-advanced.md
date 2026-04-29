# Spec 37 — LibreOffice Advanced Form Fields

> Comprehensive list of LibreOffice form fields that Gotenberg
> supports but Folio is missing. These 30+ fields control PDF
> export options, bookmarks, notes, viewer preferences, and native
> watermarks.

## Goal

Implement all missing LibreOffice form fields to achieve full parity
with Gotenberg's LibreOffice conversion capabilities.

## Scope

**In:**

All LibreOffice form fields from Gotenberg that Folio is missing:

### Bookmarks & Index (5 fields)
- `exportBookmarks` - Export bookmarks to PDF
- `exportBookmarksToPdfDestination` - Export to Named Destination
- `updateIndexes` - Update document indexes
- `autoIndexBookmarks` - Auto-index bookmarks (merge)
- `bookmarks` (for merge) - Custom bookmarks with offsets

### Form Fields & Placeholders (3 fields)
- `exportFormFields` - Export as interactive form widgets
- `allowDuplicateFieldNames` - Allow duplicate field names
- `exportPlaceholders` - Export placeholder markings

### Notes & Margins (4 fields)
- `exportNotes` - Export notes to PDF
- `exportNotesPages` - Export notes pages (Impress)
- `exportOnlyNotesPages` - Export only notes pages
- `exportNotesInMargin` - Export notes in margin

### Advanced Options (8 fields)
- `convertOooTargetToPdfTarget` - Convert .od* links to .pdf
- `exportLinksRelativeFsys` - Export links as relative
- `exportHiddenSlides` - Export hidden slides (Impress)
- `skipEmptyPages` - Suppress empty pages
- `addOriginalDocumentAsStream` - Add source doc as stream
- `singlePageSheets` - Single page sheets
- `losslessImageCompression` - Use lossless compression
- `reduceImageResolution` - Reduce image resolution

### Native Watermarks (6 fields)
- `nativeWatermarkText` - Watermark text
- `nativeWatermarkColor` - RGB color
- `nativeWatermarkFontHeight` - Font height
- `nativeWatermarkRotateAngle` - Rotation angle
- `nativeWatermarkFontName` - Font name
- `nativeTiledWatermarkText` - Tiled watermark text

### PDF Viewer Preferences (15 fields, Gotenberg 8.29.0+)
- `initialView` - Initial view mode
- `initialPage` - Initial page number
- `magnification` - Magnification level
- `zoom` - Zoom level
- `pageLayout` - Page layout
- `firstPageOnLeft` - First page on left
- `resizeWindowToInitialPage` - Resize to initial page
- `centerWindow` - Center window
- `openInFullScreenMode` - Open fullscreen
- `displayPDFDocumentTitle` - Display document title
- `hideViewerMenubar` - Hide menu bar
- `hideViewerToolbar` - Hide toolbar
- `hideViewerWindowControls` - Hide window controls
- `useTransitionEffects` - Use transition effects
- `openBookmarkLevels` - Open bookmark levels

**Out:**

- Fields that require LibreOffice API access beyond command-line flags
- Fields that are deprecated in LibreOffice 7.x+

## Form Fields (Missing in Folio)

### 1. Bookmarks & Index

| Field | Type | Gotenberg Source | LibreOffice Flag | Description |
|-------|------|------------------|-------------------|-------------|
| `exportBookmarks` | boolean | `pkg/modules/libreoffice/formfield.go:ExportBookmarks` | `--export-bookmarks` | Export bookmarks to PDF outline |
| `exportBookmarksToPdfDestination` | boolean | `pkg/modules/libreoffice/formfield.go:ExportBookmarksToPdfDestination` | `--export-bookmarks-to-pdf-destination` | Export to PDF Named Destination |
| `updateIndexes` | boolean | `pkg/modules/libreoffice/formfield.go:UpdateIndexes` | `--update-indexes` | Update document indexes |
| `autoIndexBookmarks` | boolean | `pkg/modules/libreoffice/formfield.go:AutoIndexBookmarks` | (merge only) | Auto-index when merging |
| `bookmarks` | JSON | `pkg/modules/libreoffice/formfield.go:Bookmarks` | (merge only) | Custom bookmarks with page offsets |

#### `bookmarks` JSON Format

```json
[
  {
    "title": "Chapter 1",
    "page": 1,
    "children": [
      {"title": "Section 1.1", "page": 2, "children": []}
    ]
  }
]
```

### 2. Form Fields & Placeholders

| Field | Type | Gotenberg Source | LibreOffice Flag | Description |
|-------|------|------------------|-------------------|-------------|
| `exportFormFields` | boolean | `pkg/modules/libreoffice/formfield.go:ExportFormFields` | `--export-form-fields` | Export as interactive form widgets |
| `allowDuplicateFieldNames` | boolean | `pkg/modules/libreoffice/formfield.go:AllowDuplicateFieldNames` | `--allow-duplicate-field-names` | Allow duplicate field names |
| `exportPlaceholders` | boolean | `pkg/modules/libreoffice/formfield.go:ExportPlaceholders` | `--export-placeholders` | Export placeholder markings |

### 3. Notes & Margins

| Field | Type | Gotenberg Source | LibreOffice Flag | Description |
|-------|------|------------------|-------------------|-------------|
| `exportNotes` | boolean | `pkg/modules/libreoffice/formfield.go:ExportNotes` | `--export-notes` | Export notes to PDF |
| `exportNotesPages` | boolean | `pkg/modules/libreoffice/formfield.go:ExportNotesPages` | `--export-notes-pages` | Export notes pages (Impress) |
| `exportOnlyNotesPages` | boolean | `pkg/modules/libreoffice/formfield.go:ExportOnlyNotesPages` | `--export-only-notes-pages` | Export only notes pages |
| `exportNotesInMargin` | boolean | `pkg/modules/libreoffice/formfield.go:ExportNotesInMargin` | `--export-notes-in-margin` | Export notes in margin |

### 4. Advanced Options

| Field | Type | Gotenberg Source | LibreOffice Flag | Description |
|-------|------|------------------|-------------------|-------------|
| `convertOooTargetToPdfTarget` | boolean | `pkg/modules/libreoffice/formfield.go:ConvertOooTargetToPdfTarget` | `--convert-ooo-target-to-pdf-target` | Convert .od* links to .pdf |
| `exportLinksRelativeFsys` | boolean | `pkg/modules/libreoffice/formfield.go:ExportLinksRelativeFsys` | `--export-links-relative-fsys` | Export links as relative |
| `exportHiddenSlides` | boolean | `pkg/modules/libreoffice/formfield.go:ExportHiddenSlides` | `--export-hidden-slides` | Export hidden slides (Impress) |
| `skipEmptyPages` | boolean | `pkg/modules/libreoffice/formfield.go:SkipEmptyPages` | `--skip-empty-pages` | Suppress empty pages |
| `addOriginalDocumentAsStream` | boolean | `pkg/modules/libreoffice/formfield.go:AddOriginalDocumentAsStream` | `--add-original-document-as-stream` | Add source doc as stream |
| `singlePageSheets` | boolean | `pkg/modules/libreoffice/formfield.go:SinglePageSheets` | `--single-page-sheets` | Single page sheets |
| `losslessImageCompression` | boolean | `pkg/modules/libreoffice/formfield.go:LosslessImageCompression` | `--lossless-image-compression` | Use lossless compression |
| `reduceImageResolution` | boolean | `pkg/modules/libreoffice/formfield.go:ReduceImageResolution` | `--reduce-image-resolution` | Reduce image resolution |

### 5. Native Watermarks (LibreOffice-side)

| Field | Type | Gotenberg Source | LibreOffice Flag | Description |
|-------|------|------------------|-------------------|-------------|
| `nativeWatermarkText` | string | `pkg/modules/libreoffice/formfield.go:NativeWatermarkText` | `--watermark-text` | Watermark text |
| `nativeWatermarkColor` | integer | `pkg/modules/libreoffice/formfield.go:NativeWatermarkColor` | `--watermark-color` | RGB color (0xRRGGBB) |
| `nativeWatermarkFontHeight` | integer | `pkg/modules/libreoffice/formfield.go:NativeWatermarkFontHeight` | `--watermark-font-height` | Font height in points |
| `nativeWatermarkRotateAngle` | integer | `pkg/modules/libreoffice/formfield.go:NativeWatermarkRotateAngle` | `--watermark-rotate-angle` | Rotation angle (degrees) |
| `nativeWatermarkFontName` | string | `pkg/modules/libreoffice/formfield.go:NativeWatermarkFontName` | `--watermark-font-name` | Font name |
| `nativeTiledWatermarkText` | string | `pkg/modules/libreoffice/formfield.go:NativeTiledWatermarkText` | `--tiled-watermark-text` | Tiled watermark text |

### 6. PDF Viewer Preferences (Gotenberg 8.29.0+)

| Field | Type | Gotenberg Source | Description |
|-------|------|------------------|-------------|
| `initialView` | integer | `pkg/modules/libreoffice/formfield.go:InitialView` | Initial view mode (0=Default, 1=Bookmarks, 2=Thumbnails, 3=Layers) |
| `initialPage` | integer | `pkg/modules/libreoffice/formfield.go:InitialPage` | Initial page number (1-indexed) |
| `magnification` | integer | `pkg/modules/libreoffice/formfield.go:Magnification` | Magnification level (0=Default, 1=Fit width, 2=Fit page, 3=10-400%) |
| `zoom` | integer | `pkg/modules/libreoffice/formfield.go:Zoom` | Zoom level (percentage) |
| `pageLayout` | integer | `pkg/modules/libreoffice/formfield.go:PageLayout` | Page layout (0=Default, 1=Single page, 2=Continuous, 3=Facing, 4=Continuous facing) |
| `firstPageOnLeft` | boolean | `pkg/modules/libreoffice/formfield.go:FirstPageOnLeft` | First page on left |
| `resizeWindowToInitialPage` | boolean | `pkg/modules/libreoffice/formfield.go:ResizeWindowToInitialPage` | Resize to initial page |
| `centerWindow` | boolean | `pkg/modules/libreoffice/formfield.go:CenterWindow` | Center window |
| `openInFullScreenMode` | boolean | `pkg/modules/libreoffice/formfield.go:OpenInFullScreenMode` | Open fullscreen |
| `displayPDFDocumentTitle` | boolean | `pkg/modules/libreoffice/formfield.go:DisplayPDFDocumentTitle` | Display document title |
| `hideViewerMenubar` | boolean | `pkg/modules/libreoffice/formfield.go:HideViewerMenubar` | Hide menu bar |
| `hideViewerToolbar` | boolean | `pkg/modules/libreoffice/formfield.go:HideViewerToolbar` | Hide toolbar |
| `hideViewerWindowControls` | boolean | `pkg/modules/libreoffice/formfield.go:HideViewerWindowControls` | Hide window controls |
| `useTransitionEffects` | boolean | `pkg/modules/libreoffice/formfield.go:UseTransitionEffects` | Use transition effects |
| `openBookmarkLevels` | integer | `pkg/modules/libreoffice/formfield.go:OpenBookmarkLevels` | Open bookmark levels (0=none, 1+=expand N levels) |

## Implementation

### 1. Extend `OfficeOptions` in `crates/engine/src/libreoffice/mod.rs`

```rust
pub struct OfficeOptions {
    // ... existing fields ...

    // Bookmarks & Index
    pub export_bookmarks: bool,
    pub export_bookmarks_to_pdf_destination: bool,
    pub update_indexes: bool,
    pub auto_index_bookmarks: bool,
    pub bookmarks: Option<Vec<Bookmark>>,

    // Form Fields
    pub export_form_fields: bool,
    pub allow_duplicate_field_names: bool,
    pub export_placeholders: bool,

    // Notes
    pub export_notes: bool,
    pub export_notes_pages: bool,
    pub export_only_notes_pages: bool,
    pub export_notes_in_margin: bool,

    // Advanced
    pub convert_ooo_target_to_pdf_target: bool,
    pub export_links_relative_fsys: bool,
    pub export_hidden_slides: bool,
    pub skip_empty_pages: bool,
    pub add_original_document_as_stream: bool,
    pub single_page_sheets: bool,
    pub lossless_image_compression: bool,
    pub reduce_image_resolution: bool,

    // Native Watermarks
    pub native_watermark_text: Option<String>,
    pub native_watermark_color: Option<u32>,  // RGB as 0xRRGGBB
    pub native_watermark_font_height: Option<u32>,
    pub native_watermark_rotate_angle: Option<i32>,
    pub native_watermark_font_name: Option<String>,
    pub native_tiled_watermark_text: Option<String>,

    // PDF Viewer Preferences
    pub initial_view: Option<i32>,
    pub initial_page: Option<i32>,
    pub magnification: Option<i32>,
    pub zoom: Option<i32>,
    pub page_layout: Option<i32>,
    pub first_page_on_left: bool,
    pub resize_window_to_initial_page: bool,
    pub center_window: bool,
    pub open_in_full_screen_mode: bool,
    pub display_pdf_document_title: bool,
    pub hide_viewer_menubar: bool,
    pub hide_viewer_toolbar: bool,
    pub hide_viewer_window_controls: bool,
    pub use_transition_effects: bool,
    pub open_bookmark_levels: Option<i32>,
}
```

### 2. Build LibreOffice Command Args

```rust
impl OfficeOptions {
    pub fn to_libreoffice_args(&self) -> Vec<String> {
        let mut args = Vec::new();

        // Bookmarks
        if self.export_bookmarks {
            args.push("--export-bookmarks".into());
        }
        if self.export_bookmarks_to_pdf_destination {
            args.push("--export-bookmarks-to-pdf-destination".into());
        }
        if self.update_indexes {
            args.push("--update-indexes".into());
        }

        // Form Fields
        if self.export_form_fields {
            args.push("--export-form-fields".into());
        }
        if self.allow_duplicate_field_names {
            args.push("--allow-duplicate-field-names".into());
        }
        if self.export_placeholders {
            args.push("--export-placeholders".into());
        }

        // Notes
        if self.export_notes {
            args.push("--export-notes".into());
        }
        if self.export_notes_pages {
            args.push("--export-notes-pages".into());
        }
        if self.export_only_notes_pages {
            args.push("--export-only-notes-pages".into());
        }
        if self.export_notes_in_margin {
            args.push("--export-notes-in-margin".into());
        }

        // Advanced
        if self.convert_ooo_target_to_pdf_target {
            args.push("--convert-ooo-target-to-pdf-target".into());
        }
        if self.export_links_relative_fsys {
            args.push("--export-links-relative-fsys".into());
        }
        if self.export_hidden_slides {
            args.push("--export-hidden-slides".into());
        }
        if self.skip_empty_pages {
            args.push("--skip-empty-pages".into());
        }
        if self.add_original_document_as_stream {
            args.push("--add-original-document-as-stream".into());
        }
        if self.single_page_sheets {
            args.push("--single-page-sheets".into());
        }
        if self.lossless_image_compression {
            args.push("--lossless-image-compression".into());
        }
        if self.reduce_image_resolution {
            args.push("--reduce-image-resolution".into());
        }

        // Native Watermarks
        if let Some(ref text) = self.native_watermark_text {
            args.push(format!("--watermark-text={}", text));
        }
        if let Some(color) = self.native_watermark_color {
            args.push(format!("--watermark-color={}", color));
        }
        // ... etc.

        // Viewer Preferences
        if let Some(view) = self.initial_view {
            args.push(format!("--initial-view={}", view));
        }
        // ... etc.

        args
    }
}
```

### 3. Form Field Parsing in `crates/server/src/routes/libreoffice.rs`

```rust
// Parse all new fields from form data:
if let Some(val) = form.get("exportBookmarks") {
    opts.export_bookmarks = val == "true";
}

if let Some(json) = form.get("bookmarks") {
    opts.bookmarks = serde_json::from_str(json).ok();
}

// ... parse all 30+ fields
```

## References to Gotenberg Source

| Feature | Gotenberg File | Line Numbers |
|---------|------------------|-------------|
| All form fields | `pkg/modules/libreoffice/formfield.go` | Full file (300+ lines) |
| Command arg building | `pkg/modules/libreoffice/libreoffice.go` | ~L100-200 |
| Viewer preferences | `pkg/modules/libreoffice/formfield.go` | ~L200-300 |

To read Gotenberg source:
```bash
cd /Users/__deesh_reddy__/projects/personal_git/rust_builds/folio/gotenberg
cat pkg/modules/libreoffice/formfield.go | grep -A3 "ExportBookmarks"
```

## Expected Behavior

### Bookmarks
- `exportBookmarks=true` → PDF has outline/bookmarks panel open
- `bookmarks` JSON → Custom bookmark tree with page offsets
- `autoIndexBookmarks=true` → Auto-generate bookmarks when merging

### Form Fields
- `exportFormFields=true` → PDF has interactive form widgets
- `allowDuplicateFieldNames=true` → Allow duplicate field names in forms

### Notes
- `exportNotes=true` → Writer notes exported to PDF
- `exportNotesPages=true` → Impress notes pages included
- `exportNotesInMargin=true` → Notes appear in margin

### Viewer Preferences
- `initialView=1` → Open with bookmarks panel visible
- `zoom=150` → Default zoom level 150%
- `openInFullScreenMode=true` → Open in fullscreen
- `hideViewerToolbar=true` → Hide toolbar

## Test Plan

### Unit Tests

- `parse_export_bookmarks_from_form`
- `parse_native_watermark_text`
- `parse_viewer_preferences_all_fields`
- `bookmarks_json_deserializes_correctly`

### Integration Tests

- `export_bookmarks_creates_outline`
- `native_watermark_appears_in_pdf`
- `viewer_preference_initial_view`
- `export_notes_pages_impress`

## Acceptance

- [ ] `OfficeOptions` extended with all 30+ fields
- [ ] Form field parsing in `libreoffice.rs` route
- [ ] LibreOffice command args built correctly
- [ ] Unit tests for all parsers
- [ ] Integration tests for key features
- [ ] `cargo clippy -p engine -- -D warnings` clean

## References

- Gotenberg LibreOffice form fields: `/Users/__deesh_reddy__/projects/personal_git/rust_builds/folio/gotenberg/pkg/modules/libreoffice/formfield.go`
- LibreOffice CLI options: https://help.libreoffice.org/latest/en-US/text/shared/guide/pdf_params.html
- PDF viewer preferences: PDF spec ISO 32000-2, clause 12.3
