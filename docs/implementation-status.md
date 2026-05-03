# pdfbro Implementation Status (Ground Truth)

> **Single source of truth** derived from direct code inspection.  
> Last audited: 2026-04-29.  
> If a spec status here conflicts with `README.md` or `docs/specs/20-missing-features-roadmap.md`, **this file wins** — the others have documented drift.

---

## Legend

| Symbol | Meaning |
|--------|---------|
| `✅` | **Shipped** — code exists, compiles, has unit tests, and server routes (if applicable) are wired. |
| `🚧` | **Partial** — structural code exists but has functional gaps, stubbed integration, or incomplete test coverage. |
| `❌` | **Not Started** — placeholder crate or spec-only; no production code. |
| `⚠️` | **Stale Docs** — the spec/README claims this is missing, but it is actually implemented. |

---

## Core Engine

| Spec | Feature | README Claim | **Actual** | Evidence | Notes |
|------|---------|--------------|------------|----------|-------|
| 10 | Engine Types | ✅ Done | ✅ Done | `crates/engine/src/types.rs` (30KB) | Core error types, options, configs all present. |
| 11 | Chromium Engine | ✅ Done | ✅ Done | `crates/engine/src/chromium/` — `launch.rs`, `render.rs`, `screenshot.rs`, `wait.rs` | Full CDP integration via `chromiumoxide`. |
| 12 | LibreOffice Engine | ✅ Done | ✅ Done | `crates/engine/src/libreoffice/` | Office doc conversion. |
| 13 | PDF Operations | ✅ Done | ✅ Done | `crates/engine/src/pdfops/` — merge, split, flatten, rotate, watermark, metadata | Watermark & stamp both implemented. |
| 14 | PDF/A Conversion | ❌ Not Started | **✅ Done** ⚠️ | `crates/engine/src/pdfa/mod.rs` | Ghostscript + qpdf fallback. Route `/forms/pdfengines/convert` wired. **README is wrong.** |
| 15 | Webhook System | 🚧 Partially Done | ✅ Done | `crates/server/src/webhook/` | Full async job dispatch with all engine operations (Chromium, LibreOffice, PDF ops, encrypt, bookmarks, screenshots). Retry, queue, config extraction all real. **Shipped.** |
| 16 | PDF Bookmarks | ❌ Not Started | **✅ Done** ⚠️ | `crates/engine/src/bookmarks/mod.rs` (397 lines) | Full read/write/flatten with `lopdf`. Routes `/forms/pdfengines/bookmarks/{read,write}` wired. **README is wrong.** |
| 17 | Watermark | ✅ Done | ✅ Done | Via spec 13 | — |
| 18 | Screenshots | ✅ Done / 🚧 Mixed | ✅ Done | `crates/engine/src/chromium/screenshot.rs` | PNG, JPEG, **WebP** all supported. Server routes wired. Tests present. |
| 19 | PDF Encryption | ✅ Done | **✅ Done** | `crates/engine/src/encrypt/mod.rs` (326 lines) | AES-128/256 via qpdf. CLI `pdfbro encrypt/decrypt`, server routes `/forms/pdfengines/{encrypt,decrypt}` wired. BDD tests + engine integration tests. |

---

## Server & API

| Spec | Feature | README Claim | **Actual** | Evidence | Notes |
|------|---------|--------------|------------|----------|-------|
| 30 | HTTP Server | ✅ Done | ✅ Done | `crates/server/src/app.rs` | All major routes wired. Gotenberg-compatible API. |
| 36 | Chromium Wait Conditions | 🚧 Partially Done | 🚧 Partial | `crates/engine/src/chromium/wait.rs` | Basic conditions exist. Advanced ones (e.g. `skipNetworkIdleEvent`, `failOnResourceLoadingFailed`) may need verification against Gotenberg parity. |
| 37 | LibreOffice Advanced Fields | 🚧 Partially Done | ✅ Done | `crates/engine/src/libreoffice/`, `crates/server/src/routes/libreoffice.rs` | All 30+ export options, native watermarks, and viewer preferences implemented. Form parsing, validation, and filter blob generation wired. |
| 39 | CLI Flag Parity | 🚧 Partially Done | 🚧 Partial | `crates/cli/src/args.rs` | Many flags present; full Gotenberg parity not verified. |
| 44 | Actionable Errors | 🚧 Partially Done | 🚧 Partial | `crates/server/src/error.rs` | Structured errors exist; room for enhancement per spec. |
| 50 | BDD Test Suite | 🚧 Partially Done | 🚧 Partial | `crates/server/tests/bdd/` | 25 feature files, Cucumber runner, real step definitions (HTTP, PDF, container). **README claims "52 scenarios" and "✅" — unverified.** Server binary spawn logic is solid. Pass rate unknown without running. |
| 50-batch | Batch API | 🚧 Partially Done | 🚧 Partial | CLI batch exists (`crates/cli/src/commands/batch.rs`) | Server-side bulk endpoint not present; CLI batch works. |
| 51 | Health Dashboard | 🚧 Partially Done | 🚧 Partial | `/health` JSON works | Visual HTML dashboard not started. |

---

## CLI & Bindings

| Spec | Feature | README Claim | **Actual** | Evidence | Notes |
|------|---------|--------------|------------|----------|-------|
| 20 | CLI | ✅ Done | ✅ Done | `crates/cli/src/` | Convert, merge, split, flatten, metadata, batch commands all present. |
| 40 | Python Bindings | ❌ Not Started | ❌ Not Started | `crates/py/src/lib.rs` | Literally default `add(2,2)` template. `Cargo.toml` has **zero** dependencies. |
| 41 | Node.js Bindings | ❌ Not Started | ❌ Not Started | `crates/js/src/lib.rs` | Literally default `add(2,2)` template. `Cargo.toml` has **zero** dependencies. |

---

## Infrastructure & Observability

| Feature | README Claim | **Actual** | Evidence | Notes |
|---------|--------------|------------|----------|-------|
| Structured Logging | ✅ Done | ✅ Done | `tracing` + `tracing-subscriber` with JSON/text | Request-id aware. |
| Prometheus Metrics | ✅ Done | ✅ Done | `crates/server/src/metrics.rs`, `/prometheus/metrics` | Counters, histograms, gauges present. |
| OpenTelemetry | ❌ Not Started | ✅ Done | `tracing-opentelemetry` + OTLP HTTP exporter wired | `PDFBRO_OTEL_ENABLED` / `OTEL_EXPORTER_OTLP_ENDPOINT` env vars. Batch span processor with Tokio runtime. |
| Process Supervision | 🚧 In Progress | 🚧 Partial | No restart/idle-timeout logic found | Chrome launched once per server lifetime. No auto-restart or queue backpressure yet. |

---

## Features That Are Truly Not Started

| Spec | Feature | Evidence |
|------|---------|----------|
| 38 | Multi-Backend PDF Engines (qpdf, pdfcpu, pdftk) | No code; only `lopdf` used. |
| 40-special | TLS, auth, cloud-run, remote URL download | No code found. |
| 42 | Smart PDF Optimiser | No code. |
| 43 | Font Doctor | No code. |
| 45 | Live Preview Mode | No code. |
| 46 | PDF Size Estimator | No code. |
| 47 | One-Command Install (`curl \| bash`) | No code. |
| 48 | Interactive Docs (`/docs`) | No code. |
| 49 | Template Library | No code. |

---

## Known Documentation Drift

1. **`README.md` spec table** — Claims specs 14, 16, 19 are `❌ Not Done`. They are `✅ Done`.
2. **`README.md` screenshot status** — Usage Modes table shows 🚧 for screenshot endpoints; engine and routes are implemented.
3. **`docs/specs/20-missing-features-roadmap.md`** — Claims screenshots, bookmarks, encrypt, watermark/stamp, PDF/A, Prometheus, webhooks are all `❌ Missing`. This document is **dangerously stale** and should either be archived or rewritten.
5. **`README.md` badges** — Badges point to `ghcr.io/vel/pdfbro` — update when org is created.
6. **`README.md` project structure** — Shows root `tests/` directory; actual tests live inside crate directories (`crates/*/tests/`).
7. **`README.md` BDD test count** — Claims "52 Gherkin scenarios". There are 25 `.feature` files; total scenario count unverified.

---

## Recommendation

- **Retire** `docs/specs/20-missing-features-roadmap.md` or rewrite it to reflect current reality.
- **Maintain** this `docs/implementation-status.md` as the single status source.
- **Update** `README.md` to reference this file for implementation status rather than duplicating it.
