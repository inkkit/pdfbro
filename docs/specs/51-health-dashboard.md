# Spec 51 — Health Dashboard#

> Beautiful HTML dashboard showing real-time Folio health,
> metrics, and diagnostics. Gotenberg only has JSON `/health`
> endpoint. Folio should have a visual dashboard for
> monitoring and debugging.

## Goal#

Create an HTML dashboard that shows Folio's health,
performance metrics, and system status in a
visual, easy-to-understand format. Helps users quickly
identify problems (Chromium crash, full queue, etc.).

## Problem Analysis#

### Current State (Gotenberg)#

**Gotenberg `/health` endpoint:**
```json
{
  "status": "up",
  "chromium": "up",
  "libreoffice": "up"
}
```

**Problems:**
- Raw JSON only - need to parse mentally
- No historical data (was it up 5 mins ago?)
- No queue depth visualization
- No memory/CPU metrics
- Users need external tools (Grafana) for visibility

**User Quote:**
> "I wish Gotenberg had a simple dashboard to see if
> everything is OK. The JSON `/health` is too cryptic."
> — Gotenberg Discussion #899

### Desired State (Folio Dashboard)#

```
┌───────────┐
│  📄 Folio Health Dashboard           http://localhost:3000/health/dashboard  │
├───────────┤
│  ✅ Chromium    PID 1234    42 conversions    150 MB    │
│  ✅ LibreOffice  PID 5678    10 conversions    300 MB    │
│  ⚠️  Queue       15 pending jobs                        │
│  ✅ Memory      512 MB / 2 GB (25%)                 │
│  📊 Conversions (last hour)                         │
│  ████████████████░░░  42 conversions              │
└───────────┘
```

## Scope#

**In:**

- `GET /health/dashboard` - HTML dashboard
- Real-time updates (WebSocket/SSE)
- System metrics: CPU, memory, disk
- Engine metrics: conversions, uptime, PID
- Queue depth visualization
- Historical charts (last hour/day)
- Dark mode support
- Mobile responsive

**Out:**

- Full Grafana integration (separate: spec-33)
- Alerting system (too complex)
- Multi-node dashboard (single instance for now)

## Implementation#

### 1. Dashboard HTML#

```html
<!-- crates/server/assets/dashboard/index.html -->

<!DOCTYPE html>
<html>
<head>
    <title>Folio Health Dashboard</title>
    <script src="https://cdn.jsdelivr.net/npm/chart.js"></script>
    <style>
        :root {
            --bg: #1a1a2e;
            --surface: #2d2d44;
            --text: #e2e8f0;
            --success: #10b981;
            --warning: #f59e0b;
            --error: #ef4444;
        }
        body { background: var(--bg); color: var(--text); font-family: sans-serif; margin: 0; padding: 20px; }
        .dashboard { max-width: 1200px; margin: 0 auto; }
        .card { background: var(--surface); border-radius: 8px; padding: 20px; margin: 10px; }
        .status { font-size: 24px; }
        .status.up { color: var(--success); }
        .status.down { color: var(--error); }
        .metric { font-size: 32px; font-weight: bold; }
        .grid { display: grid; grid-template-columns: repeat(auto-fit, minmax(300px, 1fr)); gap: 10px; }
    </style>
</head>
<body>
    <div class="dashboard">
        <h1>📄 Folio Health Dashboard</h1>
        
        <div class="grid">
            <div class="card" id="chromium-card">
                <h2>Chromium</h2>
                <div class="status" id="chromium-status">...</div>
                <div class="metric" id="chromium-conversions">-</div>
                <div>Conversions</div>
                <div id="chromium-pid"></div>
            </div>
            
            <div class="card" id="libreoffice-card">
                <h2>LibreOffice</h2>
                <div class="status" id="libreoffice-status">...</div>
                <div class="metric" id="libreoffice-conversions">-</div>
                <div>Conversions</div>
            </div>
            
            <div class="card">
                <h2>Queue</h2>
                <div class="metric" id="queue-depth">0</div>
                <div>Pending jobs</div>
            </div>
            
            <div class="card">
                <h2>Memory</h2>
                <div class="metric" id="memory-usage">-</div>
                <div id="memory-detail"></div>
            </div>
        </div>
        
        <div class="card">
            <h2>Conversions (Last Hour)</h2>
            <canvas id="conversions-chart" height="100"></canvas>
        </div>
    </div>
    
    <script>
        const chart = new Chart(
            document.getElementById('conversions-chart'),
            {
                type: 'line',
                data: {
                    labels: [],
                    datasets: [{
                        label: 'Conversions',
                        data: [],
                        borderColor: '#7c3aed',
                        backgroundColor: 'rgba(124, 58, 237, 0.1)'
                    }]
                }
            }
        );
        
        async function updateDashboard() {
            const response = await fetch('/health/dashboard/data');
            const data = await response.json();
            
            // Update Chromium
            document.getElementById('chromium-status').innerText =
                data.chromium.up ? '✅ Up' : '❌ Down';
            document.getElementById('chromium-status').className =
                'status ' + (data.chromium.up ? 'up' : 'down');
            document.getElementById('chromium-conversions').innerText =
                data.chromium.conversions;
            document.getElementById('chromium-pid').innerText =
                'PID ' + data.chromium.pid;
            
            // Update LibreOffice
            // ... similar
            
            // Update chart
            chart.data.labels = data.history.map(h => h.time);
            chart.data.datasets[0].data = data.history.map(h => h.count);
            chart.update();
        }
        
        // Update every 5 seconds
        updateDashboard();
        setInterval(updateDashboard, 5000);
    </script>
</body>
</html>
```

### 2. Dashboard Data Endpoint#

```rust
// crates/server/src/routes/health.rs#

/// Serve HTML dashboard.
pub async fn dashboard_handler() -> Html<&'static str> {
    let html = include_str!("../../assets/dashboard/index.html");
    Html(html)
}

/// Get dashboard data as JSON (for AJAX updates).
pub async fn dashboard_data(
    State(state): State<AppState>,
) -> Json<DashboardData> {
    let chromium_up = state.chromium.healthy().await.unwrap_or(false);
    let libreoffice_up = state.libreoffice
        .as_ref()
        .map(|lo| lo.healthy().await.unwrap_or(false))
        .unwrap_or(false);
    
    Json(DashboardData {
        chromium: EngineStatus {
            up: chromium_up,
            pid: state.chromium.pid().await,
            conversions: state.metrics.chromium_conversions.get(),
            uptime_secs: state.started_at.elapsed().as_secs() as i64,
        },
        libreoffice: EngineStatus {
            up: libreoffice_up,
            pid: state.libreoffice.as_ref().map(|lo| lo.pid()).flatten(),
            conversions: state.metrics.libreoffice_conversions.get(),
            uptime_secs: state.started_at.elapsed().as_secs() as i64,
        },
        queue: QueueStatus {
            depth: state.metrics.queue_size.get(),
            processing: state.metrics.queue_processing.get(),
        },
        memory: MemoryStatus {
            used_mb: get_memory_usage_mb(),
            total_mb: get_total_memory_mb(),
        },
        history: get_hourly_stats().await,
    })
}
```

### 3. Dashboard Data Model#

```rust
#[derive(Serialize)]
pub struct DashboardData {
    pub chromium: EngineStatus,
    pub libreoffice: EngineStatus,
    pub queue: QueueStatus,
    pub memory: MemoryStatus,
    pub history: Vec<HistoricalPoint>,
}

#[derive(Serialize)]
pub struct EngineStatus {
    pub up: bool,
    pub pid: Option<u32>,
    pub conversions: i64,
    pub uptime_secs: i64,
}

#[derive(Serialize)]
pub struct QueueStatus {
    pub depth: i64,
    pub processing: i64,
}

#[derive(Serialize)]
pub struct MemoryStatus {
    pub used_mb: u64,
    pub total_mb: u64,
}

#[derive(Serialize)]
pub struct HistoricalPoint {
    pub time: String,
    pub count: i64,
}
```

### 4. System Metrics#

```rust
// crates/server/src/metrics/system.rs#

pub fn get_memory_usage_mb() -> u64 {
    #[cfg(target_os = "linux")]
    {
        if let Ok(contents) = std::fs::read_to_string("/proc/self/status") {
            for line in contents.lines() {
                if line.starts_with("VmRSS:") {
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    if parts.len() >= 2 {
                        return parts[1].parse::<u64>().unwrap_or(0);
                    }
                }
            }
        }
    }
    
    #[cfg(target_os = "macos")]
    {
        // Use `taskinfo` or `sysctl`
        // Simplified for example
        return 512;
    }
    
    0
}

pub fn get_total_memory_mb() -> u64 {
    // Platform-specific implementation
    // Linux: /proc/meminfo
    // macOS: sysctl hw.memsize
    8 * 1024  // 8 GB default
}
```

### 5. Real-time Updates (SSE)#

```rust
/// Server-Sent Events for real-time updates.
pub async fn dashboard_stream(
    State(state): State<AppState>,
) -> Sse<impl Stream<Item = Event>> {
    let stream = unfold(state, |state| async move {
        tokio::time::sleep(Duration::from_secs(5)).await;
        
        let data = dashboard_data(State(state.clone())).await;
        let event = Event::default()
            .event("update")
            .data(serde_json::to_string(&data).unwrap());
        
        Some((event, state))
    });
    
    Sse::new(stream)
}
```

## Expected Behaviour#

### Visit Dashboard#

```
┌───────────┐
│  📄 Folio Health Dashboard           http://localhost:3000/health/dashboard  │
├───────────┤
│  ✅ Chromium    PID 1234    42 conversions    150 MB    │
│  ✅ LibreOffice  PID 5678    10 conversions    300 MB    │
│  ⚠️  Queue       15 pending jobs                        │
│  ✅ Memory      512 MB / 2 GB (25%)                 │
│  📊 Conversions (last hour)                         │
│  ████████████████░░░  42 conversions              │
└───────────┘
```

### JSON Data Endpoint#

```bash
curl http://localhost:3000/health/dashboard/data
```

```json
{
  "chromium": {
    "up": true,
    "pid": 1234,
    "conversions": 42,
    "uptime_secs": 3600
  },
  "queue": {
    "depth": 15,
    "processing": 3
  },
  "memory": {
    "used_mb": 512,
    "total_mb": 2048
  }
}
```

## Test Plan#

### Unit Tests#

- `dashboard_data_returns_correct_json`
- `memory_usage_calculation`
- `engine_status_up`

### Integration Tests#

- `dashboard_page_loads`
- `dashboard_data_realtime_updates`
- `sse_stream_updates_every_5s`

## Acceptance#

- [ ] `GET /health/dashboard` serves HTML dashboard
- [ ] `GET /health/dashboard/data` returns JSON
- [ ] Real-time updates (SSE or polling)
- [ ] System metrics: CPU, memory
- [ ] Engine metrics: conversions, uptime, PID
- [ ] Historical chart (last hour)
- [ ] Dark mode support
- [ ] Mobile responsive
- [ ] Unit tests for dashboard data
- [ ] Integration tests for dashboard
- [ ] `cargo clippy -p server -- -D warnings` clean

## References#

- Chart.js: https://www.chartjs.org/
- Server-Sent Events: https://docs.rs/axum/latest/axum/response/Sse/
- Gotenberg `/health`: https://gotenberg.dev/docs/getting-started/introduction#health-check
