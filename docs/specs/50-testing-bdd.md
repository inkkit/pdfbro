# Spec 50 — BDD Integration Testing (Gotenberg Compatibility)

> Port Gotenberg's Gherkin-based integration tests to Rust for Folio.
> Ensure API compatibility by mirroring the same test scenarios.

## Goal

Create a comprehensive BDD-style integration test suite that:
1. Mirrors Gotenberg's test scenarios (`gotenberg/test/integration/features/*.feature`)
2. Validates API compatibility (same inputs produce compatible outputs)
3. Catches regressions when adding new features
4. Documents expected behavior in human-readable format

## Scope

**In:**

- Test structure and framework selection
- Port all Gotenberg feature files to Rust test equivalents
- Reusable test helpers (HTTP client, test server, assertions)
- Test data from Gotenberg's `test/integration/testdata/`
- CI integration

**Out:**

- Property-based testing (separate spec)
- Load/performance testing (separate spec)
- Fuzzing (out of scope for now)

---

## Test Framework

### Recommended Approach: Custom BDD in Rust

Since Rust doesn't have a direct Gherkin/Cucumber equivalent as mature as Godog (Go), we'll implement a lightweight BDD structure:

**Option 1: `rstest` + custom scenario macros** (Recommended)
```toml
# crates/server/Cargo.toml
[dev-dependencies]
rstest = "0.18"
reqwest = { workspace = true, features = ["json", "multipart"] }
tokio = { workspace = true }
serde_json = { workspace = true }
```

**Option 2: `cucumber` crate**
- Full Gherkin support, but adds dependency and complexity
- Only use if we need business stakeholders reading `.feature` files

**Decision: Option 1** — Use `rstest` with scenario-named test functions that read like BDD.

---

## Test Structure

```
crates/server/tests/
├── integration/
│   ├── main.rs                    # Test harness entry point
│   ├── common/
│   │   ├── mod.rs
│   │   ├── fixtures.rs            # Test data paths, helper functions
│   │   ├── http.rs               # HTTP client helpers
│   │   ├── server.rs              # Start/stop test server
│   │   └── assertions.rs         # PDF/image validation helpers
│   ├── scenarios/
│   │   ├── mod.rs
│   │   ├── chromium_convert_url.rs
│   │   ├── chromium_convert_html.rs
│   │   ├── chromium_convert_markdown.rs
│   │   ├── chromium_screenshot_url.rs      # Phase 1
│   │   ├── chromium_screenshot_html.rs    # Phase 1
│   │   ├── chromium_screenshot_markdown.rs # Phase 1
│   │   ├── chromium_concurrent.rs
│   │   ├── libreoffice_convert.rs
│   │   ├── pdfengines_merge.rs
│   │   ├── pdfengines_split.rs
│   │   ├── pdfengines_flatten.rs
│   │   ├── pdfengines_metadata.rs
│   │   ├── pdfengines_encrypt.rs          # Phase 2
│   │   ├── pdfengines_bookmarks.rs        # Phase 2
│   │   ├── pdfengines_watermark.rs        # Phase 2
│   │   ├── pdfengines_stamp.rs            # Phase 2
│   │   ├── pdfengines_rotate.rs           # Phase 2
│   │   ├── pdfengines_embed.rs            # Phase 2
│   │   ├── webhook.rs                     # Phase 4
│   │   ├── prometheus_metrics.rs          # Phase 4
│   │   ├── health.rs
│   │   └── version.rs
│   └── testdata/                  # Copied from Gotenberg
│       ├── html/
│       ├── url/
│       ├── markdown/
│       ├── pdf/
│       ├── office/
│       └── screenshots/
└── helpers.rs                      # Shared test utilities
```

---

## Porting Strategy

### Step 1: Copy Test Data

```bash
# Copy Gotenberg's test fixtures
cp -r gotenberg/test/integration/testdata/* \
      crates/server/tests/integration/testdata/
```

### Step 2: Map Gherkin Scenarios to Rust Tests

**Gotenberg Gherkin format** (`test/integration/features/chromium_convert_url.feature`):
```gherkin
Feature: Chromium convert URL
  Scenario: Default conversion
    When I call "/forms/chromium/convert/url" with body:
      | field     | type   | value                     |
      | url       | string | "http://host.docker.internal:8000" |
    Then I should receive a valid PDF
    And the PDF should have 1 page(s)
```

**Folio Rust equivalent** (`crates/server/tests/integration/scenarios/chromium_convert_url.rs`):
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn scenario_default_conversion() {
        let server = TestServer::start().await;
        let client = reqwest::Client::new();

        let params = [("url", "http://localhost:8000")];
        let response = client
            .post(server.url("/forms/chromium/convert/url"))
            .multipart(build_form(params))
            .send()
            .await
            .unwrap();

        assert_eq!(response.status(), 200);
        let bytes = response.bytes().await.unwrap();
        assert!(is_valid_pdf(&bytes));
        assert_eq!(pdf_page_count(&bytes), 1);
    }

    #[tokio::test]
    async fn scenario_with_landscape() {
        let server = TestServer::start().await;
        let client = reqwest::Client::new();

        let params = [
            ("url", "http://localhost:8000"),
            ("landscape", "true"),
        ];
        let response = client
            .post(server.url("/forms/chromium/convert/url"))
            .multipart(build_form(params))
            .send()
            .await
            .unwrap();

        assert_eq!(response.status(), 200);
        let bytes = response.bytes().await.unwrap();
        assert!(is_valid_pdf(&bytes));
        // Verify landscape via PDF dimensions
        let (width, height) = pdf_dimensions(&bytes);
        assert!(width > height, "Expected landscape orientation");
    }
}
```

---

## Test Scenarios to Port

### Priority 1: Core Chromium (Phase 1)

| Gotenberg Feature File | Folio Test File | Status |
|-----------------------|-----------------|--------|
| `chromium_convert_url.feature` | `chromium_convert_url.rs` | ❌ Missing |
| `chromium_convert_html.feature` | `chromium_convert_html.rs` | ❌ Missing |
| `chromium_convert_markdown.feature` | `chromium_convert_markdown.rs` | ❌ Missing |
| `chromium_screenshot_url.feature` | `chromium_screenshot_url.rs` | ❌ Missing |
| `chromium_screenshot_html.feature` | `chromium_screenshot_html.rs` | ❌ Missing |
| `chromium_screenshot_markdown.feature` | `chromium_screenshot_markdown.rs` | ❌ Missing |
| `chromium_concurrent.feature` | `chromium_concurrent.rs` | ❌ Missing |

**Key scenarios per file:**

From `chromium_convert_url.feature`:
- Default conversion
- Single page
- Landscape orientation
- Custom paper size
- Page ranges
- Header/footer templates
- Wait delay
- Wait for selector
- Wait for expression
- Custom user agent
- Extra HTTP headers
- Cookies
- Fail on HTTP status codes
- Native page ranges
- PDF/A conversion
- PDF/UA conversion

### Priority 2: LibreOffice (Phase 1)

| Gotenberg Feature File | Folio Test File | Status |
|-----------------------|-----------------|--------|
| `libreoffice_convert.feature` | `libreoffice_convert.rs` | ❌ Missing |

**Key scenarios:**
- Default conversion (DOCX → PDF)
- Landscape orientation
- Page ranges
- PDF/A conversion
- PDF/UA conversion
- Password-protected input
- Multiple input formats (DOC, PPT, XLS, ODT, etc.)

### Priority 3: PDF Engines (Phase 2)

| Gotenberg Feature File | Folio Test File | Status |
|-----------------------|-----------------|--------|
| `pdfengines_merge.feature` | `pdfengines_merge.rs` | ❌ Missing |
| `pdfengines_split.feature` | `pdfengines_split.rs` | ❌ Missing |
| `pdfengines_flatten.feature` | `pdfengines_flatten.rs` | ✅ Exists |
| `pdfengines_metadata.feature` | `pdfengines_metadata.rs` | ❌ Missing |
| `pdfengines_encrypt.feature` | `pdfengines_encrypt.rs` | ❌ Missing |
| `pdfengines_bookmarks.feature` | `pdfengines_bookmarks.rs` | ❌ Missing |
| `pdfengines_watermark.feature` | `pdfengines_watermark.rs` | ❌ Missing |
| `pdfengines_stamp.feature` | `pdfengines_stamp.rs` | ❌ Missing |
| `pdfengines_rotate.feature` | `pdfengines_rotate.rs` | ❌ Missing |
| `pdfengines_embed.feature` | `pdfengines_embed.rs` | ❌ Missing |

### Priority 4: Infrastructure (Phase 4)

| Gotenberg Feature File | Folio Test File | Status |
|-----------------------|-----------------|--------|
| `health.feature` | `health.rs` | ❌ Missing |
| `version.feature` | `version.rs` | ❌ Missing |
| `prometheus_metrics.feature` | `prometheus_metrics.rs` | ❌ Missing |
| `webhook.feature` | `webhook.rs` | ❌ Missing |
| `output_filename.feature` | `output_filename.rs` | ❌ Missing |

---

## Test Helpers

### `common/server.rs` — Test Server Management

```rust
pub struct TestServer {
    address: SocketAddr,
    shutdown_tx: tokio::sync::oneshot::Sender<()>,
}

impl TestServer {
    pub async fn start() -> Self {
        let app = crate::app::build_app(/* test config */).await;
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();

        let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();

        tokio::spawn(async move {
            axum::serve(listener, app)
                .with_graceful_shutdown(async {
                    shutdown_rx.await.ok();
                })
                .await
                .unwrap();
        });

        TestServer { address, shutdown_tx }
    }

    pub fn url(&self, path: &str) -> String {
        format!("http://{}{}", self.address, path)
    }

    pub async fn shutdown(self) {
        let _ = self.shutdown_tx.send(());
    }
}
```

### `common/http.rs` — HTTP Client Helpers

```rust
pub fn build_multipart_form<'a>(
    fields: &[(&str, FormField<'a>)],
) -> reqwest::multipart::Form {
    let mut form = reqwest::multipart::Form::new();
    for (name, field) in fields {
        match field {
            FormField::Text(value) => {
                form = form.text(name.to_string(), value.to_string());
            }
            FormField::File { path, filename } => {
                let file = tokio::fs::read(path).await.unwrap();
                let part = reqwest::multipart::Part::bytes(file)
                    .file_name(filename.to_string());
                form = form.part(name.to_string(), part);
            }
        }
    }
    form
}

pub enum FormField<'a> {
    Text(String),
    File { path: &'a Path, filename: String },
}
```

### `common/assertions.rs` — PDF/Image Validation

```rust
pub fn is_valid_pdf(bytes: &[u8]) -> bool {
    bytes.starts_with(b"%PDF-")
}

pub fn pdf_page_count(bytes: &[u8]) -> u32 {
    let doc = lopdf::Document::load_mem(bytes).unwrap();
    doc.get_pages().len() as u32
}

pub fn is_valid_png(bytes: &[u8]) -> bool {
    bytes.starts_with(&[0x89, 0x50, 0x4E, 0x47])
}

pub fn is_valid_jpeg(bytes: &[u8]) -> bool {
    bytes.starts_with(&[0xFF, 0xD8, 0xFF])
}

pub fn is_valid_webp(bytes: &[u8]) -> bool {
    bytes.starts_with(b"RIFF") && bytes[8..12] == *b"WEBP"
}
```

---

## Running Tests

### Cargo Test Commands

```bash
# Run all integration tests (requires Chrome + LibreOffice)
cargo test -p server --test integration

# Run specific scenario file
cargo test -p server --test integration chromium_convert_url

# Run specific scenario
cargo test -p server --test integration scenario_default_conversion

# Run with logging
RUST_LOG=debug cargo test -p server --test integration

# Run only unit tests (no Chrome required)
cargo test -p server --lib
```

### CI Configuration

```yaml
# .github/workflows/integration-tests.yml
name: Integration Tests
on: [push, pull_request]

jobs:
  test:
    runs-on: ubuntu-latest
    services:
      chrome:
        image: chromedp/headless-shell:latest
      libreoffice:
        image: gotenberg/libreoffice:latest

    steps:
      - uses: actions/checkout@v3
      - uses: actions-rs/toolchain@v1
        with:
          toolchain: stable
      - name: Install Chrome
        run: |
          wget https://dl.google.com/linux/direct/google-chrome-stable_current_amd64.deb
          sudo dpkg -i google-chrome-stable_current_amd64.deb || sudo apt-get install -f
      - name: Install LibreOffice
        run: sudo apt-get install -y libreoffice
      - name: Run integration tests
        run: cargo test -p server --test integration
        env:
          CHROME_PATH: /usr/bin/google-chrome
```

---

## Test Completion Checklist

### Phase 1: Chromium Core (Week 1)
- [ ] `chromium_convert_url.rs` — all scenarios
- [ ] `chromium_convert_html.rs` — all scenarios
- [ ] `chromium_convert_markdown.rs` — all scenarios
- [ ] `chromium_screenshot_url.rs` — all scenarios
- [ ] `chromium_screenshot_html.rs` — all scenarios
- [ ] `chromium_screenshot_markdown.rs` — all scenarios
- [ ] `chromium_concurrent.rs` — concurrency tests

### Phase 2: LibreOffice + PDF Ops (Week 2)
- [ ] `libreoffice_convert.rs` — all scenarios
- [ ] `pdfengines_merge.rs` — all scenarios
- [ ] `pdfengines_split.rs` — all scenarios
- [ ] `pdfengines_flatten.rs` — enhance existing
- [ ] `pdfengines_encrypt.rs` — all scenarios
- [ ] `pdfengines_bookmarks.rs` — all scenarios

### Phase 3: Infrastructure (Week 3)
- [ ] `health.rs` — health check tests
- [ ] `version.rs` — version endpoint tests
- [ ] `prometheus_metrics.rs` — metrics tests
- [ ] `webhook.rs` — webhook tests (after implementation)

---

## Maintenance

### When Gotenberg Adds New Tests

1. Check `gotenberg/test/integration/features/` for new `.feature` files
2. Port new scenarios to Folio's Rust tests
3. Update this spec with new checklist items
4. Run both Gotenberg and Folio with same inputs to verify compatibility

### Test Data Updates

```bash
# Refresh test data from Gotenberg (occasionally)
rm -rf crates/server/tests/integration/testdata/*
cp -r gotenberg/test/integration/testdata/* \
      crates/server/tests/integration/testdata/
```

---

## Acceptance

- [ ] All 20+ Gotenberg feature files have Rust equivalents
- [ ] Test structure matches the layout above
- [ ] Helper functions implemented (`server.rs`, `http.rs`, `assertions.rs`)
- [ ] Can run `cargo test -p server --test integration` successfully
- [ ] CI runs integration tests on PRs
- [ ] Test data copied from Gotenberg
- [ ] All scenarios documented in this spec are implemented
- [ ] `cargo clippy -p server -- -D warnings` passes in test code
