<h1 align="center">pdfbro</h1>

<p align="center">
  <em>A Rust-native, Gotenberg-compatible PDF service — with a live operator console.</em>
</p>

<p align="center">
  <img src="https://img.shields.io/github/v/release/inkkit/pdfbro?style=flat-square&label=version" alt="latest release"/>
  <img src="https://img.shields.io/badge/rust-1.85%2B-orange?style=flat-square" alt="Rust 1.85+"/>
  <img src="https://img.shields.io/badge/gotenberg_parity-~90%25-green?style=flat-square" alt="Gotenberg parity ~90%"/>
  <img src="https://img.shields.io/badge/license-AGPL--3.0-blue?style=flat-square" alt="AGPL-3.0"/>
</p>

---

pdfbro converts **HTML, URLs, Markdown, and Office documents** to PDF using real Chrome under the hood.
It speaks the same HTTP API as [Gotenberg](https://github.com/gotenberg/gotenberg), so most existing clients work with only a base-URL change.

It also ships as a **Rust library, a CLI, and a single binary** — and has a live operator console at `/_/` so you can see what your PDF service is doing without wiring up Grafana first.

> **Status:** active development — `v0.2.0`. Core conversions and PDF operations are production-ready.

---

## Quickstart

```bash
# Pull and run
docker run --rm -p 3000:3000 ghcr.io/inkkit/pdfbro:latest

# Convert a URL to PDF
curl -X POST http://localhost:3000/forms/chromium/convert/url \
  -F "url=https://example.com" \
  -o out.pdf

# Open the operator console
open http://localhost:3000/_/
```

Or build from source:

```bash
docker compose up
# or
cargo run -p server -- serve --port 3000
```

---

## Why pdfbro

| | pdfbro | Gotenberg |
|---|---|---|
| Drop-in replacement | ✅ Same routes, same multipart contract | — |
| Memory-safe core | ✅ Rust — no GC, no parser CVEs | Go |
| Deploy options | ✅ Docker · Binary · Rust library · CLI | Docker only |
| Operator console | ✅ Live RPS, p95, health at `/_/` | ❌ |
| Observability | ✅ Prometheus + OTel traces + structured logs | ✅ |
| LibreOffice integration | ✅ In-process via LibreOfficeKit | Subprocess per request |
| Chromium | ✅ Warm, persistent connection | Recycled every N requests |
| Python / Node SDK | 🚧 Coming in v0.1 | ❌ |

---

## HTTP API

All routes accept `multipart/form-data` via `POST` unless noted.

```
/forms/chromium/convert/{html,url,markdown}
/forms/chromium/screenshot/{html,url,markdown}
/forms/libreoffice/convert
/forms/pdfengines/{merge,split,flatten,rotate,watermark,stamp,convert,encrypt}
/forms/pdfengines/metadata/{read,write}
/forms/pdfengines/bookmarks/{read,write}

GET  /health              → JSON health + per-engine status
GET  /version             → plain text
GET  /prometheus/metrics  → Prometheus text format
GET  /_/                  → operator console (live RPS, p95, engine health)
GET  /_/sse               → Server-Sent Events stream
```

---

## Feature parity with Gotenberg

> Full audit: [`comparison.md`](./comparison.md)

### Chromium

| Feature | pdfbro | Gotenberg |
|---|---|---|
| HTML → PDF | ✅ | ✅ |
| URL → PDF | ✅ | ✅ |
| Markdown → PDF | ✅ | ✅ |
| Screenshot (PNG / JPEG / WebP) | ✅ | ✅ |
| Page size, margins, scale | ✅ | ✅ |
| Header / footer templates | ✅ | ✅ |
| `waitForExpression` / `waitForSelector` | ✅ | ✅ |
| `waitDelay` / `waitWindowStatus` | ✅ | ✅ |
| `failOnHttpStatusCodes` | ✅ | ✅ |
| `failOnConsoleExceptions` | ✅ | ✅ |
| Cookie injection | ✅ | ✅ |
| Extra HTTP headers | ✅ | ✅ |
| `nativePageRanges` | ✅ | ✅ |
| PDF/A output via Ghostscript | ✅ | ✅ |
| Encryption (`userPassword` / `ownerPassword`) | ✅ | ✅ |
| Metadata write | ✅ | ✅ |
| Split output | ✅ | ✅ |
| Webhook async delivery | 🚧 scaffolded | ✅ |

### LibreOffice

| Feature | pdfbro | Gotenberg |
|---|---|---|
| DOCX / XLSX / PPTX → PDF | ✅ | ✅ |
| ODT / ODS / ODP → PDF | ✅ | ✅ |
| PDF/A conversion | ✅ | ✅ |
| Password-protected detection | ✅ | ✅ |
| Encryption output | ✅ | ✅ |
| In-process (no daemon restart) | ✅ | ❌ |

### PDF Engines

| Feature | pdfbro | Gotenberg |
|---|---|---|
| Merge | ✅ | ✅ |
| Split (intervals + page ranges) | ✅ | ✅ |
| Flatten | ✅ | ✅ |
| Rotate | ✅ | ✅ |
| Watermark | ✅ | ✅ |
| Stamp | 🚧 partial | ✅ |
| Encrypt / decrypt | ✅ | ✅ |
| PDF/A + PDF/UA conversion | ✅ | ✅ |
| Metadata read / write | ✅ | ✅ |
| Bookmarks read / write | ✅ | ✅ |
| Embed files into PDF | ❌ | ✅ |
| Multi-engine fallback (qpdf / pdftk / pdfcpu) | ❌ | ✅ |

### Infrastructure

| Feature | pdfbro | Gotenberg |
|---|---|---|
| Prometheus metrics | ✅ | ✅ |
| OpenTelemetry traces | ✅ | ✅ |
| Structured logs (JSON / text) | ✅ | ✅ |
| Operator console (live UI) | ✅ | ❌ |
| Health endpoint | ✅ | ✅ |
| Basic auth | ✅ | ✅ |
| TLS termination | ❌ (use reverse proxy) | ✅ |
| SSRF allow/deny rules | partial | ✅ |
| Batch API | 🚧 | ❌ |

---

## Performance

Benchmarked: 2-CPU / 2 GB Docker cgroup, 4 concurrent clients, containers warm throughout.
Source: [`bench/results/20260504T112333Z`](./bench/results/20260504T112333Z/perf.md)

### Latency & throughput

| Workload | pdfbro p50 | Gotenberg p50 | Speedup | pdfbro RPS | Gotenberg RPS |
|---|---:|---:|:---:|---:|---:|
| HTML → PDF (small) | **233 ms** | 413 ms | 1.8× | **16.0** | 5.4 |
| HTML → PDF (large) | **353 ms** | 1,284 ms | 3.6× | **8.0** | 1.9 |
| URL → PDF | **361 ms** | 409 ms | 1.1× | **9.0** | 8.5 |
| Office → PDF (DOCX) | **49 ms** | 485 ms | 9.9× | **72.4** | 7.1 |
| PDF merge | **11 ms** | 15 ms | 1.4× | **296.7** | 207.2 |

pdfbro wins on latency and throughput across every workload.

### Memory (peak RSS, container-wide)

| Workload | pdfbro | Gotenberg | Note |
|---|---:|---:|---|
| HTML → PDF (small) | 314 MiB | 323 MiB | ≈ parity |
| HTML → PDF (large) | 375 MiB | 349 MiB | +7% |
| URL → PDF | 423 MiB | 333 MiB | +27% |
| Office → PDF | 474 MiB | 310 MiB | +53% |
| PDF merge | 497 MiB | 302 MiB | +64% |

pdfbro's higher RSS on LibreOffice/merge workloads reflects **warm engines staying resident** — Chrome and LibreOffice are already loaded from prior workloads, which is what makes the latency wins possible. Gotenberg recycles them periodically (default: Chrome every 100 requests, LibreOffice every 10), trading startup latency for lower idle RSS.

> CV 23–136% on all Chrome workloads — treat numbers as indicative. Chrome PDF rendering is non-deterministic.

---

## CLI

```bash
pdfbro convert --html  index.html          --output out.pdf
pdfbro convert --url   https://example.com --output out.pdf
pdfbro convert --markdown README.md        --output out.pdf
pdfbro convert --office report.docx        --output out.pdf

pdfbro merge   a.pdf b.pdf c.pdf  --output combined.pdf
pdfbro split   input.pdf --mode uniform --span 1 --output-dir ./pages/
pdfbro rotate  input.pdf --angle 90    --output rotated.pdf
pdfbro metadata read  input.pdf
pdfbro metadata write input.pdf '{"Title":"Q2 Review"}'
```

Install: `cargo install --path crates/cli`

Shell completions: `pdfbro completion zsh > ~/.zfunc/_pdfbro`

---

## Library

```rust
use engine::ChromiumEngine;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let engine = ChromiumEngine::launch().await?;
    let pdf = engine
        .html_to_pdf("<h1>Hello</h1>", None, &Default::default(), &Default::default())
        .await?;
    std::fs::write("out.pdf", pdf)?;
    Ok(())
}
```

The engine crate has zero dependency on `axum` — same code path the server uses, without the HTTP layer.

---

## Configuration

Common flags (every flag is also `PDFBRO_*` env-overridable):

```bash
pdfbro-server serve \
  --host 0.0.0.0 --port 3000 \
  --concurrency 8 \
  --request-timeout 120s \
  --chrome /usr/bin/google-chrome \
  --no-sandbox \
  --log-level info --log-format json \
  --otel-enabled --otel-endpoint http://localhost:4318/v1/traces
```

Run `pdfbro-server serve --help` for the full flag reference.

TLS is intentionally not handled in-process — put nginx, Caddy, or Envoy in front.

---

## Docker variants

| Target | Contains | Use case |
|---|---|---|
| `pdfbro` | Chromium + LibreOffice | Default |
| `pdfbro-chromium` | Chromium only | HTML/URL/Markdown (~30% smaller) |
| `pdfbro-libreoffice` | LibreOffice only | Office docs (~40% smaller) |
| `pdfbro-cloudrun` | Full + Cloud Run env | Google Cloud Run |
| `pdfbro-lambda` | Full + Lambda Web Adapter | AWS Lambda |

```bash
docker pull ghcr.io/inkkit/pdfbro:latest
docker pull ghcr.io/inkkit/pdfbro:0.2.0
docker pull ghcr.io/inkkit/pdfbro:latest-chromium
docker pull ghcr.io/inkkit/pdfbro:latest-libreoffice
```

---

## Roadmap

### v0.1 (next)
- **Python SDK** — `pip install pdfbro` wrapping the HTTP API with typed models
- **Node.js SDK** — `npm install pdfbro` with the same
- **Webhook delivery** — callback URL, retry with exponential back-off, HMAC signing
- **Batch API** — submit N jobs, poll status, download ZIP
- **PDF embed** — attach arbitrary files inside a PDF (`/forms/pdfengines/embed`)
- **Multi-engine fallback** — qpdf → pdfcpu → pdftk chain per operation

### v0.2
- **SSRF controls** — fine-grained allow/deny rules for URL and download routes
- **TLS termination** — native cert/key support as an alternative to a reverse proxy
- **Stamp (full)** — overlay-on-pages variant matching Gotenberg's full contract
- **Published Rust crate** — `engine` on crates.io

### Longer term
- **WASM target** — run conversion jobs in the browser via Wasm
- **Distributed mode** — stateless workers behind a shared job queue
- **LLM-powered extraction** — structured data from PDFs using an embedded model

---

## Development

```bash
git clone https://github.com/inkkit/pdfbro && cd pdfbro
cargo build --release
cargo test
make check          # fmt + clippy + unit tests — run before PRs
make run            # docker compose up (full image)
make docker-test    # BDD scenarios in Docker (188 scenarios)
```

**Useful env vars:** `CHROME_PATH`, `LO_PROGRAM_PATH`, `RUST_LOG`, `PDFBRO_PORT`, `PDFBRO_CONCURRENCY`

---

## Contributing

PRs welcome.

1. `make check` passes locally.
2. Conventional Commits style (`feat:`, `fix:`, `docs:`, `chore:`).
3. One feature or fix per PR — split mixed work.

For larger changes, open an issue first so we can agree on the shape before code.

---

## Acknowledgements

- [Gotenberg](https://github.com/gotenberg/gotenberg) — the API contract pdfbro implements
- [chromiumoxide](https://github.com/mattsse/chromiumoxide) — Chrome DevTools Protocol client
- [lopdf](https://github.com/J-F-Liu/lopdf) — pure-Rust PDF manipulation
- [axum](https://github.com/tokio-rs/axum) — HTTP server

---

## License

AGPL-3.0-only. See [LICENSE](./LICENSE).
