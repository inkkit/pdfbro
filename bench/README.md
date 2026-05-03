# pdfbro Benchmark Suite

HTTP load-testing tool that measures throughput, latency, and peak memory for pdfbro and Gotenberg side-by-side.

## Quick start

```bash
# 1. Start containers
docker compose -f docker-compose.bench.yml up -d

# 2. Run benchmark (default accumulated mode)
cargo run -p bench -- perf
```

Report lands in `bench/results/<timestamp>/perf.md`.

---

## Benchmark modes

The benchmark supports three distinct modes. Each answers a different question.

### Mode 1 — Accumulated (default)

```bash
docker compose -f docker-compose.bench.yml up -d
cargo run -p bench -- perf
```

Both containers run warm for the entire benchmark. Workloads execute sequentially without restarting anything between them.

**What it measures:** steady-state throughput and latency for a server that has been running for a while.

**Memory caveat:** RSS numbers accumulate. By the time the `pdfengines-merge` workload runs, the container holds Chrome (started for the HTML workloads) and LibreOffice (started for the DOCX workload) simultaneously — even though a PDF merge needs neither. The merge row's RSS is therefore the combined baseline of all three processes, not the merge operation's own footprint.

**Gotenberg behaviour in this mode:** uses its defaults — lazy Chrome start (first request), restart Chrome every 100 requests, restart LibreOffice every 10 requests. Memory for later workloads may be lower than pdfbro's because Gotenberg may have recycled its engines mid-run.

---

### Mode 2 — Isolated (per-workload container restart)

```bash
docker compose -f docker-compose.bench.yml up -d
cargo run -p bench -- perf --isolated
```

Before each workload the bench runner calls `docker restart` on both containers and waits for `/health` to respond (up to 120 s). Each workload therefore starts from a completely fresh container with no accumulated engine state.

**What it measures:** the memory footprint of each individual workload in isolation. The merge row now shows only what a pure PDF merge actually costs, without Chrome or LibreOffice residue.

**Latency note:** because containers are cold at the start of each workload, the 60-second warm-up phase (already baked in) lets engines reach steady state before measurements begin.

---

### Mode 3 — Steady-state (both servers on equal footing)

```bash
docker compose \
  -f docker-compose.bench.yml \
  -f docker-compose.bench.steady-state.yml \
  up -d

cargo run -p bench -- perf
```

The compose override (`docker-compose.bench.steady-state.yml`) reconfigures both servers to the same lifecycle model:

| Setting | pdfbro | Gotenberg (override) |
|---------|--------|---------------------|
| Chrome startup | eager (at boot) | `--chromium-auto-start=true` |
| Chrome recycling | never | `--chromium-restart-after=0` |
| LibreOffice startup | eager (at boot) | `--libreoffice-auto-start=true` |
| LibreOffice recycling | never | `--libreoffice-restart-after=0` |

**What it measures:** apples-to-apples comparison of two always-warm, never-recycling servers — the model that matches a real always-on production deployment.

**Memory note:** RSS numbers will be higher for both servers than Mode 1 Gotenberg numbers, because Gotenberg's normal recycling is disabled. This reveals pdfbro's true steady-state baseline.

---

### Mode 4 — Steady-state + Isolated (most rigorous)

```bash
docker compose \
  -f docker-compose.bench.yml \
  -f docker-compose.bench.steady-state.yml \
  up -d

cargo run -p bench -- perf --isolated
```

Combines Modes 2 and 3: both servers configured identically (no recycling), and containers restarted between workloads so each measurement is clean. This is the most controlled comparison.

---

## Why the RSS numbers look the way they do

`docker stats` measures the entire container's RSS — the Rust server process plus every child process (Chrome, LibreOffice, unoserver). It cannot isolate individual operation costs.

| Scenario | What inflates RSS |
|----------|------------------|
| Accumulated mode, late workloads | All engines from earlier workloads are still alive |
| Gotenberg in default mode | Chrome/LibreOffice recycled periodically → lower peak for later workloads |
| pdfbro, always | Both engines kept warm → consistent baseline from the first Chrome workload onward |

**Rule of thumb:** for a fair memory comparison, always use `--isolated` mode. For a fair production-parity comparison, also use the `steady-state` compose override.

---

## CLI reference

```
cargo run -p bench -- perf [OPTIONS]

Options:
  --pdfbro-url <URL>           [default: http://localhost:3001]
  --gotenberg-url <URL>        [default: http://localhost:3002]
  --pdfbro-container <NAME>    [default: bench-pdfbro]
  --gotenberg-container <NAME> [default: bench-gotenberg]
  --concurrency <N>            Parallel clients per workload [default: 4]
  --warmup-secs <N>            Warm-up phase duration [default: 60]
  --duration-secs <N>          Timed run duration per repetition [default: 120]
  --repetitions <N>            Number of timed repetitions [default: 3]
  --skip <LIST>                Comma-separated workload names to skip
  --skip-preflight             Skip Chrome version check
  --isolated                   Restart containers before each workload
  --output-dir <PATH>          Custom report output directory
```

---

## Workloads

| Name | Route | What it tests |
|------|-------|---------------|
| `html-small` | `/forms/chromium/convert/html` | Minimal HTML, no external assets |
| `html-large` | `/forms/chromium/convert/html` | HTML with web fonts and a data table |
| `url-local` | `/forms/chromium/convert/url` | Local nginx fixture URL (no real network) |
| `libreoffice-docx` | `/forms/libreoffice/convert` | 50 KB DOCX → PDF |
| `pdfengines-merge` | `/forms/pdfengines/merge` | 5 × 20-page PDFs merged |

---

## Understanding stability warnings (CV)

The report flags workloads where **CV (Coefficient of Variation) > 15%**:

```
CV = standard deviation / mean × 100
```

| CV range | Interpretation |
|----------|---------------|
| < 5% | Very stable — results reliable |
| 5–15% | Acceptable — minor system noise |
| 15–40% | Unstable — treat as indicative only |
| > 40% | Unreliable — noise dominates signal |

High CV is common for URL and LibreOffice workloads because Chrome's renderer and LibreOffice's JVM have non-deterministic warm-up behaviour. To reduce CV: increase `--duration-secs`, ensure no other workloads compete for CPU, and run on a quiet machine.

---

## Comparing results across runs

Results in `bench/results/` are timestamped directories, each containing `perf.md`. The report header includes the bench mode so you know exactly what configuration produced each result.

When comparing two result files:
- Match mode labels (accumulated vs isolated vs steady-state)
- Check Chrome versions match (preflight enforces this, but `--skip-preflight` bypasses it)
- Hardware differences make absolute numbers incomparable; ratios between pdfbro and Gotenberg in the same run are meaningful
