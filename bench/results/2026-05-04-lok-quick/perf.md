# pdfbro vs Gotenberg — Performance Report

Generated: 2026-05-04T06:09:09Z  
Mode: `accumulated (containers warm throughout)`

## Latency (ms)

| Workload | Server | p50 | p95 | p99 | RPS | Errors |
|----------|--------|-----|-----|-----|-----|--------|
| libreoffice-docx | pdfbro | 27 | 52 | 97 | 63.8 | 0.0% |
| libreoffice-docx | gotenberg | 217 | 505 | 542 | 7.3 | 0.0% |

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
| libreoffice-docx | 388 | 114 | Gotenberg (−70%) |

## Stability Warnings (CV > 15%)

- pdfbro/libreoffice-docx: CV=46.0% (unstable)
- gotenberg/libreoffice-docx: CV=39.7% (unstable)

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
