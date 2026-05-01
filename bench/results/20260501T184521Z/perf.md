# Folio vs Gotenberg — Performance Report

Generated: 2026-05-01T19:55:31Z

## Latency (ms)

| Workload | Server | p50 | p95 | p99 | RPS | Errors |
|----------|--------|-----|-----|-----|-----|--------|
| html-small | folio | 195 | 230 | 258 | 20.2 | 0.0% |
| html-small | gotenberg | 297 | 402 | 526 | 13.2 | 0.0% |
| html-large | folio | 214 | 265 | 293 | 18.0 | 0.0% |
| html-large | gotenberg | 303 | 408 | 576 | 12.6 | 0.0% |
| url-local | folio | 210 | 257 | 291 | 18.4 | 0.0% |
| url-local | gotenberg | 302 | 404 | 590 | 12.7 | 0.0% |
| libreoffice-docx | folio | 254 | 282 | 326 | 15.5 | 0.0% |
| libreoffice-docx | gotenberg | 406 | 629 | 658 | 8.3 | 0.0% |
| pdfengines-merge | folio | 9 | 13 | 19 | 412.1 | 0.0% |
| pdfengines-merge | gotenberg | 13 | 25 | 34 | 259.7 | 0.0% |

## Peak RSS (MiB)

| Workload | Folio | Gotenberg |
|----------|-------|-----------|
| html-small | 415 | 436 |
| html-large | 530 | 454 |
| url-local | 641 | 472 |
| libreoffice-docx | 1538 | 471 |
| pdfengines-merge | 2041 | 446 |

## Stability Warnings (CV > 15%)

- gotenberg/html-small: CV=20.1% (unstable)
- gotenberg/html-large: CV=20.4% (unstable)
- gotenberg/url-local: CV=20.6% (unstable)
- gotenberg/libreoffice-docx: CV=22.2% (unstable)
- folio/pdfengines-merge: CV=30.3% (unstable)
- gotenberg/pdfengines-merge: CV=32.1% (unstable)

## Caveats

- Results are hardware-specific and not portable across machines.
- Both servers ran under `cpus: "2"` / `mem_limit: 2g` Docker cgroups.
- Chrome PDF rendering is non-deterministic; latency varies across runs.
- 60-second warm-up discarded before measurements.
