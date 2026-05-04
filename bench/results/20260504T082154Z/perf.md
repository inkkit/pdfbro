# pdfbro vs Gotenberg — Performance Report

Generated: 2026-05-04T09:32:20Z  
Mode: `accumulated (containers warm throughout)`

## Latency (ms)

| Workload | Server | p50 | p95 | p99 | RPS | Errors |
|----------|--------|-----|-----|-----|-----|--------|
| html-small | pdfbro | 436 | 953 | 1278 | 8.2 | 0.0% |
| html-small | gotenberg | 419 | 722 | 1098 | 8.5 | 0.0% |
| html-large | pdfbro | 306 | 886 | 1426 | 10.1 | 0.0% |
| html-large | gotenberg | 494 | 1106 | 1981 | 6.9 | 0.0% |
| url-local | pdfbro | 294 | 603 | 990 | 12.0 | 0.0% |
| url-local | gotenberg | 394 | 690 | 1010 | 8.9 | 0.0% |
| libreoffice-docx | pdfbro | 47 | 74 | 112 | 76.4 | 0.0% |
| libreoffice-docx | gotenberg | 449 | 755 | 1160 | 7.4 | 0.0% |
| pdfengines-merge | pdfbro | 9 | 27 | 58 | 309.4 | 0.0% |
| pdfengines-merge | gotenberg | 16 | 42 | 67 | 186.8 | 0.0% |

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
| html-small | 291 | 331 | pdfbro (−12%) |
| html-large | 361 | 348 | Gotenberg (−3%) |
| url-local | 422 | 354 | Gotenberg (−16%) |
| libreoffice-docx | 471 | 318 | Gotenberg (−32%) |
| pdfengines-merge | 492 | 302 | Gotenberg (−38%) |

## Stability Warnings (CV > 15%)

- pdfbro/html-small: CV=47.5% (unstable)
- gotenberg/html-small: CV=39.9% (unstable)
- pdfbro/html-large: CV=61.1% (unstable)
- gotenberg/html-large: CV=52.1% (unstable)
- pdfbro/url-local: CV=46.0% (unstable)
- gotenberg/url-local: CV=155.6% (unstable)
- pdfbro/libreoffice-docx: CV=29.8% (unstable)
- gotenberg/libreoffice-docx: CV=35.7% (unstable)
- pdfbro/pdfengines-merge: CV=147.2% (unstable)
- gotenberg/pdfengines-merge: CV=77.4% (unstable)

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
