# Folio Operator Console — Design Spec

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a real-time operations dashboard served at `/_/` inside the Folio binary — Variation B · Bars Edition from the approved wireframe.

**Architecture:** Svelte 5 SPA embedded in the Folio binary via `rust-embed`. The Rust server adds a rolling metrics history store, request/error ring buffers, a `GET /_/api/stream` SSE endpoint that pushes a full snapshot every 5 seconds, a one-shot `GET /_/api/metrics` for initial load, and static file serving for `/_/`. The frontend subscribes via `EventSource` using Svelte 5 runes — no polling.

**Tech Stack:** Rust (Axum, rust-embed, tokio), Svelte 5 (runes forced), Tailwind CSS v4, shadcn-svelte, Lucide icons, Geist font, Vite, TypeScript.

---

## 1. Visual Specification

Taken directly from `variation-b-bars.jsx`. Do not deviate.

### Colors

| Token | Light | Dark |
|---|---|---|
| `bg` | `#f7f7f5` | `#0e0f12` |
| `surface` | `#ffffff` | `#15171c` |
| `surface2` | `#fbfbf9` | `#1a1d24` |
| `ink` | `#1a1c1f` | `#e6e7ea` |
| `muted` | `rgba(26,28,31,0.55)` | `rgba(230,231,234,0.55)` |
| `faint` | `rgba(26,28,31,0.06)` | `rgba(230,231,234,0.10)` |
| `rule` | `rgba(26,28,31,0.08)` | `rgba(255,255,255,0.08)` |
| `ok` | `#2f9967` | `#3fb27f` |
| `warn` | `#b8860b` | `#e0a93c` |
| `err` | `#c25151` | `#e26464` |
| `accent` | user-selectable (default blue) | same |

### Typography

- UI text: `"Geist Variable", ui-sans-serif, system-ui, -apple-system, sans-serif` (Geist is installed via `@fontsource-variable/geist`)
- Numbers / code: `ui-monospace, monospace` (JetBrains Mono from CDN is optional; Geist Mono can substitute)

### Card

- `background: surface`, `border: 1px solid rule`, `border-radius: 12px`
- No drop shadows
- Card header: `border-bottom: 1px solid rule`, title `11.5px 600`, sub `10.5px muted`

### Density presets (tweaks panel)

| Density | `gap` | `pad` | `rowPy` | `fz` | `kpiFz` |
|---|---|---|---|---|---|
| compact | 8 | 8 | 2 | 10.5 | 18 |
| regular | 10 | 10 | 3 | 11.5 | 20 |
| comfy | 14 | 14 | 5 | 12 | 22 |

---

## 2. Layout

Five horizontal strips, 1400px wide canvas:

```
┌─────────────────────────────── Header ───────────────────────────────────┐
├─────────────────────────────── Ticker (8 KPIs) ──────────────────────────┤
│                                                                           │
│   Routes table (8fr)              │  Side rail (4fr)                     │
│   Route / Method / RPS /          │  ┌ Engines (Chromium + LibreOffice) ┐│
│   p50 / p95 / p99 / Err% /        │  ├ Concurrency (64-slot grid)       ││
│   In-flight / Load bar            │  ├ Batches (progress list)          ││
│                                   │  └ Resources (CPU + Memory bars)    ┘│
├─────────────────────────────────────────────────────────────────────────┤
│   RPS bar chart (1fr)             │  p95 Latency bar chart (1fr)         │
├─────────────────────────────────────────────────────────────────────────┤
│   Request log                     │  Error log                           │
└─────────────────────────────────────────────────────────────────────────┘
```

---

## 3. Server Additions

### 3.1 New types — `crates/server/src/console_store.rs`

A new file holding everything the `/_/api/metrics` endpoint needs.

```rust
/// One data point captured every 30 seconds.
pub struct MetricsSample {
    pub ts: u64,              // unix seconds
    pub rps: f64,
    pub p95_ms: f64,
    pub error_pct: f64,
    pub queue_size: u32,
    pub concurrency_active: u32,
    pub cpu_pct: f64,         // 0.0–100.0, always 0.0 on non-Linux
    pub memory_mb: f64,
}

/// Rolling 30-minute window (60 samples at 30s cadence).
pub struct MetricsHistory {
    pub samples: VecDeque<MetricsSample>,  // cap 60
}

pub struct RequestLogEntry {
    pub time: String,         // "HH:MM:SS"
    pub method: String,
    pub route: String,
    pub status: u16,
    pub duration_ms: u64,
}

pub struct ErrorLogEntry {
    pub time: String,
    pub route: String,
    pub message: String,
    pub request_id: String,
}

/// Shared console state — Arc-wrapped in AppState.
pub struct ConsoleStore {
    pub history: Mutex<MetricsHistory>,                      // rolling samples
    pub request_log: Mutex<VecDeque<RequestLogEntry>>,       // cap 100
    pub error_log: Mutex<VecDeque<ErrorLogEntry>>,           // cap 100
    pub broadcast: tokio::sync::broadcast::Sender<String>,   // SSE fan-out (JSON payload)
}
```

`ConsoleStore` is `Arc<ConsoleStore>` in `AppState`. Added as `pub console: Arc<ConsoleStore>`.

### 3.2 Background sampler task

Spawned in `main.rs` at startup. Every **5 seconds**:
1. Read gauge values from `METRICS` (queue_size, concurrency, RPS delta, etc.)
2. Push a `MetricsSample` into `ConsoleStore::history`, evicting the oldest if at cap (cap = 360 samples = 30 min at 5s cadence)
3. Build the full `ConsolePayload` JSON string
4. Broadcast via `ConsoleStore::broadcast.send(payload)` — all active SSE subscribers receive it immediately

RPS is computed as `(http_requests_total_now - http_requests_total_prev) / 5.0`.

p95 is approximated: keep a `Mutex<f64>` in `ConsoleStore` updated by handlers with each observed duration. The sampler reads the current value as the rolling p95 approximation. This is labelled "p95 (approx)" in the UI.

### 3.3 Request/error log middleware

The existing `record_http_request` call site in `app.rs` already fires per request. Add a new `record_console_request(state, method, route, status, duration_ms)` call alongside the existing metrics call. This pushes to `ConsoleStore::request_log`. If status >= 500, also push to `error_log`.

### 3.4 `GET /_/api/stream` (SSE) and `GET /_/api/metrics` (one-shot JSON)

**`GET /_/api/stream`** — Server-Sent Events endpoint. Returns `Content-Type: text/event-stream`.

```rust
async fn console_stream(State(state): State<AppState>) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let mut rx = state.console.broadcast.subscribe();
    let stream = async_stream::stream! {
        // Send current snapshot immediately on connect (no waiting for next tick)
        let snapshot = build_console_payload(&state);
        yield Ok(Event::default().data(serde_json::to_string(&snapshot).unwrap()));
        loop {
            match rx.recv().await {
                Ok(payload) => yield Ok(Event::default().data(payload)),
                Err(RecvError::Lagged(_)) => continue, // skip missed ticks, keep going
                Err(RecvError::Closed) => break,
            }
        }
    };
    Sse::new(stream).keep_alive(
        KeepAlive::new().interval(Duration::from_secs(15)).text("ping")
    )
}
```

The broadcast channel is created with capacity 4 (buffer 4 missed ticks before dropping slow subscribers). `EventSource` auto-reconnects on drop; the new connection immediately receives a fresh snapshot.

**`GET /_/api/metrics`** — One-shot JSON snapshot. Same payload as SSE events. Used for debugging and curl inspection.

### 3.5 `ConsolePayload` — shared JSON shape (SSE event `data:` + one-shot JSON response)

```json
{
  "version": "0.1.0",
  "git_hash": "a91f02e",
  "uptime_seconds": 51738,
  "ticker": {
    "rps": 78.4,
    "p95_ms": 1400.0,
    "error_pct": 0.82,
    "concurrency_active": 54,
    "concurrency_max": 64,
    "chromium_status": "up",
    "chromium_restarts": 4,
    "chromium_idle_ms": 4000,
    "libreoffice_status": "up",
    "libreoffice_restarts": 11,
    "libreoffice_idle_ms": 0,
    "queue_size": 12
  },
  "routes": [
    {
      "path": "/forms/chromium/convert/html",
      "method": "POST",
      "rps": 14.2,
      "p50_ms": 120.0,
      "p95_ms": 480.0,
      "p99_ms": 1200.0,
      "error_pct": 0.4,
      "in_flight": 3,
      "load_pct": 65.0
    }
  ],
  "engines": [
    {
      "name": "Chromium",
      "status": "up",
      "restarts": 4,
      "uptime_seconds": 8040,
      "mode": "lazy",
      "mini_series": [0.3, 0.5, 0.4, 0.6, 0.2]
    },
    {
      "name": "LibreOffice",
      "status": "up",
      "restarts": 11,
      "uptime_seconds": 862,
      "mode": "eager",
      "mini_series": [0.5, 0.7, 0.4, 0.8, 0.6]
    }
  ],
  "concurrency": {
    "active": 54,
    "max": 64,
    "warn_threshold": 38,
    "crit_threshold": 54
  },
  "resources": {
    "cpu_series": [62.0, 58.0, 70.0],
    "memory_series": [1500.0, 1520.0, 1480.0],
    "memory_max_mb": 4096.0
  },
  "throughput": {
    "rps_series": [78.0, 82.0, 75.0],
    "rps_baseline": 74.0,
    "p95_series": [1.2, 1.4, 1.1],
    "p95_target_s": 2.0
  },
  "batches": [
    { "id": "b_8af21c", "status": "running", "progress_pct": 62, "elapsed": "2m 14s" }
  ],
  "recent_requests": [
    { "time": "10:42:18", "method": "POST", "route": "/forms/chromium/convert/html", "status": 200, "duration_ms": 142 }
  ],
  "recent_errors": [
    { "time": "10:42:16", "route": "/forms/chromium/convert/url", "message": "upstream timeout", "request_id": "cid_94aa2" }
  ]
}
```

Route-level p50/p95/p99 for V1: derived from the conversion_duration histogram values that are already recorded per endpoint. Expose the `_sum/_count` bucket ratio as a mean (p50 approximation) and the 0.95 quantile from the histogram's bucket data.

### 3.6 Static file serving — `GET /_/` and `GET /_/{*path}`

Use `rust-embed` to embed `ui/build/` at compile time into the binary.

```rust
#[derive(RustEmbed)]
#[folder = "../../ui/build/"]
struct ConsoleAssets;
```

Handler:
- `GET /_/` → serve `index.html` from the embedded assets
- `GET /_/{*path}` → serve the matching file; if not found, fall back to `index.html` (SPA routing)
- Content-Type derived from file extension
- ETag from embedded file hash; respond 304 if If-None-Match matches

**Dev mode**: If `FOLIO_CONSOLE_DEV=1`, skip rust-embed and serve from `./ui/build/` on disk instead. This allows `vite build --watch` to work without recompiling Rust.

**Opt-out**: `FOLIO_DISABLE_CONSOLE=true` disables the `/_/` routes entirely.

---

## 4. Frontend File Structure

```
ui/
├── package.json
├── svelte.config.js
├── vite.config.ts
├── src/
│   ├── app.html               # base HTML shell
│   ├── routes/
│   │   └── +page.svelte       # full dashboard (single route)
│   └── lib/
│       ├── types.ts           # ConsolePayload TypeScript types (mirrors Rust JSON)
│       ├── metrics.svelte.ts  # $state store + SSE subscription
│       ├── theme.svelte.ts    # $state for dark/accent/density
│       └── components/
│           ├── Header.svelte
│           ├── Ticker.svelte
│           ├── RoutesTable.svelte
│           ├── side-rail/
│           │   ├── Engines.svelte
│           │   ├── Concurrency.svelte
│           │   ├── Batches.svelte
│           │   └── Resources.svelte
│           ├── ThroughputStrip.svelte
│           ├── ActivityStrip.svelte
│           └── shared/
│               ├── Card.svelte        # thin wrapper — border + radius + optional header
│               ├── Pill.svelte        # status badge (ok/warn/err/accent tones)
│               └── SlimBar.svelte     # horizontal load bar (route table + batches)
```

### 4.1 `metrics.svelte.ts`

SSE-based — no polling. `EventSource` auto-reconnects on drop; on reconnect the server sends a fresh snapshot immediately (see §3.4).

```typescript
import type { ConsolePayload } from './types';

// Svelte 5 runes — module-level reactive state
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
    // EventSource reconnects automatically; no manual retry needed
  };
}

export function stopSSE() {
  es?.close();
  es = null;
  connected = false;
}

export function manualRefresh() {
  // Reconnect triggers an immediate snapshot from the server
  stopSSE();
  startSSE();
}
```

### 4.2 `theme.svelte.ts`

```typescript
export let dark = $state(false);
export let accent = $state('#4f8ef7');
export let density = $state<'compact' | 'regular' | 'comfy'>('regular');

// Derived theme tokens — matches wireframe's useThemeBB()
export let theme = $derived({
  bg:       dark ? '#0e0f12' : '#f7f7f5',
  surface:  dark ? '#15171c' : '#ffffff',
  surface2: dark ? '#1a1d24' : '#fbfbf9',
  ink:      dark ? '#e6e7ea' : '#1a1c1f',
  muted:    dark ? 'rgba(230,231,234,0.55)' : 'rgba(26,28,31,0.55)',
  faint:    dark ? 'rgba(230,231,234,0.10)' : 'rgba(26,28,31,0.06)',
  rule:     dark ? 'rgba(255,255,255,0.08)' : 'rgba(26,28,31,0.08)',
  ok:       dark ? '#3fb27f' : '#2f9967',
  warn:     dark ? '#e0a93c' : '#b8860b',
  err:      dark ? '#e26464' : '#c25151',
  accent,
});
```

### 4.3 Charts — shadcn-svelte `chart` component

Do **not** build custom SVG bar charts. Use shadcn-svelte's `chart` primitives throughout.

**Setup**: `npx shadcn-svelte@latest add chart` installs `ChartContainer`, `ChartTooltip`, and the layerchart-based bar chart components into `$lib/components/ui/chart/`.

**Chart CSS variable remapping** — update `layout.css` to replace the neutral gray chart vars with semantic colors matching the wireframe:

```css
:root {
  --chart-1: #4f8ef7;  /* accent */
  --chart-2: #2f9967;  /* ok */
  --chart-3: #b8860b;  /* warn */
  --chart-4: #c25151;  /* err */
  --chart-5: rgba(26,28,31,0.4); /* muted */
}
.dark {
  --chart-1: #6aa3f8;
  --chart-2: #3fb27f;
  --chart-3: #e0a93c;
  --chart-4: #e26464;
  --chart-5: rgba(230,231,234,0.4);
}
```

**Usage pattern** in section components:
- `Resources.svelte` — two stacked `BarChart`s (CPU, Memory), `chartConfig` maps `value` → `--chart-1` with threshold-based color override
- `ThroughputStrip.svelte` — two side-by-side `BarChart`s (RPS, p95), threshold line via a `ReferenceLine`
- `Engines.svelte` — `MiniBars` replaced by a compact `BarChart` with `--chart-2` (ok) or `--chart-3` (warn) based on restart count
- All charts share `ChartTooltip` with `cursor={false}` (no hover cursor line — matches the wireframe's clean look)

### 4.4 `+page.svelte`

Orchestrates the full layout. Calls `startSSE()` in an `$effect` on mount, `stopSSE()` on destroy. Passes `data` and `theme` as props to every section component. Shows a skeleton loading state (`loading === true`) until the first SSE message arrives.

### 4.5 Tweaks panel

A fixed bottom-right panel with three controls:
- **Theme** toggle: light / dark
- **Accent** color picker (5 swatches: blue, violet, teal, orange, rose)
- **Density** segmented control: compact / regular / comfy

---

## 5. Vite / SvelteKit config

SvelteKit with `adapter-static` is already scaffolded. Two changes needed:

1. Add `paths: { base: '/_' }` so all asset URLs are prefixed with `/_`
2. Add `fallback: 'index.html'` to adapter options for SPA routing

```javascript
// svelte.config.js — update kit section
kit: {
  adapter: adapter({ fallback: 'index.html' }),
  paths: { base: '/_' },
}
```

```typescript
// vite.config.ts
import { sveltekit } from '@sveltejs/kit/vite';
export default { plugins: [sveltekit()] };
```

Build output goes to `ui/build/` (adapter-static default). The rust-embed folder path is `../../ui/build/`.

---

## 6. Rust crate additions

### `Cargo.toml` (server crate)

```toml
[dependencies]
rust-embed = { version = "8", features = ["mime-guess"] }
mime_guess = "2"
```

### New files

| File | Purpose |
|---|---|
| `crates/server/src/console_store.rs` | `ConsoleStore`, `MetricsSample`, `MetricsHistory`, `RequestLogEntry`, `ErrorLogEntry` |
| `crates/server/src/routes/console.rs` | `console_stream` (SSE), `console_metrics_json` (one-shot), `console_asset` (static files) |

### Modified files

| File | Change |
|---|---|
| `crates/server/src/state.rs` | Add `pub console: Arc<ConsoleStore>` field |
| `crates/server/src/app.rs` | Mount `/_/api/stream`, `/_/api/metrics`, and `/_/{*path}` routes; call `record_console_request` alongside `record_http_request` |
| `crates/server/src/main.rs` | Spawn background sampler task |
| `crates/server/src/lib.rs` | `pub mod console_store;` |

---

## 7. Route registration

```rust
// In build_router():
use crate::routes::console;

untimed = untimed
    .route("/_/api/stream",  get(console::console_stream))       // SSE — long-lived
    .route("/_/api/metrics", get(console::console_metrics_json)) // one-shot JSON
    .route("/_/",            get(console::console_asset))
    .route("/_/{*path}",     get(console::console_asset));
```

Both `/_/api/stream` and `/_/api/metrics` live in `untimed` — they bypass the request timeout middleware. The SSE handler must never be cancelled by the timeout layer.

---

## 8. Build integration

Add to `Makefile`:

```makefile
.PHONY: ui-build
ui-build: ## Build the operator console UI
	cd ui && npm run build

.PHONY: ui-dev
ui-dev: ## Start UI dev server with hot reload
	cd ui && npm run dev
```

The Docker build adds a `ui-builder` stage before the Rust builder stages:

```dockerfile
FROM node:22-slim AS ui-builder
WORKDIR /ui
COPY ui/package*.json ./
RUN npm ci
COPY ui/ ./
RUN npm run build

FROM chef AS builder-full
# ...existing...
COPY --from=ui-builder /ui/build /app/ui/build
# then cargo build sees ui/build/ for rust-embed
```

---

## 9. Constraints

- The `/_/api/stream` and `/_/api/metrics` endpoints are **not** protected by BasicAuth even when the API has auth enabled. `EventSource` in the browser cannot set Authorization headers, so SSE endpoints must be unauthenticated at the HTTP level. If auth is needed, operators should set `FOLIO_DISABLE_CONSOLE=true` and run the UI behind a reverse proxy instead.
- `mini_series` for engines (the small bar columns) is computed from the rolling history filtered by engine.
- Route-level p50/p95/p99 are approximated from the Prometheus histogram sum/count/buckets in V1 — not exact quantiles.
- Memory max (`memory_max_mb`) is read from `/proc/meminfo` on Linux; hardcoded to `0` on macOS (the UI will show "— GB" in that case).
