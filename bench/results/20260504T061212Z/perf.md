# pdfbro vs Gotenberg — Performance Report

Generated: 2026-05-04T07:13:37Z  
Mode: `accumulated (containers warm throughout)`

## Latency (ms)

| Workload | Server | p50 | p95 | p99 | RPS | Errors |
|----------|--------|-----|-----|-----|-----|--------|
| html-small | pdfbro | 233 | 629 | 1523 | 12.5 | 0.0% |
| html-small | gotenberg | 387 | 686 | 1011 | 9.6 | 0.0% |
| html-large | pdfbro | 236 | 338 | 441 | 18.8 | 15.9% |
| html-large | gotenberg | 482 | 914 | 1475 | 7.6 | 0.0% |
| url-local | pdfbro | 231 | 409 | 931 | 14.9 | 0.0% |
| url-local | gotenberg | 384 | 817 | 1238 | 9.4 | 0.0% |
| libreoffice-docx | pdfbro | 54 | 158 | 411 | 50.7 | 0.0% |
| libreoffice-docx | gotenberg | 822 | 6175 | 11095 | 2.2 | 0.0% |
| pdfengines-merge | pdfbro | 25 | 74 | 171 | 117.8 | 0.0% |
| pdfengines-merge | gotenberg | 16 | 47 | 90 | 177.6 | 0.0% |

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
| html-small | 499 | 393 | Gotenberg (−21%) |
| html-large | 532 | 420 | Gotenberg (−21%) |
| url-local | 384 | 441 | pdfbro (−12%) |
| libreoffice-docx | 436 | 327 | Gotenberg (−25%) |
| pdfengines-merge | 445 | 306 | Gotenberg (−31%) |

## Stability Warnings (CV > 15%)

- pdfbro/html-small: CV=84.4% (unstable)
- gotenberg/html-small: CV=36.0% (unstable)
- pdfbro/html-large: CV=33.5% (unstable)
- gotenberg/html-large: CV=49.7% (unstable)
- pdfbro/url-local: CV=43.9% (unstable)
- gotenberg/url-local: CV=47.0% (unstable)
- pdfbro/libreoffice-docx: CV=141.6% (unstable)
- gotenberg/libreoffice-docx: CV=126.5% (unstable)
- pdfbro/pdfengines-merge: CV=138.7% (unstable)
- gotenberg/pdfengines-merge: CV=98.6% (unstable)

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
