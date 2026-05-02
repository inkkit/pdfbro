# Folio vs Gotenberg — Performance Report

Generated: 2026-05-02T06:25:39Z

## Latency (ms)

| Workload | Server | p50 | p95 | p99 | RPS | Errors |
|----------|--------|-----|-----|-----|-----|--------|
| html-small | folio | 195 | 250 | 310 | 19.9 | 0.0% |
| html-small | gotenberg | 302 | 489 | 662 | 12.3 | 0.0% |
| html-large | folio | 213 | 271 | 309 | 17.9 | 0.0% |
| html-large | gotenberg | 299 | 421 | 591 | 11.9 | 0.0% |
| url-local | folio | 290 | 628 | 895 | 12.3 | 0.0% |
| url-local | gotenberg | 478 | 941 | 1411 | 7.6 | 0.0% |
| libreoffice-docx | folio | 286 | 528 | 744 | 12.3 | 0.0% |
| libreoffice-docx | gotenberg | 688 | 1262 | 1875 | 5.1 | 0.0% |
| pdfengines-merge | folio | 14 | 35 | 88 | 213.9 | 0.0% |
| pdfengines-merge | gotenberg | 15 | 51 | 123 | 181.6 | 0.0% |

## Peak RSS (MiB)

| Workload | Folio | Gotenberg |
|----------|-------|-----------|
| html-small | 340 | 320 |
| html-large | 457 | 327 |
| url-local | 544 | 358 |
| libreoffice-docx | 1306 | 402 |
| pdfengines-merge | 1747 | 384 |

## Stability Warnings (CV > 15%)

- gotenberg/html-small: CV=25.6% (unstable)
- gotenberg/html-large: CV=22.7% (unstable)
- folio/url-local: CV=43.7% (unstable)
- gotenberg/url-local: CV=40.6% (unstable)
- folio/libreoffice-docx: CV=31.6% (unstable)
- gotenberg/libreoffice-docx: CV=38.7% (unstable)
- folio/pdfengines-merge: CV=107.2% (unstable)
- gotenberg/pdfengines-merge: CV=117.5% (unstable)

## Caveats

- Results are hardware-specific and not portable across machines.
- Both servers ran under `cpus: "2"` / `mem_limit: 2g` Docker cgroups.
- Chrome PDF rendering is non-deterministic; latency varies across runs.
- 60-second warm-up discarded before measurements.
