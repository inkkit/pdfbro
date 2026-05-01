// crates/server/src/console_store.rs
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU32};
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
    // Rolling-max p95 approximation; updated by request log middleware, reset to 0 by sampler each tick
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
        // Use std::time instead of chrono (chrono not in server crate)
        use std::time::{SystemTime, UNIX_EPOCH};
        let secs = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
        let h = (secs % 86400) / 3600;
        let m = (secs % 3600) / 60;
        let s = secs % 60;
        let time = format!("{h:02}:{m:02}:{s:02}");

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

impl Default for ConsoleStore {
    fn default() -> Self {
        Self::new()
    }
}

// ── ConsolePayload (the JSON shape sent to the frontend) ──────────────────

use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

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
    pub chromium_status: String,
    pub chromium_restarts: u32,
    pub libreoffice_status: String,
    pub libreoffice_restarts: u32,
    pub queue_size: f64,
    pub uptime_seconds: u64,
}

#[derive(Clone, Debug, Serialize)]
pub struct RoutePayload {
    pub path: String,
    pub method: String,
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
    pub name: String,
    pub status: String,
    pub restarts: u32,
    pub mode: String,
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

// ── build_console_payload ─────────────────────────────────────────────────

pub async fn build_console_payload(state: &crate::state::AppState, started_at: Instant) -> ConsolePayload {
    let uptime_seconds = started_at.elapsed().as_secs();
    let concurrency_max = state.config.concurrency as u32;
    let concurrency_active = (concurrency_max as usize)
        .saturating_sub(state.sem.available_permits()) as u32;

    // Read history for series data
    let (rps_series, p95_series, cpu_series, memory_series, last_rps, last_p95_ms, error_pct) = {
        let history = state.console.history.lock().await;
        let rps_series: Vec<f64> = history.samples.iter().map(|s| s.rps).collect();
        let p95_series: Vec<f64> = history.samples.iter().map(|s| s.p95_ms / 1000.0).collect();
        let cpu_series: Vec<f64> = history.samples.iter().map(|s| s.cpu_pct).collect();
        let memory_series: Vec<f64> = history.samples.iter().map(|s| s.memory_mb).collect();
        let last_rps = rps_series.last().copied().unwrap_or(0.0);
        let last_p95_ms = p95_series.last().copied().unwrap_or(0.0) * 1000.0;
        let error_pct = history.samples.back().map_or(0.0, |s| s.error_pct);
        (rps_series, p95_series, cpu_series, memory_series, last_rps, last_p95_ms, error_pct)
    };

    // Queue size from metrics gauge
    let queue_size = state.metrics.queue_size.get();

    // Engine status from health gauges
    let chromium_status = if state.metrics.chromium_healthy.get() >= 1.0 {
        "up".to_string()
    } else {
        "down".to_string()
    };
    let chromium_restarts = state.console.chromium_restarts.load(Ordering::SeqCst);

    let libreoffice_status = if state.metrics.libreoffice_healthy.get() >= 1.0 {
        "up".to_string()
    } else {
        "down".to_string()
    };
    let libreoffice_restarts = state.console.libreoffice_restarts.load(Ordering::SeqCst);

    // Engine mini_series (last 20 concurrency samples as proxy for load)
    let mini: Vec<f64> = {
        let h = state.console.history.lock().await;
        h.samples.iter().rev().take(20).rev()
            .map(|s| s.concurrency_active as f64 / concurrency_max.max(1) as f64)
            .collect()
    };

    let routes = build_route_payloads(state, concurrency_max);

    let recent_requests: Vec<RequestLogEntry> = {
        let log = state.console.request_log.lock().await;
        log.iter().rev().take(12).cloned().collect::<Vec<_>>().into_iter().rev().collect()
    };
    let recent_errors: Vec<ErrorLogEntry> = {
        let log = state.console.error_log.lock().await;
        log.iter().rev().take(6).cloned().collect::<Vec<_>>().into_iter().rev().collect()
    };

    let batches = build_batch_payloads(state).await;
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
            chromium_status: chromium_status.clone(),
            chromium_restarts,
            libreoffice_status: libreoffice_status.clone(),
            libreoffice_restarts,
            queue_size,
            uptime_seconds,
        },
        routes,
        engines: vec![
            EnginePayload {
                name: "Chromium".to_string(),
                status: chromium_status,
                restarts: chromium_restarts,
                mode: if state.config.chromium_lazy_start { "lazy".to_string() } else { "eager".to_string() },
                mini_series: mini.clone(),
            },
            EnginePayload {
                name: "LibreOffice".to_string(),
                status: libreoffice_status,
                restarts: libreoffice_restarts,
                mode: if state.config.libreoffice_lazy_start { "lazy".to_string() } else { "eager".to_string() },
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

// ── Helper functions ──────────────────────────────────────────────────────

fn build_route_payloads(state: &crate::state::AppState, concurrency_max: u32) -> Vec<RoutePayload> {
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
            entry.0 += count;
            if status.starts_with('5') || status.starts_with('4') {
                entry.1 += count;
            }
        }
        for (path, (total, errors)) in route_map {
            let error_pct = if total > 0.0 { (errors / total) * 100.0 } else { 0.0 };
            routes.push(RoutePayload {
                path,
                method: "POST".to_string(),
                rps: 0.0,
                p50_ms: 0.0,
                p95_ms: 0.0,
                p99_ms: 0.0,
                error_pct,
                in_flight: 0,
                load_pct: ((concurrency_max as usize).saturating_sub(state.sem.available_permits()) as f64
                    / concurrency_max.max(1) as f64 * 100.0),
            });
        }
        break;
    }
    routes.sort_by(|a, b| b.error_pct.partial_cmp(&a.error_pct).unwrap_or(std::cmp::Ordering::Equal));
    routes
}

async fn build_batch_payloads(state: &crate::state::AppState) -> Vec<BatchPayload> {
    // batch_manager.list_batches() returns Vec<BatchId> (just IDs, no status)
    // Return empty vec to avoid an expensive full scan; console can query batch API separately
    let Some(ref _bm) = state.batch_manager else { return vec![] };
    vec![]
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

// ── spawn_console_sampler ─────────────────────────────────────────────────

pub fn spawn_console_sampler(state: crate::state::AppState, started_at: Instant) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(5));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

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
            let mut prev = state.console.prev_http_total.lock().await;
            let rps = (http_total - *prev) / 5.0;
            *prev = http_total;
            drop(prev);

            // Error rate
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

            // Memory from prometheus gauge (bytes -> MB)
            let memory_mb = state.metrics.process_resident_memory.get() / (1024.0 * 1024.0);

            let p95_ms = {
                let mut p95 = state.console.last_p95_ms.lock().await;
                let val = *p95;
                *p95 = 0.0; // reset each tick
                val
            };

            let sample = MetricsSample {
                ts: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs(),
                rps,
                p95_ms,
                error_pct,
                queue_size: state.metrics.queue_size.get() as u32,
                concurrency_active,
                cpu_pct: 0.0,
                memory_mb,
            };

            state.console.history.lock().await.push(sample);

            // Detect engine restarts via health gauge transitions
            // chromium: health gauge 1.0 = up, 0.0 = down
            let chromium_now_running = state.metrics.chromium_healthy.get() >= 1.0;
            let chromium_was = state.console.chromium_was_running.load(Ordering::SeqCst);
            if !chromium_was && chromium_now_running {
                state.console.chromium_restarts.fetch_add(1, Ordering::SeqCst);
            }
            state.console.chromium_was_running.store(chromium_now_running, Ordering::SeqCst);

            #[cfg(feature = "libreoffice")]
            {
                let lo_now_running = state.metrics.libreoffice_healthy.get() >= 1.0;
                let lo_was = state.console.libreoffice_was_running.load(Ordering::SeqCst);
                if !lo_was && lo_now_running {
                    state.console.libreoffice_restarts.fetch_add(1, Ordering::SeqCst);
                }
                state.console.libreoffice_was_running.store(lo_now_running, Ordering::SeqCst);
            }

            // Build + broadcast
            let payload = build_console_payload(&state, started_at).await;
            if let Ok(json) = serde_json::to_string(&payload) {
                let _ = state.console.broadcast.send(json);
            }
        }
    });
}
