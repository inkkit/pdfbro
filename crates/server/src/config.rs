//! `folio-server serve` CLI surface and runtime configuration.
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

/// Top-level CLI for the `folio-server` binary.
#[derive(Debug, Parser)]
#[command(
    name = "folio-server",
    version,
    about = "Folio HTTP server (Gotenberg-compatible)",
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

/// CLI arguments for `folio-server serve`.
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

    /// Override the LibreOffice / `soffice` executable path.
    #[arg(long, value_name = "PATH")]
    pub soffice: Option<PathBuf>,

    /// Log level filter (default `info`).
    #[arg(long, value_name = "LEVEL")]
    pub log_level: Option<String>,

    /// Log output format. `text` for human-readable, `json` for structured.
    /// Default: `text` on a TTY, `json` otherwise.
    #[arg(long, value_name = "FORMAT")]
    pub log_format: Option<LogFormat>,
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
    /// Override path to `soffice`, if any.
    pub soffice_path: Option<PathBuf>,
    /// Tracing filter directive (e.g. `info`, `server=debug,tower=warn`).
    pub log_level: String,
    /// Log output format.
    pub log_format: LogFormat,
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
        let host_str = pick_string(args.host.as_deref(), env, "FOLIO_HOST", DEFAULT_HOST);
        let host = host_str.parse::<IpAddr>().map_err(|e| ConfigError::Parse {
            field: "host",
            message: e.to_string(),
        })?;

        let port = match args.port {
            Some(p) => p,
            None => match env.get("FOLIO_PORT") {
                Some(v) => v.parse::<u16>().map_err(|e| ConfigError::Parse {
                    field: "port",
                    message: e.to_string(),
                })?,
                None => DEFAULT_PORT,
            },
        };

        let concurrency = match args.concurrency {
            Some(c) => c,
            None => match env.get("FOLIO_CONCURRENCY") {
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
            None => match env.get("FOLIO_MAX_BODY") {
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
            "FOLIO_REQUEST_TIMEOUT",
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

        // --no-sandbox / --sandbox / FOLIO_NO_SANDBOX (truthy: 1/true/yes).
        let no_sandbox = if args.no_sandbox {
            Some(true)
        } else if args.sandbox {
            Some(false)
        } else {
            env.get("FOLIO_NO_SANDBOX").map(|v| is_truthy(v))
        };

        let soffice_path = args
            .soffice
            .clone()
            .or_else(|| env.get("LIBREOFFICE_PATH").map(PathBuf::from));

        let log_level = pick_string(args.log_level.as_deref(), env, "RUST_LOG", "info");

        let log_format = match args.log_format {
            Some(f) => f,
            None => match env.get("FOLIO_LOG_FORMAT").map(|s| s.as_str()) {
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

        Ok(Self {
            host,
            port,
            concurrency,
            max_body_bytes,
            request_timeout,
            chrome_path,
            no_sandbox,
            soffice_path,
            log_level,
            log_format,
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
        assert!(cfg.soffice_path.is_none());
    }

    #[test]
    fn env_overrides_default() {
        let args = ServerArgs::default();
        let cfg = ServerConfig::resolve(
            &args,
            &env(&[
                ("FOLIO_HOST", "127.0.0.1"),
                ("FOLIO_PORT", "8080"),
                ("FOLIO_CONCURRENCY", "12"),
                ("FOLIO_MAX_BODY", "1048576"),
                ("FOLIO_REQUEST_TIMEOUT", "30s"),
                ("CHROME_PATH", "/opt/chrome"),
                ("LIBREOFFICE_PATH", "/opt/soffice"),
                ("RUST_LOG", "debug"),
                ("FOLIO_LOG_FORMAT", "json"),
                ("FOLIO_NO_SANDBOX", "true"),
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
            cfg.soffice_path.as_deref().map(|p| p.to_str().unwrap()),
            Some("/opt/soffice")
        );
        assert_eq!(cfg.log_level, "debug");
        assert_eq!(cfg.log_format, LogFormat::Json);
        assert_eq!(cfg.no_sandbox, Some(true));
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
                ("FOLIO_HOST", "127.0.0.1"),
                ("FOLIO_PORT", "8080"),
                ("FOLIO_REQUEST_TIMEOUT", "30s"),
                ("FOLIO_NO_SANDBOX", "false"),
                ("FOLIO_LOG_FORMAT", "json"),
            ]),
        )
        .unwrap();
        // Flags win.
        assert_eq!(cfg.host.to_string(), "10.0.0.1");
        assert_eq!(cfg.port, 9000);
        assert_eq!(cfg.request_timeout, Duration::from_secs(5));
        assert_eq!(cfg.no_sandbox, Some(true));
        assert_eq!(cfg.log_format, LogFormat::Text);
    }

    #[test]
    fn explicit_sandbox_flag_forces_off() {
        let args = ServerArgs {
            sandbox: true,
            ..ServerArgs::default()
        };
        let cfg = ServerConfig::resolve(&args, &env(&[("FOLIO_NO_SANDBOX", "true")])).unwrap();
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
        let err = ServerConfig::resolve(&args, &env(&[("FOLIO_LOG_FORMAT", "yaml")])).unwrap_err();
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
                ServerConfig::resolve(&ServerArgs::default(), &env(&[("FOLIO_NO_SANDBOX", v)]))
                    .unwrap();
            assert_eq!(cfg.no_sandbox, Some(true), "value: {v}");
        }
        let cfg = ServerConfig::resolve(&ServerArgs::default(), &env(&[("FOLIO_NO_SANDBOX", "0")]))
            .unwrap();
        assert_eq!(cfg.no_sandbox, Some(false));
    }
}
