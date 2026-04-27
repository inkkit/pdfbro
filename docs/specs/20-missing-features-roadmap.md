# Missing Features Roadmap - Folio vs Gotenberg

> **Goal**: Bring Folio to feature parity with Gotenberg, phase by phase.

## Phase 1: Core Chromium Features (Screenshots)

### 1.1 Screenshot Support (Chromium)

**Status**: ❌ Missing (Gotenberg has 3 endpoints)

**New Endpoints**:
| Endpoint | Method | Input | Output | Priority |
|----------|--------|-------|--------|----------|
| `/forms/chromium/screenshot/html` | POST | HTML | PNG/JPEG/WebP | P1 |
| `/forms/chromium/screenshot/url` | POST | URL | PNG/JPEG/WebP | P1 |
| `/forms/chromium/screenshot/markdown` | POST | Markdown | PNG/JPEG/WebP | P2 |

**Form Fields**:
| Field | Type | Description |
|-------|------|-------------|
| `format` | string | `png`, `jpeg`, or `webp` (default: `png`) |
| `quality` | int | JPEG/WebP quality 0-100 |
| `clip.x` | float | Clip rectangle x (optional) |
| `clip.y` | float | Clip rectangle y (optional) |
| `clip.width` | float | Clip rectangle width |
| `clip.height` | float | Clip rectangle height |
| `fullPage` | bool | Capture full scrollable page |
| `viewport.width` | int | Viewport width (default: 1920) |
| `viewport.height` | int | Viewport height (default: 1080) |
| `viewport.scale` | float | Device scale factor (default: 1.0) |

**Engine Changes** (`crates/engine/src/chromium/`):
- Add `screenshot_html()` to `ChromiumEngine`
- Add `screenshot_url()` to `ChromiumEngine`
- Add `screenshot_markdown()` to `ChromiumEngine` (convert MD → HTML → screenshot)
- Use `chromiumoxide::page::Page::screenshot()` with `ScreenshotParams`

**Server Changes** (`crates/server/src/routes/chromium.rs`):
- Add route handlers for all 3 screenshot endpoints
- Handle format conversion (PNG/JPEG/WebP)
- Validate clip parameters

---

## Phase 2: Advanced PDF Operations

### 2.1 PDF Bookmarks

**Status**: ❌ Missing

**New Endpoints**:
| Endpoint | Method | Input | Output |
|----------|--------|-------|--------|
| `/forms/pdfengines/bookmarks/read` | POST | PDF | JSON bookmarks |
| `/forms/pdfengines/bookmarks/write` | POST | PDF + JSON | Modified PDF |

**Bookmark JSON Format**:
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

**Engine Changes** (`crates/engine/src/pdfops/`):
- Add `read_bookmarks()` to `PdfOps` struct
- Add `write_bookmarks()` to `PdfOps` struct
- Use `lopdf` to parse/write PDF outline/bookmark structure

### 2.2 PDF Encryption

**Status**: ❌ Missing

**New Endpoint**: `/forms/pdfengines/encrypt`

**Form Fields**:
| Field | Type | Description |
|-------|------|-------------|
| `userPassword` | string | User password (open) |
| `ownerPassword` | string | Owner password (permissions) |
| `permissions` | JSON | Permission flags |

**Permissions Object**:
```json
{
  "print": true,
  "modify": false,
  "copy": true,
  "annotate": false,
  "fillForms": true,
  "accessibility": true,
  "assemble": false,
  "printHighQuality": true
}
```

**Engine Changes**:
- Add `encrypt()` to `PdfOps`
- Use `lopdf::EncryptionSettings` for AES-128/AES-256

### 2.3 Stamp & Watermark

**Status**: ❌ Missing (Gotenberg has separate stamp/watermark)

**New Endpoints**:
| Endpoint | Method | Input | Output |
|----------|--------|-------|--------|
| `/forms/pdfengines/watermark` | POST | PDF + watermark | Watermarked PDF |
| `/forms/pdfengines/stamp` | POST | PDF + stamp | Stamped PDF |

**Watermark Form Fields**:
| Field | Type | Description |
|-------|------|-------------|
| `type` | string | `text` or `image` |
| `text` | string | Watermark text (if type=text) |
| `image` | file | Image file (if type=image) |
| `opacity` | float | 0.0 to 1.0 |
| `position` | string | `tile`, `center`, `top-left`, etc. |
| `fontSize` | int | Text font size |
| `color` | string | Hex color (e.g., `#FF0000`) |
| `pages` | string | Page ranges (e.g., "1-5,7") |

**Stamp Form Fields**: Similar to watermark but overlayed as fixed position

**Engine Changes**:
- Extend existing `watermark()` in `pdfops/watermark.rs`
- Add `stamp()` to `PdfOps`
- Consider using `lopdf` for overlay or external tool like `pdftk`

### 2.4 PDF/A & PDF/UA Conversion

**Status**: ❌ Missing

**New Endpoint**: `/forms/pdfengines/convert`

**Form Fields**:
| Field | Type | Description |
|-------|------|-------------|
| `pdfa` | string | `1a`, `2a`, `3a`, `3b` |
| `pdfua` | bool | Enable PDF/UA compliance |

**Engine Changes**:
- Add `convert_to_pdfa()` to `PdfOps`
- Add `convert_to_pdfua()` to `PdfOps`
- May require external tools (qpdf, verapdf) or `lopdf` manipulation

### 2.5 Embed Files in PDF

**Status**: ❌ Missing

**New Endpoint**: `/forms/pdfengines/embed`

**Form Fields**:
| Field | Type | Description |
|-------|------|-------------|
| `files` | files[] | Files to embed |
| `relationship` | string | PDF/A-3 relationship (optional) |

**Engine Changes**:
- Add `embed_files()` to `PdfOps`
- Use `lopdf` to add file attachments to PDF

---

## Phase 3: Process Supervision & Advanced Options

### 3.1 Chromium Process Supervision

**Status**: ⚠️ Basic (Folio launches once, no restart logic)

**New `BrowserConfig` Options**:
| Field | Type | Description |
|-------|------|-------------|
| `restart_after` | int | Restart Chrome after N conversions (0 = never) |
| `idle_timeout` | duration | Shutdown after idle period |
| `max_concurrency` | int | Max concurrent renders (default: 6) |
| `max_queue_size` | int | Max pending renders |

**Implementation** (`crates/engine/src/chromium/`):
- Add conversion counter to `ChromiumEngine`
- Auto-restart logic in render methods
- Queue management with backpressure
- Health check integration

### 3.2 LibreOffice Process Supervision

**Status**: ⚠️ Basic (similar to Chromium)

**New `LibreOfficeConfig` Options**: Same as Chromium supervision

### 3.3 Advanced Chromium Options

**Missing Form Fields** (add to existing endpoints):
| Field | Type | Description |
|-------|------|-------------|
| `headerTemplate` | string | HTML header template |
| `footerTemplate` | string | HTML footer template |
| `nativePageRanges` | string | Native Chrome page ranges |
| `generateDocumentOutline` | bool | Generate PDF outline |
| `generateTaggedPdf` | bool | Generate tagged PDF |
| `omitBackground` | bool | Omit background graphics |
| `emulatedMediaFeatures` | JSON | CSS media features |
| `failOnResourceHttpStatusCodes` | JSON | Fail on resource status |
| `failOnResourceLoadingFailed` | bool | Fail on resource errors |
| `failOnConsoleExceptions` | bool | Fail on JS exceptions |

**Engine Changes**:
- Extend `PdfOptions` struct in `types.rs`
- Pass options to `chromiumoxide::page::PdfParams`

---

## Phase 4: Infrastructure & Observability

### 4.1 Webhook System

**Status**: ❌ Missing

**New Endpoint**: `/forms/webhook`

**Form Fields**:
| Field | Type | Description |
|-------|------|-------------|
| `webhookURL` | string | Callback URL |
| `webhookMethod` | string | HTTP method (default: POST) |
| `webhookExtraHeaders` | JSON | Custom headers |
| `webhookRetryMinWait` | duration | Min retry delay |
| `webhookRetryMaxWait` | duration | Max retry delay |
| `webhookMaxRetries` | int | Max retry attempts |

**Behavior**:
- Queue conversion asynchronously
- Call webhook with result (PDF bytes or error)
- Retry with exponential backoff
- Support allow/deny lists for webhook URLs

**Implementation**:
- Add `crates/server/src/routes/webhook.rs`
- Use background task queue (Tokio tasks)
- Store results temporarily (filesystem or in-memory)

### 4.2 Prometheus Metrics

**Status**: ❌ Missing

**New Endpoint**: `/prometheus/metrics`

**Metrics to Export**:
| Metric | Type | Description |
|--------|------|-------------|
| `folio_requests_total` | Counter | Total requests by route |
| `folio_request_duration_seconds` | Histogram | Request duration |
| `folio_conversions_total` | Counter | Total conversions by engine |
| `folio_conversion_duration_seconds` | Histogram | Conversion duration |
| `folio_queue_size` | Gauge | Current queue size |
| `folio_active_conversions` | Gauge | Active conversions |
| `folio_chromium_healthy` | Gauge | Chromium health status |
| `folio_libreoffice_healthy` | Gauge | LibreOffice health status |

**Implementation**:
- Add `prometheus` crate dependency
- Create metrics module in `crates/server/src/metrics.rs`
- Integrate with Axum middleware
- Export `/prometheus/metrics` endpoint

### 4.3 OpenTelemetry (Tracing)

**Status**: ⚠️ Basic (has `tracing`, not OTEL)

**Enhancements**:
- Add `tracing-opentelemetry` crate
- Export traces to OTEL collector
- Add span attributes for conversion type, duration, etc.
- Distributed tracing support

### 4.4 Additional Endpoints

**Missing Endpoints**:
| Endpoint | Method | Description |
|----------|--------|-------------|
| `/version` | GET | Return version info |
| `/debug` | GET | Debug info (config, health, stats) |

---

## Phase 5: Testing & Compatibility

### 5.1 BDD Integration Tests

**Status**: ⚠️ Has unit tests, missing BDD

**Approach**: Port Gotenberg's Gherkin scenarios to Rust

**Test Structure**:
```
crates/server/tests/integration/
├── main.rs                    # Test entry point
├── features/                  # Scenario definitions (Rust equivalents)
│   ├── chromium_convert_url.rs
│   ├── chromium_screenshot_url.rs  ← New
│   ├── pdfengines_encrypt.rs       ← New
│   └── ...
├── helpers.rs                 # HTTP client, test fixtures
└── testdata/                  # Reuse Gotenberg's test files
```

**Scenarios to Port** (from Gotenberg's `test/integration/features/`):
1. `chromium_convert_url.feature` → `chromium_convert_url.rs`
2. `chromium_screenshot_url.feature` → `chromium_screenshot_url.rs` (new)
3. `pdfengines_encrypt.feature` → `pdfengines_encrypt.rs` (new)
4. `pdfengines_bookmarks.feature` → `pdfengines_bookmarks.rs` (new)
5. ... (20+ scenarios)

**Test Data**: Copy relevant files from `gotenberg/test/integration/testdata/`

### 5.2 API Compatibility Tests

**Goal**: Ensure Gotenberg clients work with Folio

**Tests**:
- Send identical requests to both Gotenberg and Folio
- Compare PDF outputs (structure, not exact bytes)
- Verify same form fields accepted
- Verify same error responses

---

## Implementation Priority

| Phase | Features | Estimated Effort |
|-------|----------|------------------|
| **Phase 1** | Screenshots (3 endpoints) | 2-3 days |
| **Phase 2** | Bookmarks, Encrypt, Stamp, PDF/A | 5-7 days |
| **Phase 3** | Supervision, Advanced Options | 3-5 days |
| **Phase 4** | Webhooks, Metrics, OTEL | 4-6 days |
| **Phase 5** | BDD Tests, Compatibility | 3-5 days |

**Total**: ~17-26 days for full parity

---

## Next Steps

1. **Review this spec** - Confirm priorities and phases
2. **Start Phase 1** - Implement screenshot support
3. **Write tests first** - TDD approach for each feature
4. **Update docs** - Keep `docs/specs/` in sync

---

## References

- Gotenberg API: `docs/gotenberg-spec.md`
- Gotenberg tests: `gotenberg/test/integration/features/`
- Existing Folio specs: `docs/specs/*.md`
