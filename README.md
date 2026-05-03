<h1 align="center">pdfbro</h1>

<p align="center">
  <em>A Rust-native, Gotenberg-compatible PDF service — with a live operator console.</em>
</p>

<p align="center">
  <img src="https://img.shields.io/badge/rust-1.85%2B-orange?style=flat-square" alt="Rust 1.85+"/>
  <img src="https://img.shields.io/badge/gotenberg_parity-~85%25-blue?style=flat-square" alt="Gotenberg parity ~85%"/>
  <img src="https://img.shields.io/badge/license-MIT-blue?style=flat-square" alt="MIT"/>
</p>

---

pdfbro converts **HTML, URLs, Markdown, and Office documents** to PDF using real Chrome under the hood.
It speaks the same HTTP API as [Gotenberg](https://github.com/gotenberg/gotenberg), so most existing clients work with only a base-URL change.

It also ships as a **Rust library, a CLI, and a single binary** — and has a live operator console at `/_/` so you can see what your PDF service is doing without wiring up Grafana first.

> **Status:** active. Core conversions and PDF operations are production-ready. Webhook delivery, batch ZIP output, and a few advanced Chromium wait conditions are in progress.

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

- **Gotenberg-compatible.** Same routes (`/forms/chromium/*`, `/forms/libreoffice/convert`, `/forms/pdfengines/*`), same multipart contract. Drop-in for ~85% of workloads.
- **Memory-safe.** Rust core — no GC pauses, no parser-level CVEs from malformed inputs.
- **Four ways to run.** HTTP server, CLI, Rust library, or Docker. The library is the source of truth; the server and CLI are thin wrappers.
- **Observability-first.** Prometheus metrics, OpenTelemetry traces, and a built-in operator console at `/_/` showing live RPS, p95 latency, per-engine health, concurrency, and active batches over SSE.
- **Slim Docker targets.** Multi-stage Dockerfile produces full, Chromium-only, LibreOffice-only, Cloud Run, and Lambda images.
- **LibreOffice integration via LibreOfficeKit (LOK)** — in-process Rust bindings, no Python daemon, lower memory footprint.

For an honest comparison (parity, gaps, extras) see [`comparison.md`](./comparison.md).

---

## HTTP API

All routes accept `multipart/form-data` via `POST` unless noted.

```
/forms/chromium/convert/{html,url,markdown}
/forms/chromium/screenshot/{html,url,markdown}
/forms/libreoffice/convert
/forms/pdfengines/{merge,split,flatten,rotate,watermark,convert,encrypt}
/forms/pdfengines/metadata/{read,write}
/forms/pdfengines/bookmarks/{read,write}

GET  /health              → JSON health + per-engine status
GET  /version             → plain text
GET  /prometheus/metrics  → Prometheus text format
GET  /_/                  → operator console
GET  /_/sse               → Server-Sent Events stream
```

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

**TLS** is intentionally not handled in-process — put nginx, Caddy, or Envoy in front.

---

## Docker variants

| Target | Contains | Use case |
|--------|----------|----------|
| `pdfbro` | Chromium + LibreOffice | Default |
| `pdfbro-chromium` | Chromium only | HTML/URL/Markdown (~30% smaller) |
| `pdfbro-libreoffice` | LibreOffice only | Office docs (~40% smaller) |
| `pdfbro-cloudrun` | Full + Cloud Run env | Google Cloud Run |
| `pdfbro-lambda` | Full + Lambda Web Adapter | AWS Lambda |

```bash
# Pull a specific variant
docker pull ghcr.io/inkkit/pdfbro:latest
docker pull ghcr.io/inkkit/pdfbro:latest-chromium
docker pull ghcr.io/inkkit/pdfbro:latest-libreoffice

# Build locally
docker build --target pdfbro-chromium -t pdfbro:chromium .
```

---

## Performance

Benchmarked on a 2-CPU / 2 GB Docker cgroup, 4 concurrent clients, containers restarted before each workload for a clean baseline.

| Workload | p50 pdfbro | p50 Gotenberg | p95 pdfbro | p95 Gotenberg | RPS pdfbro | RPS Gotenberg |
|---|---|---|---|---|---|---|
| HTML → PDF (small) | **251 ms** | 413 ms | **425 ms** | 740 ms | **14.2** | 8.7 |
| HTML → PDF (large) | **302 ms** | 406 ms | **508 ms** | 694 ms | **11.9** | 9.0 |
| URL → PDF | **303 ms** | 417 ms | **455 ms** | 726 ms | **12.2** | 8.6 |
| DOCX → PDF | **306 ms** | 492 ms | **557 ms** | 812 ms | **10.9** | 7.1 |
| PDF merge | **11 ms** | 18 ms | **17 ms** | 35 ms | **328** | 192 |

pdfbro starts Chrome **and** LibreOffice eagerly at boot; Gotenberg starts them lazily. Chrome workloads show identical RSS (~310–330 MiB). On LibreOffice workloads, pdfbro's RSS is higher because Chrome is already resident. See [`bench/README.md`](./bench/README.md) for the full methodology and steady-state results.

> CV 20–40% on Chrome workloads — treat numbers as indicative. Chrome PDF rendering is non-deterministic and hardware-specific.

---

## Roadmap

**Active:**
- Webhook callback delivery (scaffold done, delivery in progress)
- Batch API ZIP/merge output (worker exists, output formats in progress)
- Advanced Chromium wait/fail conditions (`waitForSelector`, `failOn*`)

**Planned:**
- Published Docker images (`ghcr.io`)
- Python and Node.js bindings on their respective package registries

---

## Development

```bash
git clone https://github.com/vel/pdfbro && cd pdfbro
cargo build --release
cargo test
make check          # fmt + clippy + unit tests — run before PRs
make run            # docker compose up (full image)
make test-integration  # BDD scenarios in Docker
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

## License

MIT. See [LICENSE](./LICENSE).
