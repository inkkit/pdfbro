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
