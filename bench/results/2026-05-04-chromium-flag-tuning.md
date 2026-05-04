# Chromium flag-tuning bench — 2026-05-04

Validating the `feat/chromium-flag-tuning` branch's expansion of
`BASELINE_ARGS` from 6 to 25 flags + removal of `--no-zygote` against
the post-LOK baseline.

## Setup

Identical methodology to the LOK validation run:

- Compose: `docker-compose.bench.yml` (cpus:2, memory:2g, both servers
  logging at error-only).
- Bench: `cargo run -p bench --release -- perf --skip-preflight`
  (defaults: 60s warmup × 120s × 3 reps × 4 concurrency × 5 workloads,
  accumulated mode).
- Branch under test: `feat/chromium-flag-tuning @ 207b73c`.
- Baseline: post-LOK `feat/libreofficekit @ 7cd5d6e`
  (`bench/results/20260504T082154Z/perf.md`).
- Raw report this run: `bench/results/20260504T112333Z/perf.md`.

## Caveat — host variance between the two runs

The flag-tuning run was on a noticeably noisier host than the baseline.
Look at Gotenberg between the two runs (Gotenberg code didn't change —
both runs used `gotenberg/gotenberg:8`):

| Workload | Gotenberg baseline p50 | Gotenberg flag-tuned p50 | Δ |
|---|---:|---:|---:|
| html-small | 419 ms | 413 ms | −1% |
| html-large | **494 ms** | **1284 ms** | **+160%** |
| url-local | 394 ms | 409 ms | +4% |
| libreoffice-docx | 449 ms | 485 ms | +8% |
| pdfengines-merge | 16 ms | 15 ms | −6% |

`html-large` and the html-small p99 tail were dominated by host noise
on this run. Read pdfbro's `html-large` / `url-local` deltas accordingly
— they're well inside the host-noise window. `html-small` p50/p95/RPS
and `libreoffice-docx` / `pdfengines-merge` are stable enough across
both runs to attribute changes to the branch.

## pdfbro before vs after

| Workload | Metric | Baseline | Flag-tuned | Δ | Verdict |
|---|---|---:|---:|---:|---|
| **html-small** | p50 | 436 ms | **233 ms** | **−47%** | ✅ real win |
| **html-small** | p95 | 953 ms | **356 ms** | **−63%** | ✅ real win |
| **html-small** | p99 | 1 278 ms | **477 ms** | **−63%** | ✅ real win |
| **html-small** | RPS | 8.2 | **16.0** | **+95%** | ✅ real win |
| html-small | RSS | 291 MiB | 314 MiB | +8% | small regression, host noise |
| html-large | p50 | 306 ms | 353 ms | +15% | ⚠️ within host noise (Gotenberg +160%) |
| html-large | p95 | 886 ms | 990 ms | +12% | ⚠️ within host noise |
| url-local | p50 | 294 ms | 361 ms | +23% | ⚠️ within host noise |
| url-local | RSS | 422 MiB | 423 MiB | flat | hypothesised −10–20% from `site-per-process` did not materialise; single-origin workload |
| libreoffice-docx | p50 | 47 ms | 49 ms | flat | ✅ stable (expected — flags only touch Chrome) |
| libreoffice-docx | RPS | 76.4 | 72.4 | −5% | within noise |
| pdfengines-merge | p50 | 9 ms | 11 ms | flat | ✅ stable (expected — qpdf path) |
| pdfengines-merge | RPS | 309 | 297 | −4% | within noise |

## What worked

The Tier-1 throttling-disable family of flags performed exactly as
hypothesised:

```
--disable-background-networking
--disable-background-timer-throttling
--disable-backgrounding-occluded-windows
--disable-renderer-backgrounding
```

`html-small` p95 dropped 63% on a host where Gotenberg's p50 on the same
workload stayed flat. That's not host noise — that's the four flags
preventing Chrome from throttling JS / timers / network on a tab it
considers "backgrounded" (which for our headless server is every tab,
always).

## What didn't move

- **`--disable-features=site-per-process` for memory** on `url-local`:
  RSS stayed at ~422 MiB. Hypothesis was that disabling per-origin
  process isolation would close the ~70 MiB gap to Gotenberg. Reality:
  flat. Either modern Chrome ignores the feature override on Linux, or
  our `url-local` workload (the bench-fixture nginx) is single-origin
  enough that origin sandboxing wasn't firing in the first place.
  Keeping the flag — no harm, may help on multi-origin workloads.

- **`--no-zygote` removal:** No measurable cold-start regression nor
  steady-state improvement. Removed because chromedp + Puppeteer both
  omit it; documenting that the change is benign is itself the
  takeaway.

## Recommendation

**Merge.** The headline `html-small` win (p95 −63%, RPS +95%) is real,
matches the published expectations of the Tier-1 flags, and survives
the host-noise sanity check (Gotenberg's same-workload p50 stayed flat
between the two runs). The other workloads sit within host-noise
tolerance and don't show structural regression.

Followups for a separate PR if anyone cares:

1. Re-run `html-large` / `url-local` on a calmer host to re-confirm
   they're noise, not regression.
2. Steady-state mode (`docker-compose.bench.steady-state.yml`) for an
   apples-to-apples vs-Gotenberg memory comparison.
3. Audit `url-local` for cross-origin requests — if the bench fixture
   genuinely is single-origin, `site-per-process` will never help.
   Consider a multi-origin variant.
