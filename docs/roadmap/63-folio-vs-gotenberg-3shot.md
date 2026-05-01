# Spec 63 — Folio vs Gotenberg: 3-Shot Plan

> Status: **Outline / planning doc — decisions accepted.** Implementation
> specs for each shot land as separate PRs.
> Branch: `spec/folio-vs-gotenberg-3shot`
> Supersedes the elaborate parts of `62-gotenberg-parity-and-benchmark-plan.md`.
> Reference clone: `gotenberg/` pinned to
> **`fe1b0020b8f211575559e6cf7e5ef6cc5a0545ca`** (chore(deps): update golang
> to 1.26.2, 2026-04-30). Recorded in `bench/parity/baseline.json` per run.

## Why this exists

Spec 62 ballooned. This doc collapses the work into **three sequential
shots** that each take ~2–3 days, ship independently, and answer one
question:

1. **Shot 1 — Parity:** Does Folio behave like Gotenberg on real
   requests?
2. **Shot 2 — Performance:** Is Folio actually faster / leaner?
3. **Shot 3 — Modernization:** Drop Gotenberg's legacy hacks Folio
   doesn't need.

Each shot is independently merge-able and produces a concrete
artifact. Stop after any shot if priorities change.

---

## Shot 1 — Gherkin parity harness

**Goal:** replay Gotenberg's own integration suite against Folio and
report a pass rate per endpoint.

### Deliverables

- New crate `bench/` (workspace member, not part of release build).
- Subcommand `bench parity` runs Gotenberg's `.feature` files against
  a running Folio instance and emits a JSON + Markdown report.
- ~30 step definitions (Given / When / Then) ported from
  `gotenberg/test/integration/scenario/scenario.go` to Rust via the
  `cucumber` crate.
- `bench/parity/error-map.yaml` — translates Gotenberg plain-text
  error fragments to Folio's structured-JSON error codes.
- `bench/parity/scenario-skips.yaml` — explicit skip list with reason
  per scenario (e.g. `@debug` returns Gotenberg-specific JSON).
- `docker-compose.bench.yml` — Folio image built `FROM
  gotenberg/gotenberg:8.x` so Chromium / LibreOffice / qpdf are
  byte-identical between services.

### Out of scope for Shot 1

- AST extraction of form fields (covered indirectly by Gherkin).
- Bruno collection replay (redundant).
- CI gate (Shot 1 produces local-only report; CI wiring is Shot 2).

### Acceptance criteria

- [ ] `bench parity --tag chromium-convert-html` runs all 50
      scenarios against Folio, exits non-zero on regressions, prints
      pass/fail/skip table.
- [ ] First baseline committed to `bench/parity/baseline.json`.
- [ ] Report at `bench/results/<ts>/parity.md` lists each tag with
      pass-rate + failing scenario titles.

### Estimated effort: ~2.5 days

---

## Shot 2 — Performance benchmark harness

**Goal:** measure Folio vs Gotenberg head-to-head on identical
hardware with identical Chrome/LO/qpdf builds. Honest numbers, no
marketing.

### Deliverables

- Subcommand `bench perf` drives 5 workloads × 1 profile against
  both services in the same compose stack.
- Workloads (only these in v1):

  | ID | Endpoint | Payload |
  |----|----------|---------|
  | P1 | `/forms/chromium/convert/html` | 2 KB inline HTML |
  | P2 | `/forms/chromium/convert/html` | 200 KB HTML + web fonts |
  | P3 | `/forms/chromium/convert/url` | local Caddy fixture |
  | P4 | `/forms/libreoffice/convert` | 50 KB .docx |
  | P5 | `/forms/pdfengines/merge` | 5 × 20-page PDFs |

- One profile: 4 concurrent clients, 2 min/workload, 30 s warm-up
  discarded.
- Metrics: p50/p95/p99 latency, RPS, error rate, peak container RSS
  via `docker stats --no-stream` sampling. Three repetitions, report
  median, flag CV > 15 %.
- Load generator: hand-rolled `reqwest` + `tokio` driver in the
  `bench` crate (multipart support out-of-the-box, avoids `oha`
  multipart hacks).
- Report at `bench/results/<ts>/perf.md`: one latency bar chart
  (PNG via `plotters` or static HTML), one resource table, mandatory
  caveats section.

### Out of scope for Shot 2

- **CI gate / GitHub Actions wiring (deferred).** Local-run only for v1.
- 16-concurrent burst profile.
- PDF visual-diff / SSIM.
- GitHub Pages publishing.

### Acceptance criteria

- [ ] `bench perf` produces NDJSON + Markdown report with stable
      numbers across 3 reps.
- [ ] Both services run under identical `cpus: 2.0`, `mem_limit: 2g`.
- [ ] First Folio-vs-Gotenberg comparison committed at
      `bench/results/baseline-perf.json` and linked from README.

### Estimated effort: ~2.5 days (depends on Shot 1's compose stack)

---

## Shot 3 — Modernization (drop legacy hacks)

**Goal:** Folio is greenfield. Drop or invert Gotenberg's
backward-compat workarounds where Rust + modern OS primitives give us
a cleaner path. Use the perf harness from Shot 2 to confirm we
didn't regress.

### Items to address (pick before implementation)

| # | Gotenberg hack | Source | Folio direction |
|---|----------------|--------|-----------------|
| M1 | Restart Chrome after N requests (`maxReqLimit`) | `gotenberg/pkg/gotenberg/supervisor.go:424` | Drop the counter. Use per-request `BrowserContext` (incognito) via `chromiumoxide` + RAII tab cleanup. Idle-shutdown only. |
| M2 | Manual `kill -9` of `chrome/chromium` processes by `/proc` walk | `gotenberg/pkg/modules/chromium/browser.go:199-251` | Drop. Use `tokio::process::Command::kill_on_drop(true)` + `prctl(PR_SET_PDEATHSIG, SIGKILL)` on Linux. RAII handles cleanup. |
| M3 | String-grep CDP errors (`"Printing failed (-32000)"`) | `gotenberg/pkg/modules/chromium/browser.go:493-511` | Map CDP error codes to typed Rust enum once in `crates/engine/src/chromium/errors.rs`. Spec 44 already partially done. |
| M4 | Pinning proxy for DNS rebinding (10 KB Go) | `gotenberg/pkg/modules/chromium/pinning_proxy.go` | Replace with pre-resolve + `--host-resolver-rules=MAP host <ip>` flag passed to Chrome. Simpler. |
| M5 | Symlink hyphen-data into per-launch profile dir | `gotenberg/pkg/modules/chromium/browser.go:87-95` | Bake hyphen data at canonical path in Folio Docker image. One-line Dockerfile. |
| M6 | `no-sandbox`, `no-zygote`, `disable-dev-shm-usage` defaults-on | `gotenberg/pkg/modules/chromium/browser.go:100-113` | Invert: sandbox + zygote ON by default. Mount `/dev/shm` 512 MB in compose. Opt-in `--folio-unsafe-disable-sandbox` for constrained envs. |
| M7 | Always-write multipart parts to disk | `gotenberg/pkg/modules/api/form.go` pattern | Verify `crates/server/src/multipart.rs` streams to memory under threshold (8 MB), spills above. If not, fix. |
| M8 | 14 `--pdfengines-*-engines` flags + fallback chains | `gotenberg/pkg/modules/pdfengines/multi.go` | Don't reintroduce. Document lopdf-only choice; shell to qpdf only for AES-256. |
| M9 | `--chromium-max-queue-size` (custom queue) | `gotenberg/pkg/modules/chromium/chromium.go` | Use `tokio::sync::Semaphore` + `tower::limit::ConcurrencyLimit`. Reject with 503 on exhaustion. Single `--folio-max-concurrency` flag. |

### Triage for v1 (accepted)

In-scope for Shot 3: **M1, M2, M3, M6, M8**. M4, M5, M7, M9 are
explicit follow-ups (separate spec when scheduled).

### Deliverables (per accepted item)

- One PR per item, each titled `chore(modernize): drop <hack>` or
  `feat(engine): replace <hack>`.
- Each PR includes:
  - Before/after pointer to the Gotenberg source location.
  - Folio code change.
  - Perf-harness re-run showing no regression on P1–P5.
  - Updated `docs/implementation-status.md` line.

### Acceptance criteria

- [ ] Each chosen item has a merged PR.
- [ ] No regression on `bench perf` baseline (within noise budget).
- [ ] `README.md` "Why Folio vs Gotenberg" section updated with the
      modernizations as bullet points (honest, not marketing).

### Estimated effort: ~2 days for the M1/M2/M3/M6/M8 set

---

## Sequencing & dependencies

```
Shot 1 (parity)  ──┐
                   ├──>  Shot 3 (modernize, gated by Shot 2 baseline)
Shot 2 (perf)    ──┘
```

- Shot 1 and Shot 2 share `docker-compose.bench.yml` + the `bench/`
  crate skeleton; do Shot 1 first because it forces the compose
  stack to exist.
- Shot 2 must produce a baseline before Shot 3 starts so each
  modernization can be measured.
- Shot 3 is per-item; can be parallelised across PRs once Shot 2
  baseline is committed.

## Total estimated effort

~7 engineer-days end-to-end, splittable across 3 PR streams.

---

## Decisions (resolved)

| # | Question | Decision |
|---|----------|----------|
| 1 | Gotenberg pin | **Pinned to `fe1b0020` (current HEAD).** Re-pin via separate PR when upgrading. |
| 2 | Shot 3 item set | **M1, M2, M3, M6, M8.** Others deferred. |
| 3 | CI runner / perf gate | **Deferred.** No CI wiring in Shot 2; local-run only. Revisit after baseline is stable. |
| 4 | `bench/` crate visibility | **Workspace member.** Not published, not part of release build. |

---

## Per-shot implementation specs

When each shot starts, drop a tight implementation spec next to this
file:

- `64-shot1-parity-impl.md` (when Shot 1 starts)
- `65-shot2-perf-impl.md` (when Shot 2 starts)
- `66-shot3-modernize-impl.md` (when Shot 3 starts, one section per
  accepted M-item)

Spec 62 stays as historical context but is not the working plan
anymore.
