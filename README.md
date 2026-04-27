# Folio

> A modern, Rust-native PDF generation engine - the next generation of document conversion.

[![Rust](https://img.shields.io/badge/rust-1.75%2B-orange.svg)](https://www.rust-lang.org)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](https://opensource.org/licenses/MIT)

## What is Folio?

**Folio** (from Latin *folium*, meaning "leaf" or "sheet of paper") is a high-performance PDF generation engine built in Rust. Like a printer's folio marks the beginning of a new page, Folio marks a new chapter in document conversion technology.

Folio converts HTML, URLs, Markdown, and Office documents to PDF with **true browser-grade fidelity** by leveraging Chrome's rendering engine via the Chrome DevTools Protocol.

## Why "Folio"?

1. **Etymology**: *Folium* → *Folio* represents pages, documents, and the written word
2. **Simplicity**: Short, memorable, easy to type and search
3. **Heritage**: Honors the tradition of documents while embracing modern technology
4. **Distinctive**: Stands apart from generic "pdf-" prefixed tools

## Why Folio Over Alternatives?

| Feature | Folio | WeasyPrint | wkhtmltopdf | Gotenberg |
|---------|-------|-----------|-------------|-----------|
| **Language** | Rust | Python | C++ | Go |
| **Rendering** | Real Chrome | Python engine | QtWebKit (2012) | Chrome |
| **Modern CSS** | ✅ Full | ⚠️ Limited | ❌ Legacy | ✅ Full |
| **JavaScript** | ✅ Full V8 | ❌ None | ⚠️ ES3 | ✅ Full |
| **Usage Modes** | 4 modes (Server/CLI/Lib/Bindings) | Library only | CLI only | Server only |
| **Memory Safety** | ✅ Compile-time | Runtime | Manual | GC |
| **Gotenberg API** | ✅ Compatible | ❌ | ❌ | ✅ Native |

### The Architecture Pattern

| Pattern | Tools | How It Works |
|---------|-------|--------------|
| **Pure implementation** | WeasyPrint | Custom Python rendering engine (limited CSS/JS) |
| **Bundled engine** | wkhtmltopdf | Statically linked QtWebKit (outdated, unmaintained) |
| **Subprocess wrapper** | PDFKit | Thin wrapper spawning wkhtmltopdf |
| **CDP client** | **Folio**, Gotenberg, Puppeteer | Control real Chrome via DevTools Protocol |

Folio uses the **CDP client pattern** - controlling real Chrome for true browser fidelity.

## Documentation

- **[Architecture & Design](./docs/proposal.md)** - Full technical specification
- **[Gotenberg API Compatibility](./docs/gotenberg-spec.md)** - API routes and form fields
- **[Implementation Phases](./docs/proposal.md#implementation-phases)** - Development roadmap
- **[Alternative Tools Analysis](./docs/proposal.md#alternative-tools-comparison)** - Comparison with WeasyPrint, wkhtmltopdf, PDFKit

## Quick Start

### 1. Server Mode (Gotenberg-Compatible API)

```bash
# Run as HTTP service
cargo run -p server -- serve --port 3000

# Or with Docker
docker run -p 3000:3000 folio

# Usage via API
curl -X POST http://localhost:3000/forms/chromium/convert/url \
  -F "url=https://example.com" \
  -F "landscape=true" \
  -o output.pdf
```

### 2. CLI Mode

```bash
# Install
cargo install --path crates/cli

# Convert HTML to PDF
folio convert --html index.html --output out.pdf

# Convert URL to PDF
folio convert --url https://example.com --output out.pdf

# Batch conversion
folio batch --input-dir ./docs/ --output-dir ./pdfs/
```

### 3. Library Mode (Rust)

```rust
use engine::ChromiumEngine;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let engine = ChromiumEngine::new().await?;
    let pdf = engine.html_to_pdf("<h1>Hello World</h1>").await?;
    std::fs::write("output.pdf", pdf)?;
    Ok(())
}
```

### 4. Python Bindings (Self-Contained)

```bash
pip install folio
```

```python
import folio

# Chrome managed internally - no external services
engine = folio.ChromiumEngine()
pdf = engine.html_to_pdf("<h1>Hello World</h1>")

with open("output.pdf", "wb") as f:
    f.write(pdf)
```

**Self-contained deployment:**
```dockerfile
FROM python:3.11-slim
RUN apt-get update && apt-get install -y chromium fonts-noto
RUN pip install folio
COPY app.py .
CMD ["python", "app.py"]
```

### 5. Node.js Bindings

```bash
npm install folio
```

```javascript
const folio = require('folio');

const engine = new folio.ChromiumEngine();
const pdf = await engine.htmlToPdf('<h1>Hello World</h1>');
fs.writeFileSync('output.pdf', pdf);
```

## Architecture

All usage modes share a **single core engine implementation** (`engine` crate):

```
┌─────────────────────────────────────────────────────────────┐
│                    USAGE MODES                              │
│  Server    CLI    Rust Lib    Python    Node.js            │
│    │        │         │          │         │               │
│    └────────┴─────────┴──────────┴─────────┘               │
│                      │                                       │
│           ┌──────────┴──────────┐                          │
│           │      engine          │  ← Single source         │
│           │  • ChromiumEngine     │    of truth              │
│           │  • LibreOfficeEngine  │                          │
│           │  • PdfOperations      │                          │
│           └──────────┬────────────┘                          │
│                      │                                       │
│           ┌──────────┴──────────┐                          │
│           │   Chrome (CDP)       │                          │
│           └──────────────────────┘                          │
└─────────────────────────────────────────────────────────────┘
```

## Gotenberg API Compatibility

Folio implements Gotenberg-compatible routes for drop-in replacement:

| Endpoint | Description |
|----------|-------------|
| `POST /forms/chromium/convert/html` | HTML file → PDF |
| `POST /forms/chromium/convert/url` | URL → PDF |
| `POST /forms/chromium/convert/markdown` | Markdown → PDF |
| `POST /forms/libreoffice/convert` | Office doc → PDF |
| `POST /forms/pdfengines/merge` | Merge PDFs |
| `POST /forms/pdfengines/split` | Split PDF by ranges |
| `POST /forms/pdfengines/flatten` | Flatten form fields |
| `GET /health` | Health check |

See [Gotenberg API Spec](./docs/gotenberg-spec.md) for full details.

## Project Structure

```
folio/
├── Cargo.toml                 # Workspace definition
├── README.md                  # This file
├── Dockerfile                 # Multi-stage build with Chrome
├── crates/
│   ├── engine/                # Shared engine (ChromiumEngine, etc.)
│   ├── cli/                   # Command-line interface
│   ├── server/                # HTTP server (axum)
│   ├── py/                    # Python bindings (PyO3)
│   └── js/                    # Node.js bindings (napi-rs)
└── docs/
    ├── proposal.md            # Full technical specification
    ├── gotenberg-spec.md      # API compatibility details
    └── gap-analysis.md        # Research findings
```

## Key Dependencies

| Crate | Purpose |
|-------|---------|
| `chromiumoxide` | Chrome DevTools Protocol client |
| `axum` | HTTP server framework |
| `clap` | CLI argument parsing |
| `lopdf` | Pure Rust PDF manipulation |
| `tokio` | Async runtime |
| `pyo3` | Python bindings |
| `napi-rs` | Node.js bindings |

## Features

- ✅ **True Native PDFs** - Vector-based, searchable text, embedded fonts
- ✅ **Modern Web Standards** - Full CSS3, JavaScript, Web Fonts support
- ✅ **Multiple Input Formats** - HTML, URL, Markdown, Office docs (via LibreOffice)
- ✅ **PDF Operations** - Merge, split, flatten, watermark, metadata
- ✅ **Four Usage Modes** - Server, CLI, Library, Language Bindings
- ✅ **Gotenberg Compatible** - Drop-in API replacement
- ✅ **Self-Contained** - Library mode requires no external HTTP services
- ✅ **Memory Safe** - Rust's compile-time guarantees

## License

MIT License - see [LICENSE](LICENSE) for details.

## Acknowledgments

- Inspired by [Gotenberg](https://github.com/gotenberg/gotenberg) - the original PDF generation API
- Chrome integration via [chromiumoxide](https://github.com/mattsse/chromiumoxide)
- PDF operations via [lopdf](https://github.com/Hopding/lopdf)

---

*Folio: A new page in PDF generation.*
