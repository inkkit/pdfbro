// crates/server/src/console_store.rs
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU32};
use serde::{Deserialize, Serialize};
use tokio::sync::{Mutex, broadcast, watch};

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
    /// Signals SSE connections to close on graceful shutdown.
    pub shutdown_tx: watch::Sender<bool>,
    // Activation tracking: counts false→true transitions on engine health
    pub chromium_restarts: AtomicU32,
    pub chromium_was_running: AtomicBool,
    pub libreoffice_restarts: AtomicU32,
    pub libreoffice_was_running: AtomicBool,
    // RPS delta tracking
    pub prev_http_total: Mutex<f64>,
    // Error rate delta tracking
    pub prev_error_total: Mutex<f64>,
    // Live count of all HTTP requests currently in flight (incremented before
    // next.run(), decremented after) — gives real-time concurrency even for
    // fast requests that complete between 5s sampler ticks.
    pub active_requests: AtomicU32,
}

impl ConsoleStore {
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
            active_requests: AtomicU32::new(0),
        }
    }

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
    fn default() -> Self { Self::new() }
}

// ── ConsolePayload ────────────────────────────────────────────────────────

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

    let (rps_series, p95_series, cpu_series, memory_series, last_rps, last_p95_ms, last_error_pct) = {
        let history = state.console.history.lock().await;
        let rps_series: Vec<f64> = history.samples.iter().map(|s| s.rps).collect();
        let p95_series: Vec<f64> = history.samples.iter().map(|s| s.p95_ms / 1000.0).collect();
        let cpu_series: Vec<f64> = history.samples.iter().map(|s| s.cpu_pct).collect();
        let memory_series: Vec<f64> = history.samples.iter().map(|s| s.memory_mb).collect();
        let last_rps = rps_series.last().copied().unwrap_or(0.0);
        let last_p95_ms = p95_series.last().copied().unwrap_or(0.0) * 1000.0;
        let last_error_pct = history.samples.back().map_or(0.0, |s| s.error_pct);
        (rps_series, p95_series, cpu_series, memory_series, last_rps, last_p95_ms, last_error_pct)
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
    let memory_max_mb = total_memory_mb();

    ConsolePayload {
        version: env!("CARGO_PKG_VERSION").to_string(),
        uptime_seconds,
        ticker: TickerPayload {
            rps: last_rps,
            p95_ms: last_p95_ms,
            error_pct: last_error_pct,
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
        engines: {
            let mut engines = Vec::new();
            #[cfg(feature = "chromium")]
            engines.push(EnginePayload {
                name: "Chromium".to_string(),
                status: chromium_status.clone(),
                restarts: chromium_restarts,
                mode: if state.config.chromium_lazy_start { "lazy".to_string() } else { "eager".to_string() },
                mini_series: mini.clone(),
            });
            #[cfg(feature = "libreoffice")]
            engines.push(EnginePayload {
                name: "LibreOffice".to_string(),
                status: libreoffice_status.clone(),
                restarts: libreoffice_restarts,
                mode: if state.config.libreoffice_lazy_start { "lazy".to_string() } else { "eager".to_string() },
                mini_series: mini,
            });
            engines
        },
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

// ── Route payload: reads Prometheus counters + histograms ─────────────────

fn build_route_payloads(state: &crate::state::AppState, concurrency_max: u32) -> Vec<RoutePayload> {
    let families = prometheus::gather();

    // Build count + error map from folio_http_requests_total
    let mut route_counts: std::collections::HashMap<String, (f64, f64)> = std::collections::HashMap::new();
    for family in &families {
        if family.get_name() != "folio_http_requests_total" { continue; }
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

    // Build latency percentiles from folio_http_request_duration_seconds histogram
    let mut route_latency: std::collections::HashMap<String, (f64, f64, f64)> = std::collections::HashMap::new();
    for family in &families {
        if family.get_name() != "folio_http_request_duration_seconds" { continue; }
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

    let load_pct = (concurrency_max as usize).saturating_sub(state.sem.available_permits()) as f64
        / concurrency_max.max(1) as f64 * 100.0;

    let mut routes: Vec<RoutePayload> = route_counts.into_iter().map(|(path, (total, errors))| {
        let error_pct = if total > 0.0 { (errors / total) * 100.0 } else { 0.0 };
        let (p50_ms, p95_ms, p99_ms) = route_latency.get(&path).copied().unwrap_or((0.0, 0.0, 0.0));
        RoutePayload {
            path,
            method: "POST".to_string(),
            rps: 0.0,
            p50_ms,
            p95_ms,
            p99_ms,
            error_pct,
            in_flight: 0,
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

async fn build_batch_payloads(_state: &crate::state::AppState) -> Vec<BatchPayload> {
    vec![]
}

/// Total system RAM in MB (cached on first call).
fn total_memory_mb() -> f64 {
    use once_cell::sync::Lazy;
    static TOTAL_MB: Lazy<f64> = Lazy::new(|| {
        let mut sys = sysinfo::System::new();
        sys.refresh_memory();
        sys.total_memory() as f64 / 1024.0 / 1024.0
    });
    *TOTAL_MB
}

// ── spawn_console_sampler ─────────────────────────────────────────────────

pub fn spawn_console_sampler(state: crate::state::AppState, started_at: Instant) {
    tokio::spawn(async move {
        use sysinfo::{System, RefreshKind, CpuRefreshKind, MemoryRefreshKind};

        let mut sys = System::new_with_specifics(
            RefreshKind::new()
                .with_cpu(CpuRefreshKind::new().with_cpu_usage())
                .with_memory(MemoryRefreshKind::new().with_ram()),
        );
        // Prime CPU baseline (sysinfo needs two samples to compute usage)
        sys.refresh_cpu_usage();
        tokio::time::sleep(Duration::from_millis(500)).await;

        let mut interval = tokio::time::interval(Duration::from_secs(5));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        loop {
            interval.tick().await;

            // ── CPU + memory via sysinfo (cross-platform) ──────────────────
            sys.refresh_cpu_usage();
            let cpu_pct = sys.global_cpu_usage() as f64;
            sys.refresh_memory();
            let memory_mb = sys.used_memory() as f64 / 1024.0 / 1024.0;
            // Update Prometheus gauge so /prometheus/metrics reflects RSS
            state.metrics.process_resident_memory.set(sys.used_memory() as f64);

            // ── Engine health: probe directly, don't read stale gauge ──────
            #[cfg(feature = "chromium")]
            let chromium_up = match state.chromium.as_ref() {
                Some(be) => be.healthy().await,
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
                .find(|f| f.get_name() == "folio_http_requests_total")
                .map(|f| f.get_metric().iter().map(|m| m.get_counter().get_value()).sum())
                .unwrap_or(0.0);

            let error_total: f64 = families.iter()
                .find(|f| f.get_name() == "folio_http_requests_total")
                .map(|f| f.get_metric().iter()
                    .filter(|m| m.get_label().iter()
                        .any(|l| l.get_name() == "status"
                            && (l.get_value().starts_with('5') || l.get_value().starts_with('4'))))
                    .map(|m| m.get_counter().get_value()).sum())
                .unwrap_or(0.0);

            let (rps, error_pct) = {
                let mut prev_http = state.console.prev_http_total.lock().await;
                let mut prev_err  = state.console.prev_error_total.lock().await;
                let http_delta  = (http_total  - *prev_http).max(0.0);
                let error_delta = (error_total - *prev_err).max(0.0);
                let rps = http_delta / 5.0;
                let epct = if http_delta > 0.0 { (error_delta / http_delta) * 100.0 } else { 0.0 };
                *prev_http = http_total;
                *prev_err  = error_total;
                (rps, epct)
            };

            // ── p95 from histogram (global, across all routes) ─────────────
            let p95_ms = families.iter()
                .find(|f| f.get_name() == "folio_http_request_duration_seconds")
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

            // ── Concurrency ────────────────────────────────────────────────
            let concurrency_max = state.config.concurrency as u32;
            let concurrency_active = state.console.active_requests.load(Ordering::SeqCst);

            // ── Push sample + broadcast ────────────────────────────────────
            let sample = MetricsSample {
                ts: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs(),
                rps,
                p95_ms,
                error_pct,
                queue_size: state.metrics.queue_size.get() as u32,
                concurrency_active,
                cpu_pct,
                memory_mb,
            };

            state.console.history.lock().await.push(sample);

            let payload = build_console_payload(&state, started_at, chromium_up, libreoffice_up).await;
            if let Ok(json) = serde_json::to_string(&payload) {
                let _ = state.console.broadcast.send(json);
            }
        }
    });
}
