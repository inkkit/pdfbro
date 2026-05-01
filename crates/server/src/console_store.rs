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
