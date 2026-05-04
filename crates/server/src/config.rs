//! `pdfbro-server serve` CLI surface and runtime configuration.
//!
//! Resolution precedence is **flag > env var > default**. The full,
//! resolved configuration lives in [`ServerConfig`]; the raw clap surface
//! is [`ServerArgs`]. Both are kept separate so unit tests can drive
//! [`ServerConfig::resolve`] with hand-crafted env maps.

use std::collections::HashMap;
use std::ffi::OsString;
use std::net::IpAddr;
use std::path::PathBuf;
use std::time::Duration;

use clap::{Parser, Subcommand, ValueEnum};

/// Default per-request body limit: 50 MiB.
pub const DEFAULT_MAX_BODY_BYTES: usize = 50 * 1024 * 1024;

/// Default per-request timeout: 120 seconds.
pub const DEFAULT_REQUEST_TIMEOUT: Duration = Duration::from_secs(120);

/// Default bind address: all interfaces.
pub const DEFAULT_HOST: &str = "0.0.0.0";

/// Default bind port.
pub const DEFAULT_PORT: u16 = 3000;

/// Top-level CLI for the `pdfbro-server` binary.
#[derive(Debug, Parser)]
#[command(
    name = "pdfbro-server",
    version,
    about = "pdfbro HTTP server (Gotenberg-compatible)",
    long_about = None,
)]
pub struct Cli {
    /// Subcommand selector. Currently only `serve` is supported.
    #[command(subcommand)]
    pub command: Command,
}

/// Top-level subcommands.
#[derive(Debug, Subcommand)]
pub enum Command {
    /// Start the HTTP server.
    Serve(ServerArgs),
}

/// CLI arguments for `pdfbro-server serve`.
///
/// Each flag has an `env`-overridable default. Resolution to a fully-typed
/// [`ServerConfig`] is performed by [`ServerConfig::resolve`].
#[derive(Debug, Default, Parser)]
#[command(name = "serve", about = "Start the HTTP server")]
pub struct ServerArgs {
    /// Bind address (default `0.0.0.0`).
    #[arg(long, value_name = "HOST")]
    pub host: Option<String>,

    /// Bind port (default `3000`).
    #[arg(long, value_name = "PORT")]
    pub port: Option<u16>,

    /// Maximum number of concurrent in-flight requests
    /// (default: number of logical CPUs).
    #[arg(long, value_name = "N")]
    pub concurrency: Option<usize>,

    /// Maximum multipart body size in bytes (default: 50 MiB).
    #[arg(long, value_name = "BYTES")]
    pub max_body_bytes: Option<usize>,

    /// Per-request timeout in humantime form, e.g. `120s`, `2m`.
    #[arg(long, value_name = "DUR")]
    pub request_timeout: Option<String>,

    /// Override the Chrome / Chromium executable path.
    #[arg(long, value_name = "PATH")]
    pub chrome: Option<PathBuf>,

    /// Disable Chrome's sandbox (often required inside Docker).
    #[arg(long, conflicts_with = "sandbox")]
    pub no_sandbox: bool,

    /// Force Chrome's sandbox on (overrides Linux default).
    #[arg(long, conflicts_with = "no_sandbox")]
    pub sandbox: bool,

    /// Override the LibreOffice program directory (the folder containing
    /// `libsofficeapp.so` / `liblibreofficekit.so`, e.g.
    /// `/usr/lib/libreoffice/program`).
    #[arg(long = "lo-program-dir", value_name = "DIR", env = "LO_PROGRAM_PATH")]
    pub lo_program_dir: Option<PathBuf>,

    /// Log level filter (default `info`).
    #[arg(long, value_name = "LEVEL")]
    pub log_level: Option<String>,

    /// Log output format. `text` for human-readable, `json` for structured.
    /// Default: `text` on a TTY, `json` otherwise.
    #[arg(long, value_name = "FORMAT")]
    pub log_format: Option<LogFormat>,

    /// Enable OpenTelemetry trace export via OTLP HTTP.
    #[arg(long, env = "PDFBRO_OTEL_ENABLED")]
    pub otel_enabled: bool,

    /// OTLP HTTP endpoint for trace export.
    #[arg(long, value_name = "URL", env = "OTEL_EXPORTER_OTLP_ENDPOINT")]
    pub otel_endpoint: Option<String>,

    // === Engine Supervision Flags ===
    /// Use lazy initialization for Chromium (start on first request).
    /// Default is false, meaning Chromium starts eagerly at server startup.
    /// Set to true to defer startup until the first request is received.
    #[arg(long, env = "CHROMIUM_LAZY_START", default_value = "false")]
    pub chromium_lazy_start: bool,

    /// Idle shutdown timeout for Chromium (e.g., "10m", "0" to disable).
    #[arg(long, value_name = "DUR", env = "CHROMIUM_IDLE_SHUTDOWN_TIMEOUT")]
    pub chromium_idle_shutdown_timeout: Option<String>,

    /// Use lazy initialization for LibreOffice (start on first request).
    /// Default is false, meaning LibreOffice starts eagerly at server startup.
    /// Set to true to defer startup until the first request is received.
    #[arg(long, env = "LIBREOFFICE_LAZY_START", default_value = "false")]
    pub libreoffice_lazy_start: bool,

    /// Idle shutdown timeout for LibreOffice (e.g., "10m", "0" to disable).
    #[arg(long, value_name = "DUR", env = "LIBREOFFICE_IDLE_SHUTDOWN_TIMEOUT")]
    pub libreoffice_idle_shutdown_timeout: Option<String>,

    // === API Server Flags ===
    /// Disable telemetry for health check route.
    #[arg(long, env = "API_DISABLE_HEALTH_ROUTE_TELEMETRY")]
    pub api_disable_health_route_telemetry: bool,

    /// Disable telemetry for root route.
    #[arg(long, env = "API_DISABLE_ROOT_ROUTE_TELEMETRY")]
    pub api_disable_root_route_telemetry: bool,

    /// Disable telemetry for debug route.
    #[arg(long, env = "API_DISABLE_DEBUG_ROUTE_TELEMETRY")]
    pub api_disable_debug_route_telemetry: bool,

    /// Disable telemetry for version route.
    #[arg(long, env = "API_DISABLE_VERSION_ROUTE_TELEMETRY")]
    pub api_disable_version_route_telemetry: bool,

    /// Enable the debug route (/debug).
    #[arg(long, env = "API_ENABLE_DEBUG_ROUTE")]
    pub api_enable_debug_route: bool,

    /// TLS certificate file path (enables HTTPS when both cert and key are set).
    #[arg(long, value_name = "PATH", env = "API_TLS_CERT_FILE")]
    pub api_tls_cert_file: Option<PathBuf>,

    /// TLS private key file path (enables HTTPS when both cert and key are set).
    #[arg(long, value_name = "PATH", env = "API_TLS_KEY_FILE")]
    pub api_tls_key_file: Option<PathBuf>,

    /// HTTP Basic Auth username (enables auth when set).
    #[arg(long, value_name = "USER", env = "API_BASIC_AUTH_USERNAME")]
    pub api_basic_auth_username: Option<String>,

    /// HTTP Basic Auth password (required when username is set).
    #[arg(long, value_name = "PASS", env = "API_BASIC_AUTH_PASSWORD")]
    pub api_basic_auth_password: Option<String>,

    /// Comma-separated list of allowed URL regex patterns for downloadFrom
    /// (empty = allow all).
    #[arg(long, value_name = "PATTERN", num_args = 0..)]
    pub api_download_from_allow_list: Vec<String>,

    /// Comma-separated list of denied URL regex patterns for downloadFrom.
    #[arg(long, value_name = "PATTERN", num_args = 0..)]
    pub api_download_from_deny_list: Vec<String>,

    /// Maximum number of download retries per URL for downloadFrom (default 3).
    #[arg(long, value_name = "N", env = "API_DOWNLOAD_FROM_MAX_RETRY")]
    pub api_download_from_max_retry: Option<u32>,

    /// Disable the downloadFrom multipart field entirely.
    #[arg(long, env = "API_DISABLE_DOWNLOAD_FROM")]
    pub api_disable_download_from: bool,

    /// Override the request-correlation header name (default: `x-request-id`).
    /// Must be a valid HTTP header name. When set, pdfbro reads this header from
    /// incoming requests and propagates it to responses and trace spans.
    #[arg(long, value_name = "HEADER", env = "API_CORRELATION_ID_HEADER")]
    pub api_correlation_id_header: Option<String>,

    /// Mount the entire API under this path prefix (default: empty / no prefix).
    /// Useful when running behind a reverse proxy that strips no path. Must
    /// start with `/` and have no trailing slash. Example: `--root-path /pdf`
    /// makes `/forms/chromium/convert/url` reachable at `/pdf/forms/chromium/convert/url`.
    #[arg(long, value_name = "PATH", env = "API_ROOT_PATH")]
    pub api_root_path: Option<String>,

    /// Maximum number of webhook delivery attempts (default: 4).
    #[arg(long, value_name = "N", env = "WEBHOOK_MAX_RETRY")]
    pub webhook_max_retry: Option<u32>,

    /// Minimum wait between webhook retries — start of exponential
    /// backoff window (default: 1s).
    #[arg(long, value_name = "DURATION", env = "WEBHOOK_RETRY_MIN_WAIT")]
    pub webhook_retry_min_wait: Option<String>,

    /// Maximum wait between webhook retries — cap of exponential
    /// backoff window (default: 30s).
    #[arg(long, value_name = "DURATION", env = "WEBHOOK_RETRY_MAX_WAIT")]
    pub webhook_retry_max_wait: Option<String>,

    /// Per-attempt webhook HTTP client timeout (default: 30s).
    #[arg(long, value_name = "DURATION", env = "WEBHOOK_CLIENT_TIMEOUT")]
    pub webhook_client_timeout: Option<String>,

    /// Regex pattern that webhook URLs must match. Repeat for multiple.
    /// Empty = allow all (subject to SSRF and deny-list).
    #[arg(long = "webhook-allow-list", value_name = "REGEX")]
    pub webhook_allow_list: Vec<String>,

    /// Regex pattern that webhook URLs must NOT match. Repeat for
    /// multiple. Evaluated after allow-list and SSRF checks.
    #[arg(long = "webhook-deny-list", value_name = "REGEX")]
    pub webhook_deny_list: Vec<String>,
}

/// Log output formats supported by the server.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
#[clap(rename_all = "lowercase")]
pub enum LogFormat {
    /// Human-readable text format (default on TTYs).
    Text,
    /// Newline-delimited JSON format (default off-TTY).
    Json,
}

impl LogFormat {
    /// Return the format as a string slice suitable for `init_logging()`.
    pub fn as_str(&self) -> &'static str {
        match self {
            LogFormat::Text => "text",
            LogFormat::Json => "json",
        }
    }
}

/// Fully-resolved runtime configuration.
///
/// Constructed via [`ServerConfig::resolve`]. All values are concrete; no
/// `Option<T>`s except where genuinely optional (engine path overrides).
#[derive(Debug, Clone)]
pub struct ServerConfig {
    /// Bind address.
    pub host: IpAddr,
    /// Bind port.
    pub port: u16,
    /// Outer concurrency cap (semaphore size).
    pub concurrency: usize,
    /// Per-request multipart body size limit, in bytes.
    pub max_body_bytes: usize,
    /// Per-request timeout (excludes `/health` and `/version`).
    pub request_timeout: Duration,
    /// Override path to chrome / chromium, if any.
    pub chrome_path: Option<PathBuf>,
    /// Whether to disable Chrome's sandbox. `None` means defer to engine default.
    pub no_sandbox: Option<bool>,
    /// Override path to the LibreOffice program directory, if any.
    pub lo_program_dir: Option<PathBuf>,
    /// Tracing filter directive (e.g. `info`, `server=debug,tower=warn`).
    pub log_level: String,
    /// Log output format.
    pub log_format: LogFormat,

    // Batch API configuration
    /// Maximum items per batch.
    pub batch_max_items: usize,
    /// Concurrent conversions per batch.
    pub batch_concurrency: usize,
    /// Maximum concurrent batches server-wide.
    pub batch_max_active: usize,
    /// Batch retention time in minutes.
    pub batch_retention_minutes: u64,
    /// Batch storage path.
    pub batch_storage_path: PathBuf,

    // OpenTelemetry
    /// Enable OpenTelemetry trace export.
    pub otel_enabled: bool,
    /// OTLP HTTP endpoint for trace export.
    pub otel_endpoint: String,

    // === Engine Supervision Config ===
    /// Use lazy initialization for Chromium (start on first request).
    pub chromium_lazy_start: bool,
    /// Idle shutdown timeout for Chromium (None = disabled).
    pub chromium_idle_shutdown_timeout: Option<Duration>,
    /// Use lazy initialization for LibreOffice (start on first request).
    pub libreoffice_lazy_start: bool,
    /// Idle shutdown timeout for LibreOffice (None = disabled).
    pub libreoffice_idle_shutdown_timeout: Option<Duration>,

    // === API Server Config ===
    /// Disable telemetry for health check route.
    pub api_disable_health_route_telemetry: bool,
    /// Disable telemetry for root route.
    pub api_disable_root_route_telemetry: bool,
    /// Disable telemetry for debug route.
    pub api_disable_debug_route_telemetry: bool,
    /// Disable telemetry for version route.
    pub api_disable_version_route_telemetry: bool,
    /// Enable the debug route (/debug).
    pub api_enable_debug_route: bool,
    /// TLS certificate file path (enables HTTPS when both cert and key are set).
    pub api_tls_cert_file: Option<PathBuf>,
    /// TLS private key file path (enables HTTPS when both cert and key are set).
    pub api_tls_key_file: Option<PathBuf>,
    /// HTTP Basic Auth username.
    pub api_basic_auth_username: Option<String>,
    /// HTTP Basic Auth password.
    pub api_basic_auth_password: Option<String>,
    /// Allowed URL regex patterns for downloadFrom (empty = allow all).
    pub api_download_from_allow_list: Vec<String>,
    /// Denied URL regex patterns for downloadFrom.
    pub api_download_from_deny_list: Vec<String>,
    /// Maximum download retries per URL.
    pub api_download_from_max_retry: u32,
    /// Whether downloadFrom is disabled.
    pub api_disable_download_from: bool,
    /// Request-correlation header name.
    pub api_correlation_id_header: String,
    /// Path prefix to mount the API under. Empty string means no prefix.
    /// Always starts with `/` and never ends with `/` (validated at resolve).
    pub api_root_path: String,

    /// Maximum webhook delivery attempts (>= 1).
    pub webhook_max_retry: u32,
    /// Minimum wait before retry (start of exponential backoff window).
    pub webhook_retry_min_wait: Duration,
    /// Maximum wait before retry (cap of exponential backoff window).
    pub webhook_retry_max_wait: Duration,
    /// Per-attempt HTTP client timeout for webhook delivery.
    pub webhook_client_timeout: Duration,
    /// Webhook URL allow-list (raw regex strings; compiled in main.rs).
    /// Empty = allow all (subject to SSRF + deny-list).
    pub webhook_allow_list: Vec<String>,
    /// Webhook URL deny-list (raw regex strings; compiled in main.rs).
    pub webhook_deny_list: Vec<String>,
}

/// Errors produced by [`ServerConfig::resolve`].
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    /// A value (CLI or env) failed to parse.
    #[error("invalid value for {field}: {message}")]
    Parse {
        /// Logical name of the offending field.
        field: &'static str,
        /// Free-form parse-error description.
        message: String,
    },
}

impl ServerConfig {
    /// Resolve CLI flags + environment variables + defaults into a final
    /// [`ServerConfig`]. The precedence order is **flag > env > default**.
    ///
    /// `env` is taken explicitly (rather than read from `std::env`) so unit
    /// tests can drive resolution deterministically.
    pub fn resolve(args: &ServerArgs, env: &HashMap<String, String>) -> Result<Self, ConfigError> {
        let host_str = pick_string(args.host.as_deref(), env, "PDFBRO_HOST", DEFAULT_HOST);
        let host = host_str.parse::<IpAddr>().map_err(|e| ConfigError::Parse {
            field: "host",
            message: e.to_string(),
        })?;

        let port = match args.port {
            Some(p) => p,
            None => match env.get("PDFBRO_PORT") {
                Some(v) => v.parse::<u16>().map_err(|e| ConfigError::Parse {
                    field: "port",
                    message: e.to_string(),
                })?,
                None => DEFAULT_PORT,
            },
        };

        let concurrency = match args.concurrency {
            Some(c) => c,
            None => match env.get("PDFBRO_CONCURRENCY") {
                Some(v) => v.parse::<usize>().map_err(|e| ConfigError::Parse {
                    field: "concurrency",
                    message: e.to_string(),
                })?,
                None => default_concurrency(),
            },
        };
        if concurrency == 0 {
            return Err(ConfigError::Parse {
                field: "concurrency",
                message: "must be >= 1".to_string(),
            });
        }

        let max_body_bytes = match args.max_body_bytes {
            Some(v) => v,
            None => match env.get("PDFBRO_MAX_BODY") {
                Some(v) => v.parse::<usize>().map_err(|e| ConfigError::Parse {
                    field: "max_body_bytes",
                    message: e.to_string(),
                })?,
                None => DEFAULT_MAX_BODY_BYTES,
            },
        };

        let request_timeout_str = pick_string(
            args.request_timeout.as_deref(),
            env,
            "PDFBRO_REQUEST_TIMEOUT",
            "120s",
        );
        let request_timeout =
            humantime::parse_duration(&request_timeout_str).map_err(|e| ConfigError::Parse {
                field: "request_timeout",
                message: e.to_string(),
            })?;

        let chrome_path = args
            .chrome
            .clone()
            .or_else(|| env.get("CHROME_PATH").map(PathBuf::from));

        // --no-sandbox / --sandbox / PDFBRO_NO_SANDBOX (truthy: 1/true/yes).
        let no_sandbox = if args.no_sandbox {
            Some(true)
        } else if args.sandbox {
            Some(false)
        } else {
            env.get("PDFBRO_NO_SANDBOX").map(|v| is_truthy(v))
        };

        // Discovery order: --lo-program-dir CLI flag / LO_PROGRAM_PATH env var
        // (clap wires the env attr at parse time, but for unit tests that
        // pass a synthetic env map we also check it here), then LOK_PROGRAM_PATH
        // (the libreofficekit crate honours it directly so we accept it as an
        // alias), else None and let the engine auto-discover via
        // Office::find_install_path().
        let lo_program_dir = args
            .lo_program_dir
            .clone()
            .or_else(|| env.get("LO_PROGRAM_PATH").map(PathBuf::from))
            .or_else(|| env.get("LOK_PROGRAM_PATH").map(PathBuf::from));

        let log_level = pick_string(args.log_level.as_deref(), env, "RUST_LOG", "info");

        let log_format = match args.log_format {
            Some(f) => f,
            None => match env.get("PDFBRO_LOG_FORMAT").map(|s| s.as_str()) {
                Some("json") => LogFormat::Json,
                Some("text") => LogFormat::Text,
                Some(other) => {
                    return Err(ConfigError::Parse {
                        field: "log_format",
                        message: format!("expected `text` or `json`, got `{other}`"),
                    });
                }
                None => default_log_format(),
            },
        };

        // Batch config defaults (can be extended with CLI/env later)
        let batch_max_items: usize = env
            .get("PDFBRO_BATCH_MAX_ITEMS")
            .and_then(|v| v.parse().ok())
            .unwrap_or(50);
        let batch_concurrency: usize = env
            .get("PDFBRO_BATCH_CONCURRENCY")
            .and_then(|v| v.parse().ok())
            .unwrap_or(4);
        let batch_max_active: usize = env
            .get("PDFBRO_BATCH_MAX_ACTIVE")
            .and_then(|v| v.parse().ok())
            .unwrap_or(10);
        let batch_retention_minutes: u64 = env
            .get("PDFBRO_BATCH_RETENTION_MINUTES")
            .and_then(|v| v.parse().ok())
            .unwrap_or(60);
        let batch_storage_path: PathBuf = env
            .get("PDFBRO_BATCH_STORAGE_PATH")
            .map(PathBuf::from)
            .unwrap_or_else(|| std::path::PathBuf::from("/tmp/pdfbro-batches"));

        let otel_enabled = args.otel_enabled
            || env.get("PDFBRO_OTEL_ENABLED").map(|v| is_truthy(v)).unwrap_or(false);

        let otel_endpoint = pick_string(
            args.otel_endpoint.as_deref(),
            env,
            "OTEL_EXPORTER_OTLP_ENDPOINT",
            "http://localhost:4318/v1/traces",
        );

        // === Engine Supervision Config Resolution ===
        let chromium_lazy_start = args.chromium_lazy_start
            || env.get("CHROMIUM_LAZY_START").map(|v| is_truthy(v)).unwrap_or(false);
        let chromium_idle_shutdown_timeout = args.chromium_idle_shutdown_timeout.as_deref()
            .or_else(|| env.get("CHROMIUM_IDLE_SHUTDOWN_TIMEOUT").map(|v| v.as_str()))
            .and_then(|v| {
                humantime::parse_duration(v)
                    .ok()
                    .filter(|d| !d.is_zero())
            });

        let libreoffice_lazy_start = args.libreoffice_lazy_start
            || env.get("LIBREOFFICE_LAZY_START").map(|v| is_truthy(v)).unwrap_or(false);
        let libreoffice_idle_shutdown_timeout = args.libreoffice_idle_shutdown_timeout.as_deref()
            .or_else(|| env.get("LIBREOFFICE_IDLE_SHUTDOWN_TIMEOUT").map(|v| v.as_str()))
            .and_then(|v| {
                humantime::parse_duration(v)
                    .ok()
                    .filter(|d| !d.is_zero())
            });

        // === API Server Config Resolution ===
        let api_disable_health_route_telemetry = args.api_disable_health_route_telemetry
            || env.get("API_DISABLE_HEALTH_ROUTE_TELEMETRY").map(|v| is_truthy(v)).unwrap_or(false);
        let api_disable_root_route_telemetry = args.api_disable_root_route_telemetry
            || env.get("API_DISABLE_ROOT_ROUTE_TELEMETRY").map(|v| is_truthy(v)).unwrap_or(false);
        let api_disable_debug_route_telemetry = args.api_disable_debug_route_telemetry
            || env.get("API_DISABLE_DEBUG_ROUTE_TELEMETRY").map(|v| is_truthy(v)).unwrap_or(false);
        let api_disable_version_route_telemetry = args.api_disable_version_route_telemetry
            || env.get("API_DISABLE_VERSION_ROUTE_TELEMETRY").map(|v| is_truthy(v)).unwrap_or(false);
        let api_enable_debug_route = args.api_enable_debug_route
            || env.get("API_ENABLE_DEBUG_ROUTE").map(|v| is_truthy(v)).unwrap_or(false);
        let api_tls_cert_file = args.api_tls_cert_file.clone()
            .or_else(|| env.get("API_TLS_CERT_FILE").map(PathBuf::from));
        let api_tls_key_file = args.api_tls_key_file.clone()
            .or_else(|| env.get("API_TLS_KEY_FILE").map(PathBuf::from));
        let api_basic_auth_username = args.api_basic_auth_username.clone()
            .or_else(|| env.get("API_BASIC_AUTH_USERNAME").cloned());
        let api_basic_auth_password = args.api_basic_auth_password.clone()
            .or_else(|| env.get("API_BASIC_AUTH_PASSWORD").cloned());

        let api_download_from_allow_list = if !args.api_download_from_allow_list.is_empty() {
            args.api_download_from_allow_list.clone()
        } else {
            env.get("API_DOWNLOAD_FROM_ALLOW_LIST")
                .map(|v| v.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect())
                .unwrap_or_default()
        };

        let api_download_from_deny_list = if !args.api_download_from_deny_list.is_empty() {
            args.api_download_from_deny_list.clone()
        } else {
            env.get("API_DOWNLOAD_FROM_DENY_LIST")
                .map(|v| v.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect())
                .unwrap_or_default()
        };

        let api_download_from_max_retry = match args.api_download_from_max_retry {
            Some(n) => n,
            None => env
                .get("API_DOWNLOAD_FROM_MAX_RETRY")
                .and_then(|v| v.parse::<u32>().ok())
                .unwrap_or(3),
        };

        let api_disable_download_from = args.api_disable_download_from
            || env.get("API_DISABLE_DOWNLOAD_FROM").map(|v| is_truthy(v)).unwrap_or(false);

        let api_correlation_id_header = args.api_correlation_id_header
            .clone()
            .or_else(|| env.get("API_CORRELATION_ID_HEADER").cloned())
            .unwrap_or_else(|| "x-request-id".to_string());

        // Validate the header name early so the server refuses to start on
        // invalid config rather than panicking at request time.
        axum::http::HeaderName::from_bytes(api_correlation_id_header.as_bytes())
            .map_err(|_| ConfigError::Parse {
                field: "api_correlation_id_header",
                message: format!("`{}` is not a valid HTTP header name", api_correlation_id_header),
            })?;

        let api_root_path = args
            .api_root_path
            .clone()
            .or_else(|| env.get("API_ROOT_PATH").cloned())
            .map(|s| s.trim().to_string())
            .unwrap_or_default();
        let api_root_path = normalize_root_path(&api_root_path).map_err(|message| {
            ConfigError::Parse {
                field: "api_root_path",
                message,
            }
        })?;

        let webhook_max_retry = args
            .webhook_max_retry
            .or_else(|| env.get("WEBHOOK_MAX_RETRY").and_then(|v| v.parse().ok()))
            .unwrap_or(4);
        if webhook_max_retry == 0 {
            return Err(ConfigError::Parse {
                field: "webhook_max_retry",
                message: "must be >= 1".to_string(),
            });
        }
        let webhook_retry_min_wait = parse_duration_opt(
            &args.webhook_retry_min_wait,
            env.get("WEBHOOK_RETRY_MIN_WAIT").map(String::as_str),
            Duration::from_secs(1),
            "webhook_retry_min_wait",
        )?;
        let webhook_retry_max_wait = parse_duration_opt(
            &args.webhook_retry_max_wait,
            env.get("WEBHOOK_RETRY_MAX_WAIT").map(String::as_str),
            Duration::from_secs(30),
            "webhook_retry_max_wait",
        )?;
        if webhook_retry_max_wait < webhook_retry_min_wait {
            return Err(ConfigError::Parse {
                field: "webhook_retry_max_wait",
                message: format!(
                    "must be >= webhook_retry_min_wait ({:?})",
                    webhook_retry_min_wait
                ),
            });
        }
        let webhook_client_timeout = parse_duration_opt(
            &args.webhook_client_timeout,
            env.get("WEBHOOK_CLIENT_TIMEOUT").map(String::as_str),
            Duration::from_secs(30),
            "webhook_client_timeout",
        )?;

        Ok(Self {
            host,
            port,
            concurrency,
            max_body_bytes,
            request_timeout,
            chrome_path,
            no_sandbox,
            lo_program_dir,
            log_level,
            log_format,
            batch_max_items,
            batch_concurrency,
            batch_max_active,
            batch_retention_minutes,
            batch_storage_path,
            otel_enabled,
            otel_endpoint,
            chromium_lazy_start,
            chromium_idle_shutdown_timeout,
            libreoffice_lazy_start,
            libreoffice_idle_shutdown_timeout,
            api_disable_health_route_telemetry,
            api_disable_root_route_telemetry,
            api_disable_debug_route_telemetry,
            api_disable_version_route_telemetry,
            api_enable_debug_route,
            api_tls_cert_file,
            api_tls_key_file,
            api_basic_auth_username,
            api_basic_auth_password,
            api_download_from_allow_list,
            api_download_from_deny_list,
            api_download_from_max_retry,
            api_disable_download_from,
            api_correlation_id_header,
            api_root_path,
            webhook_max_retry,
            webhook_retry_min_wait,
            webhook_retry_max_wait,
            webhook_client_timeout,
            webhook_allow_list: args.webhook_allow_list.clone(),
            webhook_deny_list: args.webhook_deny_list.clone(),
        })
    }

    /// Convenience wrapper that pulls env from `std::env::vars_os`.
    pub fn from_args(args: &ServerArgs) -> Result<Self, ConfigError> {
        let env: HashMap<String, String> = std::env::vars_os()
            .filter_map(|(k, v)| {
                let k: OsString = k;
                let v: OsString = v;
                Some((k.into_string().ok()?, v.into_string().ok()?))
            })
            .collect();
        Self::resolve(args, &env)
    }
}

fn pick_string(
    flag: Option<&str>,
    env: &HashMap<String, String>,
    env_key: &str,
    default: &str,
) -> String {
    if let Some(v) = flag {
        return v.to_string();
    }
    if let Some(v) = env.get(env_key) {
        return v.clone();
    }
    default.to_string()
}

/// Normalise a user-supplied root-path. Returns `Ok("")` for empty input.
/// For non-empty input, ensures it starts with `/` and has no trailing slash.
fn normalize_root_path(s: &str) -> Result<String, String> {
    let s = s.trim();
    if s.is_empty() {
        return Ok(String::new());
    }
    if !s.starts_with('/') {
        return Err(format!("must start with `/`, got `{s}`"));
    }
    let trimmed = s.trim_end_matches('/');
    if trimmed.is_empty() {
        // Input was just "/" — equivalent to no prefix.
        return Ok(String::new());
    }
    if trimmed.contains("//") {
        return Err(format!(
            "must not contain consecutive slashes, got `{s}`"
        ));
    }
    Ok(trimmed.to_string())
}

/// Parse a humantime duration from CLI arg → env → default, mapping
/// parse errors to a structured [`ConfigError::Parse`] tagged with the
/// caller's field name.
fn parse_duration_opt(
    arg: &Option<String>,
    env_val: Option<&str>,
    default: Duration,
    field: &'static str,
) -> Result<Duration, ConfigError> {
    let raw = arg
        .as_deref()
        .or(env_val)
        .map(str::trim)
        .filter(|s| !s.is_empty());
    match raw {
        Some(s) => humantime::parse_duration(s).map_err(|e| ConfigError::Parse {
            field,
            message: e.to_string(),
        }),
        None => Ok(default),
    }
}

fn is_truthy(s: &str) -> bool {
    matches!(
        s.trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "on"
    )
}

fn default_concurrency() -> usize {
    std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4)
}

fn default_log_format() -> LogFormat {
    if std::io::IsTerminal::is_terminal(&std::io::stderr()) {
        LogFormat::Text
    } else {
        LogFormat::Json
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect()
    }

    #[test]
    fn root_path_defaults_empty() {
        let args = ServerArgs::default();
        let cfg = ServerConfig::resolve(&args, &env(&[])).unwrap();
        assert_eq!(cfg.api_root_path, "");
    }

    #[test]
    fn root_path_from_env() {
        let args = ServerArgs::default();
        let cfg =
            ServerConfig::resolve(&args, &env(&[("API_ROOT_PATH", "/pdf")])).unwrap();
        assert_eq!(cfg.api_root_path, "/pdf");
    }

    #[test]
    fn root_path_strips_trailing_slash() {
        let args = ServerArgs::default();
        let cfg =
            ServerConfig::resolve(&args, &env(&[("API_ROOT_PATH", "/pdf/")])).unwrap();
        assert_eq!(cfg.api_root_path, "/pdf");
    }

    #[test]
    fn root_path_just_slash_normalised_to_empty() {
        let args = ServerArgs::default();
        let cfg = ServerConfig::resolve(&args, &env(&[("API_ROOT_PATH", "/")])).unwrap();
        assert_eq!(cfg.api_root_path, "");
    }

    #[test]
    fn root_path_missing_leading_slash_rejected() {
        let args = ServerArgs::default();
        let err = ServerConfig::resolve(&args, &env(&[("API_ROOT_PATH", "pdf")])).unwrap_err();
        let ConfigError::Parse { field, .. } = err;
        assert_eq!(field, "api_root_path");
    }

    #[test]
    fn root_path_double_slash_rejected() {
        let args = ServerArgs::default();
        let err = ServerConfig::resolve(&args, &env(&[("API_ROOT_PATH", "/a//b")]))
            .unwrap_err();
        let ConfigError::Parse { field, .. } = err;
        assert_eq!(field, "api_root_path");
    }

    #[test]
    fn defaults_when_nothing_provided() {
        let args = ServerArgs::default();
        let cfg = ServerConfig::resolve(&args, &env(&[])).unwrap();
        assert_eq!(cfg.host.to_string(), DEFAULT_HOST);
        assert_eq!(cfg.port, DEFAULT_PORT);
        assert_eq!(cfg.max_body_bytes, DEFAULT_MAX_BODY_BYTES);
        assert_eq!(cfg.request_timeout, DEFAULT_REQUEST_TIMEOUT);
        assert_eq!(cfg.log_level, "info");
        assert_eq!(cfg.no_sandbox, None);
        assert!(cfg.chrome_path.is_none());
        assert!(cfg.lo_program_dir.is_none());
        assert!(!cfg.otel_enabled);
        assert_eq!(cfg.otel_endpoint, "http://localhost:4318/v1/traces");
    }

    #[test]
    fn env_overrides_default() {
        let args = ServerArgs::default();
        let cfg = ServerConfig::resolve(
            &args,
            &env(&[
                ("PDFBRO_HOST", "127.0.0.1"),
                ("PDFBRO_PORT", "8080"),
                ("PDFBRO_CONCURRENCY", "12"),
                ("PDFBRO_MAX_BODY", "1048576"),
                ("PDFBRO_REQUEST_TIMEOUT", "30s"),
                ("CHROME_PATH", "/opt/chrome"),
                ("LO_PROGRAM_PATH", "/opt/libreoffice/program"),
                ("RUST_LOG", "debug"),
                ("PDFBRO_LOG_FORMAT", "json"),
                ("PDFBRO_NO_SANDBOX", "true"),
            ]),
        )
        .unwrap();
        assert_eq!(cfg.host.to_string(), "127.0.0.1");
        assert_eq!(cfg.port, 8080);
        assert_eq!(cfg.concurrency, 12);
        assert_eq!(cfg.max_body_bytes, 1_048_576);
        assert_eq!(cfg.request_timeout, Duration::from_secs(30));
        assert_eq!(
            cfg.chrome_path.as_deref().map(|p| p.to_str().unwrap()),
            Some("/opt/chrome")
        );
        assert_eq!(
            cfg.lo_program_dir.as_deref().map(|p| p.to_str().unwrap()),
            Some("/opt/libreoffice/program")
        );
        assert_eq!(cfg.log_level, "debug");
        assert_eq!(cfg.log_format, LogFormat::Json);
        assert_eq!(cfg.no_sandbox, Some(true));
        assert!(!cfg.otel_enabled);
        assert_eq!(cfg.otel_endpoint, "http://localhost:4318/v1/traces");
    }

    #[test]
    fn flag_beats_env_beats_default() {
        let args = ServerArgs {
            host: Some("10.0.0.1".to_string()),
            port: Some(9000),
            request_timeout: Some("5s".to_string()),
            no_sandbox: true,
            log_format: Some(LogFormat::Text),
            ..ServerArgs::default()
        };
        let cfg = ServerConfig::resolve(
            &args,
            &env(&[
                ("PDFBRO_HOST", "127.0.0.1"),
                ("PDFBRO_PORT", "8080"),
                ("PDFBRO_REQUEST_TIMEOUT", "30s"),
                ("PDFBRO_NO_SANDBOX", "false"),
                ("PDFBRO_LOG_FORMAT", "json"),
            ]),
        )
        .unwrap();
        // Flags win.
        assert_eq!(cfg.host.to_string(), "10.0.0.1");
        assert_eq!(cfg.port, 9000);
        assert_eq!(cfg.request_timeout, Duration::from_secs(5));
        assert_eq!(cfg.no_sandbox, Some(true));
        assert_eq!(cfg.log_format, LogFormat::Text);
        assert!(!cfg.otel_enabled);
        assert_eq!(cfg.otel_endpoint, "http://localhost:4318/v1/traces");
    }

    #[test]
    fn explicit_sandbox_flag_forces_off() {
        let args = ServerArgs {
            sandbox: true,
            ..ServerArgs::default()
        };
        let cfg = ServerConfig::resolve(&args, &env(&[("PDFBRO_NO_SANDBOX", "true")])).unwrap();
        assert_eq!(cfg.no_sandbox, Some(false));
    }

    #[test]
    fn invalid_host_is_parse_error() {
        let args = ServerArgs {
            host: Some("not-an-ip".to_string()),
            ..ServerArgs::default()
        };
        let err = ServerConfig::resolve(&args, &env(&[])).unwrap_err();
        let ConfigError::Parse { field, .. } = err;
        assert_eq!(field, "host");
    }

    #[test]
    fn invalid_log_format_env_rejected() {
        let args = ServerArgs::default();
        let err = ServerConfig::resolve(&args, &env(&[("PDFBRO_LOG_FORMAT", "yaml")])).unwrap_err();
        let ConfigError::Parse { field, .. } = err;
        assert_eq!(field, "log_format");
    }

    #[test]
    fn zero_concurrency_rejected() {
        let args = ServerArgs {
            concurrency: Some(0),
            ..ServerArgs::default()
        };
        let err = ServerConfig::resolve(&args, &env(&[])).unwrap_err();
        let ConfigError::Parse { field, .. } = err;
        assert_eq!(field, "concurrency");
    }

    #[test]
    fn truthy_sandbox_env_values() {
        for v in &["1", "true", "TRUE", "yes", "on"] {
            let cfg =
                ServerConfig::resolve(&ServerArgs::default(), &env(&[("PDFBRO_NO_SANDBOX", v)]))
                    .unwrap();
            assert_eq!(cfg.no_sandbox, Some(true), "value: {v}");
        }
        let cfg = ServerConfig::resolve(&ServerArgs::default(), &env(&[("PDFBRO_NO_SANDBOX", "0")]))
            .unwrap();
        assert_eq!(cfg.no_sandbox, Some(false));
    }

    #[test]
    fn api_server_flags_default() {
        let args = ServerArgs::default();
        let cfg = ServerConfig::resolve(&args, &env(&[])).unwrap();
        assert!(!cfg.api_disable_health_route_telemetry);
        assert!(!cfg.api_disable_root_route_telemetry);
        assert!(!cfg.api_disable_debug_route_telemetry);
        assert!(!cfg.api_disable_version_route_telemetry);
        assert!(!cfg.api_enable_debug_route);
        assert!(cfg.api_tls_cert_file.is_none());
        assert!(cfg.api_tls_key_file.is_none());
        assert!(cfg.api_basic_auth_username.is_none());
        assert!(cfg.api_basic_auth_password.is_none());
    }

    #[test]
    fn api_server_flags_from_env() {
        let args = ServerArgs::default();
        let cfg = ServerConfig::resolve(
            &args,
            &env(&[
                ("API_DISABLE_HEALTH_ROUTE_TELEMETRY", "true"),
                ("API_DISABLE_ROOT_ROUTE_TELEMETRY", "true"),
                ("API_DISABLE_DEBUG_ROUTE_TELEMETRY", "true"),
                ("API_DISABLE_VERSION_ROUTE_TELEMETRY", "true"),
                ("API_ENABLE_DEBUG_ROUTE", "true"),
                ("API_TLS_CERT_FILE", "/etc/tls/cert.pem"),
                ("API_TLS_KEY_FILE", "/etc/tls/key.pem"),
                ("API_BASIC_AUTH_USERNAME", "admin"),
                ("API_BASIC_AUTH_PASSWORD", "secret"),
            ]),
        )
        .unwrap();
        assert!(cfg.api_disable_health_route_telemetry);
        assert!(cfg.api_disable_root_route_telemetry);
        assert!(cfg.api_disable_debug_route_telemetry);
        assert!(cfg.api_disable_version_route_telemetry);
        assert!(cfg.api_enable_debug_route);
        assert_eq!(cfg.api_tls_cert_file.as_deref().map(|p| p.to_str().unwrap()), Some("/etc/tls/cert.pem"));
        assert_eq!(cfg.api_tls_key_file.as_deref().map(|p| p.to_str().unwrap()), Some("/etc/tls/key.pem"));
        assert_eq!(cfg.api_basic_auth_username, Some("admin".to_string()));
        assert_eq!(cfg.api_basic_auth_password, Some("secret".to_string()));
    }

    #[test]
    fn download_from_defaults() {
        let args = ServerArgs::default();
        let cfg = ServerConfig::resolve(&args, &env(&[])).unwrap();
        assert!(cfg.api_download_from_allow_list.is_empty());
        assert!(cfg.api_download_from_deny_list.is_empty());
        assert_eq!(cfg.api_download_from_max_retry, 3);
        assert!(!cfg.api_disable_download_from);
    }

    #[test]
    fn download_from_from_env() {
        let args = ServerArgs::default();
        let cfg = ServerConfig::resolve(
            &args,
            &env(&[
                ("API_DOWNLOAD_FROM_ALLOW_LIST", "https://example\\.com,https://cdn\\."),
                ("API_DOWNLOAD_FROM_DENY_LIST", "http://"),
                ("API_DOWNLOAD_FROM_MAX_RETRY", "5"),
                ("API_DISABLE_DOWNLOAD_FROM", "true"),
            ]),
        )
        .unwrap();
        assert_eq!(cfg.api_download_from_allow_list, vec!["https://example\\.com", "https://cdn\\."]);
        assert_eq!(cfg.api_download_from_deny_list, vec!["http://"]);
        assert_eq!(cfg.api_download_from_max_retry, 5);
        assert!(cfg.api_disable_download_from);
    }

    #[test]
    fn api_server_flags_cli_beats_env() {
        let args = ServerArgs {
            api_disable_health_route_telemetry: true,
            api_enable_debug_route: true,
            api_basic_auth_username: Some("superuser".to_string()),
            ..ServerArgs::default()
        };
        let cfg = ServerConfig::resolve(
            &args,
            &env(&[
                ("API_DISABLE_HEALTH_ROUTE_TELEMETRY", "false"),
                ("API_ENABLE_DEBUG_ROUTE", "false"),
                ("API_BASIC_AUTH_USERNAME", "admin"),
            ]),
        )
        .unwrap();
        assert!(cfg.api_disable_health_route_telemetry);
        assert!(cfg.api_enable_debug_route);
        assert_eq!(cfg.api_basic_auth_username, Some("superuser".to_string()));
    }

    #[test]
    fn correlation_id_header_defaults_to_x_request_id() {
        let args = ServerArgs::default();
        let cfg = ServerConfig::resolve(&args, &env(&[])).unwrap();
        assert_eq!(cfg.api_correlation_id_header, "x-request-id");
    }

    #[test]
    fn correlation_id_header_custom_value() {
        let args = ServerArgs::default();
        let cfg = ServerConfig::resolve(
            &args,
            &env(&[("API_CORRELATION_ID_HEADER", "x-trace-id")]),
        )
        .unwrap();
        assert_eq!(cfg.api_correlation_id_header, "x-trace-id");
    }

    #[test]
    fn correlation_id_header_invalid_value_rejected() {
        let args = ServerArgs::default();
        let err = ServerConfig::resolve(
            &args,
            &env(&[("API_CORRELATION_ID_HEADER", "has spaces bad")]),
        )
        .unwrap_err();
        let ConfigError::Parse { field, .. } = err;
        assert_eq!(field, "api_correlation_id_header");
    }
}
