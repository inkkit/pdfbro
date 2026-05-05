# Console Redesign Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Redesign the `/_/` operator console to the v4 layout, fix data accuracy bugs (per-route RPS/in-flight/load), add engine/queue visibility metrics, wire up the Batches card, and remove the ActivityStrip.

**Architecture:** Backend changes flow through `console_store.rs` — extend `MetricsSample`, all `*Payload` structs, and `build_console_payload`. Frontend consumes the updated SSE payload in Svelte 5 runes components. The layout moves from a vertical strip model to a 2×2 chart grid in the left column with Routes scrollable below.

**Tech Stack:** Rust + Axum (backend), Svelte 5 (runes), TypeScript, inline CSS styling (no Tailwind), Prometheus metrics via `prometheus` crate, `tokio` async runtime.

---

## File Map

**Backend (modify):**
- `crates/server/src/console_store.rs` — all payload structs, MetricsSample, ConsoleStore fields, build functions
- `crates/server/src/supervised_engine.rs` — add `idle_secs()` accessor
- `crates/server/src/app.rs` — per-route in-flight tracking in middleware

**Frontend (modify):**
- `ui/src/lib/types.ts` — update all interfaces
- `ui/src/lib/components/Ticker.svelte` — remove chromium/LO, add P50/P55
- `ui/src/lib/components/RoutesTable.svelte` — scrollable, no fixed height
- `ui/src/lib/components/ThroughputStrip.svelte` — accept ts for tooltip
- `ui/src/lib/components/side-rail/Engines.svelte` — per-engine sub-cards
- `ui/src/lib/components/side-rail/Concurrency.svelte` — add queue stats row
- `ui/src/lib/components/side-rail/Batches.svelte` — wire real batch data
- `ui/src/routes/+page.svelte` — v4 layout, remove ActivityStrip

**Frontend (create):**
- `ui/src/lib/components/StackedBarSeries.svelte` — two-series stacked SVG bar chart
- `ui/src/lib/components/EngineConvChart.svelte` — engine conversions chart card
- `ui/src/lib/components/QueueWaitChart.svelte` — queue wait p95 chart card

**Frontend (delete):**
- `ui/src/lib/components/ActivityStrip.svelte`

---

## Task 1: Create feature branch

- [ ] **Create and checkout branch**

```bash
git checkout -b feat/console-redesign
```

- [ ] **Verify clean state**

```bash
git status
```
Expected: `On branch feat/console-redesign, nothing to commit`

---

## Task 2: Backend — Add per-route in-flight tracking to ConsoleStore

`crates/server/src/console_store.rs` — add a per-route counter map so the middleware can track in-flight requests per path.

- [ ] **Add the field to ConsoleStore**

In `console_store.rs`, add `use std::collections::HashMap;` if not present (it isn't — add it), then add field to `ConsoleStore` struct after `active_requests`:

```rust
/// Per-route count of HTTP requests currently in flight.
pub active_per_route: tokio::sync::Mutex<HashMap<String, u32>>,
```

- [ ] **Initialise in ConsoleStore::new()**

Add to the `Self { ... }` block in `new()`:

```rust
active_per_route: tokio::sync::Mutex::new(HashMap::new()),
```

- [ ] **Wire increment/decrement into the middleware in `app.rs`**

In `app.rs`, find the middleware function (around line 378). Replace the current in-flight block:

```rust
state.console.active_requests.fetch_add(1, Ordering::SeqCst);
let start = Instant::now();
let response = next.run(req).await;
state.console.active_requests.fetch_sub(1, Ordering::SeqCst);
```

with:

```rust
state.console.active_requests.fetch_add(1, Ordering::SeqCst);
{
    let mut map = state.console.active_per_route.lock().await;
    *map.entry(path.clone()).or_insert(0) += 1;
}
let start = Instant::now();
let response = next.run(req).await;
state.console.active_requests.fetch_sub(1, Ordering::SeqCst);
{
    let mut map = state.console.active_per_route.lock().await;
    if let Some(c) = map.get_mut(&path) {
        *c = c.saturating_sub(1);
    }
}
```

- [ ] **Build to confirm no errors**

```bash
cargo build -p server 2>&1 | tail -5
```
Expected: `Finished` with no errors.

- [ ] **Commit**

```bash
git add crates/server/src/console_store.rs crates/server/src/app.rs
git commit -m "feat(console): add per-route in-flight tracking to ConsoleStore"
```

---

## Task 3: Backend — Add idle_secs() to supervised engines

`crates/server/src/supervised_engine.rs` — expose how long each engine has been idle.

- [ ] **Add idle_secs to SupervisedChromiumEngine**

After the existing `pub fn is_running(&self) -> bool` method (around line 219), add:

```rust
/// Seconds since this engine last handled a request. Returns 0 if never used.
pub fn idle_secs(&self) -> u64 {
    let last = self.inner.last_activity.load(std::sync::atomic::Ordering::SeqCst);
    if last == 0 { return 0; }
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    now.saturating_sub(last)
}
```

- [ ] **Add idle_secs to SupervisedLibreOfficeEngine**

After the existing `pub fn is_running(&self) -> bool` on the LibreOffice engine (around line 358), add the identical method:

```rust
pub fn idle_secs(&self) -> u64 {
    let last = self.inner.last_activity.load(std::sync::atomic::Ordering::SeqCst);
    if last == 0 { return 0; }
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    now.saturating_sub(last)
}
```

- [ ] **Build to confirm**

```bash
cargo build -p server 2>&1 | tail -5
```
Expected: `Finished` with no errors.

- [ ] **Commit**

```bash
git add crates/server/src/supervised_engine.rs
git commit -m "feat(console): expose idle_secs() on supervised engines"
```

---

## Task 4: Backend — Extend MetricsSample with new fields

`crates/server/src/console_store.rs` — add p50_ms, p55_ms, per-engine conv rates, queue wait p95.

- [ ] **Extend MetricsSample struct**

Replace the existing `MetricsSample` struct definition with:

```rust
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MetricsSample {
    pub ts: u64,
    pub rps: f64,
    pub p50_ms: f64,
    pub p55_ms: f64,
    pub p95_ms: f64,
    pub error_pct: f64,
    pub queue_size: u32,
    pub concurrency_active: u32,
    pub cpu_pct: f64,
    pub memory_mb: f64,
    pub chromium_conv_rps: f64,
    pub libreoffice_conv_rps: f64,
    pub queue_wait_p95_ms: f64,
}
```

- [ ] **Add prev_conv_totals to ConsoleStore**

Add two more fields to `ConsoleStore` struct after `prev_error_total`:

```rust
/// Previous Chromium conversion total for per-engine RPS delta.
pub prev_chromium_conv_total: tokio::sync::Mutex<f64>,
/// Previous LibreOffice conversion total for per-engine RPS delta.
pub prev_libreoffice_conv_total: tokio::sync::Mutex<f64>,
/// Previous per-route HTTP totals for per-route RPS delta.
pub prev_route_totals: tokio::sync::Mutex<HashMap<String, f64>>,
```

- [ ] **Initialise in ConsoleStore::new()**

Add to the `Self { ... }` block:

```rust
prev_chromium_conv_total: tokio::sync::Mutex::new(0.0),
prev_libreoffice_conv_total: tokio::sync::Mutex::new(0.0),
prev_route_totals: tokio::sync::Mutex::new(HashMap::new()),
```

- [ ] **Build**

```bash
cargo build -p server 2>&1 | grep -E "^error" | head -10
```
Expected: no `error` lines (warnings OK).

- [ ] **Commit**

```bash
git add crates/server/src/console_store.rs
git commit -m "feat(console): extend MetricsSample with p50/p55/conv_rps/queue_wait fields"
```

---

## Task 5: Backend — Extend all *Payload structs

`crates/server/src/console_store.rs` — update the wire payloads sent to the UI.

- [ ] **Update TickerPayload** — remove chromium/libreoffice fields, add p50_ms/p55_ms:

Replace the `TickerPayload` struct with:

```rust
#[derive(Clone, Debug, Serialize)]
pub struct TickerPayload {
    pub rps: f64,
    pub p50_ms: f64,
    pub p55_ms: f64,
    pub p95_ms: f64,
    pub error_pct: f64,
    pub concurrency_active: u32,
    pub concurrency_max: u32,
    pub queue_size: f64,
    pub uptime_seconds: u64,
}
```

- [ ] **Update EnginePayload** — add conv stats, bytes, idle:

Replace the `EnginePayload` struct with:

```rust
#[derive(Clone, Debug, Serialize)]
pub struct EnginePayload {
    pub name: String,
    pub status: String,
    pub restarts: u32,
    pub mode: String,
    pub mini_series: Vec<f64>,
    pub conversions_total: u64,
    pub error_rate: f64,
    pub bytes_mb: f64,
    pub idle_secs: u64,
}
```

- [ ] **Update ConcurrencyPayload** — add queue stats:

Replace the `ConcurrencyPayload` struct with:

```rust
#[derive(Clone, Debug, Serialize)]
pub struct ConcurrencyPayload {
    pub active: u32,
    pub max: u32,
    pub warn_threshold: u32,
    pub crit_threshold: u32,
    pub queue_wait_p95_ms: f64,
    pub queue_processing: u32,
}
```

- [ ] **Update ThroughputPayload** — add timestamps and new series:

Replace the `ThroughputPayload` struct with:

```rust
#[derive(Clone, Debug, Serialize)]
pub struct ThroughputPayload {
    pub ts_series: Vec<u64>,
    pub rps_series: Vec<f64>,
    pub rps_baseline: f64,
    pub p95_series: Vec<f64>,
    pub p95_target_s: f64,
    pub chromium_conv_series: Vec<f64>,
    pub libreoffice_conv_series: Vec<f64>,
    pub queue_wait_p95_series: Vec<f64>,
}
```

- [ ] **Update BatchPayload** — add item counts and output mode:

Replace the `BatchPayload` struct with:

```rust
#[derive(Clone, Debug, Serialize)]
pub struct BatchPayload {
    pub id: String,
    pub status: String,
    pub progress_pct: u8,
    pub elapsed: String,
    pub total_items: usize,
    pub completed_items: usize,
    pub failed_items: usize,
    pub output_mode: String,
}
```

- [ ] **Build (expect errors until build_console_payload is fixed in Task 7)**

```bash
cargo build -p server 2>&1 | grep "^error" | head -20
```
Expected: errors about missing fields in `build_console_payload` — that's fine for now.

- [ ] **Commit**

```bash
git add crates/server/src/console_store.rs
git commit -m "feat(console): update all payload structs with new fields"
```

---

## Task 6: Backend — Fix build_route_payloads (per-route RPS, in-flight, load)

`crates/server/src/console_store.rs` — fix the three data bugs in `build_route_payloads`.

- [ ] **Replace `build_route_payloads` with fixed version**

The function signature must become `async` (it needs to await the route totals lock). Replace `fn build_route_payloads(state: &crate::state::AppState, concurrency_max: u32) -> Vec<RoutePayload>` with:

```rust
async fn build_route_payloads(state: &crate::state::AppState, concurrency_max: u32) -> Vec<RoutePayload> {
    let families = prometheus::gather();

    // Count + error totals per route from pdfbro_http_requests_total
    let mut route_counts: std::collections::HashMap<String, (f64, f64)> = std::collections::HashMap::new();
    for family in &families {
        if family.get_name() != "pdfbro_http_requests_total" { continue; }
        for m in family.get_metric() {
            let labels: std::collections::HashMap<_, _> = m.get_label().iter()
                .map(|l| (l.get_name(), l.get_value()))
                .collect();
            let route = labels.get("route").copied().unwrap_or("unknown").to_string();
            let status = labels.get("status").copied().unwrap_or("0");
            let count = m.get_counter().get_value();
            let entry = route_counts.entry(route).or_insert((0.0, 0.0));
            entry.0 += count;
            if status.starts_with('5') || status.starts_with('4') {
                entry.1 += count;
            }
        }
        break;
    }

    // Latency percentiles per route from pdfbro_http_request_duration_seconds
    let mut route_latency: std::collections::HashMap<String, (f64, f64, f64)> = std::collections::HashMap::new();
    for family in &families {
        if family.get_name() != "pdfbro_http_request_duration_seconds" { continue; }
        for m in family.get_metric() {
            let labels: std::collections::HashMap<_, _> = m.get_label().iter()
                .map(|l| (l.get_name(), l.get_value()))
                .collect();
            let route = labels.get("route").copied().unwrap_or("unknown").to_string();
            let hist = m.get_histogram();
            let count = hist.get_sample_count();
            if count == 0 { continue; }
            let buckets = hist.get_bucket();
            let p50 = percentile_from_histogram(buckets, count, 0.50) * 1000.0;
            let p95 = percentile_from_histogram(buckets, count, 0.95) * 1000.0;
            let p99 = percentile_from_histogram(buckets, count, 0.99) * 1000.0;
            route_latency.insert(route, (p50, p95, p99));
        }
        break;
    }

    // Per-route RPS: compute delta from previous totals
    let route_rps: std::collections::HashMap<String, f64> = {
        let mut prev = state.console.prev_route_totals.lock().await;
        route_counts.iter().map(|(route, (total, _))| {
            let prev_total = prev.get(route).copied().unwrap_or(0.0);
            let delta = (total - prev_total).max(0.0);
            prev.insert(route.clone(), *total);
            (route.clone(), delta / 5.0)
        }).collect()
    };

    // Per-route in-flight from active_per_route map
    let in_flight_map: std::collections::HashMap<String, u32> = {
        let map = state.console.active_per_route.lock().await;
        map.clone()
    };

    let mut routes: Vec<RoutePayload> = route_counts.into_iter().map(|(path, (total, errors))| {
        let error_pct = if total > 0.0 { (errors / total) * 100.0 } else { 0.0 };
        let (p50_ms, p95_ms, p99_ms) = route_latency.get(&path).copied().unwrap_or((0.0, 0.0, 0.0));
        let rps = route_rps.get(&path).copied().unwrap_or(0.0);
        let in_flight = in_flight_map.get(&path).copied().unwrap_or(0);
        let load_pct = (in_flight as f64 / concurrency_max.max(1) as f64) * 100.0;
        RoutePayload {
            path,
            method: "POST".to_string(),
            rps,
            p50_ms,
            p95_ms,
            p99_ms,
            error_pct,
            in_flight,
            load_pct,
        }
    }).collect();

    routes.sort_by(|a, b| b.p95_ms.partial_cmp(&a.p95_ms).unwrap_or(std::cmp::Ordering::Equal));
    routes
}
```

- [ ] **Update the call site in `build_console_payload`** — add `.await`:

Find `let routes = build_route_payloads(state, concurrency_max);` and change to:

```rust
let routes = build_route_payloads(state, concurrency_max).await;
```

- [ ] **Build**

```bash
cargo build -p server 2>&1 | grep "^error" | head -20
```

- [ ] **Commit**

```bash
git add crates/server/src/console_store.rs
git commit -m "fix(console): per-route RPS, in-flight, and load_pct now computed correctly"
```

---

## Task 7: Backend — Update sampler to compute new MetricsSample fields

`crates/server/src/console_store.rs` — extend the `spawn_console_sampler` loop to compute p50, p55, per-engine conv RPS, and queue wait p95.

- [ ] **Add helper function to extract a percentile from a named histogram**

Add this helper after `percentile_from_histogram`:

```rust
/// Extract a single percentile (ms) from a named Prometheus histogram across all label combinations.
fn global_histogram_pct(families: &[prometheus::proto::MetricFamily], name: &str, pct: f64) -> f64 {
    let Some(family) = families.iter().find(|f| f.get_name() == name) else { return 0.0; };
    let mut agg_count = 0u64;
    let mut agg_buckets: Vec<(f64, u64)> = Vec::new();
    for m in family.get_metric() {
        let hist = m.get_histogram();
        agg_count += hist.get_sample_count();
        for (i, b) in hist.get_bucket().iter().enumerate() {
            if agg_buckets.len() <= i {
                agg_buckets.push((b.get_upper_bound(), b.get_cumulative_count()));
            } else {
                agg_buckets[i].1 += b.get_cumulative_count();
            }
        }
    }
    if agg_count == 0 || agg_buckets.is_empty() { return 0.0; }
    let target = (agg_count as f64 * pct) as u64;
    let mut prev_count = 0u64;
    let mut prev_bound = 0.0f64;
    for (bound, count) in &agg_buckets {
        if bound.is_infinite() { break; }
        if *count >= target {
            if *count == prev_count { return prev_bound * 1000.0; }
            return (prev_bound + (bound - prev_bound)
                * ((target - prev_count) as f64 / (count - prev_count) as f64)) * 1000.0;
        }
        prev_count = *count;
        prev_bound = *bound;
    }
    agg_buckets.iter().rev().find(|(b, _)| !b.is_infinite())
        .map(|(b, _)| b * 1000.0).unwrap_or(0.0)
}

/// Sum a counter vec for a specific engine label value.
fn engine_conv_total(families: &[prometheus::proto::MetricFamily], engine: &str) -> f64 {
    families.iter()
        .find(|f| f.get_name() == "pdfbro_conversions_total")
        .map(|f| f.get_metric().iter()
            .filter(|m| m.get_label().iter().any(|l| l.get_name() == "engine" && l.get_value() == engine))
            .map(|m| m.get_counter().get_value())
            .sum())
        .unwrap_or(0.0)
}

/// Total bytes processed by an engine from pdfbro_conversion_bytes_total.
fn engine_bytes_total(families: &[prometheus::proto::MetricFamily], engine: &str) -> f64 {
    families.iter()
        .find(|f| f.get_name() == "pdfbro_conversion_bytes_total")
        .map(|f| f.get_metric().iter()
            .filter(|m| m.get_label().iter().any(|l| l.get_name() == "engine" && l.get_value() == engine))
            .map(|m| m.get_counter().get_value())
            .sum())
        .unwrap_or(0.0)
}
```

- [ ] **Update the sampler tick to compute new fields**

In the `spawn_console_sampler` loop, after the existing `p95_ms` computation block, add:

```rust
// ── p50 + p55 from HTTP duration histogram ─────────────────────────────
let p50_ms = global_histogram_pct(&families, "pdfbro_http_request_duration_seconds", 0.50);
let p55_ms = global_histogram_pct(&families, "pdfbro_http_request_duration_seconds", 0.55);

// ── Per-engine conversion RPS ──────────────────────────────────────────
let chromium_total = engine_conv_total(&families, "chromium");
let libreoffice_total = engine_conv_total(&families, "libreoffice");
let (chromium_conv_rps, libreoffice_conv_rps) = {
    let mut prev_ch = state.console.prev_chromium_conv_total.lock().await;
    let mut prev_lo = state.console.prev_libreoffice_conv_total.lock().await;
    let ch_rps = (chromium_total - *prev_ch).max(0.0) / 5.0;
    let lo_rps = (libreoffice_total - *prev_lo).max(0.0) / 5.0;
    *prev_ch = chromium_total;
    *prev_lo = libreoffice_total;
    (ch_rps, lo_rps)
};

// ── Queue wait p95 ────────────────────────────────────────────────────
let queue_wait_p95_ms = global_histogram_pct(&families, "pdfbro_queue_wait_seconds", 0.95);
```

- [ ] **Update MetricsSample construction** to include new fields:

Find the `let sample = MetricsSample { ... }` block and add the new fields:

```rust
let sample = MetricsSample {
    ts: std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs(),
    rps,
    p50_ms,
    p55_ms,
    p95_ms,
    error_pct,
    queue_size: state.metrics.queue_size.get() as u32,
    concurrency_active,
    cpu_pct,
    memory_mb,
    chromium_conv_rps,
    libreoffice_conv_rps,
    queue_wait_p95_ms,
};
```

- [ ] **Build**

```bash
cargo build -p server 2>&1 | grep "^error" | head -20
```

- [ ] **Commit**

```bash
git add crates/server/src/console_store.rs
git commit -m "feat(console): compute p50/p55, per-engine conv RPS, queue wait p95 in sampler"
```

---

## Task 8: Backend — Update build_console_payload to assemble all new data

`crates/server/src/console_store.rs` — fix the `build_console_payload` function to use all new fields and wire up the engine stats.

- [ ] **Update the history extraction block** to read new time-series:

Find the `let (rps_series, p95_series, cpu_series, memory_series, ...) = { ... }` block. Replace it with:

```rust
let (ts_series, rps_series, p95_series, cpu_series, memory_series,
     chromium_conv_series, libreoffice_conv_series, queue_wait_p95_series,
     last_rps, last_p50_ms, last_p55_ms, last_p95_ms, last_error_pct) = {
    let history = state.console.history.lock().await;
    let ts_series: Vec<u64>  = history.samples.iter().map(|s| s.ts).collect();
    let rps_series: Vec<f64> = history.samples.iter().map(|s| s.rps).collect();
    let p95_series: Vec<f64> = history.samples.iter().map(|s| s.p95_ms / 1000.0).collect();
    let cpu_series: Vec<f64> = history.samples.iter().map(|s| s.cpu_pct).collect();
    let memory_series: Vec<f64> = history.samples.iter().map(|s| s.memory_mb).collect();
    let chromium_conv_series: Vec<f64> = history.samples.iter().map(|s| s.chromium_conv_rps).collect();
    let libreoffice_conv_series: Vec<f64> = history.samples.iter().map(|s| s.libreoffice_conv_rps).collect();
    let queue_wait_p95_series: Vec<f64> = history.samples.iter().map(|s| s.queue_wait_p95_ms).collect();
    let last_rps     = rps_series.last().copied().unwrap_or(0.0);
    let last_p50_ms  = history.samples.back().map_or(0.0, |s| s.p50_ms);
    let last_p55_ms  = history.samples.back().map_or(0.0, |s| s.p55_ms);
    let last_p95_ms  = p95_series.last().copied().unwrap_or(0.0) * 1000.0;
    let last_error_pct = history.samples.back().map_or(0.0, |s| s.error_pct);
    (ts_series, rps_series, p95_series, cpu_series, memory_series,
     chromium_conv_series, libreoffice_conv_series, queue_wait_p95_series,
     last_rps, last_p50_ms, last_p55_ms, last_p95_ms, last_error_pct)
};
```

- [ ] **Update TickerPayload construction** — remove chromium/LO fields, add p50/p55:

Replace the `ticker: TickerPayload { ... }` block with:

```rust
ticker: TickerPayload {
    rps: last_rps,
    p50_ms: last_p50_ms,
    p55_ms: last_p55_ms,
    p95_ms: last_p95_ms,
    error_pct: last_error_pct,
    concurrency_active,
    concurrency_max,
    queue_size,
    uptime_seconds,
},
```

- [ ] **Update EnginePayload construction** to include new stats

The engines block reads Prometheus for conv stats. Replace the `engines: { ... }` block:

```rust
engines: {
    let families = prometheus::gather();
    let mut engines = Vec::new();
    #[cfg(feature = "chromium")]
    {
        let ch_total = engine_conv_total(&families, "chromium");
        let ch_errors: f64 = families.iter()
            .find(|f| f.get_name() == "pdfbro_conversions_total")
            .map(|f| f.get_metric().iter()
                .filter(|m| m.get_label().iter().any(|l| l.get_name() == "engine" && l.get_value() == "chromium")
                    && m.get_label().iter().any(|l| l.get_name() == "status" && l.get_value() == "error"))
                .map(|m| m.get_counter().get_value())
                .sum())
            .unwrap_or(0.0);
        let ch_bytes_mb = engine_bytes_total(&families, "chromium") / (1024.0 * 1024.0);
        let ch_error_rate = if ch_total > 0.0 { (ch_errors / ch_total) * 100.0 } else { 0.0 };
        let ch_idle = match state.chromium.as_ref() {
            Some(be) => be.idle_secs(),
            None => 0,
        };
        engines.push(EnginePayload {
            name: "Chromium".to_string(),
            status: chromium_status.clone(),
            restarts: chromium_restarts,
            mode: if state.config.chromium_lazy_start { "lazy".to_string() } else { "eager".to_string() },
            mini_series: mini.clone(),
            conversions_total: ch_total as u64,
            error_rate: ch_error_rate,
            bytes_mb: ch_bytes_mb,
            idle_secs: ch_idle,
        });
    }
    #[cfg(feature = "libreoffice")]
    {
        let lo_total = engine_conv_total(&families, "libreoffice");
        let lo_errors: f64 = families.iter()
            .find(|f| f.get_name() == "pdfbro_conversions_total")
            .map(|f| f.get_metric().iter()
                .filter(|m| m.get_label().iter().any(|l| l.get_name() == "engine" && l.get_value() == "libreoffice")
                    && m.get_label().iter().any(|l| l.get_name() == "status" && l.get_value() == "error"))
                .map(|m| m.get_counter().get_value())
                .sum())
            .unwrap_or(0.0);
        let lo_bytes_mb = engine_bytes_total(&families, "libreoffice") / (1024.0 * 1024.0);
        let lo_error_rate = if lo_total > 0.0 { (lo_errors / lo_total) * 100.0 } else { 0.0 };
        #[cfg(feature = "libreoffice")]
        let lo_idle = match state.libreoffice.as_ref() {
            Some(lo) => lo.idle_secs(),
            None => 0,
        };
        #[cfg(not(feature = "libreoffice"))]
        let lo_idle = 0u64;
        engines.push(EnginePayload {
            name: "LibreOffice".to_string(),
            status: libreoffice_status.clone(),
            restarts: libreoffice_restarts,
            mode: if state.config.libreoffice_lazy_start { "lazy".to_string() } else { "eager".to_string() },
            mini_series: mini,
            conversions_total: lo_total as u64,
            error_rate: lo_error_rate,
            bytes_mb: lo_bytes_mb,
            idle_secs: lo_idle,
        });
    }
    engines
},
```

- [ ] **Update ConcurrencyPayload construction**:

Replace the `concurrency: ConcurrencyPayload { ... }` block with:

```rust
concurrency: ConcurrencyPayload {
    active: concurrency_active,
    max: concurrency_max,
    warn_threshold: (concurrency_max as f64 * 0.60) as u32,
    crit_threshold: (concurrency_max as f64 * 0.85) as u32,
    queue_wait_p95_ms: {
        let h = state.console.history.lock().await;
        h.samples.back().map_or(0.0, |s| s.queue_wait_p95_ms)
    },
    queue_processing: state.metrics.queue_processing.get() as u32,
},
```

- [ ] **Update ThroughputPayload construction**:

Replace the `throughput: ThroughputPayload { ... }` block with:

```rust
throughput: ThroughputPayload {
    ts_series,
    rps_series,
    rps_baseline: 0.0,
    p95_series,
    p95_target_s: 2.0,
    chromium_conv_series,
    libreoffice_conv_series,
    queue_wait_p95_series,
},
```

- [ ] **Build — should be clean now**

```bash
cargo build -p server 2>&1 | grep "^error" | head -20
```
Expected: no errors.

- [ ] **Commit**

```bash
git add crates/server/src/console_store.rs
git commit -m "feat(console): assemble all new payload fields in build_console_payload"
```

---

## Task 9: Backend — Wire up build_batch_payloads

`crates/server/src/console_store.rs` — replace the placeholder that returns `vec![]` with real data from `BatchStateManager`.

- [ ] **Replace `build_batch_payloads`**

Replace the existing `async fn build_batch_payloads` function with:

```rust
async fn build_batch_payloads(state: &crate::state::AppState) -> Vec<BatchPayload> {
    let Some(ref manager) = state.batch_manager else { return vec![]; };

    let ids = manager.list_batches().await;
    let mut batches: Vec<BatchPayload> = Vec::new();

    for id in &ids {
        let Some(b) = manager.get_batch(id).await else { continue };
        if b.is_expired() { continue; }

        let progress = b.progress();
        let progress_pct = if progress.total > 0 {
            ((progress.completed + progress.failed) * 100 / progress.total) as u8
        } else {
            0
        };

        let elapsed_secs = b.submitted_at
            .elapsed()
            .unwrap_or_default()
            .as_secs();
        let elapsed = if elapsed_secs < 60 {
            format!("{}s", elapsed_secs)
        } else {
            format!("{}m {}s", elapsed_secs / 60, elapsed_secs % 60)
        };

        let status = match b.status {
            crate::routes::batch_types::BatchStatus::Queued => "queued",
            crate::routes::batch_types::BatchStatus::Processing => "running",
            crate::routes::batch_types::BatchStatus::Completed => "completed",
            crate::routes::batch_types::BatchStatus::Failed => "failed",
        }.to_string();

        let output_mode = match b.request.output_mode {
            crate::routes::batch_types::OutputMode::Zip => "zip",
            crate::routes::batch_types::OutputMode::Merge => "merge",
        }.to_string();

        batches.push(BatchPayload {
            id: id.to_string(),
            status,
            progress_pct,
            elapsed,
            total_items: progress.total,
            completed_items: progress.completed,
            failed_items: progress.failed,
            output_mode,
        });
    }

    // Sort: running first, then queued, then completed/failed; newest first within groups
    batches.sort_by(|a, b| {
        let order = |s: &str| match s { "running" => 0, "queued" => 1, _ => 2 };
        order(&a.status).cmp(&order(&b.status))
    });
    batches.truncate(10);
    batches
}
```

- [ ] **Build cleanly**

```bash
cargo build -p server 2>&1 | grep "^error" | head -10
```
Expected: no errors.

- [ ] **Run tests**

```bash
cargo test -p server 2>&1 | tail -10
```
Expected: all pass.

- [ ] **Commit**

```bash
git add crates/server/src/console_store.rs
git commit -m "feat(console): wire up build_batch_payloads with real BatchStateManager data"
```

---

## Task 10: Frontend — Update types.ts

`ui/src/lib/types.ts` — mirror all backend struct changes.

- [ ] **Replace the entire file**

```typescript
// src/lib/types.ts
export interface MetricsSample {
    ts: number;
    rps: number;
    p50_ms: number;
    p55_ms: number;
    p95_ms: number;
    error_pct: number;
    queue_size: number;
    concurrency_active: number;
    cpu_pct: number;
    memory_mb: number;
    chromium_conv_rps: number;
    libreoffice_conv_rps: number;
    queue_wait_p95_ms: number;
}

export interface TickerPayload {
    rps: number;
    p50_ms: number;
    p55_ms: number;
    p95_ms: number;
    error_pct: number;
    concurrency_active: number;
    concurrency_max: number;
    queue_size: number;
    uptime_seconds: number;
}

export interface RoutePayload {
    path: string;
    method: string;
    rps: number;
    p50_ms: number;
    p95_ms: number;
    p99_ms: number;
    error_pct: number;
    in_flight: number;
    load_pct: number;
}

export interface EnginePayload {
    name: string;
    status: string;
    restarts: number;
    mode: string;
    mini_series: number[];
    conversions_total: number;
    error_rate: number;
    bytes_mb: number;
    idle_secs: number;
}

export interface ConcurrencyPayload {
    active: number;
    max: number;
    warn_threshold: number;
    crit_threshold: number;
    queue_wait_p95_ms: number;
    queue_processing: number;
}

export interface ResourcesPayload {
    cpu_series: number[];
    memory_series: number[];
    memory_max_mb: number;
}

export interface ThroughputPayload {
    ts_series: number[];
    rps_series: number[];
    rps_baseline: number;
    p95_series: number[];
    p95_target_s: number;
    chromium_conv_series: number[];
    libreoffice_conv_series: number[];
    queue_wait_p95_series: number[];
}

export interface BatchPayload {
    id: string;
    status: string;
    progress_pct: number;
    elapsed: string;
    total_items: number;
    completed_items: number;
    failed_items: number;
    output_mode: string;
}

export interface RequestLogEntry {
    time: string;
    method: string;
    route: string;
    status: number;
    duration_ms: number;
}

export interface ErrorLogEntry {
    time: string;
    route: string;
    message: string;
    request_id: string;
}

export interface ConsolePayload {
    version: string;
    uptime_seconds: number;
    ticker: TickerPayload;
    routes: RoutePayload[];
    engines: EnginePayload[];
    concurrency: ConcurrencyPayload;
    resources: ResourcesPayload;
    throughput: ThroughputPayload;
    batches: BatchPayload[];
    recent_requests: RequestLogEntry[];
    recent_errors: ErrorLogEntry[];
}
```

- [ ] **Commit**

```bash
git add ui/src/lib/types.ts
git commit -m "feat(console): update frontend types to match new payload structs"
```

---

## Task 11: Frontend — Update Ticker.svelte

`ui/src/lib/components/Ticker.svelte` — remove Chromium/LibreOffice blocks, add P50/P55.

- [ ] **Replace the file**

```svelte
<!-- src/lib/components/Ticker.svelte -->
<script lang="ts">
    import type { TickerPayload } from '$lib/types';
    import type { Theme } from '$lib/theme.svelte';

    let { ticker, t, D }: { ticker: TickerPayload; t: Theme; D: { kpiFz: number; fz: number; pad: number } } = $props();

    function fmtMs(ms: number) {
        return ms >= 1000 ? `${(ms / 1000).toFixed(1)}s` : `${ms.toFixed(0)}ms`;
    }

    function fmtUptime(s: number) {
        const h = Math.floor(s / 3600);
        const m = Math.floor((s % 3600) / 60);
        return `${h}h ${m}m`;
    }

    let items = $derived([
        { label: 'RPS',    value: ticker.rps.toFixed(1),             tone: 'ink' as const },
        { label: 'P50',    value: fmtMs(ticker.p50_ms),              tone: (ticker.p50_ms > 1000 ? 'err' : ticker.p50_ms > 500 ? 'warn' : 'ok') as 'ok' | 'warn' | 'err' },
        { label: 'P55',    value: fmtMs(ticker.p55_ms),              tone: (ticker.p55_ms > 1000 ? 'err' : ticker.p55_ms > 500 ? 'warn' : 'ok') as 'ok' | 'warn' | 'err' },
        { label: 'P95',    value: fmtMs(ticker.p95_ms),              tone: (ticker.p95_ms > 2000 ? 'err' : ticker.p95_ms > 1500 ? 'warn' : 'ok') as 'ok' | 'warn' | 'err' },
        { label: 'Errors', value: `${ticker.error_pct.toFixed(2)}%`, tone: (ticker.error_pct > 1 ? 'err' : ticker.error_pct > 0.5 ? 'warn' : 'ok') as 'ok' | 'warn' | 'err' },
        { label: 'Conc.',  value: `${ticker.concurrency_active} / ${ticker.concurrency_max}`, tone: 'ink' as const },
        { label: 'Queue',  value: String(Math.round(ticker.queue_size)), tone: 'ink' as const },
        { label: 'Uptime', value: fmtUptime(ticker.uptime_seconds),   tone: 'ok' as const },
    ]);
</script>

<div style="background:{t.surface};border:1px solid {t.rule};border-radius:12px;display:grid;grid-template-columns:repeat({items.length},1fr)">
    {#each items as item, i}
        {@const color = t[item.tone as keyof Theme] as string ?? t.ink}
        <div style="padding:{D.pad}px {D.pad + 2}px;{i < items.length - 1 ? `border-right:1px solid ${t.rule}` : ''}">
            <div style="color:{t.muted};font-size:10px;letter-spacing:0.06em;text-transform:uppercase;font-weight:500">{item.label}</div>
            <div style="font-family:ui-monospace,monospace;font-size:{D.kpiFz}px;font-weight:600;margin-top:2px;letter-spacing:-0.01em;color:{color}">{item.value}</div>
        </div>
    {/each}
</div>
```

- [ ] **Commit**

```bash
git add ui/src/lib/components/Ticker.svelte
git commit -m "feat(console): update Ticker — P50/P55 replace Chromium/LibreOffice blocks"
```

---

## Task 12: Frontend — Update RoutesTable.svelte (scrollable)

`ui/src/lib/components/RoutesTable.svelte` — make it flex:1 and overflow-y:auto so it fills remaining left-column height and scrolls when rows overflow.

- [ ] **Replace the Card wrapper style**

Change the outer `<Card>` call to pass a `style` prop that sets `flex:1; min-height:0; display:flex; flex-direction:column; overflow:hidden`:

```svelte
<!-- src/lib/components/RoutesTable.svelte -->
<script lang="ts">
    import type { RoutePayload } from '$lib/types';
    import type { Theme } from '$lib/theme.svelte';
    import Card from './shared/Card.svelte';
    import SlimBar from './shared/SlimBar.svelte';

    let { routes, t, D }: { routes: RoutePayload[]; t: Theme; D: { fz: number; pad: number; rowPy: number } } = $props();

    function fmtMs(ms: number) {
        if (ms <= 0) return '—';
        return ms >= 1000 ? `${(ms / 1000).toFixed(1)}s` : `${ms.toFixed(0)}ms`;
    }

    let sorted = $derived([...routes].sort((a, b) => b.p95_ms - a.p95_ms));
</script>

<Card {t} title="Routes" sub="{routes.length} endpoints · sorted by p95 desc" style="flex:1;min-height:0;display:flex;flex-direction:column;overflow:hidden">
    {#if routes.length === 0}
        <div style="padding:{D.pad}px;color:{t.muted};font-size:{D.fz}px">No route data yet</div>
    {:else}
        <div style="overflow-y:auto;flex:1;min-height:0">
            <table style="width:100%;border-collapse:collapse;font-family:ui-monospace,monospace;font-size:{D.fz}px">
                <thead>
                    <tr>
                        {#each ['Route','Method','RPS','p50','p95','p99','Err %','In-flight','Load'] as h, i}
                            <th style="padding:{D.rowPy + 4}px {D.pad + 2}px {D.rowPy + 2}px;text-align:{i < 2 ? 'left' : 'right'};font-weight:500;font-size:10px;letter-spacing:0.04em;color:{t.muted};text-transform:uppercase;border-bottom:1px solid {t.rule};position:sticky;top:0;background:{t.surface};z-index:1">{h}</th>
                        {/each}
                    </tr>
                </thead>
                <tbody style="font-variant-numeric:tabular-nums">
                    {#each sorted as r}
                        {@const p95tone = r.p95_ms > 10000 ? t.err : r.p95_ms > 5000 ? t.warn : t.ink}
                        <tr style="border-bottom:1px solid {t.rule}">
                            <td style="padding:{D.rowPy}px {D.pad + 2}px">{r.path}</td>
                            <td style="padding:{D.rowPy}px {D.pad + 2}px;color:{t.muted}">{r.method}</td>
                            <td style="padding:{D.rowPy}px {D.pad + 2}px;text-align:right">{r.rps.toFixed(1)}</td>
                            <td style="padding:{D.rowPy}px {D.pad + 2}px;text-align:right;color:{t.muted}">{fmtMs(r.p50_ms)}</td>
                            <td style="padding:{D.rowPy}px {D.pad + 2}px;text-align:right;color:{p95tone};font-weight:{r.p95_ms > 5000 ? 600 : 400}">{fmtMs(r.p95_ms)}</td>
                            <td style="padding:{D.rowPy}px {D.pad + 2}px;text-align:right;color:{t.muted}">{fmtMs(r.p99_ms)}</td>
                            <td style="padding:{D.rowPy}px {D.pad + 2}px;text-align:right;color:{r.error_pct > 1 ? t.err : r.error_pct > 0 ? t.warn : t.muted}">{r.error_pct.toFixed(2)}</td>
                            <td style="padding:{D.rowPy}px {D.pad + 2}px;text-align:right">{r.in_flight}</td>
                            <td style="padding:{D.rowPy}px {D.pad + 2}px;width:80px"><SlimBar pct={r.load_pct} {t} h={4} /></td>
                        </tr>
                    {/each}
                </tbody>
            </table>
        </div>
    {/if}
</Card>
```

- [ ] **Commit**

```bash
git add ui/src/lib/components/RoutesTable.svelte
git commit -m "feat(console): RoutesTable scrollable with sticky header"
```

---

## Task 13: Frontend — Create StackedBarSeries.svelte

`ui/src/lib/components/StackedBarSeries.svelte` — SVG bar chart with two stacked series (Chromium on bottom, LibreOffice on top).

- [ ] **Create the file**

```svelte
<!-- src/lib/components/StackedBarSeries.svelte -->
<script lang="ts">
    import type { Theme } from '$lib/theme.svelte';

    let {
        seriesA,
        seriesB,
        colorA,
        colorB,
        height = 64,
        labelA = 'A',
        labelB = 'B',
        t,
    }: {
        seriesA: number[];
        seriesB: number[];
        colorA: string;
        colorB: string;
        height?: number;
        labelA?: string;
        labelB?: string;
        t: Theme;
    } = $props();

    let len = $derived(Math.max(seriesA.length, seriesB.length));
    let combined = $derived(
        Array.from({ length: len }, (_, i) => (seriesA[i] ?? 0) + (seriesB[i] ?? 0))
    );
    let maxVal = $derived(Math.max(...combined, 0.001));

    let hoveredIdx = $state<number | null>(null);
    let svgEl = $state<SVGSVGElement | null>(null);

    function onMouseMove(e: MouseEvent) {
        if (!svgEl || len === 0) return;
        const rect = svgEl.getBoundingClientRect();
        const x = e.clientX - rect.left;
        hoveredIdx = Math.min(len - 1, Math.max(0, Math.floor((x / rect.width) * len)));
    }
    function onMouseLeave() { hoveredIdx = null; }
</script>

<div style="position:relative;width:100%;height:{height}px">
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <svg
        bind:this={svgEl}
        width="100%"
        height={height}
        style="display:block;overflow:visible"
        onmousemove={onMouseMove}
        onmouseleave={onMouseLeave}
    >
        {#each Array.from({ length: len }, (_, i) => i) as i}
            {@const w = 100 / len}
            {@const a = seriesA[i] ?? 0}
            {@const b = seriesB[i] ?? 0}
            {@const total = a + b}
            {@const totalH = (total / maxVal) * height}
            {@const aH = total > 0 ? (a / total) * totalH : 0}
            {@const bH = totalH - aH}
            {@const x = i * w}
            <!-- Series A (bottom) -->
            <rect
                x="{x + 0.3}%"
                y={height - aH}
                width="{w - 0.6}%"
                height={aH}
                fill={colorA}
                opacity={hoveredIdx === i ? 1 : 0.8}
                rx="1"
            />
            <!-- Series B (top) -->
            {#if bH > 0}
                <rect
                    x="{x + 0.3}%"
                    y={height - totalH}
                    width="{w - 0.6}%"
                    height={bH}
                    fill={colorB}
                    opacity={hoveredIdx === i ? 1 : 0.8}
                    rx="1"
                />
            {/if}
        {/each}
        {#if hoveredIdx !== null}
            {@const w = 100 / len}
            {@const cx = (hoveredIdx + 0.5) * w}
            <line x1="{cx}%" y1="0" x2="{cx}%" y2={height}
                stroke={t.muted} stroke-width="1" stroke-dasharray="2 2" opacity="0.4" />
        {/if}
    </svg>
    {#if hoveredIdx !== null}
        {@const w = 100 / len}
        {@const pctLeft = (hoveredIdx + 0.5) * w}
        {@const flipLeft = pctLeft > 70}
        {@const a = (seriesA[hoveredIdx] ?? 0).toFixed(2)}
        {@const b = (seriesB[hoveredIdx] ?? 0).toFixed(2)}
        <div style="
            position:absolute;top:-34px;
            {flipLeft ? `right:${100 - pctLeft}%` : `left:${pctLeft}%`};
            transform:{flipLeft ? 'translateX(50%)' : 'translateX(-50%)'};
            background:{t.ink};color:{t.bg};
            font-family:ui-monospace,monospace;font-size:10px;
            padding:2px 7px;border-radius:3px;white-space:nowrap;pointer-events:none;z-index:10
        ">
            <span style="color:{colorA}">{labelA} {a}</span>
            &nbsp;·&nbsp;
            <span style="color:{colorB}">{labelB} {b}</span>
        </div>
    {/if}
</div>
```

- [ ] **Commit**

```bash
git add ui/src/lib/components/StackedBarSeries.svelte
git commit -m "feat(console): add StackedBarSeries SVG chart component"
```

---

## Task 14: Frontend — Create EngineConvChart.svelte and QueueWaitChart.svelte

Two new chart card components for the left column row 2.

- [ ] **Create EngineConvChart.svelte**

```svelte
<!-- src/lib/components/EngineConvChart.svelte -->
<script lang="ts">
    import type { ThroughputPayload } from '$lib/types';
    import type { Theme } from '$lib/theme.svelte';
    import Card from './shared/Card.svelte';
    import StackedBarSeries from './StackedBarSeries.svelte';

    let { throughput, t, D }: { throughput: ThroughputPayload; t: Theme; D: { pad: number; fz: number } } = $props();

    let lastCh = $derived(throughput.chromium_conv_series.at(-1) ?? 0);
    let lastLo = $derived(throughput.libreoffice_conv_series.at(-1) ?? 0);
</script>

<Card {t} title="Engine conversions" sub="conv/sec · stacked">
    <div style="padding:{D.pad + 2}px">
        <div style="display:flex;justify-content:space-between;font-size:{D.fz - 1}px;color:{t.muted};margin-bottom:6px">
            <div style="display:flex;gap:10px">
                <span><span style="color:#14b8a6">■</span> Chromium <strong style="color:{t.ink};font-family:ui-monospace,monospace">{lastCh.toFixed(2)}</strong></span>
                <span><span style="color:#f59e0b">■</span> LibreOffice <strong style="color:{t.ink};font-family:ui-monospace,monospace">{lastLo.toFixed(2)}</strong></span>
            </div>
        </div>
        <StackedBarSeries
            seriesA={throughput.chromium_conv_series}
            seriesB={throughput.libreoffice_conv_series}
            colorA="#14b8a6"
            colorB="#f59e0b"
            labelA="Chromium"
            labelB="LibreOffice"
            height={72}
            {t}
        />
    </div>
</Card>
```

- [ ] **Create QueueWaitChart.svelte**

```svelte
<!-- src/lib/components/QueueWaitChart.svelte -->
<script lang="ts">
    import type { ThroughputPayload } from '$lib/types';
    import type { Theme } from '$lib/theme.svelte';
    import Card from './shared/Card.svelte';
    import BarSeries from './shared/BarSeries.svelte';

    let { throughput, t, D }: { throughput: ThroughputPayload; t: Theme; D: { pad: number; fz: number } } = $props();

    let lastWait = $derived(throughput.queue_wait_p95_series.at(-1) ?? 0);
    let tone = $derived(lastWait > 5000 ? t.err : lastWait > 1000 ? t.warn : t.ok);
</script>

<Card {t} title="Queue wait p95" sub="ms · time before processing starts">
    <div style="padding:{D.pad + 2}px">
        <div style="display:flex;justify-content:space-between;font-size:{D.fz - 1}px;color:{t.muted};margin-bottom:6px">
            <span>wait p95</span>
            <span style="font-family:ui-monospace,monospace;color:{tone};font-weight:600">
                {lastWait >= 1000 ? `${(lastWait / 1000).toFixed(1)}s` : `${lastWait.toFixed(0)}ms`}
            </span>
        </div>
        <BarSeries
            series={throughput.queue_wait_p95_series}
            color={tone}
            height={72}
            label="ms"
            formatValue={(v) => v >= 1000 ? `${(v/1000).toFixed(1)}s` : `${v.toFixed(0)}ms`}
            {t}
        />
    </div>
</Card>
```

- [ ] **Commit**

```bash
git add ui/src/lib/components/EngineConvChart.svelte ui/src/lib/components/QueueWaitChart.svelte
git commit -m "feat(console): add EngineConvChart and QueueWaitChart components"
```

---

## Task 15: Frontend — Update Engines.svelte with per-engine sub-cards

`ui/src/lib/components/side-rail/Engines.svelte` — add conv/err%/MB/idle stats below each engine, preserve sparklines and UP/DOWN badge.

- [ ] **Replace the file**

```svelte
<!-- src/lib/components/side-rail/Engines.svelte -->
<script lang="ts">
    import type { EnginePayload } from '$lib/types';
    import type { Theme } from '$lib/theme.svelte';
    import Card from '../shared/Card.svelte';
    import Pill from '../shared/Pill.svelte';
    import BarSeries from '../shared/BarSeries.svelte';

    let { engines, t, D }: { engines: EnginePayload[]; t: Theme; D: { fz: number; pad: number } } = $props();

    function engineTone(e: EnginePayload): 'ok' | 'warn' | 'err' | 'ink' {
        if (e.status === 'n/a') return 'ink';
        if (e.status !== 'up') return 'err';
        if (e.restarts > 5) return 'warn';
        return 'ok';
    }
    function engineColor(e: EnginePayload): string {
        const tone = engineTone(e);
        if (tone === 'ok') return t.ok;
        if (tone === 'warn') return t.warn;
        if (tone === 'err') return t.err;
        return t.muted;
    }
    function fmtIdle(s: number): string {
        if (s === 0) return 'active';
        if (s < 60) return `idle ${s}s`;
        return `idle ${Math.floor(s / 60)}m`;
    }
    function fmtBytes(mb: number): string {
        if (mb >= 1024) return `${(mb / 1024).toFixed(1)}GB`;
        return `${mb.toFixed(1)}MB`;
    }
</script>

<Card {t} title="Engines">
    <div style="padding:{D.pad}px;font-size:{D.fz}px">
        {#each engines as e, i}
            <div style="{i > 0 ? `margin-top:${D.pad - 2}px;padding-top:${D.pad - 2}px;border-top:1px solid ${t.rule}` : ''}">
                <div style="display:flex;align-items:center;justify-content:space-between;margin-bottom:4px">
                    <div style="display:flex;align-items:center;gap:6px">
                        <strong style="font-size:{D.fz + 0.5}px">{e.name}</strong>
                        <Pill tone={engineTone(e)} {t}>{e.status.toUpperCase()}</Pill>
                    </div>
                    <span style="color:{t.muted};font-size:10px;font-family:ui-monospace,monospace">
                        {e.restarts} activation{e.restarts !== 1 ? 's' : ''} · {e.mode}
                    </span>
                </div>
                {#if e.mini_series.length > 0}
                    <BarSeries
                        series={e.mini_series}
                        color={engineColor(e)}
                        height={28}
                        label="load"
                        formatValue={(v) => (v * 100).toFixed(0) + '%'}
                        {t}
                    />
                {/if}
                <!-- Stats row -->
                <div style="display:grid;grid-template-columns:1fr 1fr 1fr 1fr;gap:4px;margin-top:5px;font-size:9px;font-family:ui-monospace,monospace">
                    <div style="background:{t.faint};border-radius:3px;padding:3px 5px">
                        <div style="color:{t.muted}">conv</div>
                        <div style="font-weight:600">{e.conversions_total}</div>
                    </div>
                    <div style="background:{t.faint};border-radius:3px;padding:3px 5px">
                        <div style="color:{t.muted}">err%</div>
                        <div style="font-weight:600;color:{e.error_rate > 1 ? t.err : e.error_rate > 0 ? t.warn : t.ok}">{e.error_rate.toFixed(2)}</div>
                    </div>
                    <div style="background:{t.faint};border-radius:3px;padding:3px 5px">
                        <div style="color:{t.muted}">data</div>
                        <div style="font-weight:600">{fmtBytes(e.bytes_mb)}</div>
                    </div>
                    <div style="background:{t.faint};border-radius:3px;padding:3px 5px">
                        <div style="color:{t.muted}">state</div>
                        <div style="font-weight:600;color:{e.idle_secs === 0 ? t.ok : t.muted}">{fmtIdle(e.idle_secs)}</div>
                    </div>
                </div>
            </div>
        {/each}
        {#if engines.length === 0}
            <div style="color:{t.muted}">No engines configured</div>
        {/if}
    </div>
</Card>
```

- [ ] **Commit**

```bash
git add ui/src/lib/components/side-rail/Engines.svelte
git commit -m "feat(console): enhance Engines card with conv/err%/MB/idle sub-stats"
```

---

## Task 16: Frontend — Update Concurrency.svelte (add queue stats row)

`ui/src/lib/components/side-rail/Concurrency.svelte` — preserve all existing content, add queue wait p95 and processing count below the slot grid.

- [ ] **Add the queue stats row after the scale labels**

Find the closing `</div>` after the scale labels line (the `<div style="display:flex;justify-content:space-between;...">` div that shows `0 warn N crit N max`). After that div, inside the outer `<div style="padding:...">`, add:

```svelte
<!-- Queue stats row (new) -->
<div style="display:grid;grid-template-columns:1fr 1fr;gap:6px;margin-top:8px">
    <div style="background:{t.faint};border-radius:4px;padding:4px 8px;font-size:10px;font-family:ui-monospace,monospace">
        <div style="color:{t.muted};font-size:9px;text-transform:uppercase;letter-spacing:0.04em">queue wait p95</div>
        <div style="font-weight:600;color:{conc.queue_wait_p95_ms > 5000 ? t.err : conc.queue_wait_p95_ms > 1000 ? t.warn : t.ok}">
            {conc.queue_wait_p95_ms >= 1000 ? `${(conc.queue_wait_p95_ms / 1000).toFixed(1)}s` : `${conc.queue_wait_p95_ms.toFixed(0)}ms`}
        </div>
    </div>
    <div style="background:{t.faint};border-radius:4px;padding:4px 8px;font-size:10px;font-family:ui-monospace,monospace">
        <div style="color:{t.muted};font-size:9px;text-transform:uppercase;letter-spacing:0.04em">processing</div>
        <div style="font-weight:600">{conc.queue_processing} job{conc.queue_processing !== 1 ? 's' : ''}</div>
    </div>
</div>
```

- [ ] **Commit**

```bash
git add ui/src/lib/components/side-rail/Concurrency.svelte
git commit -m "feat(console): add queue wait p95 and processing count to Concurrency card"
```

---

## Task 17: Frontend — Update Batches.svelte

`ui/src/lib/components/side-rail/Batches.svelte` — add item counts, output mode badge. The `sub` count currently checks for `status === 'running'` — keep this logic as queued/running are the "active" states.

- [ ] **Replace the file**

```svelte
<!-- src/lib/components/side-rail/Batches.svelte -->
<script lang="ts">
    import type { BatchPayload } from '$lib/types';
    import type { Theme } from '$lib/theme.svelte';
    import Card from '../shared/Card.svelte';
    import Pill from '../shared/Pill.svelte';
    import SlimBar from '../shared/SlimBar.svelte';

    let { batches, t, D }: { batches: BatchPayload[]; t: Theme; D: { pad: number; fz: number; rowPy: number } } = $props();

    function batchTone(status: string): 'ok' | 'warn' | 'err' | 'accent' | 'ink' {
        if (status === 'failed') return 'err';
        if (status === 'completed') return 'ok';
        if (status === 'queued') return 'ink';
        return 'accent';
    }

    let activeCount = $derived(batches.filter(b => b.status === 'running' || b.status === 'queued').length);
</script>

<Card {t} title="Batches" sub="{activeCount} active">
    {#if batches.length === 0}
        <div style="padding:{D.pad}px;color:{t.muted};font-size:{D.fz}px">No recent batches</div>
    {:else}
        <div style="font-family:ui-monospace,monospace;font-size:{D.fz - 0.5}px">
            {#each batches as b, i}
                <div style="padding:{D.rowPy + 1}px {D.pad}px;{i < batches.length - 1 ? `border-bottom:1px solid ${t.rule}` : ''}">
                    <div style="display:flex;align-items:center;justify-content:space-between;margin-bottom:3px">
                        <div style="display:flex;align-items:center;gap:6px">
                            <Pill tone={batchTone(b.status)} {t}>{b.status.slice(0, 4).toUpperCase()}</Pill>
                            <span style="color:{t.muted};font-size:9px">{b.id.slice(0, 16)}…</span>
                        </div>
                        <div style="display:flex;align-items:center;gap:6px">
                            <span style="color:{t.muted};font-size:9px;background:{t.faint};padding:1px 4px;border-radius:3px">{b.output_mode.toUpperCase()}</span>
                            <span style="color:{t.muted};font-size:9px">{b.elapsed}</span>
                        </div>
                    </div>
                    <div style="display:flex;align-items:center;gap:8px">
                        <div style="flex:1"><SlimBar pct={b.progress_pct} {t} h={4} /></div>
                        <span style="color:{t.muted};font-size:9px;white-space:nowrap">
                            {b.completed_items}/{b.total_items}
                            {#if b.failed_items > 0}<span style="color:{t.err}"> · {b.failed_items} err</span>{/if}
                        </span>
                    </div>
                </div>
            {/each}
        </div>
    {/if}
</Card>
```

- [ ] **Commit**

```bash
git add ui/src/lib/components/side-rail/Batches.svelte
git commit -m "feat(console): wire Batches card with real data — progress, item counts, mode"
```

---

## Task 18: Frontend — Restructure +page.svelte to v4 layout

`ui/src/routes/+page.svelte` — v4 layout: left column has 2×2 chart grid then scrollable routes; remove ActivityStrip.

- [ ] **Replace the main content section** (inside `{:else if metricsStore.data}`)

Replace the block from `<!-- Header -->` down to `<!-- Activity -->` closing `</div>` with:

```svelte
        <!-- Header -->
        <Header data={metricsStore.data} {t} />

        <!-- Ticker -->
        <div style="margin-top:{D.gap}px">
            <Ticker ticker={metricsStore.data.ticker} {t} {D} />
        </div>

        <!-- Main split: left (8fr) + side rail (4fr) -->
        <div style="display:grid;grid-template-columns:8fr 4fr;gap:{D.gap}px;margin-top:{D.gap}px;min-height:0">

            <!-- Left column: charts → routes -->
            <div style="display:flex;flex-direction:column;gap:{D.gap}px;min-height:0">

                <!-- Row 1: HTTP throughput charts -->
                <ThroughputStrip throughput={metricsStore.data.throughput} {t} {D} />

                <!-- Row 2: engine conv + queue wait -->
                <div style="display:grid;grid-template-columns:1fr 1fr;gap:{D.gap}px">
                    <EngineConvChart throughput={metricsStore.data.throughput} {t} {D} />
                    <QueueWaitChart throughput={metricsStore.data.throughput} {t} {D} />
                </div>

                <!-- Routes: fills remaining height, scrollable -->
                <RoutesTable routes={metricsStore.data.routes} {t} {D} />
            </div>

            <!-- Right rail -->
            <div style="display:flex;flex-direction:column;gap:{D.gap}px">
                <Engines engines={metricsStore.data.engines} {t} {D} />
                <Concurrency conc={metricsStore.data.concurrency} {t} {D} />
                <Batches batches={metricsStore.data.batches} {t} {D} />
                <Resources resources={metricsStore.data.resources} {t} {D} />
            </div>
        </div>
```

- [ ] **Update the imports** at the top of the `<script>` block — remove `ActivityStrip`, add `EngineConvChart` and `QueueWaitChart`:

```typescript
    import Header from '$lib/components/Header.svelte';
    import Ticker from '$lib/components/Ticker.svelte';
    import RoutesTable from '$lib/components/RoutesTable.svelte';
    import Engines from '$lib/components/side-rail/Engines.svelte';
    import Concurrency from '$lib/components/side-rail/Concurrency.svelte';
    import Batches from '$lib/components/side-rail/Batches.svelte';
    import Resources from '$lib/components/side-rail/Resources.svelte';
    import ThroughputStrip from '$lib/components/ThroughputStrip.svelte';
    import EngineConvChart from '$lib/components/EngineConvChart.svelte';
    import QueueWaitChart from '$lib/components/QueueWaitChart.svelte';
```

- [ ] **Delete ActivityStrip.svelte**

```bash
rm ui/src/lib/components/ActivityStrip.svelte
```

- [ ] **Commit**

```bash
git add ui/src/routes/+page.svelte ui/src/lib/components/
git commit -m "feat(console): v4 layout — 2x2 chart grid, routes scrollable, ActivityStrip removed"
```

---

## Task 19: Build and verify

- [ ] **Build frontend**

```bash
cd ui && npm run build 2>&1 | tail -20
```
Expected: no TypeScript errors, build succeeds.

- [ ] **Build backend**

```bash
cargo build -p server 2>&1 | grep -E "^error" | head -10
```
Expected: no errors.

- [ ] **Run backend tests**

```bash
cargo test -p server 2>&1 | tail -10
```
Expected: all pass.

- [ ] **Start dev server and verify in browser**

```bash
cd ui && npm run dev &
cargo run -p server -- --dev 2>&1 &
```

Open `http://localhost:5173/_/` (or whatever the dev port is) and verify:
- Ticker shows 8 blocks: RPS · P50 · P55 · P95 · Errors · Conc · Queue · Uptime (no Chromium/LibreOffice)
- Left column: 2×2 chart grid (Req/sec, Lat p95, Engine conv, Queue wait) then Routes table
- Routes table scrolls if content overflows, has sticky header
- Engines card shows sub-stats (conv, err%, data, state) per engine
- Concurrency card still has slot grid + scale labels + the two new queue stats tiles below
- Batches card: "No recent batches" when empty; if you submit a batch via API it appears with progress bar
- No ActivityStrip at the bottom

- [ ] **Final commit**

```bash
git add -A
git commit -m "chore(console): final cleanup and build verification"
```

---

## Self-Review Notes

**Spec coverage check:**
- ✅ Layout v4 (routes in left col below charts) — Task 18
- ✅ Remove ActivityStrip — Task 18
- ✅ P50/P55 in ticker — Tasks 5+11
- ✅ Remove Chromium/LibreOffice from ticker — Tasks 5+11
- ✅ Per-route RPS fix — Task 6
- ✅ Per-route in-flight fix — Tasks 2+6
- ✅ Per-route load_pct fix — Task 6
- ✅ ts_series on throughput — Tasks 5+8
- ✅ Engine conv stats (conv, err%, MB, idle) — Tasks 3+5+8+15
- ✅ Engine conv chart — Tasks 7+13+14
- ✅ Queue wait p95 chart — Tasks 7+13+14
- ✅ ConcurrencyPayload queue stats — Tasks 5+8+16
- ✅ Batches wired up — Tasks 5+9+17
- ✅ RoutesTable scrollable — Task 12

**Type consistency:** All property names defined in Task 5 (backend structs) and Task 10 (TypeScript interfaces) are used consistently throughout Tasks 11–18. `chromium_conv_series`/`libreoffice_conv_series`/`queue_wait_p95_series` in ThroughputPayload match usage in Tasks 14 and 18.
