# Console Redesign — Design Spec

**Date:** 2026-05-06  
**Branch:** `feat/console-redesign`  
**Scope:** `/_/` operator console — layout, data accuracy, new visibility metrics, batch wiring

---

## 1. Layout (v4)

```
┌─ Header (unchanged) ──────────────────────────────────────────────────────┐
├─ Ticker row (8 blocks) ────────────────────────────────────────────────────┤
│  RPS · P50 · P55 · P95 · Errors · Conc · Queue · Uptime                   │
├─ Main grid (8fr left │ 4fr right) ─────────────────────────────────────────┤
│  LEFT COLUMN                          │ RIGHT RAIL                          │
│  ┌──────────────┬──────────────┐      │ Engines (enhanced sub-cards)        │
│  │ Req/sec      │ Latency p95  │      │ Concurrency (preserved + queue)     │
│  │ time-axis    │ time-axis    │      │ Batches (wired up, live data)       │
│  └──────────────┴──────────────┘      │ Resources (CPU + Memory charts)    │
│  ┌──────────────┬──────────────┐      │                                     │
│  │ Engine conv  │ Queue wait   │      │                                     │
│  │ stacked bars │ p95 line     │      │                                     │
│  └──────────────┴──────────────┘      │                                     │
│  ┌─ Routes (scrollable) ───────────┐  │                                     │
│  │ ROUTE·METHOD·RPS·P50·P95·P99·  │  │                                     │
│  │ ERR%·IN-FLIGHT·LOAD            │  │                                     │
│  │ (overflow-y: auto, flex:1)      │  │                                     │
│  └────────────────────────────────┘  │                                     │
└────────────────────────────────────────────────────────────────────────────┘
```

**Removed:** ActivityStrip ("Requests latest first" + "Errors latest first" panels)  
**Removed from ticker:** Chromium and LibreOffice blocks  
**Added to ticker:** P50, P55  
**Routes:** moved inside left column, scrollable, no expand/collapse toggle

---

## 2. Data Bugs Fixed

| Bug | Root cause | Fix |
|-----|-----------|-----|
| Per-route RPS = 0.0 | No per-route delta tracking | Add `prev_route_totals: Mutex<HashMap<String,f64>>` to `ConsoleStore`; compute delta per route each tick |
| Per-route in-flight = 0 | Single global `active_requests` atomic | Add `active_per_route: Arc<DashMap<String,u32>>` to `ConsoleStore`; increment/decrement in middleware using matched route |
| Load % same for all routes | Reads global semaphore for every route | Per-route load = `in_flight_route / concurrency_max * 100` |
| No timestamps on charts | `ThroughputPayload` / `ResourcesPayload` send only `Vec<f64>` | Add `ts_series: Vec<u64>` to both payloads; read from `MetricsSample.ts` |
| P50/P55 missing in ticker | Not computed globally | Extract p50/p55 alongside p95 in the global histogram aggregation in sampler |

---

## 3. New Visibility Data

### 3a. Ticker: P50 + P55

Add to `MetricsSample`, `TickerPayload`:
```rust
pub p50_ms: f64,
pub p55_ms: f64,
```
Computed same way as p95 in the sampler — two more `percentile_from_histogram` calls on the aggregated global histogram.

Remove from `TickerPayload`: `chromium_status`, `chromium_restarts`, `libreoffice_status`, `libreoffice_restarts`

### 3b. Engine card enhancements

Add to `EnginePayload`:
```rust
pub conversions_total: u64,  // from pdfbro_conversions_total{engine,*,status=success}
pub error_rate: f64,         // errors / total * 100
pub bytes_mb: f64,           // from pdfbro_conversion_bytes_total{engine,*}
pub idle_secs: u64,          // seconds since last_activity
```

Requires adding `pub fn idle_secs(&self) -> u64` to `SupervisedChromiumEngine` and `SupervisedLibreOfficeEngine` — reads `last_activity: AtomicU64` and subtracts from current unix time.

### 3c. Concurrency card enhancements

Add to `ConcurrencyPayload`:
```rust
pub queue_wait_p95_ms: f64,  // from pdfbro_queue_wait_seconds histogram
pub queue_processing: u32,   // from pdfbro_queue_processing gauge
```

**Preserve all existing fields:** `active`, `max`, `warn_threshold`, `crit_threshold` — the slot-blocks visualization, the 0%·OK/WARN/CRIT badge, and the scale labels (0, warn N, crit N, max N) must be retained in the UI.

### 3d. Engine conversions chart (new)

`ThroughputPayload` gains two new per-engine conv/sec time series:
```rust
pub chromium_conv_series: Vec<f64>,
pub libreoffice_conv_series: Vec<f64>,
```
Computed from `pdfbro_conversions_total{engine}` counter deltas each tick, same pattern as global RPS.

### 3e. Queue wait chart (new)

`ThroughputPayload` gains:
```rust
pub queue_wait_p95_series: Vec<f64>,  // ms, per sample
```
Stored in `MetricsSample` as `queue_wait_p95_ms: f64`, extracted from `pdfbro_queue_wait_seconds` histogram each tick.

---

## 4. Batches Card — Wired Up

### Backend

**`BatchPayload` enhanced:**
```rust
pub struct BatchPayload {
    pub id: String,
    pub status: String,        // queued/processing/completed/failed
    pub progress_pct: u8,      // 0-100
    pub elapsed: String,       // "1m 23s"
    pub total_items: usize,
    pub completed_items: usize,
    pub failed_items: usize,
    pub output_mode: String,   // "zip" or "merge"
}
```

**`build_batch_payloads(state)` implementation:**
1. If `state.batch_manager` is `None`, return `vec![]`
2. Read all batches from manager (read lock)
3. Filter to non-expired batches, sort by `submitted_at` descending
4. For each: compute elapsed from `submitted_at`, map status enum to string, compute progress_pct from `progress().completed + failed / total`
5. Cap at 10 most recent batches in payload

**UI display — Batches card:**
- Header: "Batches" + "N active" badge
- If no batches: "No recent batches"
- If batches present: list each batch with:
  - ID (truncated, e.g. `batch_01HX...`)
  - Status badge (queued/processing/completed/failed — colored)
  - Progress bar (progress_pct)
  - `X / Y items` + elapsed time
  - Output mode badge (ZIP / MERGE)

**Preserve existing:** The "N active" badge and "No recent batches" text format are kept; the card just gains real data instead of a hardcoded empty state.

---

## 5. MetricsSample Extended

```rust
pub struct MetricsSample {
    pub ts: u64,
    pub rps: f64,
    pub p50_ms: f64,       // NEW
    pub p55_ms: f64,       // NEW
    pub p95_ms: f64,
    pub error_pct: f64,
    pub queue_size: u32,
    pub concurrency_active: u32,
    pub cpu_pct: f64,
    pub memory_mb: f64,
    pub chromium_conv_rps: f64,   // NEW
    pub libreoffice_conv_rps: f64, // NEW
    pub queue_wait_p95_ms: f64,    // NEW
}
```

---

## 6. Frontend Component Changes

| Component | Change |
|-----------|--------|
| `+page.svelte` | v4 layout: 2×2 charts + routes in left col; remove ActivityStrip import/use |
| `Ticker.svelte` | Remove Chromium/LibreOffice items; add P50, P55 |
| `RoutesTable.svelte` | Move inside left column; remove max-height cap; add `overflow-y: auto; flex: 1` |
| `ThroughputStrip.svelte` | Renamed/restructured — two charts stay but move into left col row 1 |
| `ActivityStrip.svelte` | **Deleted** |
| `EnginesCard.svelte` | Add per-engine sub-cards with conv/err%/MB/idle stats; preserve sparklines |
| `ConcurrencyCard.svelte` | Preserve all existing (slot blocks, badges, scale); add queue wait p95 + processing |
| `BatchesCard.svelte` | Wire up with real batch data; show progress bars, item counts, status badges |
| `NewChart: EngineConvChart.svelte` | Stacked bar chart (Chromium teal, LibreOffice amber) |
| `NewChart: QueueWaitChart.svelte` | Line chart for queue_wait_p95_series |
| `types.ts` | Update all interfaces per §3 above |

---

## 7. What Is Not Changing

- Header component: unchanged
- Right rail card order: Engines → Concurrency → Batches → Resources
- Resources card: CPU + Memory charts preserved exactly
- Route table columns: ROUTE · METHOD · RPS · P50 · P95 · P99 · ERR% · IN-FLIGHT · LOAD
- SSE cadence: still 5 seconds
- History ring buffer: still 60 samples

---

## 8. Deferred (Out of Scope)

- Console auth (token protection of `/_/`)
- Stats persistence across restarts
- Batch item-level drill-down UI
