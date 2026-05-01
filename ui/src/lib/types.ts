// src/lib/types.ts
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
