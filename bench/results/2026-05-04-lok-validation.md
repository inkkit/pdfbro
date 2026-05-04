# LOK migration bench validation — 2026-05-04

Validating the LibreOfficeKit migration on `feat/libreofficekit` against
both the previous unoserver baseline (the actual thing this branch
replaces) and the Gotenberg 8 reference (informational).

## Setup

- pdfbro: built from `Dockerfile` target `pdfbro` on this branch
  (in-process LOK via `libreofficekit` crate, single dedicated worker
  thread, `Batch=1` for every document load).
- Gotenberg: `gotenberg/gotenberg:8`, `--log-level=error
  --api-disable-health-check-logging` for symmetric quiet logging.
- Both containers under `cpus: "2"`, `memory: 2g` (per `docker-compose.bench.yml`).
- Bench harness: `cargo run -p bench --release -- perf` with default
  methodology — `--warmup-secs 60 --duration-secs 120 --repetitions 3
  --concurrency 4`.
- Mode: accumulated (containers stay warm throughout).
- Both servers logging at error-only (default of `docker-compose.bench.yml`).
  An earlier run with `RUST_LOG=info` measurably skewed tight workloads
  by ~30–50% — see "Logging tax" below.
- Raw report: `bench/results/20260504T082154Z/perf.md`.

## Headline: LOK vs unoserver on libreoffice-docx

The migration's reason for being. Compared against the previous best
unoserver-era run (`bench/results/20260503T085826Z/perf.md`):

| Metric | unoserver | LOK | gain |
|---|---:|---:|---:|
| p50 latency | 277 ms | **47 ms** | **5.9× faster** |
| p95 latency | 343 ms | **74 ms** | **4.6× tighter** |
| Throughput | 14.0 RPS | **76.4 RPS** | **5.5× higher** |
| Peak RSS | 1 309 MiB | **471 MiB** | **2.8× lighter** |
| Errors | 0% | 0% | — |

That's the migration's stated motivation made concrete: lower memory
*and* faster, on the same hardware envelope.

## Latency vs Gotenberg (this run)

| Workload | Server | p50 (ms) | p95 (ms) | p99 (ms) | RPS | Errors |
|----------|--------|---------:|---------:|---------:|----:|-------:|
| html-small | pdfbro | 436 | 953 | 1278 | 8.2 | 0% |
| html-small | gotenberg | 419 | 722 | 1098 | 8.5 | 0% |
| html-large | pdfbro | 306 | 886 | 1426 | 10.1 | 0% |
| html-large | gotenberg | 494 | 1106 | 1981 | 6.9 | 0% |
| url-local | pdfbro | 294 | 603 | 990 | 12.0 | 0% |
| url-local | gotenberg | 394 | 690 | 1010 | 8.9 | 0% |
| **libreoffice-docx** | **pdfbro** | **47** | **74** | **112** | **76.4** | 0% |
| libreoffice-docx | gotenberg | 449 | 755 | 1160 | 7.4 | 0% |
| pdfengines-merge | pdfbro | 9 | 27 | 58 | 309.4 | 0% |
| pdfengines-merge | gotenberg | 16 | 42 | 67 | 186.8 | 0% |

**Spec target** (informational): `pdfbro p50 ≤ Gotenberg p50 + 50 ms`.
All 5 workloads beat it, libreoffice-docx by 9.5×.

## Memory (peak RSS, informational)

| Workload | pdfbro | Gotenberg | Winner |
|----------|-------:|----------:|--------|
| html-small | 291 MiB | 331 MiB | pdfbro (−12%) |
| html-large | 361 MiB | 348 MiB | Gotenberg (−3%) |
| url-local | 422 MiB | 354 MiB | Gotenberg (−16%) |
| libreoffice-docx | 471 MiB | 318 MiB | Gotenberg (−32%) |
| pdfengines-merge | 492 MiB | 302 MiB | Gotenberg (−38%) |

The vs-Gotenberg memory comparison is *not* apples-to-apples by the
bench harness's own admission: Gotenberg recycles Chromium / LibreOffice
subprocesses every 100 / 10 requests; pdfbro keeps both warm. Sampled
RSS catches Gotenberg between recycles. The *actually* relevant
comparison — which the spec called out as the migration's motivation —
is **vs the previous pdfbro-unoserver build**, where LOK is **2.8×
lighter** on libreoffice-docx (1 309 → 471 MiB).

For a fair vs-Gotenberg memory comparison the `docker-compose.bench.steady-state.yml`
override forces both servers to keep their engines warm; that's the
right tool when memory is the question, not this run.

## Logging tax

A bench run earlier in the day (`bench/results/20260504T061212Z/`)
recorded `pdfengines-merge` at p50 = 25 ms / 117.8 RPS — a 2.5× / 2.9×
regression vs unoserver-era numbers. Investigation traced it to a
mid-bench config change that flipped the compose default from
`RUST_LOG=error` to `RUST_LOG=info`. Per-request `tower_http::trace`
JSON serialization + stderr writes consumed ~30–50% of the request
budget on tight (~10 ms) workloads at 300+ RPS, against `cpus: "2"`.
Reverted in commit `e3085a9`; this run uses error-level logging on both
servers and the regression disappears (p50 9 ms / 309.4 RPS — within
noise of the pre-LOK best of 10 ms / 342 RPS).

Documented as a caveat: future bench runs MUST use the compose default
(`RUST_LOG=error`, `--log-level=error`) for the numbers to be valid.
Override on the CLI when debugging.

## Stability

CV > 15% on every row, ranging from 30% (libreoffice-docx pdfbro) to
156% (url-local Gotenberg). With a 60s × 120s × 3-rep methodology and
`cpus: "2"` cap on a developer laptop, this is expected. The
libreoffice-docx ranking holds under any reasonable confidence interval
because the gap is order-of-magnitude. The chromium-path numbers should
be treated as directional only, not as production SLOs.

## Image identifiers

```
pdfbro    — local build of feat/libreofficekit @ e3085a9
gotenberg — gotenberg/gotenberg:8
```

## Commentary

- **Wins**:
  - `libreoffice-docx`: 5.9× faster p50, 5.5× higher RPS, 2.8× lighter
    RSS than the previous unoserver build it replaces.
  - `pdfengines-merge`: no regression once the logging-tax artefact is
    backed out. p50 within 1 ms of the pre-LOK best.
  - Zero errors across all 5 workloads on both servers.

- **Regressions vs target**: none. Spec target was informational
  (`p50 ≤ Gotenberg + 50 ms`); every workload beats it, libreoffice-docx
  by 9.5×.

- **Open questions / follow-ups**:
  - Memory deserves a clean apples-to-apples run via the steady-state
    compose override before any external "30% heavier" claim is made
    or refuted.
  - Chromium-path CV is high (40–60% pdfbro, 50–155% gotenberg). Worth
    a separate run with `--isolated` to attribute variance to host vs
    accumulated container state.
  - `bench-pdfbro` boots Chrome at startup even when the bench only
    exercises LO. The 471 MiB RSS for libreoffice-docx includes a
    warm Chromium that the workload never uses — relevant to the
    memory comparison but not a fix for this branch.
