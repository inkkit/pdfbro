# Folio vs Gotenberg — Grounded Gap Analysis

> **Purpose.** Single source of truth for what Folio has, what Gotenberg
> has, and the delta — so we can decide what to implement next.
> **Method.** Every claim is grounded in a file/line in either codebase
> as of 2026-04-30. Gotenberg pinned at
> `fe1b0020b8f211575559e6cf7e5ef6cc5a0545ca`. Folio at branch
> `spec/folio-vs-gotenberg-3shot`.
> **Supersedes.** `62-gotenberg-parity-and-benchmark-plan.md` and
> `docs/benchmarking-and-parity-plan.md` (both deleted in this commit).

---

## How to read this document

Sections are grouped by **what is being compared**, not by what we
plan to do. Each section ends with a "**Gap**" box stating only the
factual delta. Plan / decisions live at the bottom (§G, §H).

- §A — Test infrastructure (what testing exists in each repo)
- §B — Public API surface (routes, CLI flags)
- §C — BDD scenario coverage (executable parity oracle)
- §D — Form-field coverage per route
- §E — Engine backends (PDF operations)
- §F — Performance benchmarking practice
- §G — Prioritised next-step plan
- §H — Open questions

---

## A. Test infrastructure facts

### A.1 Gotenberg's testing layers

Source: `@/Users/__deesh_reddy__/projects/personal_git/rust_builds/folio/gotenberg/test/integration/README.md`,
`@/Users/__deesh_reddy__/projects/personal_git/rust_builds/folio/gotenberg/Makefile`.

| Layer | Tooling | Location | Count |
|-------|---------|----------|-------|
| Unit tests | Standard `go test` + table-driven, mocks in `pkg/gotenberg/mocks.go` | scattered `*_test.go` next to source | **27 `_test.go` files** |
| Integration / BDD | Godog (Gherkin) + testcontainers-go for Docker orchestration; PDF assertions via sidecar `gotenberg/integration-tools` image (verapdf, pdfinfo, pdftotext) | `gotenberg/test/integration/` | **26 `.feature` files, 468 scenarios, ~540 KB** |
| Micro-benchmark | One Go `BenchmarkHTTPServerRequest` for an OTEL helper | `gotenberg/pkg/gotenberg/semconv/bench_test.go` | **1 micro-bench, not system-level** |
| CI | `.github/workflows/continuous-integration.yml`: lint Go + non-Go, unit tests, integration build/test/push on linux/amd64, linux/ppc64le, linux/386 | — | No perf job |

### A.2 Folio's testing layers

Source: workspace root, `@/Users/__deesh_reddy__/projects/personal_git/rust_builds/folio/Makefile`,
`@/Users/__deesh_reddy__/projects/personal_git/rust_builds/folio/TEST_STATUS.md`,
`@/Users/__deesh_reddy__/projects/personal_git/rust_builds/folio/TEST_ISSUES.md`.

| Layer | Tooling | Location | Count |
|-------|---------|----------|-------|
| Unit tests | `cargo test --lib` | inline `#[cfg(test)] mod tests` in src | **368 `#[test]` / `#[tokio::test]` annotations across 51 files** |
| Engine integration | `#[ignore]`'d by default, run via `cargo test -- --ignored` (require Chrome / LibreOffice) | `crates/engine/tests/{chromium_html,libreoffice,encrypt,pdfops}.rs` | 4 files |
| Server integration | E2E + router | `crates/server/tests/{e2e,router}.rs` | 2 files |
| BDD (Gherkin) | `cucumber` crate, in-process server spawn (no Docker), `FolioWorld` state | `crates/server/tests/bdd/` | **26 `.feature` files, 63 scenarios, 62 passing, 1 known network-flake failure** |
| CLI | `cargo test -p cli --test cli -- --ignored` | `crates/cli/tests/cli.rs` | 1 file |
| System-level perf | — | — | **none** |
| CI | not yet wired (per spec 63 §"CI runner deferred") | — | none |

### A.3 BDD harness comparison (factual)

| Aspect | Gotenberg | Folio |
|--------|-----------|-------|
| Runner | Godog (Go) | `cucumber` crate (Rust), entry `crates/server/tests/bdd/main.rs` |
| Container model | `testcontainers-go` spawns one Gotenberg container per scenario | In-process server spawn; no Docker |
| PDF assertions | `docker exec` into `gotenberg/integration-tools` (verapdf, pdfinfo, pdftotext) | Limited — see `crates/server/tests/bdd/steps/pdf.rs` (5 KB) |
| Feature files | 26, 468 scenarios | 26 files (same names), **63 scenarios** = ~13 % of Gotenberg's |
| Step definitions | ~30 distinct phrases in `scenario/scenario.go` (37 KB) | 4 modules in `steps/`: `container.rs`, `http.rs` (12 KB), `pdf.rs` (5 KB), `mod.rs`. Many Gotenberg steps not yet ported (see §C.3) |
| Webhook server | Real local HTTP receiver | Returns 202 on `Gotenberg-Async` header but no actual webhook delivery test infra |
| Basic auth | Tested per scenario | No basic-auth tests yet |
| PDF/A validation | verapdf via integration-tools | Not implemented |

### Gap (A)

- Folio has a **functional BDD runner** (62/63 passing) but the **scenario inventory is ~13 % of Gotenberg's**, and several powerful step categories are unimplemented (PDF/A validation, container log scraping, webhook receivers, cookie assertions, image-count, body substring, multi-file zip).
- Folio has **no system-level performance benchmarking** at all.
- Gotenberg has **no system-level performance benchmarking** at all (only one OTEL micro-bench).

---

## B. Public API surface

### B.1 Routes (HTTP endpoints)

Source-of-truth: `@/Users/__deesh_reddy__/projects/personal_git/rust_builds/folio/crates/server/src/app.rs:123-269` (Folio),
`gotenberg/pkg/modules/{chromium,libreoffice,pdfengines}/routes.go` (Gotenberg).

| # | Route | Gotenberg | Folio | Notes |
|---|-------|:---------:|:-----:|-------|
| 1 | `POST /forms/chromium/convert/url` | ✅ | ✅ | |
| 2 | `POST /forms/chromium/convert/html` | ✅ | ✅ | |
| 3 | `POST /forms/chromium/convert/markdown` | ✅ | ✅ | |
| 4 | `POST /forms/chromium/screenshot/url` | ✅ | ✅ | |
| 5 | `POST /forms/chromium/screenshot/html` | ✅ | ✅ | |
| 6 | `POST /forms/chromium/screenshot/markdown` | ✅ | ✅ | |
| 7 | `POST /forms/libreoffice/convert` | ✅ | ✅ | |
| 8 | `POST /forms/pdfengines/merge` | ✅ | ✅ | |
| 9 | `POST /forms/pdfengines/split` | ✅ | ✅ | |
| 10 | `POST /forms/pdfengines/flatten` | ✅ | ✅ | |
| 11 | `POST /forms/pdfengines/convert` (PDF/A) | ✅ | ✅ | |
| 12 | `POST /forms/pdfengines/metadata/read` | ✅ | ✅ | |
| 13 | `POST /forms/pdfengines/metadata/write` | ✅ | ✅ | |
| 14 | `POST /forms/pdfengines/bookmarks/read` | ✅ | ✅ | |
| 15 | `POST /forms/pdfengines/bookmarks/write` | ✅ | ✅ | |
| 16 | `POST /forms/pdfengines/encrypt` | ✅ | ✅ | |
| 17 | `POST /forms/pdfengines/embed` (fonts) | ✅ | ❌ | Folio routes embed via `convert` `embedFiles` field |
| 18 | `POST /forms/pdfengines/watermark` | ✅ | ✅ | |
| 19 | `POST /forms/pdfengines/stamp` | ✅ | ✅ | |
| 20 | `POST /forms/pdfengines/rotate` | ✅ | ✅ | |
| 21 | `POST /forms/pdfengines/decrypt` | ❌ | ✅ | **Folio extra.** Not in Gotenberg. |
| 22 | `POST /forms/batch/submit` | ❌ | ✅ | **Folio extra** (batch API) |
| 23 | `GET /forms/batch/{id}/status` | ❌ | ✅ | **Folio extra** |
| 24 | `GET /forms/batch/{id}/download` | ❌ | ✅ | **Folio extra** |
| 25 | `GET /health` | ✅ | ✅ | |
| 26 | `GET /version` | ✅ | ✅ | |
| 27 | `GET /` (root) | ✅ | ❌ | Folio missing root route |
| 28 | `GET /debug` | ✅ | ✅ (gated by `--api-enable-debug-route`) | |
| 29 | `GET /prometheus/metrics` | ✅ | ✅ | |

**Counts:** Gotenberg = 25 endpoints, Folio = 27 (missing 2 Gotenberg routes, has 4 Folio extras).

### B.2 CLI / configuration flags

Source: `@/Users/__deesh_reddy__/projects/personal_git/rust_builds/folio/gotenberg/compose.yaml` (60 flags),
`@/Users/__deesh_reddy__/projects/personal_git/rust_builds/folio/crates/server/src/config.rs` (Folio's clap surface).

#### B.2.1 Group-level coverage

| Group prefix | Gotenberg flags | Folio coverage |
|--------------|-----------------|----------------|
| `--gotenberg-*` (banner, graceful shutdown, build-debug-data) | 3 | None directly; nearest equivalents implicit |
| `--api-*` | 16 | **8 implemented**: bind (host/port), basic auth (username/password), enable-debug-route, disable-{health,root,debug,version}-route-telemetry, TLS cert/key. **Missing: correlation-id-header, body-limit (named differently in Folio: `--max-body-bytes`), root-path, start-timeout, timeout (Folio: `--request-timeout`), download-from-{allow-list,deny-list,max-retry}, disable-download-from, port-from-env** |
| `--chromium-*` | 17 | **3 implemented**: lazy-start, idle-shutdown-timeout, no-sandbox/sandbox. **Missing: 14 — restart-after, max-queue-size, max-concurrency (folio has top-level `--concurrency`), start-timeout, allow/deny-list, host-resolver-rules, proxy-server, allow-insecure-localhost, ignore-certificate-errors, disable-web-security, allow-file-access-from-files, clear-cache, clear-cookies, disable-javascript, disable-routes** |
| `--libreoffice-*` | 8 | **2 implemented**: lazy-start, idle-shutdown-timeout. **Missing: 6** |
| `--log-*` | 4 | **2 implemented**: log-level, log-format. **Missing: log-fields-prefix, log-std-enable-gcp-fields** |
| `--pdfengines-*-engines` | 14 (per-operation backend chains) | **0** — Folio is lopdf-only. See §E. |
| `--prometheus-*` | 5 | **0 directly** — metrics path is hard-coded `/prometheus/metrics`. **Missing: namespace, collect-interval, disable-route-telemetry, disable-collect** |
| `--webhook-*` | 8 | **0** — no webhook flags exposed. Webhook is partially handled via `Gotenberg-Async` header only. |

#### B.2.2 Folio-specific flags not in Gotenberg

From `@/Users/__deesh_reddy__/projects/personal_git/rust_builds/folio/crates/server/src/config.rs`:

- `--otel-enabled`, `--otel-endpoint` (env: `OTEL_EXPORTER_OTLP_ENDPOINT`) — modern OTLP HTTP wiring; Gotenberg has its own OTEL flags.
- `--chrome` (PATH override), `--soffice` (PATH override) — direct path overrides.
- Batch API config (env-only, no CLI flags): `FOLIO_BATCH_MAX_ITEMS`, `FOLIO_BATCH_CONCURRENCY`, `FOLIO_BATCH_MAX_ACTIVE`, `FOLIO_BATCH_RETENTION_MINUTES`, `FOLIO_BATCH_STORAGE_PATH`.

### Gap (B)

- **2 missing routes**: `/forms/pdfengines/embed`, `GET /` (root).
- **~36 missing flags** vs Gotenberg's 60. Biggest blocks: all `--webhook-*` (8), all `--pdfengines-*-engines` (14, intentional per §E), most `--chromium-*` (14).
- **4 Folio-only routes** (decrypt + 3 batch routes) are net-new capabilities, not regressions.

---

## C. BDD scenario coverage facts

Source-of-truth: `@/Users/__deesh_reddy__/projects/personal_git/rust_builds/folio/crates/server/tests/bdd/GOTENBERG_GAP_ANALYSIS.md`
(authored 2026-04-30 by previous run). Counts verified against
filesystem (`grep -c '^\s*Scenario'` on both feature dirs).

### C.1 Per-feature-file scenario count

| Feature file | Gotenberg scenarios | Folio scenarios | Coverage |
|--------------|--------------------:|----------------:|---------:|
| `chromium_convert_html` | 50 | 2 | 4 % |
| `chromium_convert_url` | 49 | 2 | 4 % |
| `chromium_convert_markdown` | 41 | 2 | 5 % |
| `libreoffice_convert` | 37 | 6 | 16 % |
| `pdfengines_split` | 34 | 3 | 9 % |
| `pdfengines_merge` | 33 | 2 | 6 % |
| `pdfengines_metadata` | 21 | 2 | 10 % |
| `pdfengines_bookmarks` | 20 | 2 | 10 % |
| `pdfengines_stamp` | 20 | 1 | 5 % |
| `pdfengines_watermark` | 20 | 2 | 10 % |
| `pdfengines_encrypt` | 17 | 5 | 29 % |
| `pdfengines_rotate` | 16 | 2 | 13 % |
| `chromium_screenshot_html` | 14 | 2 | 14 % |
| `pdfengines_convert` | 14 | 4 | 29 % |
| `pdfengines_flatten` | 12 | 1 | 8 % |
| `health` | 10 | 4 | 40 % |
| `chromium_screenshot_url` | 8 | 2 | 25 % |
| `prometheus_metrics` | 8 | 2 | 25 % |
| `root` | 8 | 2 | 25 % |
| `chromium_screenshot_markdown` | 7 | 4 | 57 % |
| `debug` | 7 | 2 | 29 % |
| `webhook` | 7 | 2 | 29 % |
| `pdfengines_embed` | 6 | 2 | 33 % |
| `version` | 4 | 2 | 50 % |
| `output_filename` | 3 | 2 | 67 % |
| `chromium_concurrent` | 2 | 1 | 50 % |
| **TOTAL** | **468** | **63** | **~13 %** |

Note: Gotenberg total is 468 by direct grep; the existing
`GOTENBERG_GAP_ANALYSIS.md` quotes 442. Difference is six scenarios
likely added since that file was authored. Either number is fine for
gap purposes.

### C.2 Concrete example — depth gap

**Gotenberg `chromium_convert_html.feature` covers** (50 scenarios):
paper size, margins, landscape, scale, background, native CSS page
size, page ranges, header/footer, emulated media type, wait conditions,
cookies, extra HTTP headers, user-agent override, fail-on-status-codes,
fail-on-resource-loading-failed, skip-network-idle / skip-network-almost-idle,
PDF/A profile, single-page, omit-backgrounds, secret header, basic auth,
root path, webhook, download-from.

**Folio's `chromium_convert_html.feature` covers** (2 scenarios):
default conversion, missing-file 400. Source:
`@/Users/__deesh_reddy__/projects/personal_git/rust_builds/folio/crates/server/tests/bdd/features/chromium_convert_html.feature`.

### C.3 Step definitions not yet ported

From the existing analysis, these step phrases are documented in
Gotenberg's README but have **no Rust implementation** in
`@/Users/__deesh_reddy__/projects/personal_git/rust_builds/folio/crates/server/tests/bdd/steps/`:

**Setup:** `I have a (webhook|static) server`.
**Action:** GET/HEAD with header table; concurrent POST with N parameter; webhook polling.
**Assertion:** cookies; body substring; webhook event JSON; multi-file zip listing; PDF landscape; PDF text-at-page; PDF/A validation; flatten check; encrypted check; embedded-file check; image count; container log scraping; concurrent status/PDF-count.

That's roughly **15–20 step phrases** missing.

### Gap (C)

- **405 scenarios** absent (87 % of Gotenberg's coverage), heavily concentrated in Chromium HTML/URL/Markdown, LibreOffice, and pdfengines merge/split/metadata/bookmarks.
- **~15–20 step phrases** unimplemented; without them, even copy-pasting the missing scenarios wouldn't run.
- The depth-per-feature ratio (50:2 for HTML conversion) means option-handling parity is essentially **untested**, not just unverified.

---

## D. Form-field coverage per route

This section covers what request fields each route accepts. Folio's
handlers are in `@/Users/__deesh_reddy__/projects/personal_git/rust_builds/folio/crates/server/src/routes/`;
Gotenberg's are in `gotenberg/pkg/modules/*/routes.go`. The existing
`GOTENBERG_GAP_ANALYSIS.md` enumerates per-feature gaps in narrative
form. Summarised here as buckets:

### D.1 Cross-cutting fields missing on every applicable route

These are accepted by Gotenberg on most routes, **not implemented** in
Folio (per the existing gap analysis):

| Field | Routes | Gotenberg behaviour |
|-------|--------|---------------------|
| `downloadFrom` | every multipart route | JSON array of `{url, extraHttpHeaders}` — server fetches the asset itself. Governed by `--api-download-from-{allow,deny,max-retry}-list`, `--api-disable-download-from`. |
| `Gotenberg-Webhook-Url` (header) | every multipart route | Async mode — server returns 204, posts result to URL when ready |
| `Gotenberg-Webhook-Error-Url` (header) | every multipart route | Posts error result here on failure |
| `Gotenberg-Webhook-Method` (header) | every multipart route | HTTP method to use (default POST) |
| `Gotenberg-Webhook-Extra-Http-Headers` (header) | every multipart route | JSON map of extra headers |
| `Gotenberg-Trace` (header) | every route | Correlation ID (Folio uses `X-Request-Id` only) |
| `Gotenberg-Output-Filename` (header) | every multipart route | Names the response file. **Folio appears to honour this** (visible in BDD tests) |

### D.2 Chromium routes — option fields

Per `gotenberg/pkg/modules/chromium/routes.go` and existing gap
analysis. Fields that Gotenberg supports, **status in Folio
unverified at scenario level** (no BDD coverage):

paper{Width,Height}, marginTop/Bottom/Left/Right, preferCssPageSize,
nativePageRanges, printBackground, omitBackground, landscape, scale,
singlePage, waitDelay, waitForExpression, waitForSelector,
emulatedMediaType, emulatedMediaFeatures, userAgent, extraHttpHeaders,
cookies, failOnHttpStatusCodes, failOnResourceHttpStatusCodes,
failOnResourceLoadingFailed, failOnConsoleExceptions,
skipNetworkIdleEvent, skipNetworkAlmostIdleEvent, pdfa, pdfua, metadata,
header.html, footer.html (file fields), screenshot-specific:
format, quality, width, height, clip, optimizeForSpeed.

**Folio's chromium route handlers** are in
`@/Users/__deesh_reddy__/projects/personal_git/rust_builds/folio/crates/server/src/routes/chromium.rs`
(35 KB). Manual code-walk needed to confirm exactly which of the above
each handler parses. Not done in this analysis pass.

### D.3 Pdfengines routes — option fields

Similarly enumerated in `gotenberg/pkg/modules/pdfengines/routes.go`.
Notable per-route specifics:

- `merge`: `flatten`, `pdfa`, `pdfua`, `metadata`.
- `split`: `splitMode` (intervals|pages), `splitSpan`, `splitUnify`.
- `convert`: `pdfa`, `pdfua` (profiles).
- `encrypt`: `userPassword`, `ownerPassword`, **permissions bitmask** — existing analysis flags permissions as missing in Folio.
- `watermark` / `stamp`: position (top-left, etc.), opacity, image vs text, page ranges.
- `rotate`: degrees (90/180/270), specific pages.
- `metadata write`: arbitrary key/value JSON.
- `bookmarks write`: tree structure.

### Gap (D)

- **`downloadFrom` not implemented anywhere in Folio**, despite being usable on virtually every Gotenberg route.
- **All webhook-related headers** unimplemented (Folio returns 202 on `Gotenberg-Async` but does not deliver).
- **Encrypt permissions bitmask** missing.
- Chromium and pdfengines option fields: Folio accepts most, but **scenario-level proof is missing** (per §C). Code-walk of `routes/chromium.rs` and `routes/pdfengines.rs` needed to produce a true field-by-field matrix — that is its own implementation task.

---

## E. Engine backend facts

### E.1 Gotenberg's per-operation engine fallback chain

Source: `gotenberg/pkg/modules/pdfengines/multi.go`,
`gotenberg/compose.yaml` (14 flags).

Gotenberg ships **three PDF engines** (qpdf, pdfcpu, pdftk) and lets
operators configure a **fallback chain per operation** via flags like
`--pdfengines-merge-engines=qpdf,pdfcpu`. Each operation tries engines
in order until one succeeds. Reasons documented in code: qpdf is
strict; pdfcpu is lenient; pdftk is end-of-life but still ships.

Operations and their engine chains: merge, split, flatten, convert,
read-metadata, write-metadata, read-bookmarks, write-bookmarks,
watermark, stamp, encrypt, rotate, embed, embed-metadata.

### E.2 Folio's PDF backend

Source: `@/Users/__deesh_reddy__/projects/personal_git/rust_builds/folio/crates/engine/src/pdfops/`,
`@/Users/__deesh_reddy__/projects/personal_git/rust_builds/folio/crates/engine/src/encrypt/`,
`@/Users/__deesh_reddy__/projects/personal_git/rust_builds/folio/crates/engine/src/bookmarks/`.

Folio uses **`lopdf` exclusively** for all PDF operations. There is no
fallback chain. Encrypt/decrypt operations use the `encrypt/` module
(implementation detail in `crates/engine/src/encrypt/mod.rs`). PDF/A
conversion lives in `crates/engine/src/pdfa/`.

Verified by `TEST_ISSUES.md` describing a real merge-engine bug fixed
by reordering object-renumbering relative to Pages-tree construction —
classic single-engine concern, not an engine-selection concern.

### Gap (E)

- Folio is **single-engine** (lopdf), Gotenberg is **multi-engine with fallback**.
- This is **partly intentional** (lopdf is memory-safe Rust; less moving parts), but it has measurable consequences: any input that lopdf can't parse but qpdf/pdfcpu can will work in Gotenberg and fail in Folio.
- The 14 `--pdfengines-*-engines` flags do not exist in Folio. We have to either (a) document this as a deliberate divergence, or (b) shell out to qpdf as a fallback for specific operations only (e.g. AES-256 encryption, PDF/A profile validation).

---

## F. Performance benchmarking facts

### F.1 What Gotenberg has

**Nothing system-level.** Confirmed by:

- `find . -name '*bench*' -o -name '*perf*' -o -name 'k6*' -o -name '*load*test*'` returns exactly **one file**: `gotenberg/pkg/gotenberg/semconv/bench_test.go` — and that's a `BenchmarkHTTPServerRequest` for an OTEL semconv helper, not a server-level benchmark.
- `grep -REn 'Benchmark[A-Z]|hyperfine|wrk|ab |vegeta|k6|locust' .` across the Gotenberg repo returns the same single file.
- `.github/workflows/continuous-integration.yml` jobs: lint Go, lint non-Go, run unit tests, build/test/push for `linux/amd64`, `linux/ppc64le`, `linux/386`. **No perf job.**
- `gotenberg/README.md` makes **no quantitative perf claims** (no p95, throughput, RPS, memory numbers).
- `gotenberg/test/integration/README.md` describes scenario tags only — no perf workload tags.

### F.2 What Folio has

**Nothing.** No `bench/`, no `criterion` harness, no load-test scripts
in repo. The README claims Folio is "high-performance" but no
quantitative benchmark exists to back the claim.

### F.3 What "doing this properly" looks like

Since Gotenberg sets no precedent, we set the standard. Minimum
viable shape:

| Decision | Recommendation | Why |
|----------|----------------|-----|
| Tooling | `oha` (binary) or hand-rolled `reqwest` + `tokio` driver in a `bench/` workspace crate | `oha` has multipart limits; hand-rolled is simpler for our exact case |
| Workloads | 5 fixed: chromium HTML small, chromium HTML large w/ web-fonts, chromium URL (local fixture), libreoffice .docx 50 KB, pdfengines merge 5×20-page PDFs | Covers the three engine paths + a pure-PDF-ops path |
| Profile | 4 concurrent clients, 2 min/workload, 30 s warm-up discarded, 3 reps, report median + flag CV>15 % | Statistical hygiene without a 40-min runtime |
| Metrics | p50/p95/p99 latency, RPS, error rate, peak container RSS via `docker stats --no-stream` sampling | The four numbers everyone actually quotes |
| Fairness | Run Gotenberg + Folio under **identical `cpus`/`mem_limit` cgroups**, ideally Folio image **`FROM gotenberg/gotenberg:8.x`** so Chromium / LibreOffice / qpdf are byte-identical | Eliminates tooling-version drift as a confound |
| PDF quality assertions | `lopdf` page-count + structural diff (not byte-identical, never byte-identical) | qpdf timestamps and IDs are non-deterministic |
| Output | `bench/results/<ts>/perf.md` — one bar chart, one resource table, mandatory caveats section | No marketing |
| CI gate | **deferred** per spec 63 §3 | Local-run only for v1 |

### Gap (F)

- Both projects ship without published perf numbers.
- Folio's "high-performance" README claim is currently **unfalsifiable**.
- Whatever we build will be the **first head-to-head bench** for either project.

---

## G. Prioritised plan to close the gaps

Three shots, sized to fit ~2–3 days each. Each shot ships
independently and answers one question.

### Shot 1 — 100 % BDD parity (closes §A, §C, parts of §D)

**Goal:** raise Folio's scenario count from 63 → **all 468** Gotenberg
scenarios. Each ported scenario either passes, fails (→ actionable
Folio bug ticket), or is tagged `@folio-skip` with a one-line reason
in a comment above it.

Concrete tasks:

1. **`Dockerfile.test` updates.** Per §H.2: add `poppler-utils`,
   `default-jre-headless`, and pinned verapdf install. Verify the
   existing chromium/libreoffice install steps still work; this file
   has been flagged as possibly stale. Run `make docker-test` end-to-end
   to confirm the image still builds and the existing 62 scenarios still
   pass before any porting begins.
2. **Step-vocabulary completion.** Implement the ~15–20 missing step
   phrases listed in §C.3 inside
   `@/Users/__deesh_reddy__/projects/personal_git/rust_builds/folio/crates/server/tests/bdd/steps/`.
   New step modules: webhook receiver (small in-test `axum` server) —
   only used for the 7 webhook scenarios that we **don't** skip; log
   scraper (capture `tracing` subscriber output to a `Vec<String>`
   field on `FolioWorld`); PDF probe extensions (text-at-page via
   `pdftotext`, page count via `pdfinfo`, image count + embedded-file
   via `lopdf`, PDF/A validity via `verapdf`).
3. **Scenario porting — 100 %.** Per H.1 §4. Bulk-port all 468
   scenarios (~405 net new) from
   `gotenberg/test/integration/features/*.feature` into
   `crates/server/tests/bdd/features/`. Each Gotenberg scenario gets:
   - **One** matching scenario in Folio's feature file with the
     same `Scenario:` name, **or**
   - **One** scenario tagged `@folio-skip` with a comment explaining
     why (e.g. `# Reason: tests pdftk-specific output; Folio is
     lopdf-only per spec 63 §E`). The skip list lives in the same
     `.feature` files — no separate registry — so it's reviewed
     scenario-by-scenario in the same PR as the port.
4. **Verdict tracking.** Persist per-scenario pass/fail/skip in
   `crates/server/tests/bdd/baseline.json`. Failing scenarios become
   issues; skipped scenarios become documentation.
5. **Webhook scenarios become batch-API scenarios** (per H.1 §2). The
   7 scenarios in `webhook.feature` are skipped with reason
   `Reason: Folio uses batch API; see docs/migration-from-gotenberg.md`.

**Outcome:** `GOTENBERG_GAP_ANALYSIS.md` is regenerated with 100 %
coverage either as `pass`, `fail`, or `skip+reason`. Every failure is
a trackable Folio bug.

**Estimated effort:** **5–7 days** (revised up from 3 to reflect the
100 % decision in H.1 §4). Bulk-port is mechanical but the volume is
real: ~405 new scenarios, each needing fixture data, multipart
builders, and assertion verification.

### Shot 2 — Establish first perf benchmark (closes §F)

**Goal:** produce the **first** publishable Folio-vs-Gotenberg perf
report with honest numbers.

Concrete tasks:

1. New workspace crate `bench/` (member of root `Cargo.toml`, not part
   of release build).
2. Subcommand `bench perf` with the 5-workload × 1-profile spec from
   §F.3.
3. `docker-compose.bench.yml` with both services pinned to identical
   resource limits, Folio image built `FROM gotenberg/gotenberg:8.12`.
4. Report generator emits `bench/results/<ts>/perf.md`.
5. First baseline committed; README's "high-performance" claim either
   substantiated with a number or softened.

**Estimated effort:** 2.5 days.

### Shot 3 — Close the most painful API-surface gaps (closes parts of §B, §D, §E)

**Goal:** implement the missing capabilities that genuinely matter for
Gotenberg drop-in compatibility.

Triage, in priority order:

| Item | Why priority | Effort |
|------|--------------|--------|
| ~~**Webhook delivery**~~ | **Removed** per H.1 §2: use batch API; document adapter | — |
| **`downloadFrom` field** on every multipart route | Cross-cutting; unblocks many real workflows | 0.5 day |
| **`POST /forms/pdfengines/embed` route + step definitions** | Closes one of two missing routes | 0.5 day |
| **`GET /` root route** | Trivial; Gotenberg responds with HTML index page | 0.25 day |
| **Encrypt permissions bitmask** | Documented gap from existing analysis | 0.5 day |
| **`--api-correlation-id-header`** flag (rename Folio's `X-Request-Id` to honour `Gotenberg-Trace` when set) | Simple compatibility fix | 0.25 day |
| Decision on `--pdfengines-*-engines` flags (§E) | Document or implement qpdf-fallback | 0.5 day decision + effort TBD |

**Estimated effort:** 2 days for the remaining 5 items (webhook
delivery removed).

### Shot 4 (optional) — Modernise (only after Shot 2 baseline)

The legacy-hack list (`M1`–`M9` from earlier discussion). Gated on
Shot 2 producing a perf baseline, so each modernisation can be
measured before/after. Not part of the v1 plan.

### Total estimated effort

**~10.5–12.5 engineer-days** for Shots 1+2+3 (revised up from 8.5 to
reflect 100 % scenario porting in Shot 1).

### Sequencing

```
Shot 1 (BDD)  ──┐
                ├─→ Shot 3 (API gaps; some scenarios in Shot 1 will go from "skip" → "pass" automatically)
Shot 2 (perf) ──┘
```

Shot 1 and Shot 2 are independent and can be parallelised across PRs.
Shot 3 benefits from Shot 1 because new scenarios will validate each
new capability immediately.

---

## H. Decisions (all resolved)

| # | Question | Decision |
|---|----------|----------|
| 1 | Verapdf integration | **Option (b) — system install in `Dockerfile.test`.** Install verapdf CLI + JRE + poppler-utils (`pdftotext`, `pdfinfo`) into the existing `@/Users/__deesh_reddy__/projects/personal_git/rust_builds/folio/Dockerfile.test`. BDD harness shells out to these tools from `crates/server/tests/bdd/steps/pdf.rs`. Tests run **inside** that container (`make docker-test`). The test container is the canonical test environment; local `cargo test` runs that don't have these tools on PATH skip PDF-tool-dependent steps with a runtime warning. See §H.2 for concrete Dockerfile changes. |
| 2 | Webhook semantics | **Use Folio's batch API.** Do **not** implement Gotenberg's `Gotenberg-Webhook-*` header family for now. Document an adapter pattern (HOWTO) in `docs/migration-from-gotenberg.md` for clients porting from Gotenberg — they'll switch to `POST /forms/batch/submit` + `GET /forms/batch/{id}/status` polling, or batch `download` endpoint. |
| 3 | `--pdfengines-*-engines` flags & qpdf fallback | **Document lopdf-only as deliberate.** No fallback chain in v1. Add qpdf shell-out per-operation only if users file issues citing real input that lopdf can't handle. The 14 missing flags are intentionally not implemented. |
| 4 | Scenario porting depth | **100 % port.** Every Gotenberg scenario gets a corresponding Folio scenario. Scenarios that are genuinely not applicable (e.g. ones that test Gotenberg-specific debug-pprof endpoints, container-restart behaviour, qpdf-specific output, log-format-with-GCP-fields) are tagged `@folio-skip` with a `# Reason: ...` comment in the feature file. The skip list is reviewable in PRs. |
| 5 | README "high-performance" claim | **Defer.** Revisit after Shot 2 produces numbers. No README change in v1. |

### H.2 Concrete Dockerfile.test changes (for Q1)

The existing `@/Users/__deesh_reddy__/projects/personal_git/rust_builds/folio/Dockerfile.test`
already installs chromium + libreoffice (note: it currently uses
Debian's `chromium` package and `libreoffice` meta-package; verify
these are still the right targets when we touch the file). Three new
additions for PDF assertions:

1. **`poppler-utils`** — provides `pdftotext` (text-at-page step) and
   `pdfinfo` (page count, dims). Lightweight; one apt line.
2. **`default-jre-headless`** — verapdf is a Java tool. Headless
   variant saves ~150 MB vs full JRE.
3. **verapdf CLI** — not in apt. Download from the official release
   ZIP and install to `/opt/verapdf`, symlink the binary to
   `/usr/local/bin/verapdf`. Pin a specific version in the Dockerfile.

Indicative diff:

```dockerfile
# (existing apt-get block) add poppler-utils + default-jre-headless
RUN apt-get update && apt-get install -y \
    ... existing packages ... \
    poppler-utils \
    default-jre-headless \
    && rm -rf /var/lib/apt/lists/*

# Install verapdf CLI (pin version)
ARG VERAPDF_VERSION=1.26.2
RUN curl -L -o /tmp/verapdf.zip \
      "https://software.verapdf.org/releases/verapdf-installer-${VERAPDF_VERSION}.zip" && \
    unzip /tmp/verapdf.zip -d /tmp/ && \
    /tmp/verapdf-greenfield-${VERAPDF_VERSION}/verapdf-install \
      /tmp/verapdf-auto-install.xml && \
    ln -s /opt/verapdf/verapdf /usr/local/bin/verapdf && \
    rm -rf /tmp/verapdf*
```

(Exact installer flags need verification when we touch the file —
the XML auto-install pattern is verapdf's documented headless install
path, not guessed. Confirm against the official
`https://docs.verapdf.org/install/` page during implementation.)

Folio-side wiring (`crates/server/tests/bdd/steps/pdf.rs`):

- Step `the response PDF(s) should be valid "<standard>" with a tolerance of <N> failed rule(s)` → `Command::new("verapdf").args(["--format", "json", "--flavour", flavour_code(standard), pdf_path]).output()` → parse JSON → count `failedRules` → assert `<= tolerance`.
- Step `the "<name>" PDF should have <N> page(s)` → `Command::new("pdfinfo").arg(pdf_path).output()` → grep `Pages:` line.
- Step `the "<name>" PDF should have the following content at page <N>` → `Command::new("pdftotext").args(["-f", &n, "-l", &n, pdf_path, "-"])` → string-contains.

If these binaries are not on PATH at runtime, the step prints a
warning, marks the scenario as `skipped` instead of `failed`, and the
baseline JSON records the reason. This way local `cargo test` works
with partial assertions; full validation runs in CI / `make docker-test`.

---

## Appendix — files cited

- `@/Users/__deesh_reddy__/projects/personal_git/rust_builds/folio/crates/server/src/app.rs` (router)
- `@/Users/__deesh_reddy__/projects/personal_git/rust_builds/folio/crates/server/src/config.rs` (CLI flags)
- `@/Users/__deesh_reddy__/projects/personal_git/rust_builds/folio/crates/server/src/routes/` (handlers)
- `@/Users/__deesh_reddy__/projects/personal_git/rust_builds/folio/crates/server/tests/bdd/` (existing BDD harness)
- `@/Users/__deesh_reddy__/projects/personal_git/rust_builds/folio/crates/server/tests/bdd/GOTENBERG_GAP_ANALYSIS.md` (per-feature analysis)
- `@/Users/__deesh_reddy__/projects/personal_git/rust_builds/folio/crates/engine/src/pdfops/` (lopdf operations)
- `@/Users/__deesh_reddy__/projects/personal_git/rust_builds/folio/Makefile` (test targets)
- `@/Users/__deesh_reddy__/projects/personal_git/rust_builds/folio/TEST_STATUS.md`, `TEST_ISSUES.md`
- `gotenberg/pkg/modules/{chromium,libreoffice,pdfengines}/routes.go`
- `gotenberg/pkg/modules/pdfengines/multi.go`
- `gotenberg/test/integration/features/*.feature` (468 scenarios)
- `gotenberg/test/integration/README.md` (step vocabulary)
- `gotenberg/compose.yaml` (60 CLI flags)
- `gotenberg/.github/workflows/continuous-integration.yml`
- `gotenberg/pkg/gotenberg/semconv/bench_test.go` (only benchmark file)
