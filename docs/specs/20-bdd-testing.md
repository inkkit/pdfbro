# Spec 20 — BDD Testing with Cucumber (Detailed Implementation Guide)

> Port Gotenberg's Gherkin integration tests to Folio.
> Step-by-step implementation for replicating Gotenberg's test infrastructure.

## Overview

This spec provides detailed instructions for porting Gotenberg's integration
tests from Go (Godog + testcontainers-go) to Rust (cucumber-rs + testcontainers-rs).

## Gotenberg's Test Structure (Source)

```
gotenberg/test/integration/
├── features/           # 26 .feature files (Gherkin)
│   ├── health.feature
│   ├── pdfengines_merge.feature
│   └── ...
├── scenario/
│   ├── scenario.go   # Step definitions (Go)
│   ├── containers.go # Docker container helpers
│   └── main_test.go  # Test runner setup
└── testdata/         # PDF fixtures
```

## Folio Target Structure

```
crates/server/tests/bdd/
├── features/              # Copied & adapted from Gotenberg
│   ├── health.feature
│   ├── pdfengines_merge.feature
│   └── ... (26 files)
├── steps/
│   ├── mod.rs            # Step registration
│   ├── container.rs      # testcontainers-rs wrapper
│   ├── http.rs          # HTTP client steps
│   ├── pdf.rs           # PDF assertions
│   └── gotenberg_compat.rs # Go-to-Rust step mappings
├── support/
│   ├── world.rs         # Cucumber World struct
│   └── hooks.rs         # Before/After hooks
├── testdata/            # Copied from Gotenberg
│   ├── page_1.pdf
│   ├── page_2.pdf
│   └── ...
└── main.rs              # Test runner entry point
```

## Step 1: Dependencies (Cargo.toml)

Add to `crates/server/Cargo.toml`:

```toml
[dev-dependencies]
# BDD framework
cucumber = "0.21"

# Docker testcontainers
testcontainers = "0.22"
testcontainers-modules = { version = "0.11", features = ["blocking"] }

# HTTP client for tests
reqwest = { version = "0.12", features = ["multipart", "json"] }

# PDF validation
lopdf = { workspace = true }
pdf-extract = "0.8"

# Async runtime for tests
tokio = { workspace = true }

# Temporary files
tempfile = { workspace = true }
```

## Step 2: Create Directory Structure

```bash
mkdir -p crates/server/tests/bdd/{features,steps,support,testdata}
touch crates/server/tests/bdd/main.rs
touch crates/server/tests/bdd/steps/{mod.rs,container.rs,http.rs,pdf.rs}
touch crates/server/tests/bdd/support/{world.rs,hooks.rs}
```

## Step 3: Copy Gotenberg Test Data

```bash
cp gotenberg/test/integration/testdata/*.pdf \
   crates/server/tests/bdd/testdata/
```

## Step 4: Port Feature Files

Copy and adapt each `.feature` file. Example adaptation:

**Gotenberg (original):**
```gherkin
Given I have a Gotenberg container with the following environment variable(s):
  | API_DISABLE_HEALTH_CHECK_ROUTE_TELEMETRY | false |
```

**Folio (adapted):**
```gherkin
Given I have a Folio container with the following environment variable(s):
  | RUST_LOG | info |
```

## Step 5: Implement World (support/world.rs)

The World holds test state across steps:

```rust
use cucumber::World;
use reqwest::Client;
use std::collections::HashMap;
use testcontainers::Container;

#[derive(Debug, World)]
pub struct FolioWorld {
    // HTTP client for requests
    pub client: Client,
    
    // Active container (if any)
    pub container: Option<Container<GenericImage>>,
    
    // Last HTTP response
    pub response: Option<reqwest::Response>,
    
    // Response body bytes
    pub body: Option<Vec<u8>>,
    
    // Temporary directory for test files
    pub temp_dir: tempfile::TempDir,
    
    // Container base URL
    pub base_url: Option<String>,
}

impl Default for FolioWorld {
    fn default() -> Self {
        Self {
            client: Client::new(),
            container: None,
            response: None,
            body: None,
            temp_dir: tempfile::tempdir().unwrap(),
            base_url: None,
        }
    }
}

impl FolioWorld {
    /// Start Folio container with environment variables
    pub async fn start_container(&mut self, env: HashMap<String, String>) {
        use testcontainers::{GenericImage, WaitFor};
        
        let image = GenericImage::new("deesh2025/no-name", "latest")
            .with_wait_for(WaitFor::message_on_stdout("Listening on"));
        
        // Add environment variables
        for (key, value) in env {
            let _ = image.with_env_var(key, value);
        }
        
        let container = image.start().await.unwrap();
        let port = container.get_host_port_ipv4(3000).await.unwrap();
        
        self.base_url = Some(format!("http://localhost:{}", port));
        self.container = Some(container);
    }
}
```

## Step 6: Implement Steps (steps/mod.rs)

Register all step definitions:

```rust
use cucumber::Steps;
use crate::support::world::FolioWorld;

mod container;
mod http;
mod pdf;

pub fn steps() -> Steps<FolioWorld> {
    let mut steps = Steps::new();
    
    // Container steps
    steps.given(
        "I have a default Folio container",
        container::default_container,
    );
    steps.given(
        "I have a Folio container with the following environment variable(s)",
        container::container_with_env,
    );
    
    // HTTP steps
    steps.when(
        regex r#"I make a "(GET|POST)" request to "(.+)""#,
        http::make_request,
    );
    steps.then(
        "the response status code should be {int}",
        http::check_status_code,
    );
    
    // PDF steps
    steps.then(
        "there should be {int} PDF(s) in the response",
        pdf::check_pdf_count,
    );
    steps.then(
        "the PDF should have {int} page(s)",
        pdf::check_page_count,
    );
    
    steps
}
```

## Step 7: Container Steps (steps/container.rs)

Map Gotenberg's container steps to Rust:

| Gotenberg (Go) | Folio (Rust) |
|----------------|--------------|
| `iHaveADefaultGotenbergContainer` | `default_container` |
| `iHaveAGotenbergContainerWithEnv` | `container_with_env` |
| `startGotenbergContainer` | `testcontainers::GenericImage` |

```rust
use std::collections::HashMap;
use cucumber::gherkin::Table;
use crate::support::world::FolioWorld;

pub async fn default_container(world: &mut FolioWorld) {
    world.start_container(HashMap::new()).await;
}

pub async fn container_with_env(world: &mut FolioWorld, table: &Table) {
    let mut env = HashMap::new();
    for row in table.rows.iter().skip(1) { // Skip header
        let key = row.cells[0].value.clone();
        let value = row.cells[1].value.clone();
        env.insert(key, value);
    }
    world.start_container(env).await;
}
```

## Step 8: HTTP Steps (steps/http.rs)

| Gotenberg (Go) | Folio (Rust) |
|----------------|--------------|
| `doRequest` | `reqwest::Client` |
| `s.resp` | `world.response` |
| `s.resp.Code` | `world.response.status().as_u16()` |

```rust
use cucumber::gherkin::Table;
use crate::support::world::FolioWorld;

pub async fn make_request(
    world: &mut FolioWorld,
    method: String,
    endpoint: String,
) {
    let url = format!("{}{}", world.base_url.as_ref().unwrap(), endpoint);
    
    let response = match method.as_str() {
        "GET" => world.client.get(&url).send().await.unwrap(),
        "POST" => world.client.post(&url).send().await.unwrap(),
        _ => panic!("Unsupported method: {}", method),
    };
    
    world.response = Some(response);
}

pub async fn check_status_code(world: &mut FolioWorld, expected: u16) {
    let actual = world.response.as_ref().unwrap().status().as_u16();
    assert_eq!(actual, expected, "Status code mismatch");
}
```

## Step 9: PDF Steps (steps/pdf.rs)

| Gotenberg (Go) | Folio (Rust) |
|----------------|--------------|
| `assertPDFPageCount` | `lopdf::Document::get_pages()` |
| `assertPDFContent` | `pdf_extract::extract_text()` |

```rust
use lopdf::Document;
use crate::support::world::FolioWorld;

pub async fn check_pdf_count(world: &mut FolioWorld, expected: usize) {
    // Implementation to count PDFs in multipart response
}

pub async fn check_page_count(world: &mut FolioWorld, expected: usize) {
    let body = world.body.as_ref().unwrap();
    let doc = Document::load_mem(body).unwrap();
    let actual = doc.get_pages().len();
    assert_eq!(actual, expected, "Page count mismatch");
}
```

## Step 10: Test Runner (main.rs)

```rust
use cucumber::Cucumber;
use std::path::PathBuf;

mod support;
mod steps;

use support::world::FolioWorld;
use steps::steps;

#[tokio::main]
async fn main() {
    let runner = Cucumber::<FolioWorld>::new()
        .features(&[PathBuf::from("tests/bdd/features")])
        .steps(steps())
        .run_and_exit()
        .await;
}
```

## Step 11: Run Tests

```bash
# Build Docker image first
docker build -t deesh2025/no-name:latest .

# Run all BDD tests
cargo test --test bdd

# Run specific feature
cargo test --test bdd -- health

# With debug output
cargo test --test bdd -- --nocapture

# Generate HTML report
cargo test --test bdd -- --format html --output reports/
```

## Mapping: Gotenberg Steps → Rust Steps

Complete mapping of all 26 feature file step patterns:

| Pattern | Go Function | Rust Function | Status |
|---------|-------------|---------------|--------|
| `I have a default Gotenberg container` | `iHaveADefaultGotenbergContainer` | `default_container` | ⬜ |
| `I have a Gotenberg container with env` | `iHaveAGotenbergContainerWithEnv` | `container_with_env` | ⬜ |
| `I make a "X" request to "Y"` | `iMakeARequestToGotenberg` | `make_request` | ⬜ |
| `the response status code should be X` | `theResponseStatusCodeShouldBe` | `check_status_code` | ⬜ |
| `the response header "X" should be "Y"` | `theResponseHeaderShouldBe` | `check_header` | ⬜ |
| `the response body should match JSON` | `theResponseBodyShouldMatchJSON` | `check_json_body` | ⬜ |
| `there should be X PDF(s) in the response` | `thereShouldBeXPDFs` | `check_pdf_count` | ⬜ |
| `the PDF should have X page(s)` | `thePDFShouldHaveXPages` | `check_page_count` | ⬜ |
| `the PDF content at page X should be` | `thePDFContentAtPageShouldBe` | `check_page_content` | ⬜ |
| `the container should log` | `theContainerShouldLog` | `check_logs` | ⬜ |

## Feature Porting Priority

Port features in this order:

1. **Phase 1: Core (Week 1)**
   - `health.feature` (simplest)
   - `version.feature`
   - `root.feature`

2. **Phase 2: PDF Engines (Week 2)**
   - `pdfengines_merge.feature`
   - `pdfengines_split.feature`
   - `pdfengines_flatten.feature`
   - `pdfengines_rotate.feature`

3. **Phase 3: Chromium (Week 3)**
   - `chromium_convert_html.feature`
   - `chromium_convert_url.feature`
   - `chromium_screenshot_*.feature`

4. **Phase 4: Advanced (Week 4)**
   - `pdfengines_bookmarks.feature`
   - `pdfengines_convert.feature`
   - `webhook.feature`
   - `pdfengines_encrypt.feature`

## CI/CD Integration

```yaml
# .github/workflows/bdd.yml
name: BDD Tests
on: [push, pull_request]
jobs:
  bdd:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      
      - name: Build Docker image
        run: docker build -t deesh2025/no-name:latest .
      
      - name: Install Chromium
        run: sudo apt-get install -y chromium-browser
      
      - name: Run BDD tests
        run: cargo test --test bdd -- --format junit > bdd-results.xml
      
      - name: Upload results
        uses: actions/upload-artifact@v4
        with:
          name: bdd-results
          path: bdd-results.xml
```

## Debugging Tips

1. **Container not starting**: Check Docker daemon, image tag
2. **Connection refused**: Wait for container healthy state
3. **PDF assertions failing**: Verify lopdf can parse the PDF
4. **Step not found**: Check regex pattern matches exactly

## References

- Gotenberg features: `gotenberg/test/integration/features/`
- Gotenberg steps: `gotenberg/test/integration/scenario/scenario.go`
- cucumber-rs docs: https://cucumber-rs.github.io/
- testcontainers-rs: https://docs.rs/testcontainers/


### Dependencies

```toml
[dev-dependencies]
cucumber = "0.21"
testcontainers = "0.22"
reqwest = { workspace = true }
serde_json = { workspace = true }
tempfile = { workspace = true }
```

## Implementation Phases

### Phase 1: Infrastructure (Week 1)

- [ ] Add cucumber and testcontainers dependencies
- [ ] Create test directory structure
- [ ] Implement World struct with HTTP client and temp directory
- [ ] Implement container lifecycle hooks
- [ ] Create basic step definitions (Given/When/Then)

### Phase 2: Core Feature Tests (Week 2)

- [ ] Port `health.feature` tests
- [ ] Port `version.feature` tests
- [ ] Port `pdfengines_merge.feature` tests
- [ ] Port `pdfengines_split.feature` tests

### Phase 3: Chromium Tests (Week 3)

- [ ] Port `chromium_convert_html.feature`
- [ ] Port `chromium_convert_url.feature`
- [ ] Port `chromium_screenshot_*.feature` tests

### Phase 4: Advanced Features (Week 4)

- [ ] Port PDF/A conversion tests
- [ ] Port bookmark tests
- [ ] Port webhook tests (mock server)

## Key Components

### World Implementation

```rust
pub struct World {
    /// HTTP client for requests
    client: reqwest::Client,
    /// Folio container handle
    container: Option<FolioContainer>,
    /// Last HTTP response
    response: Option<reqwest::Response>,
    /// Response body bytes
    body: Option<Vec<u8>>,
    /// Temporary directory for test files
    temp_dir: tempfile::TempDir,
    /// Test data directory
    testdata_dir: PathBuf,
}
```

### Step Definitions

Common steps to implement:

```rust
#[given("I have a default Folio container")]
async fn default_container(world: &mut World) {
    world.start_container().await;
}

#[when(regex = r#"I make a "(GET|POST)" request to "(.+)""#)]
async fn make_request(world: &mut World, method: String, path: String) {
    world.request(&method, &path).await;
}

#[then("the response status code should be {int}")]
async fn check_status(world: &mut World, expected: u16) {
    let actual = world.response.as_ref().unwrap().status().as_u16();
    assert_eq!(actual, expected);
}
```

### Container Management

Using testcontainers-rs:

```rust
pub struct FolioContainer {
    image: GenericImage,
    container: Container<GenericImage>,
    port: u16,
}

impl FolioContainer {
    pub async fn start() -> Result<Self, TestcontainersError> {
        let image = GenericImage::new("deesh2025/no-name", "latest")
            .with_wait_for(WaitFor::message_on_stdout("Listening on"));
        
        let container = image.start().await?;
        let port = container.get_host_port_ipv4(3000).await?;
        
        Ok(Self { image, container, port })
    }
    
    pub fn base_url(&self) -> String {
        format!("http://localhost:{}", self.port)
    }
}
```

### PDF Assertions

```rust
pub fn assert_pdf_page_count(pdf_bytes: &[u8], expected: u32) {
    let doc = lopdf::Document::load_mem(pdf_bytes).unwrap();
    let pages = doc.get_pages().len() as u32;
    assert_eq!(pages, expected, "PDF page count mismatch");
}

pub fn assert_pdf_contains_text(pdf_bytes: &[u8], text: &str) {
    // Use pdf-extract or similar
}
```

## Running Tests

```bash
# Run all BDD tests
cargo test --test bdd

# Run specific feature
cargo test --test bdd -- health

# With output for debugging
cargo test --test bdd -- --nocapture

# Generate HTML report
cargo test --test bdd -- --format html --output reports/
```

## CI Integration

```yaml
# .github/workflows/bdd.yml
name: BDD Tests
on: [push, pull_request]
jobs:
  bdd:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: Build Docker image
        run: docker build -t folio:test .
      - name: Run BDD tests
        run: cargo test --test bdd
```

## References

- cucumber-rs docs: https://cucumber-rs.github.io/
- testcontainers-rs: https://docs.rs/testcontainers/latest/testcontainers/
- Gotenberg features: https://github.com/gotenberg/gotenberg/tree/main/test/integration/features
