# pdfbro vs Gotenberg — Performance Report

Generated: 2026-05-04T12:34:28Z  
Mode: `accumulated (containers warm throughout)`

## Latency (ms)

| Workload | Server | p50 | p95 | p99 | RPS | Errors |
|----------|--------|-----|-----|-----|-----|--------|
| html-small | pdfbro | 233 | 356 | 477 | 16.0 | 0.0% |
| html-small | gotenberg | 413 | 1934 | 6995 | 5.4 | 0.0% |
| html-large | pdfbro | 353 | 990 | 3445 | 8.0 | 0.0% |
| html-large | gotenberg | 1284 | 6647 | 9959 | 1.9 | 0.0% |
| url-local | pdfbro | 361 | 877 | 1364 | 9.0 | 0.0% |
| url-local | gotenberg | 409 | 815 | 1275 | 8.5 | 0.0% |
| libreoffice-docx | pdfbro | 49 | 85 | 122 | 72.4 | 0.0% |
| libreoffice-docx | gotenberg | 485 | 826 | 1052 | 7.1 | 0.0% |
| pdfengines-merge | pdfbro | 11 | 23 | 39 | 296.7 | 0.0% |
| pdfengines-merge | gotenberg | 15 | 38 | 53 | 207.2 | 0.0% |

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
| html-small | 314 | 323 | pdfbro (−2%) |
| html-large | 375 | 349 | Gotenberg (−6%) |
| url-local | 423 | 333 | Gotenberg (−21%) |
| libreoffice-docx | 474 | 310 | Gotenberg (−34%) |
| pdfengines-merge | 497 | 302 | Gotenberg (−39%) |

## Stability Warnings (CV > 15%)

- pdfbro/html-small: CV=23.8% (unstable)
- gotenberg/html-small: CV=209.0% (unstable)
- pdfbro/html-large: CV=136.5% (unstable)
- gotenberg/html-large: CV=114.1% (unstable)
- pdfbro/url-local: CV=54.5% (unstable)
- gotenberg/url-local: CV=44.4% (unstable)
- pdfbro/libreoffice-docx: CV=33.6% (unstable)
- gotenberg/libreoffice-docx: CV=29.8% (unstable)
- pdfbro/pdfengines-merge: CV=62.8% (unstable)
- gotenberg/pdfengines-merge: CV=61.6% (unstable)

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
