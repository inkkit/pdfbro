# Missing Features Analysis: Folio vs Gotenberg

## Executive Summary

Comparing Folio MVP (v0.1.0) against Gotenberg v8 feature set.

**Folio Status:** ~60% feature parity with Gotenberg v8

## Feature Comparison Matrix

### API Endpoints

| Endpoint | Gotenberg | Folio | Status |
|----------|-----------|-------|--------|
| `GET /health` | ✅ | ✅ | JSON format differs slightly |
| `GET /version` | ✅ | ❌ | Missing |
| `POST /forms/chromium/convert/html` | ✅ | ✅ | Via `/convert/html` |
| `POST /forms/chromium/convert/url` | ✅ | ✅ | Via `/convert/url` |
| `POST /forms/chromium/convert/markdown` | ✅ | ❌ | Missing |
| `POST /forms/chromium/screenshot/html` | ✅ | ❌ | Missing |
| `POST /forms/libreoffice/convert` | ✅ | ✅ | Via `/convert/office` |
| `POST /forms/pdfengines/merge` | ✅ | ✅ | Via `/pdfops/merge` |
| `POST /forms/pdfengines/split` | ✅ | ✅ | Via `/pdfops/split` |
| `POST /forms/pdfengines/flatten` | ✅ | ✅ | Via `/pdfops/flatten` |
| `POST /forms/pdfengines/rotate` | ✅ | ✅ | Via `/pdfops/rotate` |
| `POST /forms/pdfengines/metadata` | ✅ | ⚠️ | Basic support |
| `POST /forms/pdfengines/convert` | ✅ | ❌ | PDF/A conversion missing |
| `POST /forms/pdfengines/bookmarks/*` | ✅ | ❌ | Missing |
| `POST /forms/pdfengines/watermark` | ✅ | ❌ | Missing |
| `POST /forms/pdfengines/stamp` | ✅ | ❌ | Missing |
| `POST /forms/pdfengines/encrypt` | ✅ | ❌ | Missing |
| `POST /forms/pdfengines/embed` | ✅ | ❌ | Missing |

### Form Fields / Parameters

| Parameter | Gotenberg | Folio | Notes |
|-----------|-----------|-------|-------|
| `files` | ✅ | ✅ | Standard file upload |
| `downloadFrom` | ✅ | ❌ | URL download |
| `pdfa` | ✅ | ❌ | PDF/A conversion target |
| `pdfua` | ✅ | ❌ | PDF/UA conversion |
| `bookmarks` | ✅ | ❌ | JSON bookmarks |
| `pages` | ✅ | ✅ | Page ranges |
| `splitMode` | ✅ | ✅ | `intervals` or `pages` |
| `splitSpan` | ✅ | ✅ | Split configuration |
| `rotate` | ✅ | ✅ | Rotation degrees |
| `password` | ✅ | ❌ | Encryption password |
| `metadata` | ✅ | ⚠️ | Basic support only |
| `paperWidth` | ✅ | ✅ | Custom paper size |
| `paperHeight` | ✅ | ✅ | Custom paper size |
| `marginTop` | ✅ | ✅ | Margins |
| `marginBottom` | ✅ | ✅ | Margins |
| `marginLeft` | ✅ | ✅ | Margins |
| `marginRight` | ✅ | ✅ | Margins |
| `landscape` | ✅ | ✅ | Orientation |
| `scale` | ✅ | ✅ | Scaling |
| `nativePageRanges` | ✅ | ❌ | LibreOffice specific |

### Headers

| Header | Gotenberg | Folio | Purpose |
|--------|-----------|-------|---------|
| `Gotenberg-Output-Filename` | ✅ | ❌ | Custom output filename |
| `Gotenberg-Trace` | ✅ | ❌ | Correlation ID |
| `Gotenberg-Webhook-Url` | ✅ | ❌ | Webhook callback |
| `Gotenberg-Webhook-Error-Url` | ✅ | ❌ | Error webhook |
| `Gotenberg-Webhook-Extra-Http-Headers` | ✅ | ❌ | Webhook headers |
| `Gotenberg-Async` | ✅ | ❌ | Async processing |

### Response Headers

| Header | Gotenberg | Folio | Notes |
|--------|-----------|-------|-------|
| `Content-Type` | ✅ | ✅ | Standard |
| `Content-Disposition` | ✅ | ⚠️ | Filename handling basic |
| `Gotenberg-Trace` | ✅ | ❌ | Missing |
| `X-Request-Id` | ❌ | ✅ | Folio uses this |

### Infrastructure Features

| Feature | Gotenberg | Folio | Priority |
|---------|-----------|-------|----------|
| Webhook callbacks | ✅ | ❌ | High |
| Async processing | ✅ | ❌ | High |
| Prometheus metrics | ✅ | ❌ | Medium |
| OpenTelemetry tracing | ✅ | ❌ | Medium |
| Basic Auth | ✅ | ❌ | Medium |
| CORS support | ✅ | ❌ | Low |
| Rate limiting | ✅ | ❌ | Medium |
| Root path prefix | ✅ | ❌ | Low |

### PDF Engines Backend Support

| Engine | Gotenberg | Folio | Notes |
|--------|-----------|-------|-------|
| qpdf | ✅ | ✅ | Primary for merge/split |
| pdfcpu | ✅ | ✅ | Alternative engine |
| pdftk | ✅ | ❌ | Legacy support |
| UniPDF | ❌ | ✅ | Go lib (Gotenberg native) |
| lopdf | ❌ | ✅ | Rust lib (Folio native) |

### Chromium Features

| Feature | Gotenberg | Folio | Notes |
|---------|-----------|-------|-------|
| HTML to PDF | ✅ | ✅ | Via Chromium |
| URL to PDF | ✅ | ✅ | Via Chromium |
| Markdown to PDF | ✅ | ❌ | Convert to HTML first |
| Screenshot (PNG/JPEG) | ✅ | ❌ | Different API |
| Header/Footer templates | ✅ | ✅ | HTML templates |
| Wait conditions | ✅ | ✅ | `load`, `networkidle` |
| Emulated media | ✅ | ✅ | `screen`/`print` |
| JavaScript execution | ✅ | ✅ | `waitForExpression` |
| Extra HTTP headers | ✅ | ⚠️ | Partial support |
| Cookies | ✅ | ⚠️ | Basic support |

### LibreOffice Features

| Feature | Gotenberg | Folio | Notes |
|---------|-----------|-------|-------|
| DOCX to PDF | ✅ | ✅ | Full support |
| ODT to PDF | ✅ | ✅ | Full support |
| PPTX to PDF | ✅ | ✅ | Full support |
| XLSX to PDF | ✅ | ✅ | Full support |
| Native page ranges | ✅ | ❌ | Missing |
| Landscape mode | ✅ | ✅ | Supported |
| PDF/A conversion | ✅ | ❌ | Via post-processing |

## Top Priority Missing Features

### 1. PDF/A & PDF/UA Conversion (High Priority)

**Gotenberg endpoint:** `POST /forms/pdfengines/convert`

**Implementation options:**
- Use `verapdf` CLI tool
- Use `pdfaPilot` (commercial)
- Use Ghostscript with PDF/A compliance
- Use qpdf with PDF/A extensions

### 2. Webhook System (High Priority)

**Gotenberg headers:**
- `Gotenberg-Webhook-Url`
- `Gotenberg-Webhook-Error-Url`
- `Gotenberg-Async`

**Implementation:**
- Add `webhook` module
- Background job processing
- Retry logic with exponential backoff
- Event callbacks

### 3. Bookmarks API (Medium Priority)

**Gotenberg endpoints:**
- `POST /forms/pdfengines/bookmarks/write`
- `POST /forms/pdfengines/bookmarks/read`

**Implementation:**
- Use `lopdf` bookmark/outline manipulation
- JSON bookmark structure

### 4. Watermark/Stamp (Medium Priority)

**Gotenberg endpoints:**
- `POST /forms/pdfengines/watermark`
- `POST /forms/pdfengines/stamp`

**Implementation:**
- Image overlay on PDF
- Text overlay with font support
- Positioning (center, corners, full page)

### 5. Screenshot API (Medium Priority)

**Gotenberg endpoints:**
- `POST /forms/chromium/screenshot/*`

**Implementation:**
- Use Chromium `Page.captureScreenshot`
- Format options: PNG, JPEG
- Full page vs viewport

### 6. PDF Encryption (Medium Priority)

**Gotenberg endpoint:** `POST /forms/pdfengines/encrypt`

**Implementation:**
- Use `qpdf --encrypt`
- Password protection
- Permission flags (print, copy, modify)

## File Structure Differences

### Gotenberg Module Layout

```
pkg/modules/
├── api/              -> HTTP handlers, routes
├── chromium/         -> Browser automation
├── libreoffice/      -> Office conversion
├── pdfengines/       -> PDF operations (merge, split, etc.)
│   ├── qpdf/         -> qpdf bindings
│   ├── pdfcpu/       -> pdfcpu bindings
│   └── etc/
└── webhook/          -> Async callbacks
```

### Folio Current Layout

```
crates/
├── server/           -> HTTP server, routes
│   └── src/
│       ├── routes/   -> API endpoints
│       └── tests/    -> E2E tests
├── engine/           -> Core logic
│   └── src/
│       ├── chromium/ -> Browser automation
│       ├── libreoffice/ -> Office conversion
│       └── pdfops/   -> PDF operations
└── cli/              -> Command line
```

## Recommendation

**Phase 1 (Immediate):** PDF/A conversion, Webhook system
**Phase 2 (Next):** Bookmarks, Watermark, Screenshot API
**Phase 3 (Later):** Encryption, File embedding, Advanced metrics

The core PDF operations (merge, split, flatten, rotate) are at feature parity.
The main gaps are in **compliance conversion** (PDF/A) and **async processing** (webhooks).
