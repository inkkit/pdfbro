# BDD Test Gap Analysis: Folio vs Gotenberg

## Test Execution Results

**Latest run:** `cargo test --test bdd`

- **Features:** 26
- **Scenarios:** 63 total
- **Passed:** 62
- **Failed:** 1
- **Steps:** 251 total (250 passed, 1 failed)

### Known Failure

| Feature | Scenario | Issue |
|---------|----------|-------|
| `chromium_convert_url` | POST /forms/chromium/convert/url (default) | 504 timeout converting `https://example.com` — network fetch exceeds 120s timeout in test environment |

---

## Scenario Coverage Gap

Gotenberg has **442 scenarios** across 26 feature files. Folio currently has **55 scenarios**.

| Feature File | Gotenberg | Folio | Gap |
|--------------|-----------|-------|-----|
| `chromium_concurrent.feature` | 2 | 1 | Missing: PDF-only concurrent test |
| `chromium_convert_html.feature` | 50 | 2 | Missing: paper size, margins, landscape, scale, background, CSS page size, page ranges, header/footer, media type, wait conditions, cookies, extra headers, user agent, fail-on-status, skip network idle, PDF/A, single page, omit backgrounds, secret header, basic auth, root path, webhook, download-from |
| `chromium_convert_markdown.feature` | 41 | 2 | Missing: all option variations, wrapper template, multiple markdown files, remote assets, basic auth, root path, webhook |
| `chromium_convert_url.feature` | 49 | 2 | Missing: all option variations, remote URL variations, basic auth, root path, webhook, download-from, secret header |
| `chromium_screenshot_html.feature` | 14 | 2 | Missing: capture mode (fullPage), viewport size, quality, background color, wait conditions, secret header, basic auth, root path, webhook |
| `chromium_screenshot_markdown.feature` | 7 | 4 | Missing: basic auth, root path, webhook |
| `chromium_screenshot_url.feature` | 8 | 2 | Missing: full page, format variations, secret header, basic auth, root path, webhook |
| `debug.feature` | 7 | 2 | Missing: enabled debug routes (vars, pprof, trace), secret header |
| `health.feature` | 10 | 4 | Missing: detailed health JSON with module status, no-logging telemetry, Gotenberg-Trace header, container log assertions |
| `libreoffice_convert.feature` | 37 | 6 | Missing: password-protected files, native page ranges, PDF/A, merge output, image compression, quality, export notes/pages/bookmarks/forms, many advanced filter options, basic auth, root path, webhook |
| `output_filename.feature` | 3 | 2 | Missing: screenshot output filename |
| `pdfengines_bookmarks.feature` | 20 | 2 | Missing: write bookmarks, complex nesting, basic auth, root path, webhook, download-from |
| `pdfengines_convert.feature` | 14 | 4 | Missing: invalid pdfa, basic auth, root path, webhook, download-from |
| `pdfengines_embed.feature` | 6 | 2 | Missing: embed metadata, download-from, basic auth, root path, webhook |
| `pdfengines_encrypt.feature` | 17 | 5 | Missing: permissions, basic auth, root path, webhook |
| `pdfengines_flatten.feature` | 12 | 1 | Missing: all variations, basic auth, root path, webhook, download-from |
| `pdfengines_merge.feature` | 33 | 2 | Missing: page ranges, metadata, flatten, PDF/A, many file combinations, basic auth, root path, webhook, download-from |
| `pdfengines_metadata.feature` | 21 | 2 | Missing: write metadata, complex metadata, basic auth, root path, webhook, download-from |
| `pdfengines_rotate.feature` | 16 | 2 | Missing: 270 degrees, specific pages, basic auth, root path, webhook, download-from |
| `pdfengines_split.feature` | 34 | 3 | Missing: many interval/page combinations, basic auth, root path, webhook, download-from |
| `pdfengines_stamp.feature` | 20 | 1 | Missing: image stamp, all position variations, pages, opacity, basic auth, root path, webhook, download-from |
| `pdfengines_watermark.feature` | 20 | 2 | Missing: image watermark, all position/size variations, pages, angle, basic auth, root path, webhook, download-from |
| `prometheus_metrics.feature` | 8 | 2 | Missing: enabled metrics endpoint, basic auth, root path |
| `root.feature` | 8 | 2 | Missing: enabled debug routes, basic auth |
| `version.feature` | 4 | 2 | Missing: version with trace header, basic auth |
| `webhook.feature` | 7 | 2 | Missing: webhook timeout, error URL, retry policy, custom headers |

---

## Missing Step Definitions

The following Gotenberg Gherkin steps have **no Rust implementation** in Folio:

### Given (Setup)
- `I have a (webhook|static) server` — requires test-local HTTP servers

### When (Action)
- `I make a "(GET|HEAD)" request to Gotenberg at the "<endpoint>" endpoint with the following header(s):` — header-only GET/HEAD
- `I make <N> concurrent "(POST)" requests` — concurrent request count parameter
- `I wait for the asynchronous request to the webhook` — webhook polling

### Then (Assertions)
- `the (response|webhook request|file request|server request) cookie "<name>" should be "<value>"` — cookie assertions
- `the (response|webhook request) body should contain string:` — substring body match
- `the webhook event should match JSON:` — webhook event JSON
- `there should be the following file(s) in the (response|webhook request):` — multi-file zip validation (partially implemented)
- `the "<name>" PDF (should|should NOT) be set to landscape orientation` — PDF property check
- `the "<name>" PDF (should|should NOT) have the following content at page <N>:` — PDF text extraction
- `the (response|webhook request) PDF(s) should be valid "<standard>" with a tolerance of <N> failed rule(s)` — PDF/A validation (verapdf)
- `the (response|webhook request) PDF(s) (should|should NOT) be flatten` — PDF flatten check
- `the (response|webhook request) PDF(s) (should|should NOT) be encrypted` — PDF encryption check
- `the (response|webhook request) PDF(s) (should|should NOT) have the "<filename>" file embedded` — embedded file check
- `the "<name>" PDF should have <N> image(s)` — image count
- `the Gotenberg container (should|should NOT) log the following entries:` — container log scraping
- `all concurrent response status codes should be <code>` — concurrent status check (partial)
- `all concurrent responses should have <N> PDF(s)` — concurrent PDF count

---

## Missing Routes / Features (No Feature File At All)

These Gotenberg endpoints have **no corresponding feature file** in Folio because the server routes themselves are missing or unimplemented:

| Gotenberg Endpoint | Status in Folio |
|--------------------|-----------------|
| `POST /forms/pdfengines/embed` | Route missing — currently tests go through `/forms/pdfengines/convert` with `embedFiles` field |
| `POST /forms/chromium/convert/html` — `downloadFrom` field | Not implemented |
| `POST /forms/chromium/convert/url` — `downloadFrom` field | Not implemented |
| `POST /forms/chromium/convert/markdown` — `downloadFrom` field | Not implemented |
| `POST /forms/libreoffice/convert` — password protected files | Commented out in feature file |
| `POST /forms/libreoffice/convert` — `downloadFrom` field | Not implemented |
| `POST /forms/pdfengines/merge` — `downloadFrom` field | Not implemented |
| `POST /forms/pdfengines/split` — `downloadFrom` field | Not implemented |
| `POST /forms/pdfengines/flatten` — `downloadFrom` field | Not implemented |
| `POST /forms/pdfengines/rotate` — `downloadFrom` field | Not implemented |
| `POST /forms/pdfengines/bookmarks/read` — `downloadFrom` field | Not implemented |
| `POST /forms/pdfengines/bookmarks/write` — `downloadFrom` field | Not implemented |
| `POST /forms/pdfengines/metadata/read` — `downloadFrom` field | Not implemented |
| `POST /forms/pdfengines/metadata/write` — `downloadFrom` field | Not implemented |
| `POST /forms/pdfengines/stamp` — `downloadFrom` field | Not implemented |
| `POST /forms/pdfengines/watermark` — `downloadFrom` field | Not implemented |
| `POST /forms/pdfengines/encrypt` — `downloadFrom` field | Not implemented |
| `POST /forms/pdfengines/decrypt` — `downloadFrom` field | Not implemented |

---

## Structural Differences

### Folio Simplifications

1. **Step text:** Folio uses `"/forms/chromium/convert/html"` (bare path); Gotenberg uses `"Gotenberg at the "/forms/chromium/convert/html" endpoint"` (full phrase)
2. **File paths:** Folio uses bare filenames (`index.html`, `page_1.pdf`); Gotenberg uses `testdata/` prefix (`testdata/index.html`, `testdata/page_1.pdf`)
3. **Container model:** Folio spawns server binary directly (no Docker); Gotenberg uses `testcontainers-go`
4. **Auth:** No basic-auth tests implemented in Folio step definitions
5. **Webhooks:** Partially implemented (`Gotenberg-Async` header returns 202) but no actual webhook server or delivery verification
6. **PDF validation:** No `verapdf`, `pdfinfo`, or `pdftotext` integration for deep PDF assertions
