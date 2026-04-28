# LibreOffice integration-test fixtures

The unit tests for `engine::libreoffice` need no fixtures.
The integration tests under `crates/engine/tests/libreoffice.rs`
read the documents in this directory to exercise an end-to-end conversion
through a system `soffice` binary. They skip gracefully when LibreOffice
is not installed.

## Files

| File         | Purpose                                                                                  |
|--------------|------------------------------------------------------------------------------------------|
| `sample.rtf` | Three-page writer document with explicit `\page` controls (RTF maps to writer filter).   |
| `sample.csv` | Ten-row spreadsheet (CSV maps to `pdf:calc_pdf_Export`).                                 |

## Deviation from spec

Spec 12 asks for `sample.docx`, `sample.xlsx`, `sample.pptx`. Those are
binary OOXML containers; we cannot author or commit them without a
running LibreOffice. The two text-format fixtures above exercise the
*same* code paths — the writer and calc filter rules, the option blob
encoder, and the subprocess plumbing — at zero binary cost.

### Why RTF and not HTML for the writer fixture?

LibreOffice's HTML importer ignores CSS `page-break-before` rules, so
HTML fixtures collapse to a single page regardless of intent. RTF's
`\page` control word is honoured 100%, giving us a deterministic
multi-page document for the page-ranges and landscape tests.

### Why CSV is *not* used for the landscape test

`IsLandscape` in the filter-options blob is honoured by the writer
export module but not by calc — calc's orientation is driven by the
document's page style, which a freshly-imported CSV does not set.

If you want true OOXML coverage locally:

```sh
cd crates/engine/tests/fixtures/office
soffice --headless --convert-to docx sample.rtf
soffice --headless --convert-to xlsx sample.csv
# A `sample.pptx` would need an Impress source. Author one in LibreOffice
# Impress and save as .pptx; keep it under 50 KB.
```
