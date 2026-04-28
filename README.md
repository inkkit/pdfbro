# Folio

<p align="center">
  <img src="./docs/assets/folio-logo.svg" alt="Folio Logo" width="200"/>
</p>

<p align="center">
  <a href="https://github.com/yourusername/folio/actions">
    <img src="https://img.shields.io/github/actions/workflow/status/yourusername/folio/ci.yml?branch=main&style=flat-square" alt="CI Status"/>
  </a>
  <a href="https://crates.io/crates/folio">
    <img src="https://img.shields.io/crates/v/folio?style=flat-square" alt="Crates.io"/>
  </a>
  <img src="https://img.shields.io/badge/rust-1.75%2B-orange?style=flat-square" alt="Rust Version"/>
  <img src="https://img.shields.io/badge/license-MIT-blue?style=flat-square" alt="License"/>
  <a href="https://github.com/yourusername/folio/releases">
    <img src="https://img.shields.io/github/v/release/yourusername/folio?style=flat-square" alt="Release"/>
  </a>
</p>

<p align="center">
  <strong>A modern, Rust-native PDF generation engine</strong><br/>
  True browser-grade fidelity • Gotenberg-compatible API • Memory safe
</p>

---

## 📖 Table of Contents

- [What is Folio?](#what-is-folio)
- [Why Folio?](#why-folio)
- [Quick Start](#quick-start)
- [Usage Modes](#usage-modes)
- [Features](#features)
- [Documentation](#documentation)
- [Project Structure](#project-structure)
- [Development](#development)
- [Testing](#testing)
- [Roadmap](#roadmap)
- [Contributing](#contributing)
- [License](#license)

---

## What is Folio?

**Folio** (from Latin *folium*, meaning "leaf" or "sheet of paper") is a high-performance PDF generation engine built in Rust. It converts HTML, URLs, Markdown, and Office documents to PDF with **true browser-grade fidelity** by leveraging Chrome's rendering engine via the Chrome DevTools Protocol (CDP).

> Like a printer's folio marks the beginning of a new page, Folio marks a new chapter in document conversion technology.

### Key Highlights

- **True Browser Fidelity**: Renders using real Chrome/Chromium — full CSS3, JavaScript, Web Fonts support
- **Gotenberg-Compatible**: Drop-in replacement for existing Gotenberg deployments
- **Memory Safe**: Rust's compile-time guarantees prevent entire classes of bugs
- **Multiple Interfaces**: HTTP API, CLI, Rust library, and language bindings (Python/Node.js)
- **Self-Contained**: Library mode requires no external HTTP services

---

## Why Folio?

### Comparison Table

| Feature | **Folio** | Gotenberg | WeasyPrint | wkhtmltopdf |
|---------|------------|-----------|-------------|-------------|
| **Language** | Rust 🦀 | Go | Python | C++ |
| **Rendering** | Chrome (CDP) | Chrome | Custom engine | QtWebKit (2012) |
| **Modern CSS** | ✅ Full | ✅ Full | ⚠️ Limited | ❌ Legacy |
| **JavaScript** | ✅ Full V8 | ✅ Full | ❌ None | ⚠️ ES3 |
| **Usage Modes** | 4 (Server/CLI/Lib/Bindings) | Server only | Library only | CLI only |
| **Memory Safety** | ✅ Compile-time | GC | Runtime | Manual |
| **Gotenberg API** | ✅ Compatible | ✅ Native | ❌ | ❌ |
| **Screenshots** | ✅ Done | ✅ | ❌ | ❌ |

### Architecture Pattern

```
┌─────────────────────────────────────────────────────────────┐
│                    USAGE MODES                              │
│  Server    CLI    Rust Lib    Python    Node.js            │
│     │        │         │          │         │               │
│     └────────┴─────────┴──────────┴─────────┘               │
│                       │                                       │
│            ┌──────────┴──────────┐                          │
│            │      engine          │  ← Single source         │
│            │  • ChromiumEngine     │    of truth              │
│            │  • LibreOfficeEngine  │                          │
│            │  • PdfOperations      │                          │
│            └──────────┬────────────┘                          │
│                       │                                       │
│            ┌──────────┴──────────┐                          │
│            │   Chrome (CDP)       │                          │
│            └──────────────────────┘                          │
└─────────────────────────────────────────────────────────────┘
```

---

## Quick Start

### Prerequisites

- **Rust** 1.75+ ([install](https://rustup.rs/))
- **Chrome/Chromium** (auto-detected) or set `CHROME_PATH`
- **LibreOffice** (optional, for Office document conversion)

### Option 1: HTTP Server (Gotenberg-Compatible)

```bash
# Build and run
cargo run -p server -- serve --port 3000

# Or with Docker
docker build -t folio:latest .
docker run -p 3000:3000 folio:latest

# Convert URL to PDF
curl -X POST http://localhost:3000/forms/chromium/convert/url \
  -F "url=https://example.com" \
  -F "landscape=true" \
  -o output.pdf
```

### Option 2: CLI

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

### Option 3: Rust Library

```toml
# Cargo.toml
[dependencies]
folio-engine = { path = "crates/engine" }
```

```rust
use engine::ChromiumEngine;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let engine = ChromiumEngine::launch().await?;
    let pdf = engine.html_to_pdf("<h1>Hello World</h1>", None, &Default::default(), &Default::default()).await?;
    std::fs::write("output.pdf", pdf)?;
    Ok(())
}
```

### Option 4: Docker Compose (Development)

```bash
# Copy example environment file
cp .env.example .env

# Start Folio with all dependencies
make run

# Run tests
make test-integration

# Stop
make stop
```

---

## Usage Modes

### 1. Server Mode (HTTP API)

Gotenberg-compatible REST API for document conversion:

| Endpoint | Method | Input | Output |
|----------|--------|-------|--------|
| `/forms/chromium/convert/html` | POST | HTML file | PDF |
| `/forms/chromium/convert/url` | POST | URL | PDF |
| `/forms/chromium/convert/markdown` | POST | Markdown | PDF |
| `/forms/chromium/screenshot/html` | POST | HTML | PNG/JPEG/WebP 🚧 |
| `/forms/libreoffice/convert` | POST | Office docs | PDF |
| `/forms/pdfengines/merge` | POST | PDFs | Merged PDF |
| `/forms/pdfengines/split` | POST | PDF | Split PDFs |
| `/health` | GET | - | Health status |

See [API Documentation](./docs/gotenberg-spec.md) for full details.

### 2. CLI Mode

Command-line interface for batch operations and scripting:

```bash
# Convert various formats
folio convert --html file.html --output out.pdf
folio convert --url https://example.com --output out.pdf
folio convert --markdown README.md --output readme.pdf

# PDF operations
folio merge --output combined.pdf file1.pdf file2.pdf
folio split input.pdf --output-dir ./split/
folio flatten input.pdf --output flat.pdf
folio metadata read input.pdf
```

### 3. Library Mode (Rust)

Use Folio as a Rust library in your applications:

```rust
// HTML to PDF
let engine = ChromiumEngine::launch().await?;
let pdf = engine.html_to_pdf(html, None, &opts, &ctx).await?;

// URL to PDF
let pdf = engine.url_to_pdf("https://example.com", &opts, &ctx).await?;

// Markdown to PDF
let pdf = engine.markdown_to_pdf(markdown, &opts, &ctx).await?;
```

### 4. Language Bindings

**Python** ([Planned]):
```python
import folio

engine = folio.ChromiumEngine()
pdf = engine.html_to_pdf("<h1>Hello</h1>")
```

**Node.js** ([Planned]):
```javascript
const folio = require('folio');
const engine = new folio.ChromiumEngine();
const pdf = await engine.htmlToPdf('<h1>Hello</h1>');
```

---

## Features

### ✅ Implemented

- **HTML/URL to PDF**: Full Chrome rendering with print CSS support
- **Markdown to PDF**: GitHub Flavored Markdown with syntax highlighting
- **Office Documents**: Convert 100+ formats via LibreOffice (DOC, DOCX, PPT, XLS, ODT, etc.)
- **PDF Operations**: Merge, split, flatten, rotate, watermark
- **PDF Metadata**: Read/write PDF metadata
- **Gotenberg Compatibility**: Drop-in API replacement
- **Health Checks**: `/health` endpoint with engine status
- **Concurrent Rendering**: Thread-safe browser instance sharing
- **Screenshots**: URL/HTML/Markdown to PNG/JPEG/WebP
- **BDD Testing**: Port Gotenberg's Gherkin scenarios to Rust

### 🚧 In Progress (Phase 2)

- **Prometheus Metrics**: `/prometheus/metrics` endpoint
- **OpenTelemetry**: Distributed tracing and structured logging
- **Process Supervision**: Auto-restart, idle shutdown, queue management

See [Roadmap](./docs/specs/20-missing-features-roadmap.md) for detailed phases.

---

## Documentation

### Core Documentation

| Document | Description |
|----------|-------------|
| [Technical Specification](./docs/proposal.md) | Full architecture and design |
| [Gotenberg API Spec](./docs/gotenberg-spec.md) | API compatibility details |
| [Gap Analysis](./docs/gap-analysis.md) | Research findings |

### Specs (Implementation Guides)

| Spec | Description | Status |
|------|-------------|--------|
| [spec-11-chromium](./docs/specs/11-engine-chromium.md) | Chromium engine | ✅ Done |
| [spec-12-libreoffice](./docs/specs/12-engine-libreoffice.md) | LibreOffice engine | ✅ Done |
| [spec-13-pdfops](./docs/specs/13-engine-pdfops.md) | PDF operations | ✅ Done |
| [spec-20-cli](./docs/specs/20-cli.md) | CLI interface | ✅ Done |
| [spec-30-server](./docs/specs/30-server.md) | HTTP server | ✅ Done |
| [spec-50-bdd-tests](./docs/specs/50-testing-bdd.md) | BDD testing | ✅ Done |
| [spec-20-roadmap](./docs/specs/20-missing-features-roadmap.md) | Feature roadmap | 🚧 New |

### API Reference

- **Chromium Routes**: `/forms/chromium/*` (convert HTML/URL/Markdown, screenshots)
- **LibreOffice Routes**: `/forms/libreoffice/*` (convert Office docs)
- **PDF Engine Routes**: `/forms/pdfengines/*` (merge, split, flatten, etc.)

---

## Project Structure

```
folio/
├── Cargo.toml                      # Workspace definition
├── README.md                       # This file
├── Dockerfile                       # Multi-stage build with Chrome
├── docker-compose.yml              # Development environment
├── Makefile                        # Build/test automation
├── .env.example                    # Configuration template
│
├── crates/
│   ├── engine/                    # Core PDF generation engine
│   │   ├── src/
│   │   │   ├── chromium/          # Chrome/Chromium integration
│   │   │   │   ├── launch.rs     # Browser discovery & launch
│   │   │   │   ├── render.rs     # HTML/URL → PDF
│   │   │   │   └── screenshot.rs # Screenshots (🚧)
│   │   │   ├── libreoffice/       # LibreOffice integration
│   │   │   └── pdfops/           # PDF manipulation
│   │   └── Cargo.toml
│   │
│   ├── server/                    # HTTP server (Gotenberg-compatible)
│   │   ├── src/
│   │   │   ├── routes/            # API route handlers
│   │   │   └── app.rs            # Router configuration
│   │   └── tests/                # Integration tests
│   │
│   ├── cli/                       # Command-line interface
│   │   └── src/commands/         # CLI subcommands
│   │
│   ├── py/                        # Python bindings (🚧 PyO3)
│   └── js/                        # Node.js bindings (🚧 napi-rs)
│
├── docs/
│   ├── proposal.md                 # Technical specification
│   ├── gotenberg-spec.md         # Gotenberg API analysis
│   ├── gap-analysis.md           # Research findings
│   ├── assets/                    # Images, logos
│   └── specs/                    # Implementation specs
│       ├── 11-engine-chromium.md
│       ├── 12-engine-libreoffice.md
│       ├── 13-engine-pdfops.md
│       ├── 20-cli.md
│       ├── 30-server.md
│       ├── 20-missing-features-roadmap.md
│       └── 50-testing-bdd.md
│
└── tests/                         # BDD integration tests
    └── integration/
```

---

## Development

### Building from Source

```bash
# Clone the repository
git clone https://github.com/yourusername/folio.git
cd folio

# Build all crates
cargo build --release

# Run tests
cargo test

# Run with specific features
cargo run -p server -- serve --help
```

### Development Commands

| Command | Description |
|---------|-------------|
| `make build` | Build Docker image |
| `make run` | Start Folio via Docker Compose |
| `make test-unit` | Run unit tests |
| `make test-integration` | Run integration tests (requires Chrome) |
| `make fmt` | Format code |
| `make lint` | Lint with Clippy |
| `make check` | Run format + lint + unit tests |
| `make clean` | Clean build artifacts |

### Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `CHROME_PATH` | Path to Chrome/Chromium executable | Auto-detected |
| `LIBREOFFICE_PATH` | Path to LibreOffice (soffice) | Auto-detected |
| `RUST_LOG` | Log level (trace, debug, info, warn, error) | `info` |
| `FOLIO_PORT` | Server port | `3000` |
| `FOLIO_CONCURRENCY` | Max concurrent renders | CPU count |

---

## Testing

### Test Structure

```
tests/
├── unit/              # Unit tests (cargo test --lib)
├── integration/       # BDD integration tests (🚧)
│   ├── scenarios/     # Test scenarios (ported from Gotenberg)
│   ├── common/       # Test helpers
│   └── testdata/     # Test fixtures
└── e2e/              # End-to-end tests
```

### Running Tests

```bash
# Unit tests (no Chrome required)
cargo test --lib

# Integration tests (skip gracefully if deps missing)
cargo test -p server --test bdd

# E2E tests (skip gracefully if deps missing)
cargo test -p server --test e2e

# All tests (skip gracefully if deps missing)
cargo test -- --test-threads=1

# All tests with Docker
make docker-test
```

### Test Coverage

We're porting Gotenberg's comprehensive BDD test suite:

- ✅ Unit tests: 50+ test cases
- ✅ Integration tests: 52 Gherkin scenarios
- ✅ E2E tests: Server + CLI smoke tests

See [BDD Testing Spec](./docs/specs/50-testing-bdd.md) for details.

---

## Roadmap

### Phase 1: Core Features ✅
- [x] HTML/URL/Markdown → PDF (Chromium)
- [x] Office documents → PDF (LibreOffice)
- [x] PDF operations (merge, split, flatten, rotate, metadata)
- [x] Gotenberg-compatible API
- [x] Screenshots (HTML/URL/Markdown → PNG/JPEG/WebP)
- [x] BDD integration tests (port Gotenberg scenarios)
- [x] Webhook support (async processing)
- [x] Advanced PDF operations (bookmarks, encryption, watermark, stamp, PDF/A)

### Phase 2: Observability & Infrastructure 🚧
- [ ] Prometheus metrics (`/prometheus/metrics` endpoint)
- [ ] OpenTelemetry tracing
- [ ] Process supervision (auto-restart, idle shutdown)
- [ ] Health check enhancements

### Phase 3: Distribution & CI/CD 📋
- [ ] GitHub Actions CI/CD
- [ ] Docker Hub publication
- [ ] Language binding packages (PyPI, npm)

### Phase 4: Bindings & Ecosystem 📋
- [ ] Python bindings (complete)
- [ ] Node.js bindings (complete)

See [Full Roadmap](./docs/specs/20-missing-features-roadmap.md) for detailed planning.

---

## Contributing

Contributions are welcome! Please read our [contributing guidelines](./CONTRIBUTING.md) before submitting a PR.

### Quick Contribution Guide

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'feat: add amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

### Development Workflow

- Use [Conventional Commits](https://www.conventionalcommits.org/) for commit messages
- Ensure `make check` passes before submitting PR
- Add tests for new functionality
- Update documentation as needed
- Keep PRs focused on a single feature/fix

---

## Acknowledgments

- **[Gotenberg](https://github.com/gotenberg/gotenberg)** - The original PDF generation API that inspired this project
- **[chromiumoxide](https://github.com/mattsse/chromiumoxide)** - Chrome DevTools Protocol client for Rust
- **[lopdf](https://github.com/Hopding/lopdf)** - Pure Rust PDF manipulation library
- **[Axum](https://github.com/tokio-rs/axum)** - Ergonomic HTTP server framework

---

## License

Folio is licensed under the MIT License - see [LICENSE](LICENSE) for details.

---

<p align="center">
  <strong>Built with ❤️ in Rust 🦀</strong><br/>
  <em>Folio: A new page in PDF generation.</em>
</p>
