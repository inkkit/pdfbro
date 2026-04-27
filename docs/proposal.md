# PDFKi: Rust-Native PDF Generation Engine

## Executive Summary

PDFKi is a proposal for a Rust-native document-to-PDF conversion engine, serving as the Rust equivalent of Gotenberg. It leverages Chrome/Chromium via the Chrome DevTools Protocol (CDP) for HTML-to-PDF conversion and provides multiple deployment modes: HTTP server (like Gotenberg), CLI tool, and embeddable library with language bindings.

## Usage Modes

### 1. HTTP Server Mode (Gotenberg-Compatible)

**Use Case:** Microservice deployment, Docker containers, API-first architecture

```bash
# Run as server
pdfki serve --port 3000

# Usage via API
curl -X POST http://localhost:3000/forms/chromium/convert/url \
  -F "url=https://example.com" \
  -o output.pdf
```

**Pros:**
- Process reuse (Chrome stays warm)
- Lower latency for multiple requests
- Gotenberg-compatible API
- Horizontal scaling support

**Cons:**
- Requires always-running process
- More resource overhead

### 2. CLI Mode

**Use Case:** Scripts, CI/CD pipelines, local development, one-off conversions

```bash
# Install as CLI tool
cargo install pdfki

# Direct HTML to PDF
pdfki convert --html file.html --output out.pdf

# URL to PDF
pdfki convert --url https://example.com --output out.pdf

# Batch conversion
pdfki batch --input-dir ./docs/ --output-dir ./pdfs/

# Pipe support
cat report.html | pdfki convert --stdin --output report.pdf
```

**Pros:**
- No server management
- Perfect for CI/CD
- Simple mental model
- Easy integration with shell scripts

**Cons:**
- Cold start overhead per invocation
- Chrome startup time (~1-2s) per conversion

### 3. Library/Package Mode

**Use Case:** Embedded in Rust applications, language bindings for Python/Node/Go

```rust
// Rust library usage
use pdfki::ChromiumEngine;

#[tokio::main]
async fn main() -> Result<()> {
    let engine = ChromiumEngine::new().await?;
    let pdf = engine.html_to_pdf("<h1>Hello</h1>").await?;
    std::fs::write("output.pdf", pdf)?;
    Ok(())
}
```

```python
# Python bindings (via PyO3)
import pdfki

engine = pdfki.ChromiumEngine()
pdf = engine.html_to_pdf("<h1>Hello</h1>")
with open("output.pdf", "wb") as f:
    f.write(pdf)
```

```javascript
// Node.js bindings (via napi-rs)
const pdfki = require('pdfki');

const engine = new pdfki.ChromiumEngine();
const pdf = await engine.htmlToPdf('<h1>Hello</h1>');
fs.writeFileSync('output.pdf', pdf);
```

**Pros:**
- Native integration with host language
- No external HTTP calls
- Can share Chrome instance within app lifecycle
- Fine-grained control

**Cons:**
- Language binding complexity
- Memory management across FFI boundary

## Architecture Overview

All usage modes (Server, CLI, Library, Language Bindings) share a **single core engine implementation**. The architecture separates the interface layer from the engine layer, ensuring consistent behavior and allowing bug fixes to apply across all modes.

```
┌─────────────────────────────────────────────────────────────────┐
│                      USAGE MODES (Interfaces)                  │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────────┐  │
│  │   HTTP API  │  │    CLI      │  │   Library/Bindings      │  │
│  │  (axum)     │  │  (clap)     │  │  (Rust/Python/Node)     │  │
│  └──────┬──────┘  └──────┬──────┘  └───────────┬─────────────┘  │
│         │                │                      │                  │
│         └────────────────┴──────────────────────┘                  │
│                         │                                        │
├─────────────────────────┼────────────────────────────────────────┤
│              CORE ENGINE (pdfki-core) - Single Source of Truth    │
│                         │                                        │
│  ┌──────────────────────┴──────────────────────────────────────┐ │
│  │                    Core Engine Layer                          │ │
│  ├─────────────────────────────────────────────────────────────┤ │
│  │  ┌──────────────┐  ┌──────────────┐  ┌──────────────────┐  │ │
│  │  │   Chromium   │  │  LibreOffice  │  │  PDF Operations  │  │ │
│  │  │   Engine     │  │    Engine     │  │    (lopdf)       │  │ │
│  │  └──────┬───────┘  └──────┬───────┘  └────────┬─────────┘  │ │
│  │         │                  │                    │             │ │
│  │         └──────────────────┴────────────────────┘             │ │
│  │                            │                                 │ │
│  └────────────────────────────┼─────────────────────────────────┘ │
│                               │                                    │
│  ┌────────────────────────────┴────────────────────────────────┐  │
│  │                    CDP / Protocol Layer                      │  │
│  ├─────────────────────────────────────────────────────────────┤  │
│  │  chromiumoxide (async)  │  headless_chrome (sync)            │  │
│  │  WebSocket to Chrome    │  WebSocket to Chrome               │  │
│  └─────────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────┘
```

### How Each Mode Uses the Core

| Mode | Entry Point | Chrome Lifecycle |
|------|-------------|------------------|
| **Server** | `pdfki serve` | Chrome pool persists across requests |
| **CLI** | `pdfki convert` | Chrome starts/stops per command |
| **Library** | `ChromiumEngine::new()` | Caller controls Chrome lifecycle |
| **Python** | `pdfki.ChromiumEngine()` | Thin wrapper over library |
| **Node** | `new pdfki.ChromiumEngine()` | Thin wrapper over library |

### Code Sharing Example

```rust
// crates/pdfki-core/src/lib.rs - Single Implementation
pub struct ChromiumEngine {
    browser: Browser,  // chromiumoxide Browser
}

impl ChromiumEngine {
    pub async fn html_to_pdf(&self, html: &str, opts: PdfOptions) -> Result<Vec<u8>> {
        // SAME code for ALL modes
        let page = self.browser.new_page("about:blank").await?;
        page.set_content(html).await?;
        page.print_to_pdf(opts.into()).await
    }
}
```

```rust
// crates/pdfki-cli/src/main.rs
let engine = ChromiumEngine::new().await?;
let pdf = engine.html_to_pdf(&html).await?;  // Uses core
```

```rust
// crates/pdfki-server/src/routes.rs
let engine = state.engine_pool.get().await?;
let pdf = engine.html_to_pdf(&html).await?;  // Same core
```

```rust
// crates/pdfki-py/src/lib.rs
#[pymethods]
impl ChromiumEngine {
    fn html_to_pdf(&self, html: &str) -> PyResult<Vec<u8>> {
        Ok(self.inner.html_to_pdf(html)?)  // Delegates to core
    }
}
```

## Module Breakdown

### 1. Chromium Module

**Purpose:** HTML/URL/Markdown to PDF conversion via Chrome

**Implementation:**
- Primary: `chromiumoxide` crate (async, actively maintained)
- Fallback: `headless_chrome` crate (sync, simpler API)

**Features:**
- `Page.printToPDF` API with full options
- Screenshot capabilities
- Custom headers/cookies
- Wait conditions (network idle, element visible)
- Page ranges, margins, orientation
- Header/footer templates

**Chrome Management:**
- Auto-download Chrome if not found (optional feature)
- Connection pooling for server mode
- Process lifecycle management

### 2. LibreOffice Module

**Purpose:** Office documents (DOC, XLS, PPT) to PDF

**Implementation:**
- Direct process execution: `soffice --headless --convert-to pdf`
- Similar to Gotenberg's implementation

**Features:**
- Format detection by extension/MIME type
- PDF/A conversion support
- Page range selection
- Quality/compression settings

### 3. PDF Operations Module

**Purpose:** Post-processing PDFs (merge, split, flatten, etc.)

**Implementation:**
- Primary: `lopdf` (pure Rust)
- Alternative: `pdfium` (Google's PDFium bindings)

**Operations:**
- Merge multiple PDFs
- Split by page ranges
- Flatten form fields
- Add watermarks
- Encrypt/Decrypt
- Extract metadata
- Optimize/compress

## Key Dependencies

| Component | Crate | Version | Notes |
|-----------|-------|---------|-------|
| HTTP Server | `axum` | ^0.7 | Modern, async, Tower-based |
| CLI Framework | `clap` | ^4 | Derive macros for args |
| Chrome CDP | `chromiumoxide` | ^0.9 | Primary CDP client |
| Chrome CDP Alt | `headless_chrome` | ^1.0 | Sync alternative |
| PDF Manipulation | `lopdf` | ^0.34 | Pure Rust PDF ops |
| Async Runtime | `tokio` | ^1.0 | For async operations |
| Serialization | `serde` + `serde_json` | ^1.0 | Config and API |
| File Uploads | `multer` | ^3.0 | Multipart form data |
| Python Bindings | `pyo3` | ^0.22 | Optional feature |
| Node Bindings | `napi-rs` | ^2.0 | Optional feature |

## API Compatibility with Gotenberg

### Routes to Implement

| Method | Endpoint | Module | Description |
|--------|----------|--------|-------------|
| POST | `/forms/chromium/convert/html` | Chromium | HTML file → PDF |
| POST | `/forms/chromium/convert/url` | Chromium | URL → PDF |
| POST | `/forms/chromium/convert/markdown` | Chromium | Markdown → PDF |
| POST | `/forms/libreoffice/convert` | LibreOffice | Office doc → PDF |
| POST | `/forms/pdfengines/merge` | PDF Ops | Merge multiple PDFs |
| POST | `/forms/pdfengines/split` | PDF Ops | Split PDF by ranges |
| POST | `/forms/pdfengines/flatten` | PDF Ops | Flatten form fields |
| POST | `/forms/pdfengines/read` | PDF Ops | Extract metadata |
| POST | `/forms/pdfengines/write` | PDF Ops | Add metadata |
| GET | `/health` | System | Health check |
| GET | `/version` | System | Version info |

### Form Fields (Gotenberg-Compatible)

**Chromium Options:**
- `paperWidth`, `paperHeight` (inches)
- `marginTop`, `marginBottom`, `marginLeft`, `marginRight` (inches)
- `landscape` (boolean)
- `printBackground` (boolean)
- `scale` (0.1 - 2.0)
- `pageRanges` (e.g., "1-3,5")
- `headerTemplate`, `footerTemplate` (HTML)
- `preferCssPageSize` (boolean)
- `emulateMediaType` (screen/print)
- `waitForExpression` (JS expression)
- `waitForNetworkIdle` (boolean)

## Research Summary

### Gotenberg Analysis

**How Gotenberg produces native PDFs:**
1. **Chromium Module:** Uses `chromedp` + `cdproto` to call Chrome DevTools Protocol `Page.printToPDF`
2. **LibreOffice Module:** Uses UNO API to convert Office docs to PDF
3. **PDF Engines:** Wraps `pdfcpu`, `qpdf`, `pdftk` for post-processing

**Key APIs Used:**
- `Page.printToPDF` - Core PDF generation
- `Page.captureScreenshot` - For image output
- `Page.getLayoutMetrics` - For page dimensions
- LibreOffice UNO API for document conversion

**True Native PDF Features:**
- Vector-based (not rasterized)
- Searchable/selectable text
- Embedded font subsets
- PDF/A compliance support
- Tagged PDF for accessibility
- Document outline/bookmarks

### Obscura Analysis

**Current State:**
- Lightweight headless browser in Rust
- CDP server for Puppeteer/Playwright compatibility
- No rendering engine (HTML/JS parsing only)
- No PDF or screenshot capabilities

**Missing for PDF:**
- Layout engine (Servo-style)
- Paint/render pipeline
- `Page.printToPDF` implementation
- Screenshot capabilities

**Effort to Add PDF:**
- Estimated: 6-12 months for basic raster PDF
- Estimated: 1-2 years for true vector PDF
- Recommendation: Not viable for PDF goal

### Servo Analysis

**Current State:**
- Full browser engine in Rust
- WebRender GPU-based renderer
- Screenshot capability: `RgbaImage` (bitmap only)
- No PDF output capability found

**Architecture:**
- HTML/CSS → Layout → Paint (WebRender) → GPU → Display/Screenshot
- Output: Raster pixels only
- No vector/text object output path

**Viability for PDF:**
- Cannot produce true native PDFs
- Would need: Display List → PDF backend (massive undertaking)
- Recommendation: Not viable for PDF goal

### Rust Alternatives for Chrome Integration

| Library | Type | Maintenance | PDF Support | Recommendation |
|---------|------|-------------|-------------|----------------|
| `chromiumoxide` | CDP Client | Active (2024-25) | ✅ `print_to_pdf()` | **Primary choice** |
| `headless_chrome` | CDP Client | Slower updates | ✅ `print_to_pdf()` | Sync alternative |
| `fantoccini` | WebDriver | Active | ❌ No PDF | Not suitable |

## Deployment Strategies

### Docker (Recommended for Server Mode)

```dockerfile
FROM rust:1.75 as builder
WORKDIR /app
COPY . .
RUN cargo build --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y \
    chromium \
    libreoffice-writer \
    libreoffice-calc \
    libreoffice-impress \
    fonts-noto fonts-liberation \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/pdfki /usr/local/bin/
EXPOSE 3000
ENTRYPOINT ["pdfki"]
```

### Cargo Install (CLI Mode)

```bash
cargo install pdfki

# Or with specific features
cargo install pdfki --features server,cli

# For Python bindings
cargo install pdfki --features python-bindings
```

### Library Integration

```toml
# Cargo.toml
[dependencies]
pdfki = { version = "0.1", default-features = false, features = ["chromium"] }
```

### Library Deployment (Self-Contained)

When using PDFKi as a **library** (e.g., `pdfki-py` in Python), everything runs self-contained in your container - no external HTTP calls needed.

**What runs inside your container:**

```
Your Python Application
    ↓ (import)
pdfki-py (Rust .so via PyO3)
    ↓ (manages via chromiumoxide)
Chrome subprocess (local WebSocket)
    ↓ (CDP protocol)
PDF generated locally
```

**Dockerfile for Library Mode:**

```dockerfile
FROM python:3.11-slim

# Install Chrome for PDFKi
RUN apt-get update && apt-get install -y \
    chromium \
    fonts-noto \
    fonts-liberation \
    && rm -rf /var/lib/apt/lists/*

# Install pdfki Python package (bundles Rust core + Chrome management)
RUN pip install pdfki

# Your application code
COPY app.py .

# Chrome is managed internally - no external services needed
CMD ["python", "app.py"]
```

**Python usage (self-contained):**

```python
import pdfki

# Chrome starts as subprocess, managed by Rust core
engine = pdfki.ChromiumEngine()

# PDF generated locally - no HTTP calls, fully offline
pdf_bytes = engine.html_to_pdf("<h1>Hello World</h1>")

with open("output.pdf", "wb") as f:
    f.write(pdf_bytes)
```

**Key Characteristics of Library Mode:**

| Aspect | Library Mode (`pdfki-py`) |
|--------|---------------------------|
| **Chrome location** | Same container |
| **Communication** | In-process Rust ↔ Python (PyO3) |
| **Network calls** | ❌ None (fully offline) |
| **External services** | ❌ None required |
| **Chrome lifecycle** | Managed by library (start/stop) |
| **Scaling** | Per-process Chrome instances |

**Comparison with Server Mode:**

| Aspect | Library Mode | Server Mode |
|--------|--------------|-------------|
| **Setup complexity** | `pip install pdfki` | Run separate container/service |
| **Chrome sharing** | One per process | Shared pool across requests |
| **Latency** | Cold start per process | Warm Chrome, lower latency |
| **Best for** | Single app, simple deploy | Multi-tenant, high throughput |
| **Offline capable** | ✅ Yes | ✅ Yes (but needs HTTP to self) |

## Implementation Phases

### Phase 1: Core Chromium Engine (MVP)
- [ ] Set up project structure with workspace crates
- [ ] Integrate `chromiumoxide` for CDP communication
- [ ] Implement `html_to_pdf()` core function
- [ ] Basic CLI: `pdfki convert --html file.html`
- [ ] Chrome auto-download feature

### Phase 2: Server Mode
- [ ] HTTP API with `axum`
- [ ] Gotenberg-compatible routes
- [ ] Form/multipart handling
- [ ] Chrome connection pooling
- [ ] Health checks and metrics

### Phase 3: LibreOffice Integration
- [ ] LibreOffice process wrapper
- [ ] Office document detection
- [ ] PDF/A conversion support
- [ ] Routes for document conversion

### Phase 4: PDF Operations
- [ ] Integrate `lopdf` for PDF manipulation
- [ ] Merge, split, flatten operations
- [ ] Metadata read/write
- [ ] Watermark/stamp support

### Phase 5: Language Bindings
- [ ] Python bindings via PyO3
- [ ] Node.js bindings via napi-rs
- [ ] Go bindings via cgo (if needed)
- [ ] Publish to PyPI, npm, crates.io

## Comparison with Gotenberg

| Feature | Gotenberg (Go) | PDFKi (Rust) |
|---------|---------------|--------------|
| **Chrome Integration** | `chromedp` + `cdproto` | `chromiumoxide` or `headless_chrome` |
| **LibreOffice** | Direct process | Same approach |
| **PDF Engines** | `pdfcpu`, `qpdf`, `pdftk` | `lopdf` (pure Rust) |
| **Memory Safety** | GC (Go) | Compile-time (Rust) |
| **Binary Size** | ~30MB + Chrome | Similar |
| **Performance** | Good | Potentially better (zero-cost) |
| **Ecosystem** | Mature | Growing |
| **Deployment** | Docker-focused | Docker + CLI + Library |

## Alternative Tools Comparison

Understanding how other HTML-to-PDF tools work highlights why PDFKi takes the Chrome-based approach.

### WeasyPrint (Python)

```python
from weasyprint import HTML
HTML(string='<h1>Hello</h1>').write_pdf('out.pdf')
```

**Architecture:**
- **No external browser** - pure Python implementation
- Custom CSS layout engine written in Python
- Parses HTML/CSS directly, renders to PDF internally

**Trade-offs:**
| Aspect | WeasyPrint |
|--------|-----------|
| **Binary size** | ~50MB (Python + deps) |
| **Chrome required** | ❌ No |
| **CSS Grid/Flexbox** | ⚠️ Limited support |
| **JavaScript** | ❌ No |
| **Web fonts** | ⚠️ Limited |
| **Modern CSS** | ❌ Often broken |

**Verdict:** Good for simple invoices, reports. Fails with modern web apps.

### wkhtmltopdf (QtWebKit)

```bash
wkhtmltopdf input.html output.pdf
```

**Architecture:**
- **Single binary** (~20MB) with statically linked QtWebKit
- Fork of WebKit from ~2012
- Renders HTML internally, no external dependencies

**Trade-offs:**
| Aspect | wkhtmltopdf |
|--------|-------------|
| **Binary size** | ~20MB (self-contained) |
| **Chrome required** | ❌ No (bundled QtWebKit) |
| **CSS support** | Legacy (pre-2012 WebKit) |
| **JavaScript** | ⚠️ ES3 era only |
| **Maintenance** | ❌ Unmaintained (archived) |
| **Modern features** | ❌ No CSS Grid, limited Flexbox |

**Verdict:** Legacy tool, outdated web standards. Not suitable for modern HTML.

### PDFKit (Python wrapper)

```python
import pdfkit
pdfkit.from_string('<h1>Hello</h1>', 'out.pdf')
```

**Architecture:**
```
pdfkit (Python wrapper)
    ↓ subprocess.Popen()
wkhtmltopdf binary
    ↓ QtWebKit rendering
PDF output
```

**Key Point:** PDFKit is **just a subprocess wrapper** around wkhtmltopdf. Same limitations, Python syntax sugar.

### Why PDFKi Uses Chrome

| Requirement | WeasyPrint | wkhtmltopdf/PDFKit | PDFKi (Chrome) |
|-------------|-----------|-------------------|----------------|
| **CSS Grid** | ❌ | ❌ | ✅ |
| **Flexbox** | ⚠️ | ⚠️ | ✅ |
| **JavaScript** | ❌ | ❌ | ✅ Full V8 |
| **Web Fonts** | ⚠️ | ⚠️ | ✅ |
| **Modern HTML5** | ⚠️ | ❌ | ✅ |
| **Size** | 50MB | 20MB | 150MB+ |
| **Accuracy** | Good | Poor | **Perfect** |

**The Trade-off:**
- **Lightweight tools** (WeasyPrint, wkhtmltopdf): Smaller, faster install, but limited web standards support
- **PDFKi (Chrome-based)**: Larger binary size, but **true browser fidelity** - pixel-perfect rendering of any modern web content

### Architecture Patterns

| Pattern | Tools | Description |
|---------|-------|-------------|
| **Pure implementation** | WeasyPrint | Custom rendering engine in host language |
| **Bundled engine** | wkhtmltopdf | Statically linked browser engine (outdated) |
| **Subprocess wrapper** | PDFKit | Thin wrapper spawning external binary |
| **CDP client** | **PDFKi**, Gotenberg, Puppeteer | Control real browser via protocol |

**PDFKi Pattern:** CDP client (like Gotenberg) - controls real Chrome instance via WebSocket, gets true browser-grade PDFs.

## Conclusion

PDFKi represents a viable Rust-native alternative to Gotenberg with the following advantages:

1. **Multiple Usage Modes:** Server, CLI, and library (unlike Gotenberg's server-only)
2. **Memory Safety:** Rust's compile-time guarantees vs Go's GC
3. **Performance:** Zero-cost abstractions, potentially lower latency
4. **Pure Rust PDF Ops:** `lopdf` eliminates external tool dependencies for some operations
5. **Language Bindings:** Can be embedded in Python/Node/Go applications

The core insight from this research: **True native PDF generation requires Chrome's `Page.printToPDF` API or equivalent.** There is no lightweight pure-Rust solution for high-fidelity HTML-to-PDF conversion without leveraging Chrome's Skia → PDF backend.

## References

- Gotenberg: https://github.com/gotenberg/gotenberg
- chromiumoxide: https://github.com/mattsse/chromiumoxide
- headless_chrome: https://github.com/rust-headless-chrome/rust-headless-chrome
- lopdf: https://github.com/Hopding/lopdf
- Chrome DevTools Protocol: https://chromedevtools.github.io/devtools-protocol/

---

*Document Version: 1.1*
*Date: April 27, 2026*
*Status: Ready for sub-agent implementation*
