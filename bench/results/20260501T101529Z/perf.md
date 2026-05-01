# Folio vs Gotenberg — Performance Report

Generated: 2026-05-01T10:39:49Z

## Latency (ms)

| Workload | Server | p50 | p95 | p99 | RPS | Errors |
|----------|--------|-----|-----|-----|-----|--------|
| html-small | folio | 880 | 1418 | 1992 | 4.2 | 0.0% |
| html-small | gotenberg | 298 | 493 | 792 | 12.4 | 0.0% |
| html-large | folio | 3073 | 13903 | 17935 | 0.9 | 0.0% |
| html-large | gotenberg | 376 | 663 | 935 | 9.7 | 0.0% |
| libreoffice-docx | folio | 19711 | 25791 | 25791 | 0.2 | 0.0% |
| libreoffice-docx | gotenberg | 665 | 1354 | 1560 | 5.4 | 0.0% |
| pdfengines-merge | folio | 25 | 38 | 52 | 152.6 | 0.0% |
| pdfengines-merge | gotenberg | 17 | 59 | 99 | 165.2 | 0.0% |

## Peak RSS (MiB)

| Workload | Folio | Gotenberg |
|----------|-------|-----------|
| html-small | N/A | 785 |
| html-large | N/A | 762 |
| libreoffice-docx | N/A | 837 |
| pdfengines-merge | N/A | 828 |

## Stability Warnings (CV > 15%)

- folio/html-small: CV=26.7% (unstable)
- gotenberg/html-small: CV=33.9% (unstable)
- folio/html-large: CV=84.2% (unstable)
- gotenberg/html-large: CV=33.1% (unstable)
- folio/libreoffice-docx: CV=28.8% (unstable)
- gotenberg/libreoffice-docx: CV=40.6% (unstable)
- folio/pdfengines-merge: CV=29.2% (unstable)
- gotenberg/pdfengines-merge: CV=82.9% (unstable)

## Caveats

- Results are hardware-specific and not portable across machines.
- Both servers ran under `cpus: "2"` / `mem_limit: 2g` Docker cgroups.
- Chrome PDF rendering is non-deterministic; latency varies across runs.
- 60-second warm-up discarded before measurements.
