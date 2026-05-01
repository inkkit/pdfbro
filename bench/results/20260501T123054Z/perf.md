# Folio vs Gotenberg — Performance Report

Generated: 2026-05-01T13:07:01Z

## Latency (ms)

| Workload | Server | p50 | p95 | p99 | RPS | Errors |
|----------|--------|-----|-----|-----|-----|--------|
| html-small | folio | 186 | 210 | 256 | 21.3 | 0.0% |
| html-small | gotenberg | 283 | 398 | 517 | 14.3 | 0.0% |
| html-large | folio | 223 | 303 | 385 | 16.9 | 0.0% |
| html-large | gotenberg | 299 | 494 | 686 | 12.4 | 0.0% |
| url-local | folio | 205 | 261 | 298 | 18.6 | 0.0% |
| url-local | gotenberg | 289 | 396 | 591 | 13.7 | 0.0% |
| libreoffice-docx | folio | 1256 | 1471 | 1710 | 3.1 | 0.0% |
| libreoffice-docx | gotenberg | 528 | 859 | 1030 | 6.7 | 0.0% |
| pdfengines-merge | folio | 11 | 22 | 37 | 316.5 | 0.0% |
| pdfengines-merge | gotenberg | 25 | 47 | 61 | 139.1 | 0.0% |

## Peak RSS (MiB)

| Workload | Folio | Gotenberg |
|----------|-------|-----------|
| html-small | 488 | 312 |
| html-large | 542 | 324 |
| url-local | 573 | 331 |
| libreoffice-docx | 697 | 317 |
| pdfengines-merge | 550 | 298 |

## Stability Warnings (CV > 15%)

- gotenberg/html-small: CV=23.3% (unstable)
- folio/html-large: CV=15.9% (unstable)
- gotenberg/html-large: CV=29.0% (unstable)
- gotenberg/url-local: CV=23.6% (unstable)
- gotenberg/libreoffice-docx: CV=27.1% (unstable)
- folio/pdfengines-merge: CV=59.2% (unstable)
- gotenberg/pdfengines-merge: CV=37.1% (unstable)

## Caveats

- Results are hardware-specific and not portable across machines.
- Both servers ran under `cpus: "2"` / `mem_limit: 2g` Docker cgroups.
- Chrome PDF rendering is non-deterministic; latency varies across runs.
- 60-second warm-up discarded before measurements.
