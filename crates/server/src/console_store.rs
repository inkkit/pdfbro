// crates/server/src/console_store.rs
//! In-memory store for console metrics, request logs, and SSE broadcasting.

use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicBool, AtomicU32};
use serde::{Deserialize, Serialize};
use tokio::sync::{Mutex, broadcast, watch};

/// Max number of metrics samples to keep (~5 min at 5s cadence, matches UI bar chart).
pub const HISTORY_CAP: usize = 60;
/// Max number of request/error log entries to keep.
pub const LOG_CAP: usize = 100;
/// Broadcast channel capacity for SSE connections.
pub const BROADCAST_CAP: usize = 4;

/// Single metrics sample collected at a point in time.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MetricsSample {
    /// Unix timestamp (seconds).
    pub ts: u64,
    /// Requests per second.
    pub rps: f64,
    /// p50 latency in milliseconds.
    pub p50_ms: f64,
    /// p55 latency in milliseconds.
    pub p55_ms: f64,
    /// p95 latency in milliseconds.
    pub p95_ms: f64,
    /// 5xx server error percentage (0-100).
    pub server_error_pct: f64,
    /// 429 rate-limit percentage (0-100).
    pub rate_limit_pct: f64,
    /// Current queue size.
    pub queue_size: u32,
    /// Active concurrent requests.
    pub concurrency_active: u32,
    /// CPU percentage (cgroup-aware in containers).
    pub cpu_pct: f64,
    /// Memory usage in MB (cgroup-aware in containers).
    pub memory_mb: f64,
    /// Chromium conversion requests per second.
    pub chromium_conv_rps: f64,
    /// LibreOffice conversion requests per second.
    pub libreoffice_conv_rps: f64,
    /// p95 queue wait time in milliseconds.
    pub queue_wait_p95_ms: f64,
}

/// Ring buffer of metrics samples for time-series display.
#[derive(Debug, Default)]
pub struct MetricsHistory {
    /// Time-series samples, oldest at front.
    pub samples: VecDeque<MetricsSample>,
}

impl MetricsHistory {
    /// Add a sample, evicting oldest if at capacity.
    pub fn push(&mut self, sample: MetricsSample) {
        if self.samples.len() >= HISTORY_CAP {
            self.samples.pop_front();
        }
        self.samples.push_back(sample);
    }
}

/// Log entry for a single HTTP request.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RequestLogEntry {
    /// Timestamp formatted as HH:MM:SS.
    pub time: String,
    /// HTTP method (GET, POST, etc.).
    pub method: String,
    /// Request path/route.
    pub route: String,
    /// HTTP status code.
    pub status: u16,
    /// Request duration in milliseconds.
    pub duration_ms: u64,
}

/// Log entry for a single error.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ErrorLogEntry {
    /// Timestamp formatted as HH:MM:SS.
    pub time: String,
    /// Request path where error occurred.
    pub route: String,
    /// Error message.
    pub message: String,
    /// Request ID for correlation.
    pub request_id: String,
}

/// Central store for console metrics, logs, and SSE broadcasting.
pub struct ConsoleStore {
    /// Time-series metrics history.
    pub history: Mutex<MetricsHistory>,
    /// Recent HTTP request log.
    pub request_log: Mutex<VecDeque<RequestLogEntry>>,
    /// Recent error log.
    pub error_log: Mutex<VecDeque<ErrorLogEntry>>,
    /// SSE broadcast channel for real-time updates.
    pub broadcast: broadcast::Sender<String>,
    /// Signals SSE connections to close on graceful shutdown.
    pub shutdown_tx: watch::Sender<bool>,
    /// Chromium activation counter (tracks restarts).
    pub chromium_restarts: AtomicU32,
    /// Last known Chromium running state (for restart detection).
    pub chromium_was_running: AtomicBool,
    /// LibreOffice activation counter (tracks restarts).
    pub libreoffice_restarts: AtomicU32,
    /// Last known LibreOffice running state (for restart detection).
    pub libreoffice_was_running: AtomicBool,
    /// Previous HTTP request total for RPS delta calculation.
    pub prev_http_total: Mutex<f64>,
    /// Previous 5xx server error total for server_error_pct delta calculation.
    pub prev_error_total: Mutex<f64>,
    /// Previous 429 rate-limit total for rate_limit_pct delta calculation.
    pub prev_rate_limit_total: Mutex<f64>,
    /// Previous Chromium conversion total for per-engine RPS delta.
    pub prev_chromium_conv_total: Mutex<f64>,
    /// Previous LibreOffice conversion total for per-engine RPS delta.
    pub prev_libreoffice_conv_total: Mutex<f64>,
    /// Previous per-route HTTP totals for per-route RPS delta.
    pub prev_route_totals: Mutex<HashMap<String, f64>>,
    /// Live count of HTTP requests currently in flight.
    pub active_requests: AtomicU32,
    /// Per-route count of HTTP requests currently in flight.
    pub active_per_route: Mutex<HashMap<String, u32>>,
}

impl ConsoleStore {
    /// Create a new console store with empty history and configured channels.
    pub fn new() -> Self {
        let (tx, _) = broadcast::channel(BROADCAST_CAP);
        let (shutdown_tx, _) = watch::channel(false);
        Self {
            history: Mutex::new(MetricsHistory::default()),
            request_log: Mutex::new(VecDeque::new()),
            error_log: Mutex::new(VecDeque::new()),
            broadcast: tx,
            shutdown_tx,
            chromium_restarts: AtomicU32::new(0),
            chromium_was_running: AtomicBool::new(false),
            libreoffice_restarts: AtomicU32::new(0),
            libreoffice_was_running: AtomicBool::new(false),
            prev_http_total: Mutex::new(0.0),
            prev_error_total: Mutex::new(0.0),
            prev_rate_limit_total: Mutex::new(0.0),
            prev_chromium_conv_total: Mutex::new(0.0),
            prev_libreoffice_conv_total: Mutex::new(0.0),
            prev_route_totals: Mutex::new(HashMap::new()),
            active_requests: AtomicU32::new(0),
            active_per_route: Mutex::new(HashMap::new()),
        }
    }

    /// Record a completed HTTP request to the request log (and error log if status >= 500).
    pub async fn record_request(&self, method: String, route: String, status: u16, duration_ms: u64) {
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
    /// Default console store (same as `new()`).
    fn default() -> Self { Self::new() }
}

// ── ConsolePayload ────────────────────────────────────────────────────────

use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

/// Full console payload sent to UI via SSE.
#[derive(Clone, Debug, Serialize)]
pub struct ConsolePayload {
    /// Server version string.
    pub version: String,
    /// Server uptime in seconds.
    pub uptime_seconds: u64,
    /// Top-level ticker metrics.
    pub ticker: TickerPayload,
    /// Per-route metrics.
    pub routes: Vec<RoutePayload>,
    /// Engine status and mini charts.
    pub engines: Vec<EnginePayload>,
    /// Current concurrency stats.
    pub concurrency: ConcurrencyPayload,
    /// CPU/memory time series.
    pub resources: ResourcesPayload,
    /// RPS/latency time series.
    pub throughput: ThroughputPayload,
    /// Active batch jobs.
    pub batches: Vec<BatchPayload>,
    /// Recent HTTP requests.
    pub recent_requests: Vec<RequestLogEntry>,
    /// Recent errors.
    pub recent_errors: Vec<ErrorLogEntry>,
}

/// Top-level ticker metrics displayed in the header.
#[derive(Clone, Debug, Serialize)]
pub struct TickerPayload {
    /// Current requests per second.
    pub rps: f64,
    /// p50 latency in milliseconds.
    pub p50_ms: f64,
    /// p55 latency in milliseconds.
    pub p55_ms: f64,
    /// p95 latency in milliseconds.
    pub p95_ms: f64,
    /// 5xx server error percentage (0-100).
    pub server_error_pct: f64,
    /// 429 rate-limit percentage (0-100).
    pub rate_limit_pct: f64,
    /// Active concurrent requests.
    pub concurrency_active: u32,
    /// Max allowed concurrent requests.
    pub concurrency_max: u32,
    /// Current queue size.
    pub queue_size: f64,
    /// Server uptime in seconds.
    pub uptime_seconds: u64,
}

/// Per-route metrics from Prometheus.
#[derive(Clone, Debug, Serialize)]
pub struct RoutePayload {
    /// Route path.
    pub path: String,
    /// HTTP method.
    pub method: String,
    /// Requests per second.
    pub rps: f64,
    /// p50 latency in milliseconds.
    pub p50_ms: f64,
    /// p95 latency in milliseconds.
    pub p95_ms: f64,
    /// p99 latency in milliseconds.
    pub p99_ms: f64,
    /// Error percentage (0-100).
    pub error_pct: f64,
    /// Requests currently in flight.
    pub in_flight: u32,
    /// Load percentage (0-100).
    pub load_pct: f64,
}

/// Engine status with mini sparkline.
#[derive(Clone, Debug, Serialize)]
pub struct EnginePayload {
    /// Engine name (Chromium/LibreOffice).
    pub name: String,
    /// Status (up/down/n/a).
    pub status: String,
    /// Number of restarts.
    pub restarts: u32,
    /// Start mode (eager/lazy).
    pub mode: String,
    /// Mini RPS sparkline (normalized 0-1).
    pub mini_series: Vec<f64>,
    /// Total conversions processed by this engine.
    pub conversions_total: u64,
    /// Error rate for this engine (0-100).
    pub error_rate: f64,
    /// Total bytes processed in MB.
    pub bytes_mb: f64,
    /// Seconds since last conversion (idle time).
    pub idle_secs: u64,
}

/// Concurrency statistics.
#[derive(Clone, Debug, Serialize)]
pub struct ConcurrencyPayload {
    /// Active concurrent requests.
    pub active: u32,
    /// Max allowed concurrent requests.
    pub max: u32,
    /// Warning threshold (60% of max).
    pub warn_threshold: u32,
    /// Critical threshold (85% of max).
    pub crit_threshold: u32,
    /// p95 queue wait time in milliseconds.
    pub queue_wait_p95_ms: f64,
    /// Number of requests currently processing in queue.
    pub queue_processing: u32,
}

/// Resource usage time series.
#[derive(Clone, Debug, Serialize)]
pub struct ResourcesPayload {
    /// CPU percentage time series.
    pub cpu_series: Vec<f64>,
    /// Memory usage time series (MB).
    pub memory_series: Vec<f64>,
    /// Maximum memory available (MB).
    pub memory_max_mb: f64,
}

/// Throughput and latency time series.
#[derive(Clone, Debug, Serialize)]
pub struct ThroughputPayload {
    /// Unix timestamps for each sample.
    pub ts_series: Vec<u64>,
    /// RPS time series.
    pub rps_series: Vec<f64>,
    /// RPS baseline for reference line.
    pub rps_baseline: f64,
    /// p95 latency time series (seconds).
    pub p95_series: Vec<f64>,
    /// Target p95 latency (seconds).
    pub p95_target_s: f64,
    /// Chromium conversions per second time series.
    pub chromium_conv_series: Vec<f64>,
    /// LibreOffice conversions per second time series.
    pub libreoffice_conv_series: Vec<f64>,
    /// p95 queue wait time series (milliseconds).
    pub queue_wait_p95_series: Vec<f64>,
}

/// Batch job status.
#[derive(Clone, Debug, Serialize)]
pub struct BatchPayload {
    /// Batch ID.
    pub id: String,
    /// Status (pending/running/completed/failed).
    pub status: String,
    /// Progress percentage (0-100).
    pub progress_pct: u8,
    /// Elapsed time string.
    pub elapsed: String,
    /// Total number of items in the batch.
    pub total_items: usize,
    /// Number of completed items.
    pub completed_items: usize,
    /// Number of failed items.
    pub failed_items: usize,
    /// Output mode (zip/stream/etc).
    pub output_mode: String,
}

// ── build_console_payload ─────────────────────────────────────────────────

/// Build the full console payload from current state.
/// Called every 5 seconds by the sampler to broadcast to all connected UI clients.
pub async fn build_console_payload(
    state: &crate::state::AppState,
    started_at: Instant,
    chromium_up: bool,
    libreoffice_up: bool,
) -> ConsolePayload {
    let uptime_seconds = started_at.elapsed().as_secs();
    let concurrency_max = state.config.concurrency as u32;
    // Use the live atomic counter (incremented/decremented in middleware) so
    // fast requests that finish between sampler ticks still appear in the UI.
    let concurrency_active = state.console.active_requests.load(Ordering::SeqCst);

    let (ts_series, rps_series, p95_series, cpu_series, memory_series,
         chromium_conv_series, libreoffice_conv_series, queue_wait_p95_series,
         last_rps, last_p50_ms, last_p55_ms, last_p95_ms,
         last_server_error_pct, last_rate_limit_pct) = {
        let history = state.console.history.lock().await;
        let ts_series: Vec<u64>   = history.samples.iter().map(|s| s.ts).collect();
        let rps_series: Vec<f64>  = history.samples.iter().map(|s| s.rps).collect();
        let p95_series: Vec<f64>  = history.samples.iter().map(|s| s.p95_ms / 1000.0).collect();
        let cpu_series: Vec<f64>  = history.samples.iter().map(|s| s.cpu_pct).collect();
        let memory_series: Vec<f64> = history.samples.iter().map(|s| s.memory_mb).collect();
        let chromium_conv_series: Vec<f64>    = history.samples.iter().map(|s| s.chromium_conv_rps).collect();
        let libreoffice_conv_series: Vec<f64> = history.samples.iter().map(|s| s.libreoffice_conv_rps).collect();
        let queue_wait_p95_series: Vec<f64>   = history.samples.iter().map(|s| s.queue_wait_p95_ms).collect();
        let last_rps    = rps_series.last().copied().unwrap_or(0.0);
        let last_p50_ms = history.samples.back().map_or(0.0, |s| s.p50_ms);
        let last_p55_ms = history.samples.back().map_or(0.0, |s| s.p55_ms);
        let last_p95_ms = p95_series.last().copied().unwrap_or(0.0) * 1000.0;
        let last_server_error_pct = history.samples.back().map_or(0.0, |s| s.server_error_pct);
        let last_rate_limit_pct   = history.samples.back().map_or(0.0, |s| s.rate_limit_pct);
        (ts_series, rps_series, p95_series, cpu_series, memory_series,
         chromium_conv_series, libreoffice_conv_series, queue_wait_p95_series,
         last_rps, last_p50_ms, last_p55_ms, last_p95_ms,
         last_server_error_pct, last_rate_limit_pct)
    };

    let queue_size = state.metrics.queue_size.get();

    #[cfg(feature = "chromium")]
    let chromium_status = if chromium_up { "up".to_string() } else { "down".to_string() };
    #[cfg(not(feature = "chromium"))]
    let chromium_status = "n/a".to_string();
    let chromium_restarts = state.console.chromium_restarts.load(Ordering::SeqCst);

    #[cfg(feature = "libreoffice")]
    let libreoffice_status = if libreoffice_up { "up".to_string() } else { "down".to_string() };
    #[cfg(not(feature = "libreoffice"))]
    let libreoffice_status = "n/a".to_string();
    let libreoffice_restarts = state.console.libreoffice_restarts.load(Ordering::SeqCst);

    // Engine mini_series: last 20 RPS samples normalised 0-1
    let mini: Vec<f64> = {
        let h = state.console.history.lock().await;
        let max_rps = h.samples.iter().map(|s| s.rps).fold(0.01f64, f64::max);
        h.samples.iter().rev().take(20).rev()
            .map(|s| s.rps / max_rps)
            .collect()
    };

    let routes = build_route_payloads(state, concurrency_max).await;

    let recent_requests: Vec<RequestLogEntry> = {
        let log = state.console.request_log.lock().await;
        log.iter().rev().take(12).cloned().collect::<Vec<_>>().into_iter().rev().collect()
    };
    let recent_errors: Vec<ErrorLogEntry> = {
        let log = state.console.error_log.lock().await;
        log.iter().rev().take(6).cloned().collect::<Vec<_>>().into_iter().rev().collect()
    };

    let batches = build_batch_payloads(state).await;
    let memory_max_mb = total_memory_mb();

    ConsolePayload {
        version: env!("CARGO_PKG_VERSION").to_string(),
        uptime_seconds,
        ticker: TickerPayload {
            rps: last_rps,
            p50_ms: last_p50_ms,
            p55_ms: last_p55_ms,
            p95_ms: last_p95_ms,
            server_error_pct: last_server_error_pct,
            rate_limit_pct: last_rate_limit_pct,
            concurrency_active,
            concurrency_max,
            queue_size,
            uptime_seconds,
        },
        routes,
        engines: {
            let eng_families = prometheus::gather();
            let mut engines = Vec::new();
            #[cfg(feature = "chromium")]
            {
                let ch_total = engine_conv_total(&eng_families, "chromium");
                let ch_errors: f64 = eng_families.iter()
                    .find(|f| f.get_name() == "pdfbro_conversions_total")
                    .map(|f| f.get_metric().iter()
                        .filter(|m| m.get_label().iter().any(|l| l.get_name() == "engine" && l.get_value() == "chromium")
                            && m.get_label().iter().any(|l| l.get_name() == "status" && l.get_value() == "error"))
                        .map(|m| m.get_counter().get_value())
                        .sum())
                    .unwrap_or(0.0);
                let ch_bytes_mb = engine_bytes_total(&eng_families, "chromium") / (1024.0 * 1024.0);
                let ch_error_rate = if ch_total > 0.0 { (ch_errors / ch_total) * 100.0 } else { 0.0 };
                let ch_idle = match state.chromium.as_ref() { Some(be) => be.idle_secs(), None => 0 };
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
                let lo_total = engine_conv_total(&eng_families, "libreoffice");
                let lo_errors: f64 = eng_families.iter()
                    .find(|f| f.get_name() == "pdfbro_conversions_total")
                    .map(|f| f.get_metric().iter()
                        .filter(|m| m.get_label().iter().any(|l| l.get_name() == "engine" && l.get_value() == "libreoffice")
                            && m.get_label().iter().any(|l| l.get_name() == "status" && l.get_value() == "error"))
                        .map(|m| m.get_counter().get_value())
                        .sum())
                    .unwrap_or(0.0);
                let lo_bytes_mb = engine_bytes_total(&eng_families, "libreoffice") / (1024.0 * 1024.0);
                let lo_error_rate = if lo_total > 0.0 { (lo_errors / lo_total) * 100.0 } else { 0.0 };
                let lo_idle = match state.libreoffice.as_ref() { Some(lo) => lo.idle_secs(), None => 0 };
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
        resources: ResourcesPayload { cpu_series, memory_series, memory_max_mb },
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
        batches,
        recent_requests,
        recent_errors,
    }
}

// ── Route payload: reads Prometheus counters + histograms ─────────────────

/// Build per-route metrics from Prometheus counters and histograms.
async fn build_route_payloads(state: &crate::state::AppState, concurrency_max: u32) -> Vec<RoutePayload> {
    let families = prometheus::gather();

    // Build count + error map from pdfbro_http_requests_total.
    // Key = "METHOD route" to preserve the real HTTP method per endpoint.
    let mut route_counts: std::collections::HashMap<String, (String, f64, f64)> = std::collections::HashMap::new();
    for family in &families {
        if family.get_name() != "pdfbro_http_requests_total" { continue; }
        for m in family.get_metric() {
            let labels: std::collections::HashMap<_, _> = m.get_label().iter()
                .map(|l| (l.get_name(), l.get_value()))
                .collect();
            let method = labels.get("method").copied().unwrap_or("GET").to_string();
            let route  = labels.get("route").copied().unwrap_or("unknown").to_string();
            let status = labels.get("status").copied().unwrap_or("0");
            let count  = m.get_counter().get_value();
            let key    = format!("{method} {route}");
            let entry  = route_counts.entry(key).or_insert((method, 0.0, 0.0));
            entry.1 += count;
            if status.starts_with('5') || status.starts_with('4') {
                entry.2 += count;
            }
        }
        break;
    }

    // Build latency percentiles from pdfbro_http_request_duration_seconds histogram.
    // Key = "METHOD route" to match route_counts.
    let mut route_latency: std::collections::HashMap<String, (f64, f64, f64)> = std::collections::HashMap::new();
    for family in &families {
        if family.get_name() != "pdfbro_http_request_duration_seconds" { continue; }
        for m in family.get_metric() {
            let labels: std::collections::HashMap<_, _> = m.get_label().iter()
                .map(|l| (l.get_name(), l.get_value()))
                .collect();
            let method = labels.get("method").copied().unwrap_or("GET");
            let route  = labels.get("route").copied().unwrap_or("unknown");
            let key    = format!("{method} {route}");
            let hist   = m.get_histogram();
            let count  = hist.get_sample_count();
            if count == 0 { continue; }
            let buckets = hist.get_bucket();
            let p50 = percentile_from_histogram(buckets, count, 0.50) * 1000.0;
            let p95 = percentile_from_histogram(buckets, count, 0.95) * 1000.0;
            let p99 = percentile_from_histogram(buckets, count, 0.99) * 1000.0;
            route_latency.insert(key, (p50, p95, p99));
        }
        break;
    }

    // Per-route RPS: compute delta from previous totals
    let route_rps: std::collections::HashMap<String, f64> = {
        let mut prev = state.console.prev_route_totals.lock().await;
        route_counts.iter().map(|(key, (_, total, _))| {
            let prev_total = prev.get(key).copied().unwrap_or(0.0);
            let delta = (total - prev_total).max(0.0);
            prev.insert(key.clone(), *total);
            (key.clone(), delta / 5.0)
        }).collect()
    };

    // Per-route in-flight from active_per_route map (keyed by route path only)
    let in_flight_map: std::collections::HashMap<String, u32> = {
        let map = state.console.active_per_route.lock().await;
        map.clone()
    };

    let mut routes: Vec<RoutePayload> = route_counts.into_iter().map(|(key, (method, total, errors))| {
        let path = key.splitn(2, ' ').nth(1).unwrap_or(&key).to_string();
        let error_pct = if total > 0.0 { (errors / total) * 100.0 } else { 0.0 };
        let (p50_ms, p95_ms, p99_ms) = route_latency.get(&key).copied().unwrap_or((0.0, 0.0, 0.0));
        let rps = route_rps.get(&key).copied().unwrap_or(0.0);
        let in_flight = in_flight_map.get(&path).copied().unwrap_or(0);
        let load_pct = (in_flight as f64 / concurrency_max.max(1) as f64) * 100.0;
        RoutePayload {
            path,
            method,
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

/// Compute a percentile from Prometheus histogram buckets using linear interpolation.
fn percentile_from_histogram(buckets: &[prometheus::proto::Bucket], total_count: u64, pct: f64) -> f64 {
    if total_count == 0 || buckets.is_empty() { return 0.0; }
    let target = (total_count as f64 * pct) as u64;
    let mut prev_count = 0u64;
    let mut prev_bound = 0.0f64;
    for bucket in buckets {
        let count = bucket.get_cumulative_count();
        let bound = bucket.get_upper_bound();
        if bound.is_infinite() { break; }
        if count >= target {
            if count == prev_count { return prev_bound; }
            return prev_bound + (bound - prev_bound)
                * ((target - prev_count) as f64 / (count - prev_count) as f64);
        }
        prev_count = count;
        prev_bound = bound;
    }
    // All observations in the last finite bucket
    buckets.iter().rev().find(|b| !b.get_upper_bound().is_infinite())
        .map(|b| b.get_upper_bound())
        .unwrap_or(0.0)
}

/// Extract a percentile (ms) from a named Prometheus histogram by aggregating all label combinations.
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

/// Sum a counter metric for a specific engine label value.
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

        let elapsed_secs = b.submitted_at.elapsed().unwrap_or_default().as_secs();
        let elapsed = if elapsed_secs < 60 {
            format!("{}s", elapsed_secs)
        } else {
            format!("{}m {}s", elapsed_secs / 60, elapsed_secs % 60)
        };

        let status = match b.status {
            crate::routes::batch_types::BatchStatus::Queued     => "queued",
            crate::routes::batch_types::BatchStatus::Processing => "running",
            crate::routes::batch_types::BatchStatus::Completed  => "completed",
            crate::routes::batch_types::BatchStatus::Failed     => "failed",
        }.to_string();

        let output_mode = match b.request.output_mode {
            crate::routes::batch_types::OutputMode::Zip   => "zip",
            crate::routes::batch_types::OutputMode::Merge => "merge",
        }.to_string();

        batches.push(BatchPayload {
            id: id.to_string(),
            status,
            progress_pct,
            elapsed,
            total_items:     progress.total,
            completed_items: progress.completed,
            failed_items:    progress.failed,
            output_mode,
        });
    }

    // Running first, then queued, then completed/failed; cap at 10
    batches.sort_by(|a, b| {
        let order = |s: &str| match s { "running" => 0, "queued" => 1, _ => 2 };
        order(&a.status).cmp(&order(&b.status))
    });
    batches.truncate(10);
    batches
}

/// Total system RAM in MB (cached on first call).
/// Returns cgroup memory limit if running in a container, otherwise host RAM.
fn total_memory_mb() -> f64 {
    use once_cell::sync::Lazy;
    static TOTAL_MB: Lazy<f64> = Lazy::new(|| {
        // Check cgroup memory limit first
        let cgroup = crate::cgroup::CgroupLimits::detect();
        if let Some(limit_mb) = cgroup.memory_limit_mb {
            return limit_mb;
        }
        // Fall back to host total memory
        let mut sys = sysinfo::System::new();
        sys.refresh_memory();
        sys.total_memory() as f64 / 1024.0 / 1024.0
    });
    *TOTAL_MB
}

// ── spawn_console_sampler ─────────────────────────────────────────────────

/// Spawn the background metrics sampler task.
/// Collects CPU/memory, engine health, RPS, error rate, and latency every 5 seconds.
pub fn spawn_console_sampler(state: crate::state::AppState, started_at: Instant) {
    tokio::spawn(async move {
        use sysinfo::{System, RefreshKind, CpuRefreshKind, MemoryRefreshKind};
        use crate::cgroup::CgroupLimits;

        let mut sys = System::new_with_specifics(
            RefreshKind::new()
                .with_cpu(CpuRefreshKind::new().with_cpu_usage())
                .with_memory(MemoryRefreshKind::new().with_ram()),
        );
        // Prime CPU baseline (sysinfo needs two samples to compute usage)
        sys.refresh_cpu_usage();
        tokio::time::sleep(Duration::from_millis(500)).await;

        // Detect cgroup limits once at startup (Docker/Kubernetes)
        let cgroup = CgroupLimits::detect();
        let num_host_cpus = sys.cpus().len().max(1);

        // For cgroup v2 CPU tracking: keep running total so we can delta each tick.
        let mut prev_cpu_usec: Option<u64> = crate::cgroup::read_cpu_usage_usec();

        let mut interval = tokio::time::interval(Duration::from_secs(5));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        loop {
            interval.tick().await;

            // ── CPU ────────────────────────────────────────────────────────
            // Prefer cgroup v2 cpu.stat delta (accurate per-container %).
            // Fall back to sysinfo global_cpu_usage when not in a container.
            let cpu_pct = {
                let new_usec = crate::cgroup::read_cpu_usage_usec();
                match (prev_cpu_usec, new_usec) {
                    (Some(prev), Some(curr)) if curr >= prev => {
                        let delta_usec = (curr - prev) as f64;
                        // delta_usec / 5_000_000 = fraction of 1 CPU used over 5 s.
                        // Divide by the cgroup CPU limit (cores) to get % of container quota.
                        let limit_cores = cgroup.cpu_limit_cores.unwrap_or(num_host_cpus as f64);
                        let pct = (delta_usec / (5_000_000.0 * limit_cores)) * 100.0;
                        prev_cpu_usec = new_usec;
                        pct.min(100.0)
                    }
                    _ => {
                        // Not on cgroup v2 — fall back to sysinfo
                        prev_cpu_usec = new_usec;
                        sys.refresh_cpu_usage();
                        let host_pct = sys.global_cpu_usage() as f64;
                        if cgroup.is_container {
                            cgroup.cpu_pct_relative_to_limit(host_pct, num_host_cpus).min(100.0)
                        } else {
                            host_pct
                        }
                    }
                }
            };

            // ── Memory ─────────────────────────────────────────────────────
            // Re-read cgroup memory.current each tick (stale startup snapshot is wrong).
            sys.refresh_memory();
            let memory_mb = crate::cgroup::read_memory_used_mb()
                .unwrap_or_else(|| sys.used_memory() as f64 / 1024.0 / 1024.0);

            // Update Prometheus gauge
            state.metrics.process_resident_memory.set(memory_mb * 1024.0 * 1024.0);

            // ── Engine health: use atomic is_alive() — never blocks on the
            // engine's internal mutex, so heavy batch load can't stall the sampler.
            #[cfg(feature = "chromium")]
            let chromium_up = match state.chromium.as_ref() {
                Some(be) => be.is_alive(),
                None => false,
            };
            #[cfg(not(feature = "chromium"))]
            let chromium_up = false;

            #[cfg(feature = "libreoffice")]
            let libreoffice_up = match state.libreoffice.as_ref() {
                Some(lo) => lo.is_running(),
                None => false,
            };
            #[cfg(not(feature = "libreoffice"))]
            let libreoffice_up = false;

            // Update health gauges so /prometheus/metrics stays accurate
            state.metrics.chromium_healthy.set(if chromium_up { 1.0 } else { 0.0 });
            state.metrics.libreoffice_healthy.set(if libreoffice_up { 1.0 } else { 0.0 });

            // ── Track false→true transitions (engine activations) ──────────
            #[cfg(feature = "chromium")]
            {
                let was = state.console.chromium_was_running.load(Ordering::SeqCst);
                if chromium_up && !was {
                    state.console.chromium_restarts.fetch_add(1, Ordering::SeqCst);
                }
                state.console.chromium_was_running.store(chromium_up, Ordering::SeqCst);
            }
            #[cfg(feature = "libreoffice")]
            {
                let was = state.console.libreoffice_was_running.load(Ordering::SeqCst);
                if libreoffice_up && !was {
                    state.console.libreoffice_restarts.fetch_add(1, Ordering::SeqCst);
                }
                state.console.libreoffice_was_running.store(libreoffice_up, Ordering::SeqCst);
            }

            // ── RPS + error% from Prometheus counter deltas ────────────────
            let families = prometheus::gather();

            let http_total: f64 = families.iter()
                .find(|f| f.get_name() == "pdfbro_http_requests_total")
                .map(|f| f.get_metric().iter().map(|m| m.get_counter().get_value()).sum())
                .unwrap_or(0.0);

            // 5xx = server errors; 429 = rate limits; 4xx (excl. 429) = client mistakes, ignored
            let server_error_total: f64 = families.iter()
                .find(|f| f.get_name() == "pdfbro_http_requests_total")
                .map(|f| f.get_metric().iter()
                    .filter(|m| m.get_label().iter()
                        .any(|l| l.get_name() == "status" && l.get_value().starts_with('5')))
                    .map(|m| m.get_counter().get_value()).sum())
                .unwrap_or(0.0);

            let rate_limit_total: f64 = families.iter()
                .find(|f| f.get_name() == "pdfbro_http_requests_total")
                .map(|f| f.get_metric().iter()
                    .filter(|m| m.get_label().iter()
                        .any(|l| l.get_name() == "status" && l.get_value() == "429"))
                    .map(|m| m.get_counter().get_value()).sum())
                .unwrap_or(0.0);

            let (rps, server_error_pct, rate_limit_pct) = {
                let mut prev_http = state.console.prev_http_total.lock().await;
                let mut prev_err  = state.console.prev_error_total.lock().await;
                let mut prev_rl   = state.console.prev_rate_limit_total.lock().await;
                let http_delta         = (http_total         - *prev_http).max(0.0);
                let server_error_delta = (server_error_total - *prev_err).max(0.0);
                let rate_limit_delta   = (rate_limit_total   - *prev_rl).max(0.0);
                let rps   = http_delta / 5.0;
                let sepct = if http_delta > 0.0 { (server_error_delta / http_delta) * 100.0 } else { 0.0 };
                let rlpct = if http_delta > 0.0 { (rate_limit_delta   / http_delta) * 100.0 } else { 0.0 };
                *prev_http = http_total;
                *prev_err  = server_error_total;
                *prev_rl   = rate_limit_total;
                (rps, sepct, rlpct)
            };

            // ── p95 from histogram (global, across all routes) ─────────────
            let p95_ms = families.iter()
                .find(|f| f.get_name() == "pdfbro_http_request_duration_seconds")
                .map(|f| {
                    // Aggregate all route histograms into one virtual histogram
                    let mut agg_count = 0u64;
                    let mut agg_buckets: Vec<(f64, u64)> = Vec::new();
                    for m in f.get_metric() {
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
                    let target = (agg_count as f64 * 0.95) as u64;
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
                })
                .unwrap_or(0.0);

            // ── p50 + p55 from HTTP duration histogram ─────────────────────
            let p50_ms = global_histogram_pct(&families, "pdfbro_http_request_duration_seconds", 0.50);
            let p55_ms = global_histogram_pct(&families, "pdfbro_http_request_duration_seconds", 0.55);

            // ── Per-engine conversion RPS ──────────────────────────────────
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

            // ── Queue wait p95 ─────────────────────────────────────────────
            let queue_wait_p95_ms = global_histogram_pct(&families, "pdfbro_queue_wait_seconds", 0.95);

            // ── Concurrency ────────────────────────────────────────────────
            let _concurrency_max = state.config.concurrency as u32;
            let concurrency_active = state.console.active_requests.load(Ordering::SeqCst);

            // ── Push sample + broadcast ────────────────────────────────────
            let sample = MetricsSample {
                ts: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs(),
                rps,
                p50_ms,
                p55_ms,
                p95_ms,
                server_error_pct,
                rate_limit_pct,
                queue_size: state.metrics.queue_size.get() as u32,
                concurrency_active,
                cpu_pct,
                memory_mb,
                chromium_conv_rps,
                libreoffice_conv_rps,
                queue_wait_p95_ms,
            };

            state.console.history.lock().await.push(sample);

            let payload = build_console_payload(&state, started_at, chromium_up, libreoffice_up).await;
            if let Ok(json) = serde_json::to_string(&payload) {
                let _ = state.console.broadcast.send(json);
            }
        }
    });
}
