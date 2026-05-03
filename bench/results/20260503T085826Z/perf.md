# pdfbro vs Gotenberg — Performance Report

Generated: 2026-05-03T10:08:38Z  
Mode: `accumulated (containers warm throughout)`

## Latency (ms)

| Workload | Server | p50 | p95 | p99 | RPS | Errors |
|----------|--------|-----|-----|-----|-----|--------|
| html-small | pdfbro | 259 | 417 | 567 | 14.2 | 0.0% |
| html-small | gotenberg | 357 | 495 | 702 | 11.0 | 0.0% |
| html-large | pdfbro | 279 | 371 | 538 | 13.9 | 0.0% |
| html-large | gotenberg | 377 | 523 | 727 | 10.4 | 0.0% |
| url-local | pdfbro | 276 | 387 | 560 | 13.7 | 0.0% |
| url-local | gotenberg | 382 | 508 | 694 | 10.4 | 0.0% |
| libreoffice-docx | pdfbro | 277 | 343 | 418 | 14.0 | 0.0% |
| libreoffice-docx | gotenberg | 412 | 538 | 666 | 9.2 | 0.0% |
| pdfengines-merge | pdfbro | 10 | 16 | 26 | 342.0 | 0.0% |
| pdfengines-merge | gotenberg | 17 | 30 | 42 | 203.4 | 0.0% |

## Peak RSS (MiB)

> **How to read this table:**  
> RSS is sampled via `docker stats` — it measures the **entire container** (server process +
> Chrome + LibreOffice + all children), not just the operation under test.
>
> Mode **accumulated**: containers ran warm throughout. Later workloads show higher
> numbers because Chrome and LibreOffice from earlier workloads remain alive in the
> container. The merge row includes both Chrome and LibreOffice baseline RSS even
> though neither is used for a pure PDF merge.

| Workload | pdfbro | Gotenberg | Winner |
|----------|--------|-----------|--------|
| html-small | 327 | 397 | pdfbro (−17%) |
| html-large | 425 | 446 | pdfbro (−4%) |
| url-local | 501 | 491 | Gotenberg (−1%) |
| libreoffice-docx | 1309 | 442 | Gotenberg (−66%) |
| pdfengines-merge | 1850 | 425 | Gotenberg (−77%) |

## Stability Warnings (CV > 15%)

- pdfbro/html-small: CV=25.7% (unstable)
- gotenberg/html-small: CV=22.1% (unstable)
- pdfbro/html-large: CV=18.0% (unstable)
- gotenberg/html-large: CV=22.5% (unstable)
- pdfbro/url-local: CV=20.8% (unstable)
- gotenberg/url-local: CV=20.3% (unstable)
- pdfbro/pdfengines-merge: CV=44.8% (unstable)
- gotenberg/pdfengines-merge: CV=39.0% (unstable)

## Caveats

- Results are hardware-specific and not portable across machines.
- Both servers ran under `cpus: "2"` / `memory: 2g` Docker resource limits.
- Chrome PDF rendering is non-deterministic; latency varies across runs.
- 60-second warm-up discarded before measurements.
- RSS = `docker stats` memory — includes all child processes in the container.
  pdfbro keeps Chrome and LibreOffice alive (warm engines); Gotenberg may recycle
  them (default: restart Chrome every 100 requests, LibreOffice every 10 requests).
  Use `--isolated` mode and/or the `steady-state` compose override for fair
  apples-to-apples memory comparisons.
