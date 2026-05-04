# LOK migration bench validation — 2026-05-04

**Quick smoke run** validating the LibreOfficeKit migration on `feat/libreofficekit`.
Not the full 60s × 3-rep methodology from the original plan — this is a single
fast pass to confirm "everything works and we're at least competitive."

## Setup

- pdfbro: built from `Dockerfile` target `pdfbro` on this branch (in-process LOK
  via `libreofficekit` crate, single dedicated worker thread, `Batch=1` for
  every document load).
- Gotenberg: `gotenberg/gotenberg:8` (default config).
- Both containers under `cpus: "2"`, `memory: 2g` (per `docker-compose.bench.yml`).
- Bench harness: `cargo run -p bench --release -- perf` with abbreviated
  timings: `--warmup-secs 5 --duration-secs 10 --repetitions 1 --concurrency 2
  --skip-preflight --skip html-small,html-large,url-local,pdfengines-merge`.
- Mode: accumulated (containers stay warm throughout).
- Raw report: `bench/results/2026-05-04-lok-quick/perf.md`.

## Latency

| Workload | Server | p50 (ms) | p95 (ms) | p99 (ms) | RPS | Errors |
|----------|--------|---------:|---------:|---------:|----:|-------:|
| libreoffice-docx | **pdfbro** | **27** | 52 | 97 | **63.8** | 0% |
| libreoffice-docx | gotenberg | 217 | 505 | 542 | 7.3 | 0% |

**Spec target** (informational, not a CI gate):
`pdfbro p50 ≤ Gotenberg p50 + 50 ms` → ≤ 267 ms. **pdfbro hit 27 ms — 240 ms under target.**

That's a ~8× p50 speedup and ~8.7× throughput improvement vs Gotenberg's stock
LibreOffice path on this hardware. The win comes from eliminating per-request
process spawn (Gotenberg restarts LibreOffice every 10 requests by default;
pdfbro keeps a single in-process `Office` instance via LOK).

## Memory (peak RSS, informational)

| Workload | pdfbro | Gotenberg | Winner |
|----------|-------:|----------:|--------|
| libreoffice-docx | 388 MiB | 114 MiB | Gotenberg (−70%) |

Caveat from the bench harness's own report: this comparison is **not
apples-to-apples**. Gotenberg recycles Chromium / LibreOffice subprocesses
every N requests, so a sampled RSS catches it between recycles when the
children are gone. pdfbro keeps both Chromium and LibreOffice warm in the
same container — the LO worker thread holds an `Office` instance for the
process lifetime by design (LOK enforces one `Office` per process, and we
`mem::forget` to bypass LO ≥ 6.5's atexit segfault).

For a fair "did the migration improve memory vs unoserver" comparison we'd
need to capture RSS against the **pre-migration unoserver build**, not
against Gotenberg. That's not in this run.

## Stability

CV warnings on both rows (pdfbro 46%, Gotenberg 40%) are expected with a
10-second × 1-repetition sample. The latency gap is large enough that the
ranking holds anyway, but the absolute numbers should not be cited as
production SLOs without re-running with at least the spec's default
60s × 3-rep methodology.

## Image identifiers

```
pdfbro    — local build of feat/libreofficekit @ f771cb9 (post-history-rewrite)
gotenberg — gotenberg/gotenberg:8
```

## Commentary

- **Wins:** Latency p50 down ~8×; throughput up ~8.7× vs Gotenberg on the
  same hardware envelope. Zero errors across both runs — the LOK
  integration is stable under load at the sampled rate.
- **Regressions vs target:** None. Spec target was informational and easily
  hit.
- **Open questions / follow-ups:**
  - Re-run with full methodology (`--warmup-secs 60 --duration-secs 120
    --repetitions 3`) before publishing perf claims externally.
  - Capture RSS against the unoserver baseline (the commit just before this
    branch's first LOK commit) to back the migration's memory motivation
    with numbers, not just ergonomics.
  - The bench harness's `--isolated` / `steady-state` override is the
    right tool for an apples-to-apples memory comparison vs Gotenberg.
