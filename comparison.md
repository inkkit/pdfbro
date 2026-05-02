# Folio vs Gotenberg — In-Depth Feature Comparison

> **Snapshot date:** 2026-05-01
> **Folio commit:** `spec/operator-console` (HEAD: `209a444`)
> **Gotenberg snapshot:** vendored at `tmp/gotenberg/`
> **Companion:** `docs/markdown-plus.md` (the new Markdown variation
> referenced in this comparison's recommendations).

This document is an audit, not a sales sheet. It records what each project
does *today*, what Folio has chosen not to do (deliberately or not), and
what is missing relative to Gotenberg parity. It is structured so that any
single section can be read in isolation by someone deciding whether Folio
is ready for their workload.

---

## 0. TL;DR

| Axis                                    | Folio                              | Gotenberg                          | Verdict                |
|-----------------------------------------|------------------------------------|------------------------------------|------------------------|
| Core conversions (HTML/URL/MD/Office)   | ✅ Implemented                     | ✅ Implemented                     | **Parity**             |
| Screenshot routes (PNG/JPEG/WebP)       | ✅ Implemented                     | ✅ Implemented                     | **Parity**             |
| PDF ops (merge/split/flatten/rotate/…)  | ✅ Implemented (single backend)    | ✅ Implemented (multi backend)     | Folio behind on choice |
| PDF/A & PDF/UA                          | ✅ via Ghostscript                 | ✅ via LibreOffice + engines       | Different paths, OK    |
| Metadata read/write                     | ✅                                 | ✅                                 | **Parity**             |
| Bookmarks read/write                    | ✅                                 | ✅                                 | **Parity**             |
| Encrypt                                 | ✅                                 | ✅                                 | **Parity**             |
| Watermark / stamp                       | ✅ (watermark) / partial (stamp)   | ✅ both                            | Folio behind on stamp  |
| Webhook async delivery                  | 🚧 Scaffolded, callback TODO       | ✅ Production-grade                | **Folio missing**      |
| Batch API                               | 🚧 Endpoints + worker, ZIP TODO    | ❌ Not offered                     | Folio ahead (in spec)  |
| Prometheus metrics                      | ✅ Rich set                        | ✅ Standard set                    | **Parity**             |
| Structured logs                         | ✅ JSON/text + request IDs         | ✅ slog                            | **Parity**             |
| OpenTelemetry traces                    | ✅ OTLP HTTP                       | ✅ OTel SDK                        | **Parity**             |
| Operator console (live UI)              | ✅ Svelte SPA, SSE, charts         | ❌ JSON only                       | **Folio ahead**        |
| Auth (Basic)                            | ✅                                 | ✅                                 | **Parity**             |
| TLS                                     | ❌ (rely on reverse proxy)         | ✅ (cert/key flags)                | **Folio missing**      |
| SSRF / download allow-deny              | partial                            | ✅ rich                            | **Folio behind**       |
| Multi-engine fallback per op            | ❌ (lopdf only)                    | ✅ qpdf/pdfcpu/pdftk/exiftool      | **Folio missing**      |
| Python / Node bindings                  | ❌ Empty crates                    | ❌ Not offered                     | Both miss              |
| CLI (convert/merge/split/…)             | ✅                                 | ❌ Not offered                     | **Folio ahead**        |
| Library (Rust crate) usage              | ✅                                 | ❌ Server-only                     | **Folio ahead**        |

**Bottom line.** Folio reaches roughly **85% of Gotenberg's HTTP-surface
capability** while exceeding it on observability, in-process usage, and
CLI ergonomics. The remaining 15% — webhook callback delivery, multi-engine
fallback chains, TLS, fine-grained SSRF controls, advanced Chromium wait
conditions, the long tail of LibreOffice export filters — is what blocks a
clean drop-in replacement claim today.

---

## 1. Architecture comparison

### 1.1 Gotenberg
- **Language:** Go
- **Framework:** Echo HTTP, modular plugin system
- **Concurrency model:** Process pools per engine (Chromium / LibreOffice
  supervised externally), goroutines per request
- **Rendering:** Each Chromium conversion launches/uses a managed Chrome
  subprocess; LibreOffice spawns `soffice` per conversion
- **Deployment shape:** Container-only — the project is explicitly a
  Docker product
- **Distribution:** Single binary inside a Debian image with all engines
  preinstalled

### 1.2 Folio
- **Language:** Rust
- **Framework:** axum / tower
- **Concurrency model:** Tokio tasks, semaphore-bounded; engines wrapped in
  `SupervisedEngine` with lazy-start / idle-shutdown
- **Rendering:** Chromium via `chromiumoxide` (CDP) — Folio holds the
  client; LibreOffice via `soffice` subprocess
- **Deployment shape:** Container *or* binary *or* Rust library *or* CLI
- **Distribution:** Multi-target Dockerfile (`folio`, `folio-chromium`,
  `folio-libreoffice`, `folio-cloudrun`, `folio-lambda`)

### 1.3 What this means in practice
Folio's choice to live as a *library* is the real architectural divergence
— it is a strict superset of "PDF microservice", whereas Gotenberg only
exists as the microservice form. That choice shapes a lot of what
follows: the supervised-engine wrapper, the operator console, the CLI all
flow from "we are not married to the HTTP surface."

---

## 2. HTTP API comparison

### 2.1 Endpoint matrix

| Route                                             | Folio | Gotenberg | Notes |
|---------------------------------------------------|-------|-----------|-------|
| `POST /forms/chromium/convert/url`                | ✅    | ✅        | parity |
| `POST /forms/chromium/convert/html`               | ✅    | ✅        | parity |
| `POST /forms/chromium/convert/markdown`           | ✅    | ✅        | parity, see §3.4 |
| `POST /forms/chromium/screenshot/url`             | ✅    | ✅        | parity |
| `POST /forms/chromium/screenshot/html`            | ✅    | ✅        | parity |
| `POST /forms/chromium/screenshot/markdown`        | ✅    | ✅        | parity |
| `POST /forms/libreoffice/convert`                 | ✅    | ✅        | parity, filter coverage differs (see §3.5) |
| `POST /forms/pdfengines/merge`                    | ✅    | ✅        | parity |
| `POST /forms/pdfengines/split`                    | ✅    | ✅        | parity |
| `POST /forms/pdfengines/flatten`                  | ✅    | ✅        | parity |
| `POST /forms/pdfengines/convert` (PDF/A, PDF/UA)  | ✅    | ✅        | different backend |
| `POST /forms/pdfengines/rotate`                   | ✅    | ✅        | parity |
| `POST /forms/pdfengines/metadata/read`            | ✅    | ✅        | parity |
| `POST /forms/pdfengines/metadata/write`           | ✅    | ✅        | parity |
| `POST /forms/pdfengines/bookmarks/read`           | ✅    | ✅        | parity |
| `POST /forms/pdfengines/bookmarks/write`          | ✅    | ✅        | parity |
| `POST /forms/pdfengines/encrypt`                  | ✅    | ✅        | parity |
| `POST /forms/pdfengines/embed`                    | ❌    | ✅        | **Folio missing** — attach files inside PDF |
| `POST /forms/pdfengines/watermark`                | ✅    | ✅        | parity |
| `POST /forms/pdfengines/stamp`                    | 🚧    | ✅        | **Folio partial** — overlay-on-pages variant |
| `POST /forms/batch/submit`                        | 🚧    | ❌        | **Folio ahead in spec** |
| `GET  /forms/batch/{id}/status`                   | 🚧    | ❌        | **Folio ahead in spec** |
| `GET  /forms/batch/{id}/download`                 | 🚧    | ❌        | **Folio ahead in spec** |
| `GET  /health`                                    | ✅    | ✅        | parity |
| `GET  /version`                                   | ✅    | ❌        | **Folio ahead** (Gotenberg ships version on root) |
| `GET  /prometheus/metrics`                        | ✅    | ✅        | parity |
| `GET  /_/`, `/_/sse`, `/_/metrics.json`           | ✅    | ❌        | **Folio ahead** — operator console |
| Webhook headers (`Webhook-Url`, etc.)             | 🚧    | ✅        | callback delivery TODO in Folio |

**Visible gaps in HTTP surface:** `embed`, full `stamp`, complete webhook
callback delivery, batch ZIP/merge output. Everything else exists.

### 2.2 Request/response shape

Gotenberg insists on multipart/form-data for *every* conversion. Folio
follows the same convention for all core routes — operators using
Gotenberg client SDKs (`gotenberg-php`, `gotenberg-js-client`,
`gotenberg-go-client`) can point at Folio with only a base-URL change for
the parity routes. This is a deliberate compatibility choice, not an
accident.

---

## 3. Conversion engines, feature by feature

### 3.1 Chromium — PDF generation

| Feature                                 | Folio | Gotenberg | Notes |
|-----------------------------------------|-------|-----------|-------|
| Paper size (named + custom WxH)         | ✅    | ✅        | parity |
| Margins (per side, inches)              | ✅    | ✅        | parity |
| Landscape                               | ✅    | ✅        | parity |
| Print background                        | ✅    | ✅        | parity |
| Omit background (transparency)          | ✅    | ✅        | parity |
| Single-page mode                        | ✅    | ✅        | parity |
| Scale (0.1–2.0)                         | ✅    | ✅        | parity |
| Page ranges                             | ✅    | ✅        | parity |
| Custom header/footer HTML w/ tokens     | ✅    | ✅        | parity |
| Prefer CSS page size                    | ✅    | ✅        | parity |
| Tagged PDF / outline                    | partial | ✅      | Folio passes flags but limited testing |
| Cookies (with sameSite)                 | ✅    | ✅        | parity |
| Extra HTTP headers (scoped)             | partial | ✅      | Folio: flat headers; Gotenberg: regex scope |
| User-Agent override                     | ✅    | ✅        | parity |
| Emulated media type                     | ✅    | ✅        | parity |
| Emulated media features (color-scheme…) | ❌    | ✅        | **Folio missing** |

### 3.2 Chromium — wait / failure conditions

| Feature                                          | Folio | Gotenberg |
|--------------------------------------------------|-------|-----------|
| `waitDelay` (fixed)                              | ✅    | ✅        |
| `waitForExpression` / custom JS predicate        | partial | ✅      |
| `waitWindowStatus`                               | ❌    | ✅        |
| `waitForSelector`                                | ❌    | ✅        |
| `skipNetworkIdleEvent`                           | ❌    | ✅        |
| `skipNetworkAlmostIdleEvent`                     | ❌    | ✅        |
| `failOnHttpStatusCodes`                          | ❌    | ✅        |
| `failOnResourceHttpStatusCodes`                  | ❌    | ✅        |
| `ignoreResourceHttpStatusDomains`                | ❌    | ✅        |
| `failOnResourceLoadingFailed`                    | ❌    | ✅        |
| `failOnConsoleExceptions`                        | ❌    | ✅        |

This is the most concrete Chromium feature gap. Spec
(archived spec) already exists; it just hasn't
been implemented past the stub. **Recommendation:** prioritise.

### 3.3 Chromium — Screenshots

Both projects support PNG/JPEG/WebP, dimensions, JPEG quality, viewport
clipping, optimize-for-speed. **Parity.** The only gap is that Folio's
"capture beyond viewport" code path has fewer integration tests covered
than Gotenberg's.

### 3.4 Markdown route

Both implementations are minimal. Both produce a wrapped HTML document and
hand it to Chromium. Differences:

- **Folio:** `pulldown_cmark` with `Options::all()` + a single embedded
  `markdown.css`. No template injection point.
- **Gotenberg:** `gomarkdown` + `bluemonday` (sanitised HTML). Requires
  the user to supply a wrapper HTML file (named `index.html` in the
  multipart) that pulls the rendered Markdown in via a documented
  mechanism, so the user can inject CSS/fonts/JS.

Each has a different opinion: Folio is "we own the template, give us
markdown"; Gotenberg is "you own the template, give us markdown + a
template."

This comparison's companion document `docs/markdown-plus.md` proposes a
**third route** that combines both philosophies plus front-matter, math,
mermaid, syntax highlighting, includes, and named themes. That work is
designed to ship alongside the existing route, not replace it.

### 3.5 LibreOffice — input formats

Both projects exercise LibreOffice's full ~100-format input matrix (DOC,
DOCX, ODT, ODS, ODP, XLS, XLSX, PPT, PPTX, RTF, CSV, EPUB, etc.). The
difference is in **export options**:

| Export option                               | Folio | Gotenberg |
|---------------------------------------------|-------|-----------|
| Landscape                                   | ✅    | ✅        |
| Native page ranges                          | partial | ✅      |
| Single-page mode (Calc/Sheet)               | ✅    | ✅        |
| Password-protected input documents          | ❌    | ✅        |
| Update indexes on conversion                | ❌    | ✅        |
| Export form fields                          | ❌    | ✅        |
| Export bookmarks                            | partial | ✅      |
| Export notes / placeholders                 | ❌    | ✅        |
| Bookmarks → PDF destinations                | ❌    | ✅        |
| Image compression (lossless / JPEG quality) | ❌    | ✅        |
| Image resolution reduction                  | ❌    | ✅        |
| Viewer preferences (initial view, zoom…)    | ❌    | ✅        |
| Native LibreOffice watermark                | ❌    | ✅        |
| PDF/A-1b / 2b / 3b output                   | ✅    | ✅        |
| PDF/UA output                               | ✅    | ✅        |

Spec (archived spec) lists most of these as
explicit TODOs.

### 3.6 PDF engine ops

Gotenberg's killer feature here is **per-operation engine selection with
fallback chains**: `qpdf → pdfcpu → pdftk` for merge, etc. If qpdf
chokes on a malformed PDF, pdfcpu retries transparently. Folio uses a
single backend (`lopdf`, pure Rust) for *every* op, which is operationally
simpler but means a malformed input has no recovery path other than
"return an error and let the caller deal with it."

This is the largest pure-feature gap. Three options for closing it:

- **(A) Re-implement engine fallback in Rust** by shelling out to qpdf /
  pdfcpu / pdftk binaries. Cheapest. Loses some of the "no external tools"
  posture but Folio already shells out to `soffice` and `gs`, so the
  posture is already mixed.
- **(B) Stay single-backend and harden lopdf** — file upstream patches for
  the malformed-input cases that arise. Highest engineering cost, slowest
  return.
- **(C) Punt** — say in the README that Folio is "well-formed PDF only"
  and let users pre-validate. Honest, but caps the addressable workload.

Spec (archived spec) exists and points at (A).

---

## 4. Async delivery — webhooks

Gotenberg's webhook module is mature: middleware POSTs the produced file
to a user-supplied URL with retry logic, allow/deny lists (literal and
regex), private/public IP filtering for SSRF, configurable retry windows,
sync vs async modes.

Folio has the **shape** of this — `Webhook-Url` and friends parse,
`crates/server/src/webhook/` exists, the worker runs — but the actual
callback delivery path is marked TODO. Until that lands, an operator
sending `Webhook-Url` headers will see a 202 and then... nothing.

**Status:** spec (archived spec) is the source of truth; the
gap is implementation, not design.

---

## 5. Batch API (Folio-only)

Folio has a server-side batch surface that Gotenberg has no equivalent
for: submit a JSON manifest of N jobs, get back a `batch_id`, poll for
progress, download a ZIP when done. The endpoints exist; the worker runs;
ZIP packaging and per-item-failure semantics are TODO.

This is a real differentiator, not just parity-plus. Worth finishing.

---

## 6. Operator console (Folio-only)

This is where Folio is unambiguously ahead.

Gotenberg gives you `/health` (JSON) and `/prometheus/metrics`
(Prometheus text). That is the entire operability surface. To get any
actual visibility you wire it into Grafana yourself.

Folio ships a Svelte SPA at `/_/` driven by Server-Sent Events that
shows, live, in one screen:

- RPS, p95 latency, error %, in-flight count
- Per-route table (RPS, p50/p95/p99, error %, load %)
- Engine status (Chromium / LibreOffice up/down + restart count)
- Concurrency grid (active vs cap, with warn/crit thresholds)
- Throughput strip (30-min windowed RPS + p95 with SLA overlay)
- Activity strip (error % + queue depth)
- Resources (CPU %, memory MB)
- Active batches (progress + per-item state)
- Last-20 request log + last-10 error log

The recent commit history (last 30 commits, all dashboard-focused) shows
this is the team's current focus and it is in active polish.

This shifts the value proposition: Folio is not "Gotenberg in Rust", it
is "Gotenberg-compatible PDF service that you can run without immediately
needing a dashboards engineer."

---

## 7. Configuration / CLI flags

Gotenberg has a wide and stable flag surface (api, webhook, pdfengines,
prometheus, basic auth). Folio's flags cover the same axes but are
narrower:

| Knob                                        | Folio | Gotenberg |
|---------------------------------------------|-------|-----------|
| API port / bind / TLS                       | port + bind ✅, TLS ❌ | ✅ |
| Body limit (multipart)                      | ✅    | ✅        |
| Per-request timeout                         | ✅    | ✅        |
| Root path (reverse-proxy mount)             | ❌    | ✅        |
| Correlation ID header                       | ✅    | ✅        |
| Basic-auth user/pass (env)                  | ✅    | ✅        |
| Download allow/deny lists                   | partial | ✅      |
| Download deny private/public IPs            | partial | ✅      |
| Download max retries                        | ✅    | ✅        |
| Disable downloads entirely                  | ❌    | ✅        |
| Enable debug route                          | ❌    | ✅        |
| Webhook allow/deny + SSRF filters           | partial | ✅      |
| Webhook retry waits / counts / timeouts     | partial | ✅      |
| Per-op engine selection (merge/split/…)     | ❌    | ✅        |
| Disable specific PDF engine routes          | ❌    | ✅        |
| Prometheus namespace / collect interval     | partial | ✅      |
| Disable route telemetry                     | ✅    | ✅        |

**Recommendation:** the gaps here are individually small; add them one
by one as `--root-path`, `--api-disable-debug`, `--api-disable-download`,
and SSRF flags. Spec (archived spec) already exists.

---

## 8. Auth & security posture

| Concern                                    | Folio | Gotenberg |
|--------------------------------------------|-------|-----------|
| HTTP Basic Auth                            | ✅    | ✅        |
| Token / JWT auth                           | ❌    | ❌        |
| Per-route authorisation                    | ❌    | ❌        |
| TLS in-process                             | ❌    | ✅        |
| `file://` rejected on URL routes           | ✅    | ✅        |
| SSRF: private IP block                     | partial | ✅      |
| SSRF: public IP block                      | ❌    | ✅        |
| Download URL allow/deny regex              | ❌    | ✅        |
| Webhook URL allow/deny regex               | partial | ✅      |
| Multipart body limit enforcement           | ✅    | ✅        |
| Memory-safe core                           | ✅ (Rust) | ❌ (Go GC) |

Folio's Rust core is a real security advantage at the parser level;
Gotenberg's mature SSRF/download/webhook filter stack is a real security
advantage at the network edge. They are not the same thing and Folio
should not pretend memory-safety substitutes for the network filters —
both matter.

---

## 9. Observability

| Surface                              | Folio | Gotenberg |
|--------------------------------------|-------|-----------|
| Structured logs (JSON / text)        | ✅    | ✅        |
| Request ID propagation               | ✅    | ✅        |
| Prometheus counters/histograms       | ✅    | ✅        |
| OpenTelemetry traces                 | ✅ (OTLP HTTP) | ✅ |
| OpenTelemetry metrics                | ✅    | ✅        |
| Live operator UI                     | ✅    | ❌        |
| SSE event stream                     | ✅    | ❌        |
| Per-engine health endpoint detail    | ✅ (per-engine) | ✅ |

**Parity, with Folio ahead on the live UI.** No gaps to call out here.

---

## 10. Distribution surfaces

| Surface                          | Folio | Gotenberg |
|----------------------------------|-------|-----------|
| HTTP server (Docker)             | ✅    | ✅        |
| HTTP server (raw binary)         | ✅    | ❌ (officially Docker-only) |
| CLI binary (`folio convert …`)   | ✅    | ❌        |
| Rust library (in-process)        | ✅    | ❌        |
| Python bindings                  | ❌ (placeholder) | ❌ |
| Node.js bindings                 | ❌ (placeholder) | ❌ |
| Cloud Run image                  | ✅ (`folio-cloudrun`) | ❌ |
| AWS Lambda image                 | ✅ (`folio-lambda`) | ❌ |
| Slim images (Chromium-only / LO-only) | ✅                | ❌ |

Folio has done real work here that Gotenberg has explicitly said no to
(Gotenberg's stance is that it is a Docker product; everything else is
the user's problem). The *empty* Python/Node bindings undercut that
narrative — the placeholder crates (`crates/py/`, `crates/js/`) imply a
roadmap commitment that has no actual code. Either ship them or remove
the placeholders; the worst state is "empty crate that suggests a
feature."

---

## 11. Test coverage

- **Folio:** ~43 unit tests passing across types, engine, pdfops, routes;
  ~25 BDD scenarios ported from Gotenberg (runner partially complete);
  5 e2e smoke tests; Docker-based PDF/A validation via verapdf.
  `TEST_STATUS.md` and `TEST_ISSUES.md` are surprisingly honest about
  what is and isn't passing.
- **Gotenberg:** mature integration test suite that has been running for
  years; thousands of cumulative production deployments worth of
  battle-testing.

The maturity gap is real. Folio's BDD harness is the right move (re-using
Gotenberg's scenarios is the cheapest path to credibility), it just needs
to finish.

---

## 12. What Folio did well, with credit

- **Library-first architecture.** Being usable as a Rust crate, a CLI,
  and a server is a substantial superset of Gotenberg's positioning, and
  was clearly an early decision rather than a retrofit (the engine crates
  have no axum imports).
- **Operator console.** The SSE-driven Svelte dashboard is a genuinely
  better operator experience than Gotenberg's bare metrics endpoint.
  This was the right thing to invest in last.
- **Supervised engines with lazy-start / idle-shutdown.** Memory profile
  on idle should be substantially better than Gotenberg's eager
  process-pool model — relevant for serverless deploys (Cloud Run /
  Lambda images exist for a reason).
- **Atomic concurrency tracking** (commit `209a444`) over sampled
  semaphore reads. Small fix, but it's the kind of correctness work that
  shows the team has actually been driving the dashboard against real
  load.
- **Honest test status docs.** `TEST_STATUS.md` and `TEST_ISSUES.md`
  exist and are not propaganda. Easy to underestimate how rare this is.

## 13. What Folio did not do, deliberately

- **No multi-engine fallback** for PDF ops. Single backend (`lopdf`)
  keeps the dependency surface small. Defensible until you hit the first
  malformed-input bug report, at which point the answer becomes "punt or
  shell out." Decide before users force the decision.
- **No batch-of-batches / DAG job system.** The batch API is a flat list
  of jobs, not a workflow. This is the right call for a PDF service —
  workflow tools belong elsewhere.
- **No template engine for Markdown.** The basic Markdown route does not
  let users inject Liquid/Handlebars/etc. The companion proposal
  (`docs/markdown-plus.md`) preserves this stance: front-matter
  substitution only, no full templating.
- **No cross-request server-side state.** Includes resolve from the
  upload only. This is a security posture, not laziness.

## 14. What Folio did not do, but should

In rough priority order (cheapest-impact-per-LOC first):

1. **Finish webhook callback delivery** ((archived spec)). The
   Async-202 path is half-built; finishing it unblocks Gotenberg client
   compatibility.
2. **Wire advanced Chromium wait conditions** (spec 36): `waitForSelector`,
   `waitWindowStatus`, `failOn*` family. Each is a single CDP call.
3. **Finish batch ZIP packaging + per-item failure semantics**
   (spec 50-batch). The endpoints already exist; finishing them turns a
   stub into a differentiator.
4. **Add `embed` + finish `stamp`** routes. Last gaps in the
   `/forms/pdfengines/*` matrix.
5. **Implement `--root-path` and SSRF/download filter flags**
   (spec 39). Small individual changes; collectively close the
   security/operations gap.
6. **Decide on multi-engine PDF ops** (spec 38). Either ship qpdf/pdfcpu
   shellout or commit to "well-formed PDFs only" in the README. Current
   middle ground is the worst of both.
7. **Either ship the Python/Node bindings or remove the placeholder
   crates.** Empty crates are a roadmap lie.
8. **Fill in LibreOffice export filters** (spec 37). Long tail; do as
   user demand surfaces, not preemptively.
9. **Build Markdown+** (`docs/markdown-plus.md`). Net-new feature, not
   Gotenberg parity, but uses the operator-console + observability
   investment as a foundation.

## 15. What Folio did not do, and arguably should not

- **TLS in-process.** Use a reverse proxy. Adding TLS to the binary adds
  cert rotation, OCSP stapling, ALPN — none of which Folio is positioned
  to do better than nginx/Caddy/envoy. The current "not implemented"
  status is correct; it should be made *explicit* in the README.
- **OAuth / JWT / RBAC.** PDF services are not where you want to be doing
  identity. Stay with Basic Auth + reverse-proxy auth headers; document
  the pattern.
- **A workflow / DAG engine on top of batch.** Out of scope. Forever.
- **A web-UI document editor.** Folio's UI is an operator console, not an
  end-user product. The line should stay there.

---

## 16. What we did vs what we did not — concise scorecard

### Done
- Six Chromium routes (HTML/URL/Markdown × convert+screenshot)
- LibreOffice convert route + 100+ input formats
- All standard PDF ops bar `embed` and full `stamp`
- PDF/A and PDF/UA via Ghostscript
- Bookmarks, metadata, encrypt
- HTTP Basic Auth
- Prometheus metrics + OpenTelemetry traces + structured logs
- Operator console (Svelte + SSE) — distinct lead over Gotenberg
- CLI with convert/merge/split/flatten/rotate/metadata
- Multi-target Docker images (full / chromium-only / lo-only / cloudrun /
  lambda)
- Library usage as a Rust crate
- BDD test harness (in progress)

### Not done
- Webhook callback delivery (scaffold only)
- Batch ZIP output / per-item failure semantics (scaffold only)
- `embed` route, full `stamp` route
- Advanced Chromium wait/fail conditions (spec 36)
- Long tail of LibreOffice export options (spec 37)
- Multi-engine PDF op fallback (spec 38)
- Several CLI flags (`--root-path`, full SSRF filters) (spec 39)
- Python and Node.js bindings (empty placeholder crates)
- Cookie/header-scope regex filtering on Chromium routes
- Emulated media features (color-scheme, prefers-reduced-motion)
- TLS in-process *(deliberately not done; document the choice)*

### Should be added (new)
- **Markdown+** — see `docs/markdown-plus.md`. Builds on existing
  Chromium pipeline; uses existing observability stack; ships standalone
  without blocking on webhook/batch/bindings.
- **Stage-level histograms** for any multi-stage route (Markdown+ is the
  obvious first user). Genuine new information, not just parity.
- **Operator console "Markdown+" panel**, conditionally rendered when
  traffic exists. Avoids polluting empty deployments.

### Should *not* be added
- TLS in-process
- Identity/RBAC inside Folio
- Workflow/DAG engine on top of batch
- A document editor
- A second Markdown route that is "just like the first but with an
  option" — extension, not duplication

---

*End of comparison. The companion proposal in `docs/markdown-plus.md`
implements the "should be added (new)" section's first item.*
