# Gotenberg - Technical Specification

> A Docker-based API for converting documents to PDF.

## Overview

Gotenberg is a production-ready document conversion API that transforms various document formats to PDF. It serves thousands of companies and is trusted in production environments.

## Architecture

### Technology Stack

- **Go** - Primary language
- **Docker** - Containerization
- **Chromium** - HTML/URL to PDF conversion
- **LibreOffice** - Office document conversion
- **PDF Engines** - PDF manipulation (qpdf, pdfcpu, pdftk)
- **Echo** - HTTP framework
- **OpenTelemetry** - Observability

### Module System

Gotenberg uses a self-registering module architecture inspired by CaddyServer:

```
cmd/gotenberg/       -> Entry point (wiring/startup)
pkg/gotenberg/      -> Core interfaces, utilities
pkg/modules/        -> Feature modules
pkg/standard/        -> Module wiring
```

### Module Types

| Module | Purpose |
|--------|---------|
| `api` | HTTP routing, form data, middleware |
| `chromium` | HTML/URL/Markdown→PDF via Chrome |
| `libreoffice` | Office docs→PDF via LibreOffice |
| `pdfengines` | PDF manipulation |
| `qpdf` | PDF operations |
| `pdfcpu` | PDF operations |
| `pdftk` | PDF operations |
| `webhook` | External webhooks |
| `prometheus` | Metrics |
| `exiftool` | Metadata extraction |

## API Routes

Gotenberg exposes 20+ HTTP endpoints for document conversion:

### Chromium Routes

| Endpoint | Method | Input | Output |
|----------|--------|-------|--------|
| `/forms/chromium/convert/url` | POST | URL | PDF |
| `/forms/chromium/convert/html` | POST | HTML | PDF |
| `/forms/chromium/convert/markdown` | POST | Markdown | PDF |
| `/forms/chromium/screenshot/url` | POST | URL | PNG/JPEG |
| `/forms/chromium/screenshot/html` | POST | HTML | PNG/JPEG |

### LibreOffice Routes

| Endpoint | Method | Input | Output |
|----------|--------|-------|--------|
| `/forms/libreoffice/convert` | POST | 100+ formats | PDF |

### PDF Engine Routes

| Endpoint | Method | Input | Output |
|----------|--------|-------|--------|
| `/forms/pdfengines/merge` | POST | PDFs | Merged PDF |
| `/forms/pdfengines/split` | POST | PDF | Split PDFs |
| `/forms/pdfengines/flatten` | POST | PDF | Flattened PDF |
| `/forms/pdfengines/convert` | POST | PDF | Reformed PDF |
| `/forms/pdfengines/metadata/read` | POST | PDF | JSON metadata |
| `/forms/pdfengines/metadata/write` | POST | PDF+metadata | Modified PDF |
| `/forms/pdfengines/bookmarks/read` | POST | PDF | JSON bookmarks |
| `/forms/pdfengines/bookmarks/write` | POST | PDF+bookmarks | Modified PDF |
| `/forms/pdfengines/encrypt` | POST | PDF | Encrypted PDF |
| `/forms/pdfengines/embed` | POST | PDF | Embedded fonts |
| `/forms/pdfengines/watermark` | POST | PDF | Watermarked PDF |
| `/forms/pdfengines/stamp` | POST | PDF | Stamped PDF |
| `/forms/pdfengines/rotate` | POST | PDF | Rotated PDF |

### Webhook Routes

| Endpoint | Method | Purpose |
|----------|--------|---------|
| `/forms/webhook` | POST | External notifications |

## Features

### Document Conversion

1. **HTML/URL to PDF**
   - Full CSS support
   - Print media queries
   - Custom margins, headers, footers
   - Page ranges
   - PDF/A and PDF/UA compliance

2. **Markdown to PDF**
   - GitHub Flavored Markdown
   - Syntax highlighting
   - Table support

3. **Office to PDF** (100+ formats)
   - Word, Excel, PowerPoint
   - OpenDocument formats
   - Rich formatting preservation

### PDF Manipulation

1. **Merge** - Combine multiple PDFs
2. **Split** - Extract pages by range or intervals
3. **Flatten** - Remove interactivity
4. **Compress** - Optimize file size

### Security

1. **Encryption**
   - User password protection
   - Owner permissions
   - 128-bit / 256-bit AES

2. **Watermarking**
   - Text watermarks
   - Image watermarks
   - Tiled watermarks
   - Custom positioning

3. **Stamping**
   - Text stamps
   - Image stamps
   - Custom positioning

### Content

1. **Bookmarks**
   - Read existing
   - Write custom

2. **Metadata**
   - Read/write standard fields
   - Custom fields

3. **Fonts**
   - Embed fonts
   - Subset fonts

### Output Formats

1. **PDF** - Standard PDF
2. **PDF/A** - Archival (1a, 2a, 3a, 3b)
3. **PDF/UA** - Universal accessibility
4. **PNG** - Screenshots
5. **JPEG** - Screenshots

## Chromium Options

Form fields supported by Chromium routes:

### Navigation

| Field | Type | Description |
|------|------|-------------|
| `url` | string | URL to fetch |
| `waitDelay` | duration | Delay before capturing |
| `waitWindowStatus` | string | Window status to wait for |
| `waitForSelector` | string | Selector to wait for |
| `waitForExpression` | string | JS expression |

### Network

| Field | Type | Description |
|------|------|-------------|
| `cookies` | JSON array | Cookies to set |
| `extraHttpHeaders` | JSON object | HTTP headers |
| `userAgent` | string | Custom user agent |
| `failOnHttpStatusCodes` | JSON array | Status codes to fail on |
| `failOnResourceHttpStatusCodes` | JSON array | Resource status codes |

### Output

| Field | Type | Description |
|------|------|-------------|
| `pdf` | JSON object | PDF options |
| `landscape` | bool | Landscape orientation |
| `scale` | float | Scale factor |
| `paperWidth` | float | Paper width in inches |
| `paperHeight` | float | Paper height in inches |
| `marginTop` | float | Top margin |
| `marginBottom` | float | Bottom margin |
| `marginLeft` | float | Left margin |
| `marginRight` | float | Right margin |
| `printBackground` | bool | Print background graphics |
| `pageRanges` | string | Page ranges (e.g., "1-5,7,9-") |

### Header/Footer

| Field | Type | Description |
|------|------|-------------|
| `headerTemplate` | string | HTML header template |
| `footerTemplate` | string | HTML footer template |

### Screenshot

| Field | Type | Description |
|------|------|-------------|
| `format` | string | png or jpeg |
| `quality` | int | JPEG quality (0-100) |
| `clip` | object | Viewport to capture |
| `fullPage` | bool | Capture full page |

### PDF/A Compliance

| Field | Type | Description |
|------|------|-------------|
| `pdfa` | string | PDF/A profile (1a, 2a, 3a, 3b) |
| `pdfua` | bool | PDF/UA compliance |

## PDF Engine Architecture

Gotenberg supports multiple PDF engines for each operation:

| Operation | Engines |
|-----------|---------|
| Merge | qpdf, pdfcpu |
| Split | qpdf, pdfcpu |
| Flatten | qpdf, pdfcpu |
| Convert | qpdf |
| Metadata | qpdf, pdfcpu |
| Bookmarks | qpdf, pdfcpu |
| Encrypt | qpdf, pdfcpu |
| Embed | qpdf |
| Watermark | qpdf, pdfcpu |
| Stamp | qpdf |
| Rotate | qpdf, pdfcpu |
| Sort | pdfcpu |

### Engine Interface

```go
type PdfEngine interface {
    Merge(ctx context.Context, srcs []string, dst string, opts Options) error
    Split(ctx context.Context, src string, dst string, splitMode SplitMode, opts Options) error
    Flatten(ctx context.Context, src string, dst string) error
    Convert(ctx context.Context, src string, dst string, forms PdfFormats) error
    Metadata(ctx context.Context, src string) (map[string]any, error)
    SetMetadata(ctx context.Context, src string, dst string, metadata map[string]any) error
    Bookmarks(ctx context.Context, src string) (any, error)
    SetBookmarks(ctx context.Context, src string, dst string, bookmarks any) error
    Encrypt(ctx context.Context, src string, dst string, userPwd, ownerPwd string, perm Permissions) error
    Embed(ctx context.Context, src string, dst string, embedPath []string) error
    Watermark(ctx context.Context, src string, dst string, watermark Watermark) error
    Stamp(ctx context.Context, src string, dst string, stamp Stamp) error
    Rotate(ctx context.Context, src string, dst string, pages []int, angle int) error
}
```

## Module System Details

### Module Interface

```go
type Module interface {
    Descriptor() Descriptor
}

type Descriptor struct {
    Name        string
    Config      any
    Provisioner Provisioner
    Defaults    Defaults
    Migrator    Migrator
    Routes     []Route
    Help       Help
}
```

### Route Definition

```go
type Route struct {
    Method      string
    Path       string
    IsMultipart bool
    Handler    func(c echo.Context) error
    Validate   func(form *FormData) error
}
```

## Configuration

Gotenberg is configured via:

1. **CLI Flags** - Command line arguments
2. **Environment Variables** - Deployment config
3. **Form Fields** - Per-request options

### CLI Flags

```bash
--api-port=3000              # API port
--api-host=0.0.0.0         # API host  
--log-level=info            # Log level
--root=/tmp/gotenberg      # Working directory

# Module-specific flags
--chromium-auto-start      # Auto-start Chromium
--libreoffice-auto-start  # Auto-start LibreOffice
--pdfengines-merge-engines=qpdf,pdfcpu
--prometheus-enabled      # Enable metrics
```

## Testing

### Integration Tests

- Gherkin/BDD format
- testcontainers-go for Docker orchestration
- Feature files in `test/integration/features/`
- Step definitions in `test/integration/scenario/`

### Unit Tests

- Table-driven tests
- Mocks in `pkg/gotenberg/mocks.go`

## Build & Deployment

### Docker

```dockerfile
FROM gotenberg/gotenberg:8
```

### Compose

Services orchestrated:
- Gotenberg API
- Chromium (in-container)
- LibreOffice (in-container)
- OpenTelemetry Collector
- OpenObserve

## License

MIT

## Sponsors

- TheCodingMachine
- pdfme
- PDFBolt

Powered by Docker and JetBrains