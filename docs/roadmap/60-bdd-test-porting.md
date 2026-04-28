# BDD Test Porting Plan from Gotenberg

## Overview

Port Gherkin-based integration tests from Gotenberg to Folio's Rust-based testing framework.

## Current Gotenberg Test Coverage (26 Feature Files)

### Core Features

| Feature File | Status | Priority | Notes |
|--------------|--------|----------|-------|
| `health.feature` | ⚠️ Partial | High | Basic health check exists, need detailed JSON |
| `version.feature` | ❌ Missing | Medium | `/version` endpoint |
| `root.feature` | ❌ Missing | Low | Root path redirect |
| `debug.feature` | ❌ Missing | Low | Debug endpoints |

### Chromium Module (7 feature files)

| Feature File | Status | Priority | Notes |
|--------------|--------|----------|-------|
| `chromium_convert_html.feature` | ⚠️ Partial | High | Basic conversion works, need edge cases |
| `chromium_convert_url.feature` | ⚠️ Partial | High | URL to PDF conversion |
| `chromium_convert_markdown.feature` | ❌ Missing | Medium | Markdown support |
| `chromium_screenshot_html.feature` | ❌ Missing | Medium | Screenshot API |
| `chromium_screenshot_url.feature` | ❌ Missing | Medium | URL screenshots |
| `chromium_screenshot_markdown.feature` | ❌ Missing | Low | Markdown screenshots |
| `chromium_concurrent.feature` | ❌ Missing | High | Load testing |

### LibreOffice Module (1 feature file)

| Feature File | Status | Priority | Notes |
|--------------|--------|----------|-------|
| `libreoffice_convert.feature` | ⚠️ Partial | High | Basic conversion exists |

### PDF Engines Module (13 feature files)

| Feature File | Status | Priority | Notes |
|--------------|--------|----------|-------|
| `pdfengines_merge.feature` | ✅ Done | High | Fully implemented |
| `pdfengines_split.feature` | ⚠️ Partial | High | Split works, need more test cases |
| `pdfengines_convert.feature` | ❌ Missing | High | PDF/A, PDF/UA conversion |
| `pdfengines_bookmarks.feature` | ❌ Missing | Medium | Read/write PDF bookmarks |
| `pdfengines_metadata.feature` | ⚠️ Partial | Medium | Basic metadata exists |
| `pdfengines_flatten.feature` | ✅ Done | Medium | Form flattening |
| `pdfengines_rotate.feature` | ✅ Done | Medium | PDF rotation |
| `pdfengines_watermark.feature` | ❌ Missing | Medium | Watermark/stamp |
| `pdfengines_encrypt.feature` | ❌ Missing | Medium | PDF encryption |
| `pdfengines_embed.feature` | ❌ Missing | Low | Embed files into PDF |
| `pdfengines_stamp.feature` | ❌ Missing | Medium | Stamp overlay |

### Infrastructure (3 feature files)

| Feature File | Status | Priority | Notes |
|--------------|--------|----------|-------|
| `webhook.feature` | ❌ Missing | High | Async webhook callbacks |
| `output_filename.feature` | ⚠️ Partial | Medium | Output filename header |
| `prometheus_metrics.feature` | ❌ Missing | Low | Metrics endpoint |

## Missing Features (Not Just Tests)

### Critical Missing Features

1. **PDF/A & PDF/UA Conversion** (`pdfengines_convert`)
   - PDF/A-1b, PDF/A-2b, PDF/A-3b compliance
   - PDF/UA-1, PDF/UA-2 accessibility
   - VeraPDF validation integration

2. **Bookmarks API** (`pdfengines_bookmarks`)
   - POST `/forms/pdfengines/bookmarks/write`
   - POST `/forms/pdfengines/bookmarks/read`
   - JSON bookmark structure

3. **Watermark/Stamp** (`pdfengines_watermark`, `pdfengines_stamp`)
   - Image/text overlay on PDFs
   - Stamp (background) vs Watermark (foreground)

4. **PDF Encryption** (`pdfengines_encrypt`)
   - Password protection
   - Permission settings

5. **File Embedding** (`pdfengines_embed`)
   - Embed source files into PDF (PDF/A-3)

6. **Markdown Support** (Chromium)
   - Convert Markdown to HTML then PDF

7. **Screenshot API** (Chromium)
   - Page/screenshot endpoint (different from PDF)

8. **Webhook System**
   - Async processing with callbacks
   - Webhook error handling
   - Webhook events

### API Compatibility Gaps

1. **Headers**
   - `Gotenberg-Output-Filename` - custom output filename
   - `Gotenberg-Trace` - correlation ID
   - `Gotenberg-Webhook-*` - webhook configuration

2. **Response Headers**
   - `Gotenberg-Trace` in response
   - `Content-Disposition` with filename

3. **Endpoints**
   - `/version` - version info
   - `/forms/chromium/screenshot/*` - screenshot variants
   - `/forms/pdfengines/bookmarks/*` - bookmark operations
   - `/forms/pdfengines/convert` - PDF/A conversion

## Test Framework Architecture

### Current Approach (Gotenberg)

```go
// Uses: godog (Gherkin) + testcontainers-go
Feature: PDF Merge
  Scenario: Merge 2 PDFs
    Given I have a default Gotenberg container
    When I make a "POST" request to "/forms/pdfengines/merge"
    Then the response status code should be 200
    Then there should be 1 PDF(s) in the response
```

### Proposed Rust Approach

Use `cucumber-rust` crate for Gherkin support with `testcontainers-rs`:

```rust
// crates/server/tests/bdd/features/pdfengines_merge.feature
Feature: /forms/pdfengines/merge

  Scenario: POST /forms/pdfengines/merge (default)
    Given a running Folio container
    When I post files "page_1.pdf,page_2.pdf" to "/pdfops/merge"
    Then the response status should be 200
    And the response should contain 1 PDF with 2 pages
```

### Test Infrastructure Needed

1. **testcontainers-rs** setup
   - Generic container for Folio
   - Volume mounting for testdata
   - Network configuration

2. **Step Definitions** (`crates/server/tests/bdd/steps/`)
   - `container.rs` - Container lifecycle
   - `http.rs` - HTTP requests/responses
   - `pdf.rs` - PDF assertions (pages, content, validation)
   - `files.rs` - File operations

3. **Test Data** (`crates/server/tests/bdd/testdata/`)
   - Copy from Gotenberg's testdata
   - Sample PDFs, HTML, Office docs

## Implementation Plan

### Phase 1: Infrastructure (Week 1)

- [ ] Add `cucumber` and `testcontainers` dependencies
- [ ] Create BDD test directory structure
- [ ] Implement basic step definitions
- [ ] Setup testcontainers for Folio

### Phase 2: Core Tests (Week 2)

- [ ] Port `health.feature` tests
- [ ] Port `pdfengines_merge.feature` tests
- [ ] Port `pdfengines_split.feature` tests
- [ ] Add missing PDF content assertions

### Phase 3: PDF/A & Advanced Features (Week 3-4)

- [ ] Implement PDF/A conversion feature
- [ ] Port `pdfengines_convert.feature` tests
- [ ] Add VeraPDF validation in testcontainers
- [ ] Implement bookmarks API

### Phase 4: Chromium & LibreOffice (Week 5)

- [ ] Port `chromium_convert_*` feature tests
- [ ] Port `chromium_screenshot_*` feature tests
- [ ] Port `libreoffice_convert.feature` tests

### Phase 5: Infrastructure Features (Week 6)

- [ ] Implement webhook system
- [ ] Port `webhook.feature` tests
- [ ] Add output filename header
- [ ] Add trace/correlation ID

## Dependencies to Add

```toml
[dev-dependencies]
cucumber = "0.21"
testcontainers = "0.22"
testcontainers-modules = { version = "0.11", features = ["blocking"] }
reqwest = { version = "0.12", features = ["multipart", "blocking"] }
pdf-extract = "0.8"  # For PDF content assertions
```

## Notes

- Gotenberg uses `veraPDF` for PDF/A validation - need equivalent
- Testcontainers approach: start Folio container per scenario
- PDF content extraction: use `pdf-extract` or `poppler-utils`
- Parallel test execution: use `--test-threads=1` for BDD tests
