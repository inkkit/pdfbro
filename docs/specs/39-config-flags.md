# Spec 39 — Configuration CLI Flags

> Comprehensive list of CLI flags and environment variables
> that Gotenberg supports but Folio is missing. These control
> Chromium, LibreOffice, API server, and PDF engine behavior.

## Goal

Implement all missing CLI flags and environment variables to achieve
full configuration parity with Gotenberg.

## Scope

**In:**

All missing CLI flags from Gotenberg:

### Chromium Options (16 flags)

| Flag | Env Variable | Gotenberg Source | Default | Description |
|------|-------------|------------------|---------|-------------|
| `--chromium-restart-after` | `CHROMIUM_RESTART_AFTER` | `pkg/modules/chromium/config.go:RestartAfter` | 0 (never) | Restart after N conversions |
| `--chromium-max-queue-size` | `CHROMIUM_MAX_QUEUE_SIZE` | `pkg/modules/chromium/config.go:MaxQueueSize` | 0 (unlimited) | Max queue size |
| `--chromium-max-concurrency` | `CHROMIUM_MAX_CONCURRENCY` | `pkg/modules/chromium/config.go:MaxConcurrency` | NumCPUs | Max concurrent renders |
| `--chromium-auto-start` | `CHROMIUM_AUTO_START` | `pkg/modules/chromium/config.go:AutoStart` | true | Auto-start Chromium |
| `--chromium-start-timeout` | `CHROMIUM_START_TIMEOUT` | `pkg/modules/chromium/config.go:StartTimeout` | 20s | Start timeout |
| `--chromium-allow-list` | `CHROMIUM_ALLOW_LIST` | `pkg/modules/chromium/config.go:AllowList` | (none) | Allowed URL patterns (regex) |
| `--chromium-deny-list` | `CHROMIUM_DENY_LIST` | `pkg/modules/chromium/config.go:DenyList` | (none) | Denied URL patterns (regex) |
| `--chromium-clear-cache` | `CHROMIUM_CLEAR_CACHE` | `pkg/modules/chromium/config.go:ClearCache` | false | Clear cache on restart |
| `--chromium-clear-cookies` | `CHROMIUM_CLEAR_COOKIES` | `pkg/modules/chromium/config.go:ClearCookies` | false | Clear cookies on restart |
| `--chromium-disable-javascript` | `CHROMIUM_DISABLE_JAVASCRIPT` | `pkg/modules/chromium/config.go:DisableJavascript` | false | Disable JavaScript |
| `--chromium-allow-insecure-localhost` | `CHROMIUM_ALLOW_INSECURE_LOCALHOST` | `pkg/modules/chromium/config.go:AllowInsecureLocalhost` | false | Allow insecure localhost |
| `--chromium-ignore-certificate-errors` | `CHROMIUM_IGNORE_CERTIFICATE_ERRORS` | `pkg/modules/chromium/config.go:IgnoreCertificateErrors` | false | Ignore cert errors |
| `--chromium-disable-web-security` | `CHROMIUM_DISABLE_WEB_SECURITY` | `pkg/modules/chromium/config.go:DisableWebSecurity` | false | Disable web security |
| `--chromium-allow-file-access-from-files` | `CHROMIUM_ALLOW_FILE_ACCESS_FROM_FILES` | `pkg/modules/chromium/config.go:AllowFileAccessFromFile` | false | Allow file access |
| `--chromium-host-resolver-rules` | `CHROMIUM_HOST_RESOLVER_RULES` | `pkg/modules/chromium/config.go:HostResolverRules` | (none) | Custom DNS rules |
| `--chromium-proxy-server` | `CHROMIUM_PROXY_SERVER` | `pkg/modules/chromium/config.go:ProxyServer` | (none) | Proxy server |
| `--chromium-idle-shutdown-timeout` | `CHROMIUM_IDLE_SHUTDOWN_TIMEOUT` | `pkg/modules/chromium/config.go:IdleShutdownTimeout` | 0 (disabled) | Idle shutdown timeout |

### LibreOffice Options (6 flags)

| Flag | Env Variable | Gotenberg Source | Default | Description |
|------|-------------|------------------|---------|-------------|
| `--libreoffice-restart-after` | `LIBREOFFICE_RESTART_AFTER` | `pkg/modules/libreoffice/config.go:RestartAfter` | 0 (never) | Restart after N conversions |
| `--libreoffice-max-queue-size` | `LIBREOFFICE_MAX_QUEUE_SIZE` | `pkg/modules/libreoffice/config.go:MaxQueueSize` | 0 (unlimited) | Max queue size |
| `--libreoffice-auto-start` | `LIBREOFFICE_AUTO_START` | `pkg/modules/libreoffice/config.go:AutoStart` | true | Auto-start LibreOffice |
| `--libreoffice-start-timeout` | `LIBREOFFICE_START_TIMEOUT` | `pkg/modules/libreoffice/config.go:StartTimeout` | 20s | Start timeout |
| `--libreoffice-disable-routes` | `LIBREOFFICE_DISABLE_ROUTES` | `pkg/modules/libreoffice/config.go:DisableRoutes` | false | Disable LibreOffice routes |
| `--libreoffice-idle-shutdown-timeout` | `LIBREOFFICE_IDLE_SHUTDOWN_TIMEOUT` | `pkg/modules/libreoffice/config.go:IdleShutdownTimeout` | 0 (disabled) | Idle shutdown timeout |

### API Server Options (9 flags)

| Flag | Env Variable | Gotenberg Source | Default | Description |
|------|-------------|------------------|---------|-------------|
| `--api-disable-health-route-telemetry` | `API_DISABLE_HEALTH_ROUTE_TELEMETRY` | `pkg/modules/api/config.go:DisableHealthRouteTelemetry` | false | Disable health telemetry |
| `--api-disable-root-route-telemetry` | `API_DISABLE_ROOT_ROUTE_TELEMETRY` | `pkg/modules/api/config.go:DisableRootRouteTelemetry` | false | Disable root telemetry |
| `--api-disable-debug-route-telemetry` | `API_DISABLE_DEBUG_ROUTE_TELEMETRY` | `pkg/modules/api/config.go:DisableDebugRouteTelemetry` | false | Disable debug telemetry |
| `--api-disable-version-route-telemetry` | `API_DISABLE_VERSION_ROUTE_TELEMETRY` | `pkg/modules/api/config.go:DisableVersionRouteTelemetry` | false | Disable version telemetry |
| `--api-enable-debug-route` | `API_ENABLE_DEBUG_ROUTE` | `pkg/modules/api/config.go:EnableDebugRoute` | false | Enable debug route |
| Basic auth username | `API_BASIC_AUTH_USERNAME` | `pkg/modules/api/config.go:BasicAuthUsername` | (none) | HTTP basic auth username |
| Basic auth password | `API_BASIC_AUTH_PASSWORD` | `pkg/modules/api/config.go:BasicAuthPassword` | (none) | HTTP basic auth password |
| TLS cert file | `API_TLS_CERT_FILE` | `pkg/modules/api/config.go:TlsCertFile` | (none) | TLS certificate file |
| TLS key file | `API_TLS_KEY_FILE` | `pkg/modules/api/config.go:TlsKeyFile` | (none) | TLS key file |

### PDF Engines Options (14 flags)

Already documented in spec-38, but need CLI flags:

| Flag | Env Variable |
|------|-------------|
| `--pdfengines-disable-routes` | `PDFENGINES_DISABLE_ROUTES` |
| `--pdfengines-merge-engines` | `PDFENGINES_MERGE_ENGINES` |
| `--pdfengines-split-engines` | `PDFENGINES_SPLIT_ENGINES` |
| (14 total, see spec-38) |

## Implementation

### 1. Extend `BrowserConfig` in `crates/engine/src/chromium/mod.rs`

```rust
pub struct BrowserConfig {
    // ... existing fields ...

    // Supervision
    pub restart_after: u32,           // --chromium-restart-after
    pub max_queue_size: usize,          // --chromium-max-queue-size
    pub max_concurrency: usize,         // --chromium-max-concurrency

    // Lifecycle
    pub auto_start: bool,               // --chromium-auto-start
    pub start_timeout: Duration,         // --chromium-start-timeout

    // Security
    pub allow_list: Vec<String>,        // --chromium-allow-list (regex)
    pub deny_list: Vec<String>,         // --chromium-deny-list (regex)
    pub clear_cache: bool,              // --chromium-clear-cache
    pub clear_cookies: bool,            // --chromium-clear-cookies
    pub disable_javascript: bool,       // --chromium-disable-javascript
    pub allow_insecure_localhost: bool, // --chromium-allow-insecure-localhost
    pub ignore_certificate_errors: bool, // --chromium-ignore-certificate-errors
    pub disable_web_security: bool,     // --chromium-disable-web-security
    pub allow_file_access_from_files: bool, // --chromium-allow-file-access-from-files

    // Network
    pub host_resolver_rules: Option<String>, // --chromium-host-resolver-rules
    pub proxy_server: Option<String>,       // --chromium-proxy-server

    // Idle
    pub idle_shutdown_timeout: Option<Duration>, // --chromium-idle-shutdown-timeout
}
```

### 2. Extend `LibreOfficeConfig` in `crates/engine/src/libreoffice/mod.rs`

```rust
pub struct LibreOfficeConfig {
    // ... existing fields ...

    // Supervision
    pub restart_after: u32,
    pub max_queue_size: usize,

    // Lifecycle
    pub auto_start: bool,
    pub start_timeout: Duration,

    // Routes
    pub disable_routes: bool,

    // Idle
    pub idle_shutdown_timeout: Option<Duration>,
}
```

### 3. Extend `ServerConfig` in `crates/server/src/config.rs`

```rust
pub struct ServerConfig {
    // ... existing fields ...

    // API telemetry
    pub disable_health_route_telemetry: bool,
    pub disable_root_route_telemetry: bool,
    pub disable_debug_route_telemetry: bool,
    pub disable_version_route_telemetry: bool,
    pub enable_debug_route: bool,

    // Basic auth
    pub basic_auth_username: Option<String>,
    pub basic_auth_password: Option<String>,

    // TLS
    pub tls_cert_file: Option<PathBuf>,
    pub tls_key_file: Option<PathBuf>,

    // PDF engines config
    pub pdfengines: PdfEnginesConfig,  // from spec-38
}
```

### 4. CLI Flag Definitions

```rust
// crates/server/src/config.rs

pub fn clap_app() -> Command {
    Command::new("folio-server")
        // ... existing flags ...

        // Chromium flags
        .arg(Arg::new("chromium-restart-after")
            .long("chromium-restart-after")
            .env("CHROMIUM_RESTART_AFTER")
            .default_value("0"))
        .arg(Arg::new("chromium-max-queue-size")
            .long("chromium-max-queue-size")
            .env("CHROMIUM_MAX_QUEUE_SIZE")
            .default_value("0"))
        // ... all 16 chromium flags

        // LibreOffice flags
        .arg(Arg::new("libreoffice-restart-after")
            .long("libreoffice-restart-after")
            .env("LIBREOFFICE_RESTART_AFTER")
            .default_value("0"))
        // ... all 6 libreoffice flags

        // API flags
        .arg(Arg::new("api-disable-health-route-telemetry")
            .long("api-disable-health-route-telemetry")
            .env("API_DISABLE_HEALTH_ROUTE_TELEMETRY")
            .action(clap::ArgAction::SetTrue))
        // ... all 9 API flags
}
```

## References to Gotenberg Source

| Feature | Gotenberg File | Line Numbers |
|---------|------------------|-------------|
| Chromium config | `pkg/modules/chromium/config.go` | Full file (~150 lines) |
| LibreOffice config | `pkg/modules/libreoffice/config.go` | Full file (~80 lines) |
| API config | `pkg/modules/api/config.go` | Full file (~120 lines) |
| PDF engines config | `pkg/modules/pdfengines/config.go` | Full file (~100 lines) |

To read Gotenberg source:
```bash
cd /Users/__deesh_reddy__/projects/personal_git/rust_builds/folio/gotenberg
cat pkg/modules/chromium/config.go | grep -A2 "RestartAfter"
```

## Expected Behavior

### Flag Priority
1. CLI flag (highest priority)
2. Environment variable
3. Default value (lowest priority)

### URL Allow/Deny Lists
```bash
# Only allow example.com and subdomains
--chromium-allow-list="^https://.*\.example\.com"

# Deny tracking domains
--chromium-deny-list="^https://.*\.google-analytics\.com"
```

### Idle Shutdown
```bash
# Shutdown Chromium after 10 minutes idle
--chromium-idle-shutdown-timeout=10m

# Disable idle shutdown
--chromium-idle-shutdown-timeout=0
```

### Basic Auth
```bash
# Enable HTTP basic auth
--api-basic-auth-username=admin --api-basic-auth-password=secret
```

## Test Plan

### Unit Tests

- `chromium_restart_after_parses_correctly`
- `url_allow_list_regex_matches`
- `url_deny_list_blocks_tracking`
- `basic_auth_credentials_parsed`

### Integration Tests

- `idle_shutdown_stops_chromium`
- `url_allow_list_blocks_denied`
- `basic_auth_rejects_unauthorized`

## Acceptance

- [ ] `BrowserConfig` extended with all 16 Chromium flags
- [ ] `LibreOfficeConfig` extended with all 6 LibreOffice flags
- [ ] `ServerConfig` extended with all 9 API flags
- [ ] CLI flag parsing with env var fallback
- [ ] Flag priority: CLI > env > default
- [ ] URL allow/deny list regex matching
- [ ] Basic auth middleware
- [ ] TLS support in Axum
- [ ] Unit tests for all flag parsers
- [ ] `cargo clippy -p server -- -D warnings` clean

## References

- Gotenberg config files: `/Users/__deesh_reddy__/projects/personal_git/rust_builds/folio/gotenberg/pkg/modules/*/config.go`
- clap crate: https://docs.rs/clap/
- Axum TLS: https://docs.rs/axum/latest/axum/#tls
- HTTP basic auth: https://docs.rs/axum/latest/axum/middleware/#basic-auth
