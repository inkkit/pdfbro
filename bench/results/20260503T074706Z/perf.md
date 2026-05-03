# pdfbro vs Gotenberg — Performance Report

Generated: 2026-05-03T08:57:35Z  
Mode: `isolated (container restarted before each workload)`

## Latency (ms)

| Workload | Server | p50 | p95 | p99 | RPS | Errors |
|----------|--------|-----|-----|-----|-----|--------|
| html-small | pdfbro | 251 | 425 | 589 | 14.2 | 0.0% |
| html-small | gotenberg | 413 | 740 | 1116 | 8.7 | 0.0% |
| html-large | pdfbro | 302 | 508 | 817 | 11.9 | 0.0% |
| html-large | gotenberg | 406 | 694 | 1086 | 9.0 | 0.0% |
| url-local | pdfbro | 303 | 455 | 674 | 12.2 | 0.0% |
| url-local | gotenberg | 417 | 726 | 1196 | 8.6 | 0.0% |
| libreoffice-docx | pdfbro | 306 | 557 | 1703 | 10.9 | 0.0% |
| libreoffice-docx | gotenberg | 492 | 812 | 967 | 7.1 | 0.0% |
| pdfengines-merge | pdfbro | 11 | 17 | 25 | 328.1 | 0.0% |
| pdfengines-merge | gotenberg | 18 | 35 | 48 | 192.1 | 0.0% |

## Peak RSS (MiB)

> **How to read this table:**  
> RSS is sampled via `docker stats` — it measures the **entire container** (server process +
> Chrome + LibreOffice + all children), not just the operation under test.
>
> Mode **isolated**: containers were restarted before each workload, so each row
> reflects that engine's steady-state memory for that workload only — no carryover
> from previous operations.

| Workload | pdfbro | Gotenberg | Winner |
|----------|--------|-----------|--------|
| html-small | 313 | 329 | pdfbro (−4%) |
| html-large | 325 | 321 | Gotenberg (−1%) |
| url-local | 307 | 331 | pdfbro (−7%) |
| libreoffice-docx | 810 | 129 | Gotenberg (−84%) |
| pdfengines-merge | 244 | 23 | Gotenberg (−90%) |

## Stability Warnings (CV > 15%)

- pdfbro/html-small: CV=31.6% (unstable)
- gotenberg/html-small: CV=34.0% (unstable)
- pdfbro/html-large: CV=39.1% (unstable)
- gotenberg/html-large: CV=33.7% (unstable)
- pdfbro/url-local: CV=25.9% (unstable)
- gotenberg/url-local: CV=35.0% (unstable)
- pdfbro/libreoffice-docx: CV=63.2% (unstable)
- gotenberg/libreoffice-docx: CV=26.6% (unstable)
- pdfbro/pdfengines-merge: CV=36.7% (unstable)
- gotenberg/pdfengines-merge: CV=39.9% (unstable)

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
