//! Prometheus metrics for Folio monitoring.
//!
//! Exposes conversion counts, latencies, error rates, queue depths,
//! and engine health metrics in Prometheus text format.

use axum::extract::State;
use axum::middleware::Next;
use axum::response::Response;
use prometheus::{self, CounterVec, Encoder, Gauge, HistogramOpts, HistogramVec, TextEncoder, register};
use std::time::{SystemTime, UNIX_EPOCH};

/// All Prometheus metrics for the Folio server.
pub struct FolioMetrics {
    // Conversion metrics
    pub conversions_total: CounterVec,
    pub conversion_duration: HistogramVec,
    pub conversion_bytes: CounterVec,

    // Queue metrics
    pub queue_size: Gauge,
    pub queue_processing: Gauge,
    pub queue_completed: CounterVec,
    pub queue_wait: HistogramVec,

    // Engine health
    pub chromium_healthy: Gauge,
    pub libreoffice_healthy: Gauge,
    pub chromium_conversions: CounterVec,
    pub libreoffice_conversions: CounterVec,

    // HTTP metrics
    pub http_requests: CounterVec,
    pub http_request_duration: HistogramVec,
    pub http_active_requests: Gauge,

    // System metrics
    pub process_start_time: Gauge,
    pub process_resident_memory: Gauge,
    pub process_virtual_memory: Gauge,
}

impl FolioMetrics {
    /// Create and register all metrics with the default Prometheus registry.
    pub fn new() -> Result<Self, prometheus::Error> {
        let conversions_total = {
            let opts = prometheus::opts!(
                "folio_conversions_total",
                "Total conversions by engine, endpoint, and status"
            );
            let metric = CounterVec::new(opts, &["engine", "endpoint", "status"])?;
            register(Box::new(metric.clone()))?;
            metric
        };

        let conversion_duration = {
            let opts = HistogramOpts::new(
                "folio_conversion_duration_seconds",
                "Conversion duration in seconds",
            )
            .buckets(vec![0.1, 0.5, 1.0, 2.0, 5.0, 10.0, 30.0, 60.0, 120.0]);
            let metric = HistogramVec::new(opts, &["engine", "endpoint"])?;
            register(Box::new(metric.clone()))?;
            metric
        };

        let conversion_bytes = {
            let opts = prometheus::opts!(
                "folio_conversion_bytes_total",
                "Total bytes processed by engine and endpoint"
            );
            let metric = CounterVec::new(opts, &["engine", "endpoint"])?;
            register(Box::new(metric.clone()))?;
            metric
        };

        let queue_size = {
            let metric = Gauge::new("folio_queue_size", "Current queue size")?;
            register(Box::new(metric.clone()))?;
            metric
        };

        let queue_processing = {
            let metric = Gauge::new("folio_queue_processing", "Currently processing jobs")?;
            register(Box::new(metric.clone()))?;
            metric
        };

        let queue_completed = {
            let opts = prometheus::opts!(
                "folio_queue_completed_total",
                "Completed jobs by status"
            );
            let metric = CounterVec::new(opts, &["status"])?;
            register(Box::new(metric.clone()))?;
            metric
        };

        let queue_wait = {
            let opts = HistogramOpts::new(
                "folio_queue_wait_seconds",
                "Time spent in queue",
            )
            .buckets(vec![0.1, 0.5, 1.0, 2.0, 5.0, 10.0, 30.0, 60.0]);
            let metric = HistogramVec::new(opts, &["status"])?;
            register(Box::new(metric.clone()))?;
            metric
        };

        let chromium_healthy = {
            let metric = Gauge::new(
                "folio_chromium_healthy",
                "Chromium health status (1=up, 0=down)",
            )?;
            register(Box::new(metric.clone()))?;
            metric
        };

        let libreoffice_healthy = {
            let metric = Gauge::new(
                "folio_libreoffice_healthy",
                "LibreOffice health status (1=up, 0=down)",
            )?;
            register(Box::new(metric.clone()))?;
            metric
        };

        let chromium_conversions = {
            let opts = prometheus::opts!(
                "folio_chromium_conversions_total",
                "Chromium conversion count"
            );
            let metric = CounterVec::new(opts, &["endpoint"])?;
            register(Box::new(metric.clone()))?;
            metric
        };

        let libreoffice_conversions = {
            let opts = prometheus::opts!(
                "folio_libreoffice_conversions_total",
                "LibreOffice conversion count"
            );
            let metric = CounterVec::new(opts, &["endpoint"])?;
            register(Box::new(metric.clone()))?;
            metric
        };

        let http_requests = {
            let opts = prometheus::opts!(
                "folio_http_requests_total",
                "Total HTTP requests by method, route, and status"
            );
            let metric = CounterVec::new(opts, &["method", "route", "status"])?;
            register(Box::new(metric.clone()))?;
            metric
        };

        let http_request_duration = {
            let opts = HistogramOpts::new(
                "folio_http_request_duration_seconds",
                "HTTP request duration in seconds",
            )
            .buckets(vec![0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0]);
            let metric = HistogramVec::new(opts, &["method", "route"])?;
            register(Box::new(metric.clone()))?;
            metric
        };

        let http_active_requests = {
            let metric = Gauge::new("folio_http_active_requests", "Active HTTP requests")?;
            register(Box::new(metric.clone()))?;
            metric
        };

        let process_start_time = {
            let metric = Gauge::new(
                "folio_process_start_time_seconds",
                "Process start time as Unix timestamp",
            )?;
            register(Box::new(metric.clone()))?;
            metric
        };

        let process_resident_memory = {
            let metric = Gauge::new(
                "folio_process_resident_memory_bytes",
                "Resident memory size in bytes (RSS)",
            )?;
            register(Box::new(metric.clone()))?;
            metric
        };

        let process_virtual_memory = {
            let metric = Gauge::new(
                "folio_process_virtual_memory_bytes",
                "Virtual memory size in bytes",
            )?;
            register(Box::new(metric.clone()))?;
            metric
        };

        Ok(Self {
            conversions_total,
            conversion_duration,
            conversion_bytes,
            queue_size,
            queue_processing,
            queue_completed,
            queue_wait,
            chromium_healthy,
            libreoffice_healthy,
            chromium_conversions,
            libreoffice_conversions,
            http_requests,
            http_request_duration,
            http_active_requests,
            process_start_time,
            process_resident_memory,
            process_virtual_memory,
        })
    }

    /// Initialize static metrics that don't change (like start time).
    pub fn init(&self) {
        let start_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs_f64();
        self.process_start_time.set(start_time);
    }

    /// Update engine health gauges.
    pub fn update_engine_health(&self, chromium_up: bool, libreoffice_up: bool) {
        self.chromium_healthy.set(if chromium_up { 1.0 } else { 0.0 });
        self.libreoffice_healthy.set(if libreoffice_up { 1.0 } else { 0.0 });
    }

    /// Record a conversion with its outcome.
    pub fn record_conversion(
        &self,
        engine: &str,
        endpoint: &str,
        success: bool,
        duration_secs: f64,
        bytes: u64,
    ) {
        let status = if success { "success" } else { "error" };
        self.conversions_total
            .with_label_values(&[engine, endpoint, status])
            .inc();
        self.conversion_duration
            .with_label_values(&[engine, endpoint])
            .observe(duration_secs);
        self.conversion_bytes
            .with_label_values(&[engine, endpoint])
            .inc_by(bytes as f64);
    }

    /// Record engine-specific conversion.
    pub fn record_engine_conversion(&self, engine: &str, endpoint: &str) {
        match engine {
            "chromium" => {
                self.chromium_conversions
                    .with_label_values(&[endpoint])
                    .inc();
            }
            "libreoffice" => {
                self.libreoffice_conversions
                    .with_label_values(&[endpoint])
                    .inc();
            }
            _ => {}
        }
    }

    /// Record HTTP request metrics.
    pub fn record_http_request(
        &self,
        method: &str,
        route: &str,
        status: u16,
        duration_secs: f64,
    ) {
        self.http_requests
            .with_label_values(&[method, route, &status.to_string()])
            .inc();
        self.http_request_duration
            .with_label_values(&[method, route])
            .observe(duration_secs);
    }

    /// Update memory metrics by reading /proc/self/status (Linux) or using
    /// platform-specific APIs.
    #[cfg(target_os = "linux")]
    pub fn update_memory_metrics(&self) {
        use std::fs;
        if let Ok(status) = fs::read_to_string("/proc/self/status") {
            for line in status.lines() {
                if line.starts_with("VmRSS:") {
                    if let Some(kb) = extract_kb_value(line) {
                        self.process_resident_memory.set(kb * 1024);
                    }
                } else if line.starts_with("VmSize:") {
                    if let Some(kb) = extract_kb_value(line) {
                        self.process_virtual_memory.set(kb * 1024);
                    }
                }
            }
        }
    }

    #[cfg(not(target_os = "linux"))]
    pub fn update_memory_metrics(&self) {
        // Platform-specific memory metrics not implemented
    }
}

#[cfg(target_os = "linux")]
fn extract_kb_value(line: &str) -> Option<u64> {
    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.len() >= 2 {
        parts[1].parse::<u64>().ok()
    } else {
        None
    }
}

/// Export all registered metrics in Prometheus text format.
pub fn export_metrics() -> String {
    let encoder = TextEncoder::new();
    let metric_families = prometheus::gather();
    let mut buffer = Vec::new();
    encoder.encode(&metric_families, &mut buffer).unwrap();
    String::from_utf8(buffer).unwrap()
}

/// Metrics middleware for Axum that records HTTP request duration and counts.
pub async fn metrics_middleware(
    State(state): State<crate::AppState>,
    request: axum::extract::Request,
    next: Next,
) -> Response {
    use std::time::Instant;

    let start = Instant::now();
    let method = request.method().clone();
    let uri = request.uri().clone();

    state.metrics.http_active_requests.inc();

    let response = next.run(request).await;

    let duration = start.elapsed().as_secs_f64();
    let status = response.status().as_u16();

    state.metrics.record_http_request(
        method.as_str(),
        uri.path(),
        status,
        duration,
    );
    state.metrics.http_active_requests.dec();

    response
}
