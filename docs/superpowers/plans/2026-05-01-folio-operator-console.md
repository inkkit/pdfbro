# Folio Operator Console Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the Folio Operator Console — a real-time operations dashboard served at `/_/` inside the Folio binary, implementing the B · Bars wireframe with SSE data transport.

**Architecture:** Two phases. Phase 1 (Tasks 1–7): Rust server additions — `ConsoleStore` ring buffers, SSE broadcast, `/_/api/stream` + `/_/api/metrics` endpoints, request log middleware, and `rust-embed` static file serving. Phase 2 (Tasks 8–15): Svelte 5 SPA implementing the wireframe layout using shadcn-svelte chart components, connected to the SSE endpoint. Phase 3 (Task 16): build integration (Makefile + Docker).

**Tech Stack:** Rust 1.88 (Axum 0.8, tokio, futures-util, rust-embed 8, serde_json), Svelte 5 (runes), SvelteKit, Tailwind CSS v4, shadcn-svelte, TypeScript.

**Spec:** `docs/superpowers/specs/2026-05-01-folio-operator-console-design.md`

---

## File Map

### Created
| File | Purpose |
|---|---|
| `crates/server/src/console_store.rs` | `ConsoleStore`, ring buffers, SSE broadcast channel |
| `crates/server/src/routes/console.rs` | SSE handler, one-shot JSON handler, static asset handler |
| `ui/build/.gitkeep` | Placeholder so rust-embed compiles before UI is built |
| `ui/src/lib/types.ts` | `ConsolePayload` TypeScript types |
| `ui/src/lib/metrics.svelte.ts` | `$state` store + `EventSource` subscription |
| `ui/src/lib/theme.svelte.ts` | `$state` for dark/accent/density + `$derived` theme tokens |
| `ui/src/lib/components/shared/Card.svelte` | Card wrapper |
| `ui/src/lib/components/shared/Pill.svelte` | Status badge |
| `ui/src/lib/components/shared/SlimBar.svelte` | Horizontal progress bar |
| `ui/src/lib/components/Header.svelte` | Top bar |
| `ui/src/lib/components/Ticker.svelte` | 8-KPI strip |
| `ui/src/lib/components/RoutesTable.svelte` | Route ladder table |
| `ui/src/lib/components/side-rail/Engines.svelte` | Engine health + mini bars |
| `ui/src/lib/components/side-rail/Concurrency.svelte` | 64-slot semaphore grid |
| `ui/src/lib/components/side-rail/Batches.svelte` | Batch job list |
| `ui/src/lib/components/side-rail/Resources.svelte` | CPU + Memory bar charts |
| `ui/src/lib/components/ThroughputStrip.svelte` | RPS + p95 bar charts |
| `ui/src/lib/components/ActivityStrip.svelte` | Request + error logs |

### Modified
| File | Change |
|---|---|
| `crates/server/src/lib.rs` | `pub mod console_store;` |
| `crates/server/src/state.rs` | `pub console: Arc<ConsoleStore>` field |
| `crates/server/src/main.rs` | Spawn sampler task, create ConsoleStore |
| `crates/server/src/app.rs` | Mount console routes, add request log middleware |
| `crates/server/src/routes/mod.rs` | `pub mod console;` |
| `crates/server/src/supervised_engine.rs` | Add `pub fn is_running()` to both engines |
| `crates/server/Cargo.toml` | Add `rust-embed`, `mime_guess` |
| `ui/svelte.config.js` | Add `paths.base` + `fallback` |
| `ui/src/routes/layout.css` | Remap `--chart-1..5` to semantic colors |
| `ui/src/routes/+page.svelte` | Full dashboard layout |
| `Makefile` | `ui-build`, `ui-dev` targets |
| `Dockerfile` | Add `ui-builder` stage, copy `ui/build/` into Rust builder |

---

## Phase 1 — Rust Backend

---

### Task 1: `ConsoleStore` — ring buffers + broadcast channel

**Files:**
- Create: `crates/server/src/console_store.rs`
- Modify: `crates/server/src/lib.rs`
- Modify: `crates/server/src/state.rs`

- [ ] **Step 1: Create `console_store.rs`**

```rust
// crates/server/src/console_store.rs
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use serde::{Deserialize, Serialize};
use tokio::sync::{Mutex, broadcast};

pub const HISTORY_CAP: usize = 360;  // 30 min at 5s cadence
pub const LOG_CAP: usize = 100;
pub const BROADCAST_CAP: usize = 4;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MetricsSample {
    pub ts: u64,
    pub rps: f64,
    pub p95_ms: f64,
    pub error_pct: f64,
    pub queue_size: u32,
    pub concurrency_active: u32,
    pub cpu_pct: f64,
    pub memory_mb: f64,
}

#[derive(Debug, Default)]
pub struct MetricsHistory {
    pub samples: VecDeque<MetricsSample>,
}

impl MetricsHistory {
    pub fn push(&mut self, sample: MetricsSample) {
        if self.samples.len() >= HISTORY_CAP {
            self.samples.pop_front();
        }
        self.samples.push_back(sample);
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RequestLogEntry {
    pub time: String,
    pub method: String,
    pub route: String,
    pub status: u16,
    pub duration_ms: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ErrorLogEntry {
    pub time: String,
    pub route: String,
    pub message: String,
    pub request_id: String,
}

pub struct ConsoleStore {
    pub history: Mutex<MetricsHistory>,
    pub request_log: Mutex<VecDeque<RequestLogEntry>>,
    pub error_log: Mutex<VecDeque<ErrorLogEntry>>,
    pub broadcast: broadcast::Sender<String>,
    // Restart tracking (sampler detects false→true transition on is_running)
    pub chromium_restarts: AtomicU32,
    pub chromium_was_running: AtomicBool,
    pub libreoffice_restarts: AtomicU32,
    pub libreoffice_was_running: AtomicBool,
    // Rolling p95 approximation updated by request log middleware
    pub last_p95_ms: Mutex<f64>,
    // RPS delta tracking
    pub prev_http_total: Mutex<f64>,
}

impl ConsoleStore {
    pub fn new() -> Self {
        let (tx, _) = broadcast::channel(BROADCAST_CAP);
        Self {
            history: Mutex::new(MetricsHistory::default()),
            request_log: Mutex::new(VecDeque::new()),
            error_log: Mutex::new(VecDeque::new()),
            broadcast: tx,
            chromium_restarts: AtomicU32::new(0),
            chromium_was_running: AtomicBool::new(false),
            libreoffice_restarts: AtomicU32::new(0),
            libreoffice_was_running: AtomicBool::new(false),
            last_p95_ms: Mutex::new(0.0),
            prev_http_total: Mutex::new(0.0),
        }
    }

    pub async fn record_request(&self, method: String, route: String, status: u16, duration_ms: u64) {
        use chrono::Local;
        let time = Local::now().format("%H:%M:%S").to_string();

        {
            let mut log = self.request_log.lock().await;
            if log.len() >= LOG_CAP { log.pop_front(); }
            log.push_back(RequestLogEntry { time: time.clone(), method: method.clone(), route: route.clone(), status, duration_ms });
        }

        // Update p95 approximation (rolling max over recent requests)
        {
            let mut p95 = self.last_p95_ms.lock().await;
            if duration_ms as f64 > *p95 {
                *p95 = duration_ms as f64;
            }
        }

        if status >= 500 {
            let mut log = self.error_log.lock().await;
            if log.len() >= LOG_CAP { log.pop_front(); }
            log.push_back(ErrorLogEntry {
                time,
                route,
                message: format!("HTTP {status}"),
                request_id: String::new(),
            });
        }
    }
}
```

Note: `chrono` is not yet in the server crate. Use `std::time::SystemTime` instead:

```rust
// Replace the chrono import with this helper at top of record_request
use std::time::{SystemTime, UNIX_EPOCH};
let secs = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
let h = (secs % 86400) / 3600;
let m = (secs % 3600) / 60;
let s = secs % 60;
let time = format!("{h:02}:{m:02}:{s:02}");
```

- [ ] **Step 2: Add `pub mod console_store;` to `lib.rs`**

In `crates/server/src/lib.rs`, add after `pub mod batch_worker;`:
```rust
pub mod console_store;
```

- [ ] **Step 3: Add `console` field to `AppState`**

In `crates/server/src/state.rs`:

```rust
// Add import at top
use std::sync::Arc;
use crate::console_store::ConsoleStore;

// Add field to AppState struct
pub struct AppState {
    // ...existing fields...
    /// Shared operator console state (ring buffers + SSE broadcast).
    pub console: Arc<ConsoleStore>,
}

// In AppState::new(), add to the Self { } literal:
console: Arc::new(ConsoleStore::new()),
```

- [ ] **Step 4: Build to confirm it compiles**

```bash
cargo build -p server 2>&1 | head -30
```
Expected: compiles (may have unused warnings, that's fine)

- [ ] **Step 5: Commit**

```bash
git add crates/server/src/console_store.rs crates/server/src/lib.rs crates/server/src/state.rs
git commit -m "feat(console): add ConsoleStore with ring buffers and SSE broadcast channel"
```

---

### Task 2: Expose `is_running()` on supervised engines

**Files:**
- Modify: `crates/server/src/supervised_engine.rs`

The ConsoleStore sampler needs to detect when an engine restarts (false→true transition on `is_running`). The `is_running` field is private — expose it via a public method on both engine wrappers.

- [ ] **Step 1: Add `pub fn is_running()` to `SupervisedChromiumEngine`**

Find the `impl SupervisedChromiumEngine` block (around line 39). Add after the existing public methods:

```rust
/// Returns true if the Chromium engine is currently running.
pub fn is_running(&self) -> bool {
    self.inner.is_running.load(std::sync::atomic::Ordering::SeqCst)
}
```

- [ ] **Step 2: Add `pub fn is_running()` to `SupervisedLibreOfficeEngine`**

Find `impl SupervisedLibreOfficeEngine` (around line 228). Add the identical method:

```rust
/// Returns true if the LibreOffice engine is currently running.
pub fn is_running(&self) -> bool {
    self.inner.is_running.load(std::sync::atomic::Ordering::SeqCst)
}
```

- [ ] **Step 3: Build to confirm**

```bash
cargo build -p server 2>&1 | head -20
```
Expected: compiles cleanly.

- [ ] **Step 4: Commit**

```bash
git add crates/server/src/supervised_engine.rs
git commit -m "feat(console): expose is_running() on supervised engines for console sampler"
```

---

### Task 3: Background sampler task + `ConsolePayload` builder

**Files:**
- Modify: `crates/server/src/main.rs`
- Modify: `crates/server/src/console_store.rs`

The sampler runs every 5 seconds, builds the full `ConsolePayload` JSON, pushes a `MetricsSample` to history, and broadcasts the payload to all SSE subscribers.

- [ ] **Step 1: Add `ConsolePayload` + `build_console_payload()` to `console_store.rs`**

Append to `crates/server/src/console_store.rs`:

```rust
// ── ConsolePayload (the JSON shape sent to the frontend) ──────────────────

#[derive(Clone, Debug, Serialize)]
pub struct ConsolePayload {
    pub version: String,
    pub uptime_seconds: u64,
    pub ticker: TickerPayload,
    pub routes: Vec<RoutePayload>,
    pub engines: Vec<EnginePayload>,
    pub concurrency: ConcurrencyPayload,
    pub resources: ResourcesPayload,
    pub throughput: ThroughputPayload,
    pub batches: Vec<BatchPayload>,
    pub recent_requests: Vec<RequestLogEntry>,
    pub recent_errors: Vec<ErrorLogEntry>,
}

#[derive(Clone, Debug, Serialize)]
pub struct TickerPayload {
    pub rps: f64,
    pub p95_ms: f64,
    pub error_pct: f64,
    pub concurrency_active: u32,
    pub concurrency_max: u32,
    pub chromium_status: &'static str,
    pub chromium_restarts: u32,
    pub libreoffice_status: &'static str,
    pub libreoffice_restarts: u32,
    pub queue_size: f64,
    pub uptime_seconds: u64,
}

#[derive(Clone, Debug, Serialize)]
pub struct RoutePayload {
    pub path: String,
    pub method: &'static str,
    pub rps: f64,
    pub p50_ms: f64,
    pub p95_ms: f64,
    pub p99_ms: f64,
    pub error_pct: f64,
    pub in_flight: u32,
    pub load_pct: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct EnginePayload {
    pub name: &'static str,
    pub status: &'static str,
    pub restarts: u32,
    pub mode: &'static str,
    pub mini_series: Vec<f64>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ConcurrencyPayload {
    pub active: u32,
    pub max: u32,
    pub warn_threshold: u32,
    pub crit_threshold: u32,
}

#[derive(Clone, Debug, Serialize)]
pub struct ResourcesPayload {
    pub cpu_series: Vec<f64>,
    pub memory_series: Vec<f64>,
    pub memory_max_mb: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct ThroughputPayload {
    pub rps_series: Vec<f64>,
    pub rps_baseline: f64,
    pub p95_series: Vec<f64>,
    pub p95_target_s: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct BatchPayload {
    pub id: String,
    pub status: String,
    pub progress_pct: u8,
    pub elapsed: String,
}
```

- [ ] **Step 2: Add `build_console_payload()` function to `console_store.rs`**

This function is `async` because it reads from `Mutex`-protected fields in `ConsoleStore` and `AppState`.

Append to `crates/server/src/console_store.rs`:

```rust
use std::time::Instant;
use crate::state::AppState;
use crate::metrics::METRICS;
use prometheus::core::Collector;

/// Build the full console payload from current server state.
/// Called by the sampler task and by the one-shot JSON endpoint.
pub async fn build_console_payload(state: &AppState, started_at: Instant) -> ConsolePayload {
    let uptime_seconds = started_at.elapsed().as_secs();
    let concurrency_max = state.config.concurrency as u32;
    let concurrency_active = (concurrency_max as usize)
        .saturating_sub(state.sem.available_permits()) as u32;

    // Read history for series data
    let history = state.console.history.lock().await;
    let rps_series: Vec<f64> = history.samples.iter().map(|s| s.rps).collect();
    let p95_series: Vec<f64> = history.samples.iter().map(|s| s.p95_ms / 1000.0).collect();
    let cpu_series: Vec<f64> = history.samples.iter().map(|s| s.cpu_pct).collect();
    let memory_series: Vec<f64> = history.samples.iter().map(|s| s.memory_mb).collect();
    let last_rps = rps_series.last().copied().unwrap_or(0.0);
    let last_p95_ms = p95_series.last().copied().unwrap_or(0.0) * 1000.0;
    drop(history);

    // Queue size from Prometheus gauge
    let queue_size = METRICS.queue_size.get();
    let error_pct = {
        // Approximate: errors / total over last interval
        let series = state.console.history.lock().await;
        let s = series.samples.back();
        s.map_or(0.0, |s| s.error_pct)
    };

    // Engine status
    #[cfg(feature = "chromium")]
    let (chromium_status, chromium_restarts) = {
        let up = state.chromium.as_ref().map_or(false, |_| METRICS.chromium_healthy.get() > 0.5);
        let restarts = state.console.chromium_restarts.load(Ordering::SeqCst);
        (if up { "up" } else { "down" }, restarts)
    };
    #[cfg(not(feature = "chromium"))]
    let (chromium_status, chromium_restarts) = ("n/a", 0u32);

    #[cfg(feature = "libreoffice")]
    let (libreoffice_status, libreoffice_restarts) = {
        let up = METRICS.libreoffice_healthy.get() > 0.5;
        let restarts = state.console.libreoffice_restarts.load(Ordering::SeqCst);
        (if up { "up" } else { "down" }, restarts)
    };
    #[cfg(not(feature = "libreoffice"))]
    let (libreoffice_status, libreoffice_restarts) = ("n/a", 0u32);

    // Engine mini_series from history (last 20 concurrency samples as proxy)
    let mini: Vec<f64> = {
        let h = state.console.history.lock().await;
        h.samples.iter().rev().take(20).rev()
            .map(|s| s.concurrency_active as f64 / concurrency_max.max(1) as f64)
            .collect()
    };

    // Routes: derive from Prometheus metric families (best-effort V1)
    // We expose the known folio routes with data from the metrics we have.
    let routes = build_route_payloads(state, concurrency_max);

    // Recent requests + errors
    let recent_requests: Vec<RequestLogEntry> = {
        let log = state.console.request_log.lock().await;
        log.iter().rev().take(12).cloned().collect::<Vec<_>>().into_iter().rev().collect()
    };
    let recent_errors: Vec<ErrorLogEntry> = {
        let log = state.console.error_log.lock().await;
        log.iter().rev().take(6).cloned().collect::<Vec<_>>().into_iter().rev().collect()
    };

    // Batches from batch manager
    let batches = build_batch_payloads(state).await;

    // Memory
    let memory_max_mb = read_total_memory_mb();

    ConsolePayload {
        version: env!("CARGO_PKG_VERSION").to_string(),
        uptime_seconds,
        ticker: TickerPayload {
            rps: last_rps,
            p95_ms: last_p95_ms,
            error_pct,
            concurrency_active,
            concurrency_max,
            chromium_status,
            chromium_restarts,
            libreoffice_status,
            libreoffice_restarts,
            queue_size,
            uptime_seconds,
        },
        routes,
        engines: vec![
            EnginePayload {
                name: "Chromium",
                status: chromium_status,
                restarts: chromium_restarts,
                mode: if state.config.chromium_lazy_start { "lazy" } else { "eager" },
                mini_series: mini.clone(),
            },
            EnginePayload {
                name: "LibreOffice",
                status: libreoffice_status,
                restarts: libreoffice_restarts,
                mode: if state.config.libreoffice_lazy_start { "lazy" } else { "eager" },
                mini_series: mini,
            },
        ],
        concurrency: ConcurrencyPayload {
            active: concurrency_active,
            max: concurrency_max,
            warn_threshold: (concurrency_max as f64 * 0.60) as u32,
            crit_threshold: (concurrency_max as f64 * 0.85) as u32,
        },
        resources: ResourcesPayload { cpu_series, memory_series, memory_max_mb },
        throughput: ThroughputPayload {
            rps_series,
            rps_baseline: 0.0,
            p95_series,
            p95_target_s: 2.0,
        },
        batches,
        recent_requests,
        recent_errors,
    }
}

fn build_route_payloads(state: &AppState, concurrency_max: u32) -> Vec<RoutePayload> {
    // V1: expose known routes from Prometheus folio_http_requests_total counter.
    // Gather metric families to read per-route counts.
    use prometheus::proto::MetricType;
    let families = prometheus::gather();
    let mut routes = Vec::new();

    for family in &families {
        if family.get_name() != "folio_http_requests_total" { continue; }
        let mut route_map: std::collections::HashMap<String, (f64, f64)> = std::collections::HashMap::new();
        for m in family.get_metric() {
            let labels: std::collections::HashMap<_, _> = m.get_label().iter()
                .map(|l| (l.get_name(), l.get_value()))
                .collect();
            let route = labels.get("route").copied().unwrap_or("unknown");
            let status = labels.get("status").copied().unwrap_or("0");
            let count = m.get_counter().get_value();
            let entry = route_map.entry(route.to_string()).or_insert((0.0, 0.0));
            entry.0 += count; // total
            if status.starts_with('5') || status.starts_with('4') {
                entry.1 += count; // errors
            }
        }
        for (path, (total, errors)) in route_map {
            let error_pct = if total > 0.0 { (errors / total) * 100.0 } else { 0.0 };
            routes.push(RoutePayload {
                path,
                method: "POST",
                rps: 0.0,
                p50_ms: 0.0,
                p95_ms: 0.0,
                p99_ms: 0.0,
                error_pct,
                in_flight: 0,
                load_pct: (state.sem.available_permits() as f64 / concurrency_max.max(1) as f64 * 100.0),
            });
        }
        break;
    }
    routes.sort_by(|a, b| b.error_pct.partial_cmp(&a.error_pct).unwrap_or(std::cmp::Ordering::Equal));
    routes
}

async fn build_batch_payloads(state: &AppState) -> Vec<BatchPayload> {
    let Some(ref bm) = state.batch_manager else { return vec![] };
    bm.list_jobs().await.into_iter().take(6).map(|j| BatchPayload {
        id: j.id.chars().take(10).collect(),
        status: format!("{:?}", j.status).to_lowercase(),
        progress_pct: j.progress_pct.unwrap_or(0),
        elapsed: format_elapsed(j.created_at),
    }).collect()
}

fn format_elapsed(created: std::time::SystemTime) -> String {
    let secs = created.elapsed().unwrap_or_default().as_secs();
    if secs < 60 { format!("{}s", secs) }
    else { format!("{}m {}s", secs / 60, secs % 60) }
}

#[cfg(target_os = "linux")]
fn read_total_memory_mb() -> f64 {
    std::fs::read_to_string("/proc/meminfo").ok()
        .and_then(|s| s.lines().find(|l| l.starts_with("MemTotal:"))
            .and_then(|l| l.split_whitespace().nth(1))
            .and_then(|v| v.parse::<f64>().ok()))
        .map(|kb| kb / 1024.0)
        .unwrap_or(0.0)
}

#[cfg(not(target_os = "linux"))]
fn read_total_memory_mb() -> f64 { 0.0 }
```

- [ ] **Step 3: Add sampler function to `console_store.rs`**

Append to `crates/server/src/console_store.rs`:

```rust
use std::time::Duration;

/// Spawn the background console sampler task.
/// Runs every 5 seconds, pushes MetricsSample to history, broadcasts payload.
pub fn spawn_console_sampler(state: crate::state::AppState, started_at: Instant) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(5));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        let mut prev_http_total = 0.0_f64;

        loop {
            interval.tick().await;

            // Read current http total for RPS delta
            let http_total: f64 = {
                let families = prometheus::gather();
                families.iter()
                    .find(|f| f.get_name() == "folio_http_requests_total")
                    .map(|f| f.get_metric().iter().map(|m| m.get_counter().get_value()).sum())
                    .unwrap_or(0.0)
            };
            let rps = (http_total - prev_http_total) / 5.0;
            prev_http_total = http_total;

            // Read error rate
            let error_total: f64 = {
                let families = prometheus::gather();
                families.iter()
                    .find(|f| f.get_name() == "folio_http_requests_total")
                    .map(|f| f.get_metric().iter()
                        .filter(|m| m.get_label().iter()
                            .any(|l| l.get_name() == "status" && (l.get_value().starts_with('5') || l.get_value().starts_with('4'))))
                        .map(|m| m.get_counter().get_value()).sum())
                    .unwrap_or(0.0)
            };
            let error_pct = if http_total > 0.0 { (error_total / http_total) * 100.0 } else { 0.0 };

            // Concurrency
            let concurrency_max = state.config.concurrency as u32;
            let concurrency_active = (concurrency_max as usize)
                .saturating_sub(state.sem.available_permits()) as u32;

            // Memory
            let memory_mb = METRICS.process_resident_memory.get() / 1024.0 / 1024.0;
            METRICS.update_memory_metrics();

            let p95_ms = *state.console.last_p95_ms.lock().await;
            // Reset p95 approximation each interval
            *state.console.last_p95_ms.lock().await = 0.0;

            let sample = MetricsSample {
                ts: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs(),
                rps,
                p95_ms,
                error_pct,
                queue_size: METRICS.queue_size.get() as u32,
                concurrency_active,
                cpu_pct: 0.0, // not tracked in V1
                memory_mb,
            };

            state.console.history.lock().await.push(sample);

            // Detect engine restarts
            #[cfg(feature = "chromium")]
            {
                let now_running = state.chromium.as_ref().map_or(false, |_| METRICS.chromium_healthy.get() > 0.5);
                let was = state.console.chromium_was_running.load(Ordering::SeqCst);
                if now_running && !was {
                    state.console.chromium_restarts.fetch_add(1, Ordering::SeqCst);
                }
                state.console.chromium_was_running.store(now_running, Ordering::SeqCst);
            }
            #[cfg(feature = "libreoffice")]
            {
                let now_running = METRICS.libreoffice_healthy.get() > 0.5;
                let was = state.console.libreoffice_was_running.load(Ordering::SeqCst);
                if now_running && !was {
                    state.console.libreoffice_restarts.fetch_add(1, Ordering::SeqCst);
                }
                state.console.libreoffice_was_running.store(now_running, Ordering::SeqCst);
            }

            // Build + broadcast payload
            let payload = build_console_payload(&state, started_at).await;
            if let Ok(json) = serde_json::to_string(&payload) {
                let _ = state.console.broadcast.send(json); // ignore if no subscribers
            }
        }
    });
}
```

- [ ] **Step 4: Call `spawn_console_sampler` in `main.rs`**

In `crates/server/src/main.rs`, after the `let state = AppState::new(...)` block and before `let router = build_router(...)`:

```rust
// Spawn operator console sampler
use server::console_store::spawn_console_sampler;
let console_started_at = std::time::Instant::now();
spawn_console_sampler(state.clone(), console_started_at);
```

- [ ] **Step 5: Add `batch_manager` helper to BatchStateManager (if `list_jobs` doesn't exist)**

Check if `BatchStateManager` has a `list_jobs()` method:
```bash
grep -n "pub.*fn list\|pub.*fn jobs\|pub.*fn all" crates/server/src/routes/batch_state.rs | head -10
```

If it doesn't exist, use a simpler batch payload builder that returns empty (the console still works, just no batch data):
```rust
async fn build_batch_payloads(_state: &AppState) -> Vec<BatchPayload> {
    vec![] // V1: batch listing not yet exposed; add when batch_state has list_jobs()
}
```

- [ ] **Step 6: Build to confirm**

```bash
cargo build -p server 2>&1 | grep "error\|warning: unused" | head -30
```
Expected: compiles. Fix any type errors.

- [ ] **Step 7: Commit**

```bash
git add crates/server/src/console_store.rs crates/server/src/main.rs
git commit -m "feat(console): add ConsolePayload builder and background sampler task"
```

---

### Task 4: SSE + JSON endpoints (`routes/console.rs`)

**Files:**
- Create: `crates/server/src/routes/console.rs`
- Modify: `crates/server/src/routes/mod.rs`
- Modify: `crates/server/src/app.rs`

- [ ] **Step 1: Create `routes/console.rs`** (SSE + JSON handlers only; static assets in Task 6)

```rust
// crates/server/src/routes/console.rs
use std::convert::Infallible;
use std::time::Instant;

use axum::Json;
use axum::extract::State;
use axum::response::sse::{Event, KeepAlive, Sse};
use futures_util::stream::{self, Stream, StreamExt};
use tokio::sync::broadcast::error::RecvError;

use crate::console_store::{build_console_payload, ConsolePayload};
use crate::state::AppState;

/// SSE endpoint — streams ConsolePayload events to all connected browsers.
pub async fn console_stream(
    State(state): State<AppState>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let started_at = state.started_at; // Instant from AppState
    let mut rx = state.console.broadcast.subscribe();

    // Send initial snapshot on connect (no waiting for next tick)
    let initial = build_console_payload(&state, started_at).await;
    let initial_json = serde_json::to_string(&initial).unwrap_or_default();
    let initial_stream = stream::once(async move {
        Ok::<Event, Infallible>(Event::default().data(initial_json))
    });

    let broadcast_stream = stream::unfold(rx, |mut rx| async move {
        loop {
            match rx.recv().await {
                Ok(payload) => return Some((Ok(Event::default().data(payload)), rx)),
                Err(RecvError::Lagged(_)) => continue,
                Err(RecvError::Closed) => return None,
            }
        }
    });

    Sse::new(initial_stream.chain(broadcast_stream))
        .keep_alive(KeepAlive::new().interval(std::time::Duration::from_secs(15)).text("ping"))
}

/// One-shot JSON snapshot — same payload as SSE events, useful for curl/debug.
pub async fn console_metrics_json(
    State(state): State<AppState>,
) -> Json<ConsolePayload> {
    let started_at = state.started_at;
    Json(build_console_payload(&state, started_at).await)
}
```

Note: `state.started_at` is already on `AppState` (it's an `Instant` used for the `/health` uptime). Check:
```bash
grep "started_at" crates/server/src/state.rs
```
If it's not public, make it `pub` or pass it separately.

- [ ] **Step 2: Add `pub mod console;` to `routes/mod.rs`**

In `crates/server/src/routes/mod.rs`, add:
```rust
pub mod console;
```

- [ ] **Step 3: Mount console routes in `app.rs`**

In `crates/server/src/app.rs`, find the `use crate::routes::{...}` import block and add:
```rust
use crate::routes::console;
```

Then in the `untimed` route block (near the `/debug`, `/openapi.json`, `/docs` routes), add:
```rust
untimed = untimed
    .route("/_/api/stream",  get(console::console_stream))
    .route("/_/api/metrics", get(console::console_metrics_json));
// Static asset routes added in Task 6
```

- [ ] **Step 4: Build**

```bash
cargo build -p server 2>&1 | grep "^error" | head -20
```

- [ ] **Step 5: Smoke test the SSE endpoint**

In one terminal:
```bash
cargo run -p server -- serve --port 3001 &
sleep 3
```
In another:
```bash
curl -N http://localhost:3001/_/api/stream
```
Expected: a JSON `data: {...}` event printed every 5 seconds, then `data: ping` keep-alives.

```bash
curl http://localhost:3001/_/api/metrics | jq .uptime_seconds
```
Expected: a small number (seconds since start).

Kill the server: `pkill folio-server`

- [ ] **Step 6: Commit**

```bash
git add crates/server/src/routes/console.rs crates/server/src/routes/mod.rs crates/server/src/app.rs
git commit -m "feat(console): add SSE stream and one-shot metrics JSON endpoints"
```

---

### Task 5: Request log middleware

**Files:**
- Modify: `crates/server/src/app.rs`

Add a Tower middleware layer that captures every HTTP request/response pair and pushes it to `ConsoleStore`.

- [ ] **Step 1: Add middleware function to `app.rs`**

Add this function near the bottom of `app.rs` (before `handle_timeout_error`):

```rust
use axum::middleware::Next;
use axum::extract::Request;
use axum::response::Response;
use std::time::Instant;

async fn console_log_middleware(
    State(state): State<AppState>,
    req: Request,
    next: Next,
) -> Response {
    let method = req.method().to_string();
    let path = req.uri().path().to_string();

    // Skip the console routes themselves to avoid noise
    if path.starts_with("/_/") {
        return next.run(req).await;
    }

    let start = Instant::now();
    let response = next.run(req).await;
    let duration_ms = start.elapsed().as_millis() as u64;
    let status = response.status().as_u16();

    state.console.record_request(method, path, status, duration_ms).await;
    response
}
```

- [ ] **Step 2: Add the middleware layer in `build_router()`**

In `build_router()`, after all routes are defined and before the final `router` construction, add the middleware:

```rust
// Console request logger (wraps the entire router)
use axum::middleware;
let router = router.layer(middleware::from_fn_with_state(state.clone(), console_log_middleware));
```

Add this just before the `router` is returned (or before the auth layer wrapping, to ensure it runs for all routes).

- [ ] **Step 3: Build and verify no regressions**

```bash
cargo build -p server 2>&1 | grep "^error" | head -20
cargo test -p server --lib 2>&1 | tail -5
```
Expected: builds and unit tests pass.

- [ ] **Step 4: Commit**

```bash
git add crates/server/src/app.rs
git commit -m "feat(console): add request log middleware feeding ConsoleStore ring buffer"
```

---

### Task 6: Static asset serving with `rust-embed`

**Files:**
- Modify: `crates/server/Cargo.toml`
- Modify: `crates/server/src/routes/console.rs`
- Modify: `crates/server/src/app.rs`
- Create: `ui/build/.gitkeep`

- [ ] **Step 1: Create placeholder so rust-embed doesn't fail before UI is built**

```bash
mkdir -p ui/build
touch ui/build/.gitkeep
echo "build/" >> ui/.gitignore || true  # don't commit build output
echo "!build/.gitkeep" >> ui/.gitignore
```

- [ ] **Step 2: Add `rust-embed` and `mime_guess` to `crates/server/Cargo.toml`**

In the `[dependencies]` section:
```toml
rust-embed = { version = "8", features = ["mime-guess"] }
mime_guess = "2"
```

- [ ] **Step 3: Add `ConsoleAssets` struct and `console_asset` handler to `routes/console.rs`**

Add to `crates/server/src/routes/console.rs`:

```rust
use axum::body::Body;
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::IntoResponse;
use rust_embed::RustEmbed;

#[derive(RustEmbed)]
#[folder = "../../ui/build/"]
struct ConsoleAssets;

/// Serves the embedded Svelte SPA.
/// Path "" or "/" → index.html; everything else → matching asset or index.html fallback.
pub async fn console_asset(
    axum::extract::Path(path): axum::extract::Path<String>,
) -> impl IntoResponse {
    serve_asset(&path)
}

pub async fn console_asset_root() -> impl IntoResponse {
    serve_asset("index.html")
}

fn serve_asset(path: &str) -> impl IntoResponse {
    let path = path.trim_start_matches('/');
    let asset = ConsoleAssets::get(path)
        .or_else(|| ConsoleAssets::get("index.html"));

    match asset {
        Some(content) => {
            let mime = mime_guess::from_path(path).first_or_octet_stream();
            let body = Body::from(content.data.into_owned());
            (
                StatusCode::OK,
                [(header::CONTENT_TYPE, HeaderValue::from_str(mime.as_ref()).unwrap())],
                body,
            ).into_response()
        }
        None => StatusCode::NOT_FOUND.into_response(),
    }
}
```

- [ ] **Step 4: Mount the asset routes in `app.rs`**

In the `untimed` route block (alongside the SSE routes added in Task 4):

```rust
untimed = untimed
    .route("/_/",        get(console::console_asset_root))
    .route("/_/{*path}", get(console::console_asset));
```

- [ ] **Step 5: Build**

```bash
cargo build -p server 2>&1 | grep "^error" | head -20
```
Expected: compiles. The `ui/build/` folder exists (even if empty) so rust-embed doesn't panic.

- [ ] **Step 6: Commit**

```bash
git add ui/build/.gitkeep ui/.gitignore crates/server/Cargo.toml crates/server/src/routes/console.rs crates/server/src/app.rs
git commit -m "feat(console): serve embedded Svelte SPA at /_/ via rust-embed"
```

---

### Task 7: Makefile + Docker integration

**Files:**
- Modify: `Makefile`
- Modify: `Dockerfile`

- [ ] **Step 1: Add `ui-build` and `ui-dev` targets to `Makefile`**

Add after the existing `build-release` target:

```makefile
.PHONY: ui-build
ui-build: ## Build the operator console UI (requires Node/bun in ui/)
	cd ui && npm run build

.PHONY: ui-dev
ui-dev: ## Start UI dev server with hot reload (run alongside folio-server)
	cd ui && npm run dev

.PHONY: build-with-ui
build-with-ui: ui-build build-release ## Build UI then Rust binary (for local testing)
```

- [ ] **Step 2: Add `ui-builder` stage to `Dockerfile`**

At the very top of the `Dockerfile`, before the `chef` stage, add:

```dockerfile
# =============================================================================
# Stage: ui-builder — builds the operator console SPA
# =============================================================================
FROM node:22-slim AS ui-builder
WORKDIR /ui
COPY ui/package*.json ui/bun.lock* ./
RUN npm install
COPY ui/ ./
RUN npm run build
```

Then in each `builder-full`, `builder-chromium`, `builder-libreoffice` stage, after the `COPY --link . .` line and before the `cargo build` line, add:

```dockerfile
COPY --link --from=ui-builder /ui/build /app/ui/build
```

This ensures `ui/build/` exists when `cargo build` runs, so rust-embed can embed the real assets.

- [ ] **Step 3: Build the UI and verify the binary serves it**

```bash
make ui-build
cargo build -p server
cargo run -p server -- serve --port 3001 &
sleep 3
curl -I http://localhost:3001/_/
```
Expected: `HTTP/1.1 200 OK` with `Content-Type: text/html`.

```bash
curl http://localhost:3001/_/api/metrics | jq .version
```
Expected: `"0.1.0"`.

Kill: `pkill folio-server`

- [ ] **Step 4: Commit**

```bash
git add Makefile Dockerfile
git commit -m "feat(console): add ui-build Makefile target and ui-builder Docker stage"
```

---

## Phase 2 — Svelte Frontend

---

### Task 8: SvelteKit config + chart setup

**Files:**
- Modify: `ui/svelte.config.js`
- Modify: `ui/src/routes/layout.css`
- Run: `npx shadcn-svelte@latest add chart`

- [ ] **Step 1: Update `svelte.config.js` — add base path and SPA fallback**

```javascript
// ui/svelte.config.js
import adapter from '@sveltejs/adapter-static';

const config = {
    compilerOptions: {
        runes: ({ filename }) => (filename.split(/[/\\]/).includes('node_modules') ? undefined : true)
    },
    kit: {
        adapter: adapter({ fallback: 'index.html' }),
        paths: { base: '/_' },
    }
};

export default config;
```

- [ ] **Step 2: Run SvelteKit sync to update generated types**

```bash
cd ui && npm run prepare
```

- [ ] **Step 3: Remap chart CSS variables in `layout.css`**

Find the `:root` block in `ui/src/routes/layout.css` and replace the `--chart-*` lines:

```css
/* Replace the 5 chart lines in :root with: */
--chart-1: #4f8ef7;   /* accent blue */
--chart-2: #2f9967;   /* ok green */
--chart-3: #b8860b;   /* warn amber */
--chart-4: #c25151;   /* err red */
--chart-5: rgba(26,28,31,0.4); /* muted */
```

And in the `.dark` block (or `@media (prefers-color-scheme: dark)`, or `.dark *`):
```css
--chart-1: #6aa3f8;
--chart-2: #3fb27f;
--chart-3: #e0a93c;
--chart-4: #e26464;
--chart-5: rgba(230,231,234,0.4);
```

- [ ] **Step 4: Install shadcn-svelte chart component**

```bash
cd ui && npx shadcn-svelte@latest add chart
```
Expected: installs `src/lib/components/ui/chart/` with `ChartContainer`, `ChartTooltip`, `ChartLegend`, `index.ts`.

- [ ] **Step 5: Verify build still works**

```bash
cd ui && npm run build 2>&1 | tail -10
```
Expected: `✓ built in Xs`.

- [ ] **Step 6: Commit**

```bash
git add ui/svelte.config.js ui/src/routes/layout.css ui/src/lib/components/ui/
git commit -m "feat(console-ui): configure base path, remap chart colors, install shadcn chart"
```

---

### Task 9: Types + SSE store + theme store

**Files:**
- Create: `ui/src/lib/types.ts`
- Create: `ui/src/lib/metrics.svelte.ts`
- Create: `ui/src/lib/theme.svelte.ts`

- [ ] **Step 1: Create `types.ts`** — mirrors `ConsolePayload` Rust struct exactly

```typescript
// ui/src/lib/types.ts
export interface MetricsSample {
    ts: number;
    rps: number;
    p95_ms: number;
    error_pct: number;
    queue_size: number;
    concurrency_active: number;
    cpu_pct: number;
    memory_mb: number;
}

export interface TickerPayload {
    rps: number;
    p95_ms: number;
    error_pct: number;
    concurrency_active: number;
    concurrency_max: number;
    chromium_status: string;
    chromium_restarts: number;
    libreoffice_status: string;
    libreoffice_restarts: number;
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
}

export interface ConcurrencyPayload {
    active: number;
    max: number;
    warn_threshold: number;
    crit_threshold: number;
}

export interface ResourcesPayload {
    cpu_series: number[];
    memory_series: number[];
    memory_max_mb: number;
}

export interface ThroughputPayload {
    rps_series: number[];
    rps_baseline: number;
    p95_series: number[];
    p95_target_s: number;
}

export interface BatchPayload {
    id: string;
    status: string;
    progress_pct: number;
    elapsed: string;
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

- [ ] **Step 2: Create `metrics.svelte.ts`**

```typescript
// ui/src/lib/metrics.svelte.ts
import type { ConsolePayload } from './types';

export let data = $state<ConsolePayload | null>(null);
export let loading = $state(true);
export let connected = $state(false);
export let error = $state<string | null>(null);
export let lastRefreshed = $state<Date | null>(null);

let es: EventSource | null = null;

export function startSSE() {
    if (es) return;
    es = new EventSource('/_/api/stream');

    es.onopen = () => {
        connected = true;
        error = null;
    };

    es.onmessage = (event: MessageEvent) => {
        try {
            data = JSON.parse(event.data) as ConsolePayload;
            lastRefreshed = new Date();
            error = null;
        } catch {
            error = 'Failed to parse server data';
        } finally {
            loading = false;
        }
    };

    es.onerror = () => {
        connected = false;
        error = 'Connection lost — reconnecting…';
    };
}

export function stopSSE() {
    es?.close();
    es = null;
    connected = false;
}

export function manualRefresh() {
    stopSSE();
    startSSE();
}
```

- [ ] **Step 3: Create `theme.svelte.ts`**

```typescript
// ui/src/lib/theme.svelte.ts
export let dark = $state(false);
export let accent = $state('#4f8ef7');
export let density = $state<'compact' | 'regular' | 'comfy'>('regular');

export type Theme = typeof theme;

export let theme = $derived({
    bg:       dark ? '#0e0f12' : '#f7f7f5',
    surface:  dark ? '#15171c' : '#ffffff',
    ink:      dark ? '#e6e7ea' : '#1a1c1f',
    muted:    dark ? 'rgba(230,231,234,0.55)' : 'rgba(26,28,31,0.55)',
    faint:    dark ? 'rgba(230,231,234,0.10)' : 'rgba(26,28,31,0.06)',
    rule:     dark ? 'rgba(255,255,255,0.08)' : 'rgba(26,28,31,0.08)',
    ok:       dark ? '#3fb27f' : '#2f9967',
    warn:     dark ? '#e0a93c' : '#b8860b',
    err:      dark ? '#e26464' : '#c25151',
    accent,
});

export let D = $derived(
    density === 'compact'
        ? { gap: 8,  pad: 8,  rowPy: 2, fz: 10.5, kpiFz: 18 }
        : density === 'comfy'
            ? { gap: 14, pad: 14, rowPy: 5, fz: 12,   kpiFz: 22 }
            : { gap: 10, pad: 10, rowPy: 3, fz: 11.5, kpiFz: 20 }
);

// Persist dark mode in localStorage
if (typeof window !== 'undefined') {
    dark = localStorage.getItem('folio-dark') === 'true';
}

$effect.root(() => {
    $effect(() => {
        if (typeof window !== 'undefined') {
            localStorage.setItem('folio-dark', String(dark));
            document.documentElement.classList.toggle('dark', dark);
        }
    });
});
```

- [ ] **Step 4: Check TypeScript compiles**

```bash
cd ui && npm run check 2>&1 | grep -E "error|Error" | head -20
```
Expected: no errors.

- [ ] **Step 5: Commit**

```bash
git add ui/src/lib/types.ts ui/src/lib/metrics.svelte.ts ui/src/lib/theme.svelte.ts
git commit -m "feat(console-ui): add ConsolePayload types, SSE store, and theme store"
```

---

### Task 10: Shared primitives — `Card`, `Pill`, `SlimBar`

**Files:**
- Create: `ui/src/lib/components/shared/Card.svelte`
- Create: `ui/src/lib/components/shared/Pill.svelte`
- Create: `ui/src/lib/components/shared/SlimBar.svelte`

- [ ] **Step 1: Create `Card.svelte`**

```svelte
<!-- ui/src/lib/components/shared/Card.svelte -->
<script lang="ts">
    import type { Theme } from '$lib/theme.svelte';

    let { title, sub, t, style = '', children }: {
        title?: string;
        sub?: string;
        t: Theme;
        style?: string;
        children: import('svelte').Snippet;
    } = $props();
</script>

<div style="background:{t.surface};border:1px solid {t.rule};border-radius:12px;overflow:hidden;{style}">
    {#if title}
        <div style="display:flex;align-items:baseline;gap:8px;padding:10px 14px;border-bottom:1px solid {t.rule}">
            <span style="font-size:11.5px;font-weight:600;letter-spacing:0.02em;color:{t.ink}">{title}</span>
            {#if sub}<span style="font-size:10.5px;color:{t.muted}">{sub}</span>{/if}
        </div>
    {/if}
    {@render children()}
</div>
```

- [ ] **Step 2: Create `Pill.svelte`**

```svelte
<!-- ui/src/lib/components/shared/Pill.svelte -->
<script lang="ts">
    import type { Theme } from '$lib/theme.svelte';

    let { tone = 'ink', t, children }: {
        tone?: 'ok' | 'warn' | 'err' | 'accent' | 'ink';
        t: Theme;
        children: import('svelte').Snippet;
    } = $props();

    let color = $derived(t[tone as keyof Theme] as string ?? t.ink);
</script>

<span style="
    color:{color};
    background:{color}22;
    padding:1px 7px;
    font-family:ui-monospace,monospace;
    font-size:10px;
    font-weight:600;
    border-radius:999px;
    letter-spacing:0.04em;
    display:inline-block;
    line-height:16px;
    text-transform:uppercase;
">
    {@render children()}
</span>
```

- [ ] **Step 3: Create `SlimBar.svelte`**

```svelte
<!-- ui/src/lib/components/shared/SlimBar.svelte -->
<script lang="ts">
    import type { Theme } from '$lib/theme.svelte';

    let { pct, t, h = 6 }: { pct: number; t: Theme; h?: number } = $props();
    let color = $derived(pct > 85 ? t.err : pct > 60 ? t.warn : t.ok);
</script>

<div style="position:relative;height:{h}px;background:{t.faint};border-radius:999px;overflow:hidden">
    <div style="height:100%;width:{Math.min(100, pct)}%;background:{color};border-radius:999px;transition:width 0.4s ease"></div>
</div>
```

- [ ] **Step 4: Check types**

```bash
cd ui && npm run check 2>&1 | grep "error" | head -10
```

- [ ] **Step 5: Commit**

```bash
git add ui/src/lib/components/shared/
git commit -m "feat(console-ui): add Card, Pill, SlimBar shared primitives"
```

---

### Task 11: `Header.svelte` + `Ticker.svelte`

**Files:**
- Create: `ui/src/lib/components/Header.svelte`
- Create: `ui/src/lib/components/Ticker.svelte`

- [ ] **Step 1: Create `Header.svelte`**

```svelte
<!-- ui/src/lib/components/Header.svelte -->
<script lang="ts">
    import type { ConsolePayload } from '$lib/types';
    import type { Theme } from '$lib/theme.svelte';
    import Pill from './shared/Pill.svelte';
    import { lastRefreshed, manualRefresh, connected } from '$lib/metrics.svelte';

    let { data, t }: { data: ConsolePayload; t: Theme } = $props();

    function formatUptime(s: number) {
        const h = Math.floor(s / 3600);
        const m = Math.floor((s % 3600) / 60);
        return `${h}h ${m}m`;
    }

    let refreshed = $derived(lastRefreshed
        ? `${lastRefreshed.toLocaleTimeString('en-GB')} UTC · refreshed just now`
        : 'connecting…'
    );
</script>

<div style="background:{t.surface};border:1px solid {t.rule};border-radius:12px;padding:8px 14px;display:flex;align-items:center;gap:12px;font-size:11.5px">
    <span style="font-weight:700;font-size:14px;letter-spacing:-0.01em">Folio</span>
    <span style="color:{t.muted};font-family:ui-monospace,monospace;font-size:10.5px">v{data.version}</span>
    <Pill tone="accent" {t}>prod</Pill>
    <Pill tone={connected ? 'ok' : 'err'} {t}>● {connected ? 'ok' : 'disconnected'}</Pill>
    <span style="flex:1"></span>
    <span style="color:{t.muted};font-family:ui-monospace,monospace;font-size:10.5px">{refreshed}</span>
    <button
        onclick={manualRefresh}
        style="border:1px solid {t.rule};background:transparent;color:{t.ink};padding:3px 9px;border-radius:7px;font-family:ui-monospace,monospace;font-size:10.5px;cursor:pointer"
    >
        r refresh
    </button>
</div>
```

- [ ] **Step 2: Create `Ticker.svelte`**

```svelte
<!-- ui/src/lib/components/Ticker.svelte -->
<script lang="ts">
    import type { TickerPayload } from '$lib/types';
    import type { Theme } from '$lib/theme.svelte';
    import Pill from './shared/Pill.svelte';

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
        { label: 'RPS',      value: ticker.rps.toFixed(1),              tone: 'ink' as const },
        { label: 'p95',      value: fmtMs(ticker.p95_ms),               tone: (ticker.p95_ms > 2000 ? 'err' : ticker.p95_ms > 1500 ? 'warn' : 'ok') as 'ok' | 'warn' | 'err' },
        { label: 'Errors',   value: `${ticker.error_pct.toFixed(2)}%`,   tone: (ticker.error_pct > 1 ? 'err' : ticker.error_pct > 0.5 ? 'warn' : 'ok') as 'ok' | 'warn' | 'err' },
        { label: 'Conc.',    value: `${ticker.concurrency_active} / ${ticker.concurrency_max}`, tone: 'ink' as const },
        { label: 'Chromium', value: ticker.chromium_status.toUpperCase(), tone: (ticker.chromium_status === 'up' ? 'ok' : 'err') as 'ok' | 'err' },
        { label: 'LibreOff', value: ticker.libreoffice_status.toUpperCase(), tone: (ticker.libreoffice_status === 'up' ? 'ok' : 'err') as 'ok' | 'err' },
        { label: 'Queue',    value: String(ticker.queue_size),            tone: 'ink' as const },
        { label: 'Uptime',   value: fmtUptime(ticker.uptime_seconds),    tone: 'ok' as const },
    ]);
</script>

<div style="background:{t.surface};border:1px solid {t.rule};border-radius:12px;display:grid;grid-template-columns:repeat({items.length},1fr)">
    {#each items as item, i}
        <div style="padding:{D.pad}px {D.pad + 2}px;{i < items.length - 1 ? `border-right:1px solid ${t.rule}` : ''}">
            <div style="color:{t.muted};font-size:10px;letter-spacing:0.06em;text-transform:uppercase;font-weight:500">{item.label}</div>
            <div style="font-family:ui-monospace,monospace;font-size:{D.kpiFz}px;font-weight:600;margin-top:2px;letter-spacing:-0.01em">{item.value}</div>
        </div>
    {/each}
</div>
```

- [ ] **Step 3: Type-check**

```bash
cd ui && npm run check 2>&1 | grep "error" | head -10
```

- [ ] **Step 4: Commit**

```bash
git add ui/src/lib/components/Header.svelte ui/src/lib/components/Ticker.svelte
git commit -m "feat(console-ui): add Header and Ticker components"
```

---

### Task 12: `RoutesTable.svelte`

**Files:**
- Create: `ui/src/lib/components/RoutesTable.svelte`

- [ ] **Step 1: Create `RoutesTable.svelte`**

```svelte
<!-- ui/src/lib/components/RoutesTable.svelte -->
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

<Card {t} title="Routes" sub="{routes.length} endpoints · sorted by p95 desc">
    <table style="width:100%;border-collapse:collapse;font-family:ui-monospace,monospace;font-size:{D.fz}px">
        <thead>
            <tr>
                {#each ['Route','Method','RPS','p50','p95','p99','Err %','In-flight','Load'] as h, i}
                    <th style="padding:{D.rowPy + 4}px {D.pad + 2}px {D.rowPy + 2}px;text-align:{i < 2 ? 'left' : 'right'};font-weight:500;font-size:10px;letter-spacing:0.04em;color:{t.muted};text-transform:uppercase;border-bottom:1px solid {t.rule}">{h}</th>
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
</Card>
```

- [ ] **Step 2: Check + commit**

```bash
cd ui && npm run check 2>&1 | grep "error" | head -10
git add ui/src/lib/components/RoutesTable.svelte
git commit -m "feat(console-ui): add RoutesTable component"
```

---

### Task 13: Side rail — Engines, Concurrency, Batches, Resources

**Files:**
- Create: `ui/src/lib/components/side-rail/Engines.svelte`
- Create: `ui/src/lib/components/side-rail/Concurrency.svelte`
- Create: `ui/src/lib/components/side-rail/Batches.svelte`
- Create: `ui/src/lib/components/side-rail/Resources.svelte`

- [ ] **Step 1: Create `Engines.svelte`**

Uses shadcn `BarChart` for the engine mini sparklines.

```svelte
<!-- ui/src/lib/components/side-rail/Engines.svelte -->
<script lang="ts">
    import type { EnginePayload } from '$lib/types';
    import type { Theme } from '$lib/theme.svelte';
    import Card from '../shared/Card.svelte';
    import Pill from '../shared/Pill.svelte';
    import { BarChart, Bar, ChartContainer } from '$lib/components/ui/chart/index.js';

    let { engines, t, D }: { engines: EnginePayload[]; t: Theme; D: { fz: number; pad: number } } = $props();

    function engineTone(e: EnginePayload): 'ok' | 'warn' | 'err' {
        if (e.status !== 'up') return 'err';
        if (e.restarts > 5) return 'warn';
        return 'ok';
    }
</script>

<Card {t} title="Engines">
    <div style="padding:{D.pad}px;font-size:{D.fz}px">
        {#each engines as e, i}
            <div style="display:grid;grid-template-columns:1fr auto;align-items:center;{i === 0 ? `border-bottom:1px solid ${t.rule}` : ''};padding:{D.pad - 4}px 0">
                <div>
                    <div style="display:flex;align-items:center;gap:6px">
                        <strong style="font-size:{D.fz + 0.5}px">{e.name}</strong>
                        <Pill tone={engineTone(e)} {t}>{e.status.toUpperCase()}</Pill>
                    </div>
                    <div style="color:{t.muted};font-size:10.5px;margin-top:2px;font-family:ui-monospace,monospace">
                        {e.restarts} restarts · {e.mode}
                    </div>
                </div>
                <!-- Mini bar chart using shadcn chart -->
                {#if e.mini_series.length > 0}
                    {@const chartData = e.mini_series.map((v, idx) => ({ i: idx, v }))}
                    {@const chartConfig = { v: { color: `var(--chart-${engineTone(e) === 'ok' ? 2 : engineTone(e) === 'warn' ? 3 : 4})` } }}
                    <ChartContainer config={chartConfig} class="h-7 w-[70px]">
                        <BarChart data={chartData} dataKey="i">
                            <Bar dataKey="v" fill="var(--color-v)" radius={1} />
                        </BarChart>
                    </ChartContainer>
                {/if}
            </div>
        {/each}
    </div>
</Card>
```

- [ ] **Step 2: Create `Concurrency.svelte`**

```svelte
<!-- ui/src/lib/components/side-rail/Concurrency.svelte -->
<script lang="ts">
    import type { ConcurrencyPayload } from '$lib/types';
    import type { Theme } from '$lib/theme.svelte';
    import Card from '../shared/Card.svelte';
    import Pill from '../shared/Pill.svelte';

    let { conc, t, D }: { conc: ConcurrencyPayload; t: Theme; D: { pad: number; fz: number } } = $props();

    let tone = $derived(conc.active >= conc.crit_threshold ? 'err' : conc.active >= conc.warn_threshold ? 'warn' : 'ok') as 'ok' | 'warn' | 'err';
    let pct = $derived(Math.round((conc.active / conc.max) * 100));

    function slotColor(i: number): string {
        const filled = i < conc.active;
        if (!filled) return t.faint;
        if (i >= conc.crit_threshold) return t.err;
        if (i >= conc.warn_threshold) return t.warn;
        return t.ok;
    }
</script>

<Card {t} title="Concurrency" sub="semaphore · {conc.max} slots">
    <div style="padding:{D.pad}px;font-size:{D.fz}px">
        <div style="display:flex;align-items:baseline;justify-content:space-between">
            <div style="font-family:ui-monospace,monospace;font-size:26px;font-weight:600;letter-spacing:-0.01em">
                {conc.active}<span style="color:{t.muted};font-weight:400"> / {conc.max}</span>
            </div>
            <Pill {tone} {t}>{pct}% · {tone}</Pill>
        </div>
        <div style="display:grid;grid-template-columns:repeat(32,1fr);gap:1px;margin-top:8px">
            {#each Array.from({ length: conc.max }, (_, i) => i) as i}
                <div style="height:14px;background:{slotColor(i)};border-radius:2px"></div>
            {/each}
        </div>
        <div style="display:flex;justify-content:space-between;margin-top:6px;font-family:ui-monospace,monospace;font-size:10px;color:{t.muted}">
            <span>0</span><span>warn {conc.warn_threshold}</span><span>crit {conc.crit_threshold}</span><span>{conc.max}</span>
        </div>
    </div>
</Card>
```

- [ ] **Step 3: Create `Batches.svelte`**

```svelte
<!-- ui/src/lib/components/side-rail/Batches.svelte -->
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
</script>

<Card {t} title="Batches" sub="{batches.filter(b => b.status === 'running').length} active">
    {#if batches.length === 0}
        <div style="padding:{D.pad}px;color:{t.muted};font-size:{D.fz}px">No recent batches</div>
    {:else}
        <table style="width:100%;border-collapse:collapse;font-family:ui-monospace,monospace;font-size:{D.fz - 0.5}px">
            <tbody>
                {#each batches as b, i}
                    <tr style="{i < batches.length - 1 ? `border-bottom:1px solid ${t.rule}` : ''}">
                        <td style="padding:{D.rowPy + 1}px {D.pad}px">{b.id}</td>
                        <td style="padding:{D.rowPy + 1}px 4px"><Pill tone={batchTone(b.status)} {t}>{b.status.slice(0,4)}</Pill></td>
                        <td style="padding:{D.rowPy + 1}px 4px;width:70px"><SlimBar pct={b.progress_pct} {t} h={4} /></td>
                        <td style="padding:{D.rowPy + 1}px {D.pad}px;text-align:right;color:{t.muted}">{b.elapsed}</td>
                    </tr>
                {/each}
            </tbody>
        </table>
    {/if}
</Card>
```

- [ ] **Step 4: Create `Resources.svelte`**

```svelte
<!-- ui/src/lib/components/side-rail/Resources.svelte -->
<script lang="ts">
    import type { ResourcesPayload } from '$lib/types';
    import type { Theme } from '$lib/theme.svelte';
    import Card from '../shared/Card.svelte';
    import { BarChart, Bar, ReferenceLine, ChartContainer, ChartTooltip, ChartTooltipContent } from '$lib/components/ui/chart/index.js';

    let { resources, t, D }: { resources: ResourcesPayload; t: Theme; D: { pad: number } } = $props();

    let cpuData = $derived(resources.cpu_series.map((v, i) => ({ i, v })));
    let memData = $derived(resources.memory_series.map((v, i) => ({ i, v })));
    let lastCpu = $derived(resources.cpu_series.at(-1) ?? 0);
    let lastMem = $derived(resources.memory_series.at(-1) ?? 0);

    const cpuConfig = { v: { label: 'CPU', color: 'var(--chart-1)' } };
    const memConfig = { v: { label: 'Memory', color: 'var(--chart-2)' } };
</script>

<Card {t} title="Resources" sub="last 30 min">
    <div style="padding:{D.pad + 2}px;display:flex;flex-direction:column;gap:10px">
        <!-- CPU -->
        <div>
            <div style="display:flex;justify-content:space-between;font-size:10.5px;color:{t.muted};margin-bottom:4px">
                <span>CPU</span>
                <span style="font-family:ui-monospace,monospace;color:{t.ink};font-weight:600">{lastCpu.toFixed(0)}<span style="color:{t.muted};font-weight:400">%</span></span>
            </div>
            <ChartContainer config={cpuConfig} class="h-14 w-full">
                <BarChart data={cpuData} dataKey="i">
                    <Bar dataKey="v" fill="var(--color-v)" radius={1.5} />
                    <ReferenceLine y={85} stroke="var(--chart-3)" strokeDasharray="3 3" strokeOpacity={0.55} />
                    <ChartTooltip content={ChartTooltipContent} cursor={false} />
                </BarChart>
            </ChartContainer>
        </div>
        <div style="height:1px;background:{t.rule}"></div>
        <!-- Memory -->
        <div>
            <div style="display:flex;justify-content:space-between;font-size:10.5px;color:{t.muted};margin-bottom:4px">
                <span>Memory</span>
                <span style="font-family:ui-monospace,monospace;color:{t.ink};font-weight:600">
                    {(lastMem / 1024).toFixed(2)}<span style="color:{t.muted};font-weight:400"> GB{resources.memory_max_mb > 0 ? ` / ${(resources.memory_max_mb / 1024).toFixed(0)} GB` : ''}</span>
                </span>
            </div>
            <ChartContainer config={memConfig} class="h-14 w-full">
                <BarChart data={memData} dataKey="i">
                    <Bar dataKey="v" fill="var(--color-v)" radius={1.5} />
                    <ChartTooltip content={ChartTooltipContent} cursor={false} />
                </BarChart>
            </ChartContainer>
        </div>
    </div>
</Card>
```

- [ ] **Step 5: Check types**

```bash
cd ui && npm run check 2>&1 | grep "error" | head -20
```
Fix any import paths (shadcn chart exports vary — check the generated `$lib/components/ui/chart/index.ts` for actual export names).

- [ ] **Step 6: Commit**

```bash
git add ui/src/lib/components/side-rail/
git commit -m "feat(console-ui): add side rail — Engines, Concurrency, Batches, Resources"
```

---

### Task 14: `ThroughputStrip.svelte` + `ActivityStrip.svelte`

**Files:**
- Create: `ui/src/lib/components/ThroughputStrip.svelte`
- Create: `ui/src/lib/components/ActivityStrip.svelte`

- [ ] **Step 1: Create `ThroughputStrip.svelte`**

```svelte
<!-- ui/src/lib/components/ThroughputStrip.svelte -->
<script lang="ts">
    import type { ThroughputPayload } from '$lib/types';
    import type { Theme } from '$lib/theme.svelte';
    import Card from './shared/Card.svelte';
    import { BarChart, Bar, ReferenceLine, ChartContainer, ChartTooltip, ChartTooltipContent } from '$lib/components/ui/chart/index.js';

    let { throughput, t, D }: { throughput: ThroughputPayload; t: Theme; D: { pad: number } } = $props();

    let rpsData  = $derived(throughput.rps_series.map((v, i) => ({ i, v })));
    let p95Data  = $derived(throughput.p95_series.map((v, i) => ({ i, v })));
    let lastRps  = $derived(throughput.rps_series.at(-1) ?? 0);
    let lastP95  = $derived(throughput.p95_series.at(-1) ?? 0);

    const rpsConfig = { v: { label: 'RPS',  color: 'var(--chart-1)' } };
    const p95Config = { v: { label: 'p95s', color: 'var(--chart-3)' } };
</script>

<div style="display:grid;grid-template-columns:1fr 1fr;gap:{D.pad}px">
    <Card {t} title="Requests / sec" sub="last 30 min{throughput.rps_baseline > 0 ? ` · baseline ${throughput.rps_baseline.toFixed(0)}` : ''}">
        <div style="padding:{D.pad + 2}px">
            <div style="display:flex;justify-content:space-between;font-size:10.5px;color:{t.muted};margin-bottom:4px">
                <span>RPS</span>
                <span style="font-family:ui-monospace,monospace;color:{t.ink};font-weight:600">{lastRps.toFixed(1)}</span>
            </div>
            <ChartContainer config={rpsConfig} class="h-22 w-full">
                <BarChart data={rpsData} dataKey="i">
                    <Bar dataKey="v" fill="var(--color-v)" radius={1.5} />
                    {#if throughput.rps_baseline > 0}
                        <ReferenceLine y={throughput.rps_baseline} stroke="var(--chart-3)" strokeDasharray="3 3" strokeOpacity={0.55} />
                    {/if}
                    <ChartTooltip content={ChartTooltipContent} cursor={false} />
                </BarChart>
            </ChartContainer>
        </div>
    </Card>

    <Card {t} title="Latency p95" sub="seconds · target < {throughput.p95_target_s}s">
        <div style="padding:{D.pad + 2}px">
            <div style="display:flex;justify-content:space-between;font-size:10.5px;color:{t.muted};margin-bottom:4px">
                <span>p95</span>
                <span style="font-family:ui-monospace,monospace;color:{t.ink};font-weight:600">{lastP95.toFixed(2)}<span style="color:{t.muted};font-weight:400">s</span></span>
            </div>
            <ChartContainer config={p95Config} class="h-22 w-full">
                <BarChart data={p95Data} dataKey="i">
                    <Bar dataKey="v" fill="var(--color-v)" radius={1.5} />
                    <ReferenceLine y={throughput.p95_target_s} stroke="var(--chart-4)" strokeDasharray="3 3" strokeOpacity={0.55} />
                    <ChartTooltip content={ChartTooltipContent} cursor={false} />
                </BarChart>
            </ChartContainer>
        </div>
    </Card>
</div>
```

- [ ] **Step 2: Create `ActivityStrip.svelte`**

```svelte
<!-- ui/src/lib/components/ActivityStrip.svelte -->
<script lang="ts">
    import type { RequestLogEntry, ErrorLogEntry } from '$lib/types';
    import type { Theme } from '$lib/theme.svelte';
    import Card from './shared/Card.svelte';

    let { requests, errors, t, D }: {
        requests: RequestLogEntry[];
        errors: ErrorLogEntry[];
        t: Theme;
        D: { pad: number; fz: number; rowPy: number };
    } = $props();

    function statusColor(code: number): string {
        if (code >= 500) return t.err;
        if (code >= 400) return t.warn;
        return t.muted;
    }
</script>

<div style="display:grid;grid-template-columns:1fr 1fr;gap:{D.pad}px">
    <Card {t} title="Requests" sub="latest first">
        <div style="font-family:ui-monospace,monospace;font-size:{D.fz - 0.5}px">
            {#each requests as r, i}
                <div style="display:grid;grid-template-columns:76px 50px 1fr 50px 60px;align-items:center;gap:8px;padding:{D.rowPy + 2}px {D.pad + 4}px;{i < requests.length - 1 ? `border-bottom:1px solid ${t.rule}` : ''}">
                    <span style="color:{t.muted}">{r.time}</span>
                    <span style="color:{t.muted}">{r.method}</span>
                    <span style="overflow:hidden;text-overflow:ellipsis;white-space:nowrap">{r.route}</span>
                    <span style="color:{statusColor(r.status)};font-weight:600;text-align:right">{r.status}</span>
                    <span style="text-align:right">{r.duration_ms}ms</span>
                </div>
            {/each}
            {#if requests.length === 0}
                <div style="padding:{D.pad}px;color:{t.muted}">No requests yet</div>
            {/if}
        </div>
    </Card>

    <Card {t} title="Errors" sub="{errors.length} recent">
        <div style="font-family:ui-monospace,monospace;font-size:{D.fz - 0.5}px">
            {#each errors as r, i}
                <div style="display:grid;grid-template-columns:76px 130px 1fr 80px;align-items:center;gap:8px;padding:{D.rowPy + 2}px {D.pad + 4}px;{i < errors.length - 1 ? `border-bottom:1px solid ${t.rule}` : ''}">
                    <span style="color:{t.muted}">{r.time}</span>
                    <span style="overflow:hidden;text-overflow:ellipsis;white-space:nowrap">{r.route}</span>
                    <span style="color:{t.err};overflow:hidden;text-overflow:ellipsis;white-space:nowrap">{r.message}</span>
                    <span style="text-align:right;color:{t.muted}">{r.request_id || '—'}</span>
                </div>
            {/each}
            {#if errors.length === 0}
                <div style="padding:{D.pad}px;color:{t.muted}">No errors</div>
            {/if}
        </div>
    </Card>
</div>
```

- [ ] **Step 3: Check + commit**

```bash
cd ui && npm run check 2>&1 | grep "error" | head -20
git add ui/src/lib/components/ThroughputStrip.svelte ui/src/lib/components/ActivityStrip.svelte
git commit -m "feat(console-ui): add ThroughputStrip and ActivityStrip components"
```

---

### Task 15: `+page.svelte` — full dashboard layout + tweaks panel

**Files:**
- Modify: `ui/src/routes/+page.svelte`

- [ ] **Step 1: Replace the default page content**

```svelte
<!-- ui/src/routes/+page.svelte -->
<script lang="ts">
    import { onMount, onDestroy } from 'svelte';
    import { data, loading, startSSE, stopSSE } from '$lib/metrics.svelte';
    import { theme, dark, accent, density, D } from '$lib/theme.svelte';
    import Header from '$lib/components/Header.svelte';
    import Ticker from '$lib/components/Ticker.svelte';
    import RoutesTable from '$lib/components/RoutesTable.svelte';
    import Engines from '$lib/components/side-rail/Engines.svelte';
    import Concurrency from '$lib/components/side-rail/Concurrency.svelte';
    import Batches from '$lib/components/side-rail/Batches.svelte';
    import Resources from '$lib/components/side-rail/Resources.svelte';
    import ThroughputStrip from '$lib/components/ThroughputStrip.svelte';
    import ActivityStrip from '$lib/components/ActivityStrip.svelte';

    onMount(() => startSSE());
    onDestroy(() => stopSSE());

    let tweaksOpen = $state(false);

    const ACCENTS = [
        { label: 'Blue',   value: '#4f8ef7' },
        { label: 'Violet', value: '#8b5cf6' },
        { label: 'Teal',   value: '#14b8a6' },
        { label: 'Orange', value: '#f97316' },
        { label: 'Rose',   value: '#f43f5e' },
    ];
</script>

<div style="background:{theme.bg};color:{theme.ink};font-family:'Geist Variable',ui-sans-serif,system-ui,sans-serif;min-height:100vh;padding:{D.gap + 4}px;transition:background 0.25s ease,color 0.25s ease">
    {#if loading}
        <div style="display:flex;align-items:center;justify-content:center;height:80vh;color:{theme.muted}">
            Connecting to Folio…
        </div>
    {:else if data}
        <!-- Header -->
        <Header {data} t={theme} />

        <!-- Ticker -->
        <div style="margin-top:{D.gap}px">
            <Ticker ticker={data.ticker} t={theme} {D} />
        </div>

        <!-- Main split: routes (8fr) + side rail (4fr) -->
        <div style="display:grid;grid-template-columns:8fr 4fr;gap:{D.gap}px;margin-top:{D.gap}px">
            <RoutesTable routes={data.routes} t={theme} {D} />
            <div style="display:flex;flex-direction:column;gap:{D.gap}px">
                <Engines engines={data.engines} t={theme} {D} />
                <Concurrency conc={data.concurrency} t={theme} {D} />
                <Batches batches={data.batches} t={theme} {D} />
                <Resources resources={data.resources} t={theme} {D} />
            </div>
        </div>

        <!-- Throughput strip -->
        <div style="margin-top:{D.gap}px">
            <ThroughputStrip throughput={data.throughput} t={theme} {D} />
        </div>

        <!-- Activity -->
        <div style="margin-top:{D.gap}px">
            <ActivityStrip requests={data.recent_requests} errors={data.recent_errors} t={theme} {D} />
        </div>
    {/if}
</div>

<!-- Tweaks panel (fixed bottom-right) -->
<div style="position:fixed;bottom:16px;right:16px;z-index:50">
    {#if tweaksOpen}
        <div style="background:{theme.surface};border:1px solid {theme.rule};border-radius:12px;padding:12px 16px;margin-bottom:8px;width:200px;display:flex;flex-direction:column;gap:10px;font-size:11px">
            <!-- Theme toggle -->
            <div>
                <div style="color:{theme.muted};font-size:10px;text-transform:uppercase;letter-spacing:0.05em;margin-bottom:4px">Theme</div>
                <div style="display:flex;gap:6px">
                    {#each [['Light', false], ['Dark', true]] as [label, val]}
                        <button
                            onclick={() => { dark = val as boolean; }}
                            style="flex:1;padding:3px 0;border:1px solid {dark === val ? theme.ink : theme.rule};border-radius:6px;background:{dark === val ? theme.ink : 'transparent'};color:{dark === val ? theme.bg : theme.ink};font-size:10.5px;cursor:pointer"
                        >{label}</button>
                    {/each}
                </div>
            </div>
            <!-- Accent swatches -->
            <div>
                <div style="color:{theme.muted};font-size:10px;text-transform:uppercase;letter-spacing:0.05em;margin-bottom:4px">Accent</div>
                <div style="display:flex;gap:5px">
                    {#each ACCENTS as a}
                        <button
                            onclick={() => { accent = a.value; }}
                            style="width:20px;height:20px;border-radius:999px;background:{a.value};border:2px solid {accent === a.value ? theme.ink : 'transparent'};cursor:pointer"
                            title={a.label}
                        ></button>
                    {/each}
                </div>
            </div>
            <!-- Density -->
            <div>
                <div style="color:{theme.muted};font-size:10px;text-transform:uppercase;letter-spacing:0.05em;margin-bottom:4px">Density</div>
                <div style="display:flex;gap:4px">
                    {#each ['compact', 'regular', 'comfy'] as d}
                        <button
                            onclick={() => { density = d as 'compact' | 'regular' | 'comfy'; }}
                            style="flex:1;padding:2px 0;border:1px solid {density === d ? theme.ink : theme.rule};border-radius:5px;background:{density === d ? theme.ink : 'transparent'};color:{density === d ? theme.bg : theme.ink};font-size:10px;cursor:pointer"
                        >{d.slice(0,1).toUpperCase() + d.slice(1)}</button>
                    {/each}
                </div>
            </div>
        </div>
    {/if}
    <button
        onclick={() => { tweaksOpen = !tweaksOpen; }}
        style="background:{theme.surface};border:1px solid {theme.rule};border-radius:9px;padding:6px 12px;font-size:11px;color:{theme.muted};cursor:pointer;display:block;margin-left:auto"
    >
        ⚙ tweaks
    </button>
</div>
```

- [ ] **Step 2: Build the UI and check for TypeScript errors**

```bash
cd ui && npm run check 2>&1 | grep "error" | head -30
cd ui && npm run build 2>&1 | tail -15
```
Expected: `✓ built`. Fix any type errors that appear.

- [ ] **Step 3: Run `folio-server` locally and open the console**

```bash
# Terminal 1: run server
cargo build -p server
cargo run -p server -- serve --port 3000 &
sleep 3

# Terminal 2: open browser
open http://localhost:3000/_/
```
Expected: the dashboard loads, the ticker shows uptime counting up, SSE connected indicator shows green.

- [ ] **Step 4: Commit**

```bash
git add ui/src/routes/+page.svelte
git commit -m "feat(console-ui): complete dashboard page layout with tweaks panel"
```

---

## Phase 3 — Integration & Verification

---

### Task 16: Full integration build + smoke test

**Files:** (none new — verification only)

- [ ] **Step 1: Build UI and embed into Rust binary**

```bash
cd ui && npm run build
cd ..
cargo build -p server 2>&1 | grep "^error" | head -10
```
Expected: both succeed.

- [ ] **Step 2: Start server and verify all endpoints**

```bash
cargo run -p server -- serve --port 3001 &
sleep 3

# Console loads
curl -s -o /dev/null -w "%{http_code}" http://localhost:3001/_/
# Expected: 200

# SSE streams events
curl -N --max-time 8 http://localhost:3001/_/api/stream 2>&1 | head -5
# Expected: data: {"version":... (JSON payload)

# One-shot JSON
curl -s http://localhost:3001/_/api/metrics | jq '{version, uptime_seconds}'
# Expected: {"version": "0.1.0", "uptime_seconds": <small number>}

# Existing API still works
curl -s http://localhost:3001/health | jq .status
# Expected: "ok"

pkill folio-server
```

- [ ] **Step 3: Run unit tests to confirm no regressions**

```bash
cargo test -p server --lib 2>&1 | tail -10
```
Expected: all pass.

- [ ] **Step 4: Final commit**

```bash
git add -A
git commit -m "feat(console): complete Folio Operator Console — SSE dashboard at /_/"
```

---

## Self-Review

**Spec coverage check:**

| Spec section | Covered by |
|---|---|
| SSE endpoint `/_/api/stream` | Task 4 |
| One-shot `/_/api/metrics` | Task 4 |
| `ConsoleStore` ring buffers + broadcast | Task 1 |
| Background 5s sampler | Task 3 |
| Request/error log middleware | Task 5 |
| Static file serving `/_/` | Task 6 |
| `MetricsHistory` rolling window | Task 1 + Task 3 |
| Engine restart tracking | Task 2 + Task 3 |
| `ConsolePayload` JSON shape | Task 3 |
| SvelteKit base path `/_` | Task 8 |
| Chart CSS var remapping | Task 8 |
| `types.ts` mirroring Rust structs | Task 9 |
| SSE `EventSource` store | Task 9 |
| Theme + density rune store | Task 9 |
| Card, Pill, SlimBar primitives | Task 10 |
| Header + Ticker | Task 11 |
| Routes table | Task 12 |
| Engines, Concurrency, Batches, Resources | Task 13 |
| Throughput strip (2 bar charts) | Task 14 |
| Activity strip (request + error log) | Task 14 |
| Full page layout + tweaks panel | Task 15 |
| Makefile `ui-build` target | Task 7 |
| Docker `ui-builder` stage | Task 7 |
| `EventSource` auth note (no auth on stream) | Handled in app.rs — `/_/api/stream` is in `untimed` outside auth layer |

**Type consistency check:**
- `ConsolePayload` Rust struct fields match `types.ts` interface exactly ✓
- `theme` and `D` objects passed consistently as props to all components ✓
- `startSSE`/`stopSSE` called in `+page.svelte` `onMount`/`onDestroy` ✓
- `BarChart`, `Bar`, `ChartContainer`, `ReferenceLine`, `ChartTooltip`, `ChartTooltipContent` — these are the expected shadcn chart exports; verify against generated `ui/src/lib/components/ui/chart/index.ts` after `npx shadcn-svelte add chart` runs in Task 8.
