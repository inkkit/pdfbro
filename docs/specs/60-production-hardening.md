# Spec 60 — Production Hardening & Edge Case Handling

> Comprehensive hardening spec addressing security vulnerabilities, edge cases,
> and production readiness gaps. Includes ULID migration for better identifier
> properties (lexicographic sorting, collision resistance, no special formatting).

## Goal

Close all critical security gaps and edge case holes before production deployment.
Migrate from UUIDv4 to ULID for better operational characteristics (sortable,
collision-resistant, lowercase-friendly).

## Scope

### In Scope

1. **ULID Migration** — Replace all UUIDv4 with lowercase ULID
2. **Security Hardening** — SSRF, header injection, path traversal, macro isolation
3. **Error Handling** — Timeout classification, partial success, error chains
4. **Resource Management** — Memory limits, zombie process cleanup, concurrency limits
5. **Robustness** — PDF validation, recovery modes, graceful degradation

### Out of Scope

- New features (webhook improvements, new endpoints)
- Performance optimizations
- UI/console improvements

---

## Part A: ULID Migration (UUID → ULID)

### Background

UUIDv4 has operational drawbacks:
- Not lexicographically sortable (no time ordering)
- Contains version bits that complicate parsing
- Mixed formatting (hyphens vs simple)
- 128-bit but not base32-encoded

ULID provides:
- 48-bit timestamp + 80-bit randomness = sortable
- Crockford's base32 encoding = URL-safe, lowercase
- No special characters, 26 characters fixed length
- Millisecond precision embedded

### Implementation

#### 1. Dependency Changes

```toml
# workspace Cargo.toml
[workspace.dependencies]
# Remove: uuid = { version = "1", features = ["v4"] }
ulid = "1"
```

```toml
# crates/server/Cargo.toml
[dependencies]
# Remove: uuid = { workspace = true }
ulid = { workspace = true }
```

#### 2. Type Replacements

| Location | Current | New |
|----------|---------|-----|
| `BatchId::new()` | `Uuid::new_v4().simple()` | `Ulid::new().to_string().to_lowercase()` |
| `UuidRequestId` | `uuid::Uuid::new_v4()` | `Ulid::new().to_string().to_lowercase()` |
| `WebhookJob::id` | `Uuid::new_v4()` | `Ulid::new().to_string().to_lowercase()` |

#### 3. String Format

All ULIDs must be:
- Lowercase: `01hqrqhp6qw2v3c5x7z9abcd8e`
- No hyphens or special characters
- Exactly 26 characters
- Valid Crockford base32 characters: `0123456789abcdefghjkmnpqrstvwxyz`

#### 4. Validation

```rust
use ulid::Ulid;

/// Validate a string is a valid ULID format.
pub fn is_valid_ulid(s: &str) -> bool {
    if s.len() != 26 {
        return false;
    }
    // All lowercase Crockford base32
    s.chars().all(|c| matches!(c, '0'..='9' | 'a'..='z') && !matches!(c, 'i' | 'l' | 'o' | 'u'))
}

/// Parse ULID from string with proper error.
pub fn parse_ulid(s: &str) -> Result<Ulid, ApiError> {
    if !is_valid_ulid(s) {
        return Err(ApiError::InvalidField {
            field: "id",
            message: format!("Invalid ULID format: '{}' (expected 26 lowercase chars)", s),
        });
    }
    s.parse::<Ulid>()
        .map_err(|e| ApiError::InvalidField {
            field: "id",
            message: format!("Failed to parse ULID: {}", e),
        })
}
```

#### 5. Sorting Benefits

ULIDs enable time-sorting without timestamp fields:
```rust
// Batch jobs naturally ordered by creation time
let job_ids: Vec<String> = vec![
    "01hqrqhp6qw2v3c5x7z9abcd8e", // created first
    "01hqrqhq1jg7w4d6y8z0bcde9f", // created second
    "01hqrqhqb8m8x5e7z9a1cdef0g", // created third
];
// Can sort lexicographically for chronological order
```

---

## Part B: Security Hardening

### B1: Server-Side Request Forgery (SSRF) Prevention

**Problem:** `url_to_pdf` can access internal services (`http://localhost:22/`, `http://169.254.169.254/`)

**Implementation:**

```rust
// crates/server/src/security/url_validator.rs

use std::net::{IpAddr, SocketAddr};
use tokio::net::lookup_host;

#[derive(Debug, Clone)]
pub struct UrlValidationConfig {
    /// Block these CIDR ranges (default: private/reserved)
    pub blocked_cidrs: Vec<IpNet>,
    /// Only allow these schemes (default: http, https)
    pub allowed_schemes: Vec<String>,
    /// Block these hostnames/patterns
    pub blocked_hosts: Vec<String>,
    /// Require explicit allowlist match (deny-by-default mode)
    pub allowlist_only: bool,
    /// Allowed hostnames/patterns (if allowlist_only)
    pub allowed_hosts: Vec<String>,
}

impl Default for UrlValidationConfig {
    fn default() -> Self {
        Self {
            blocked_cidrs: vec![
                "127.0.0.0/8".parse().unwrap(),      // Loopback
                "10.0.0.0/8".parse().unwrap(),       // Private
                "172.16.0.0/12".parse().unwrap(),    // Private
                "192.168.0.0/16".parse().unwrap(),   // Private
                "169.254.0.0/16".parse().unwrap(),   // Link-local
                "0.0.0.0/8".parse().unwrap(),          // Current network
                "fc00::/7".parse().unwrap(),         // IPv6 private
                "fe80::/10".parse().unwrap(),        // IPv6 link-local
                "::1/128".parse().unwrap(),          // IPv6 loopback
            ],
            allowed_schemes: vec!["http".into(), "https".into()],
            blocked_hosts: vec![
                "localhost".into(),
                "*.local".into(),
                "*.internal".into(),
            ],
            allowlist_only: false,
            allowed_hosts: vec![],
        }
    }
}

pub async fn validate_url(url: &str, config: &UrlValidationConfig) -> Result<(), ApiError> {
    let parsed = url::Url::parse(url)
        .map_err(|e| ApiError::InvalidField {
            field: "url",
            message: format!("Invalid URL: {}", e),
        })?;
    
    // Scheme check
    let scheme = parsed.scheme();
    if !config.allowed_schemes.contains(&scheme.to_string()) {
        return Err(ApiError::InvalidField {
            field: "url",
            message: format!("URL scheme '{}' not allowed (only http/https)", scheme),
        });
    }
    
    // Host extraction
    let host = parsed.host_str()
        .ok_or_else(|| ApiError::InvalidField {
            field: "url",
            message: "URL missing host".into(),
        })?;
    
    // Hostname pattern matching
    for blocked in &config.blocked_hosts {
        if host_matches_pattern(host, blocked) {
            return Err(ApiError::InvalidField {
                field: "url",
                message: format!("Host '{}' matches blocked pattern '{}'", host, blocked),
            });
        }
    }
    
    if config.allowlist_only {
        let mut allowed = false;
        for pattern in &config.allowed_hosts {
            if host_matches_pattern(host, pattern) {
                allowed = true;
                break;
            }
        }
        if !allowed {
            return Err(ApiError::InvalidField {
                field: "url",
                message: format!("Host '{}' not in allowlist", host),
            });
        }
    }
    
    // DNS resolution and IP check
    let addrs = lookup_host(format!("{}:{}", host, parsed.port().unwrap_or(80)))
        .await
        .map_err(|e| ApiError::InvalidField {
            field: "url",
            message: format!("DNS lookup failed: {}", e),
        })?;
    
    for addr in addrs {
        let ip = addr.ip();
        for cidr in &config.blocked_cidrs {
            if cidr.contains(&ip) {
                return Err(ApiError::InvalidField {
                    field: "url",
                    message: format!(
                        "URL resolves to blocked IP {} (range: {})",
                        ip, cidr
                    ),
                });
            }
        }
    }
    
    Ok(())
}

fn host_matches_pattern(host: &str, pattern: &str) -> bool {
    if pattern.starts_with("*.") {
        let suffix = &pattern[2..];
        host == suffix || host.ends_with(&format!(".{}", suffix))
    } else {
        host == pattern
    }
}
```

**Integration:**
```rust
// In chromium_url route handler
validate_url(url, &state.config.url_validation).await?;
```

**Configuration:**
```yaml
# Server config
url_validation:
  blocked_cidrs:
    - "127.0.0.0/8"
    - "10.0.0.0/8"
    - "192.168.0.0/16"
  blocked_hosts:
    - "localhost"
    - "*.internal.company.com"
  allowlist_only: false  # Set true for strict mode
  allowed_hosts: []      # Required if allowlist_only: true
```

### B2: HTTP Header Injection Prevention

**Problem:** `extraHttpHeaders` field can contain `\r\n` for response splitting

**Implementation:**

```rust
// crates/server/src/security/header_validator.rs

use axum::http::HeaderName;

/// Validate header name and value are safe.
pub fn validate_header(name: &str, value: &str) -> Result<(HeaderName, String), ApiError> {
    // Check for CRLF injection
    if name.contains('\r') || name.contains('\n') {
        return Err(ApiError::InvalidField {
            field: "extraHttpHeaders",
            message: format!("Header name contains illegal character: {:?}", name),
        });
    }
    if value.contains('\r') || value.contains('\n') {
        return Err(ApiError::InvalidField {
            field: "extraHttpHeaders",
            message: format!("Header value contains illegal character in '{}'", name),
        });
    }
    
    // Validate header name format
    let header_name = HeaderName::from_bytes(name.as_bytes())
        .map_err(|e| ApiError::InvalidField {
            field: "extraHttpHeaders",
            message: format!("Invalid header name '{}': {}", name, e),
        })?;
    
    // Block dangerous headers
    let lower_name = name.to_lowercase();
    let blocked = vec![
        "host", "content-length", "transfer-encoding",
        "connection", "keep-alive", "upgrade",
        "proxy-authorization", "proxy-authenticate",
    ];
    if blocked.contains(&lower_name.as_str()) {
        return Err(ApiError::InvalidField {
            field: "extraHttpHeaders",
            message: format!("Header '{}' cannot be overridden for security", name),
        });
    }
    
    Ok((header_name, value.to_string()))
}
```

### B3: Path Traversal Defense

**Problem:** `files` field with `../../../etc/passwd` filename

**Current Status:** `UnsafeFilename` error exists - verify coverage:

```rust
// Verify this exists and is comprehensive
crates/server/src/multipart.rs

pub fn sanitize_filename(name: &str) -> Result<String, ApiError> {
    // Must handle:
    // - "../../../etc/passwd" → reject
    // - "..\\..\\windows\\system32" → reject (Windows)
    // - "/etc/passwd" → reject (absolute)
    // - "file\x00.txt" → reject (null byte)
    // - "file..txt" → accept (not traversal)
    
    if name.contains('\0') {
        return Err(ApiError::UnsafeFilename(
            "Null byte in filename".into()
        ));
    }
    
    let name = name.replace('\\', "/");
    
    if name.starts_with('/') {
        return Err(ApiError::UnsafeFilename(
            "Absolute path in filename".into()
        ));
    }
    
    for part in name.split('/') {
        if part == ".." {
            return Err(ApiError::UnsafeFilename(
                "Path traversal detected".into()
            ));
        }
    }
    
    Ok(name)
}
```

### B4: LibreOffice Macro Isolation

**Problem:** Office files with malicious macros

**Implementation:**

```rust
// crates/engine/src/libreoffice/mod.rs

fn build_soffice_args(&self, input: &Path, outdir: &Path, user_dir: &Path) -> Vec<String> {
    vec![
        "--headless".into(),
        "--norestore".into(),
        "--nologo".into(),
        "--nodefault".into(),
        "--nofirststartwizard".into(),
        // SECURITY: Disable macros
        "--infilter".into(), "html:UTF8".into(),
        "--outfilter".into(), "pdf".into(),
        // Disable macro execution
        "-env:UserInstallation=file:///".into() + &user_dir.to_string_lossy(),
        // Macro security settings
        format!("--accept=socket,host=localhost,port={};urp;StarOffice.ServiceManager", self.port),
    ]
}

// Alternative: Use filter options to disable macros
// In the filter options JSON:
// {
//   "FilterData": {
//     "MacroExecutionMode": 0  // Never execute
//   }
// }
```

---

## Part C: Error Handling Improvements

### C1: Timeout Classification

**Current:** Single `TIMEOUT` code
**New:** Granular timeout codes

```rust
// Add to EngineError and ApiErrorResponse

pub enum TimeoutType {
    Navigation,      // Page failed to load within timeout
    Render,          // PDF generation hung
    Idle,            // Network idle not reached
    Resource,        // Specific resource load timeout
    LibreOffice,     // soffice conversion timeout
}

// Error response example:
{
  "error": "Page navigation timed out after 30s",
  "code": "NAVIGATION_TIMEOUT",
  "details": {
    "url": "https://slow-site.com",
    "timeout_ms": 30000,
    "timeout_type": "navigation"
  },
  "suggestion": "Check URL accessibility. Try increasing --request-timeout or use --wait-for-idle",
  "documentation": "https://folio.dev/docs/troubleshooting#navigation-timeout"
}
```

### C2: Partial Success with Resource Errors

**Current:** `ResourceErrors(Vec<ResourceError>)` fails entire request
**New:** Allow partial success with warnings

```rust
// New response type for partial success
#[derive(Debug, Clone, Serialize)]
pub struct ConversionResult {
    pub pdf_bytes: Vec<u8>,
    pub warnings: Vec<ResourceWarning>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ResourceWarning {
    pub url: String,
    pub status_code: Option<u16>,
    pub message: String,
    pub severity: WarningSeverity,
}

pub enum WarningSeverity {
    Info,      // Resource failed but not critical (e.g., tracking pixel)
    Warning,   // Resource failed, quality may be degraded
    Critical,  // Resource failed, consider retry
}

// New option in PdfOptions
pub struct PdfOptions {
    // ... existing ...
    /// Fail conversion if any resource fails (default: false)
    pub fail_on_resource_error: bool,
}
```

### C3: Error Chain Tracking

**Problem:** Cascading failures (CSS imports failing, causing fonts to fail)

```rust
#[derive(Debug, Clone, Serialize)]
pub struct ResourceError {
    pub url: String,
    pub status_code: Option<u16>,
    pub error: String,
    /// Errors that caused this failure (circular imports, dependencies)
    pub related_errors: Option<Vec<String>>,
    /// Original resource that triggered this chain
    pub root_cause: Option<String>,
}
```

### C4: Multiple Validation Error Collection

**Problem:** Only first validation error returned
**New:** Collect all validation errors

```rust
#[derive(Debug, Clone, Serialize)]
pub struct ValidationErrors {
    pub errors: Vec<FieldError>,
}

#[derive(Debug, Clone, Serialize)]
pub struct FieldError {
    pub field: String,
    pub message: String,
    pub value: Option<String>,
}

impl ApiError {
    pub fn validation_errors(errors: Vec<FieldError>) -> Self {
        ApiError::MultiValidation(errors)
    }
}

// Add new variant
pub enum ApiError {
    // ... existing ...
    MultiValidation(Vec<FieldError>),
}

// Response:
{
  "error": "Multiple validation errors",
  "code": "VALIDATION_ERRORS",
  "details": {
    "errors": [
      {"field": "scale", "message": "Must be between 0.1 and 2.0", "value": "5.0"},
      {"field": "paperWidth", "message": "Must be positive", "value": "-8.5"},
      {"field": "marginTop", "message": "Exceeds half page height", "value": "100"}
    ]
  }
}
```

---

## Part D: Resource Management

### D1: Memory Limits for Large Renders

```rust
// crates/engine/src/chromium/mod.rs

pub struct BrowserConfig {
    // ... existing ...
    /// Maximum memory per page in MB (default: 512)
    pub max_page_memory_mb: usize,
    /// Maximum total browser memory in MB (default: 2048)
    pub max_browser_memory_mb: usize,
}

// Chrome flags to add:
// --js-flags=--max-old-space-size=512
// --memory-model=low
// --max-memory-for-tab=524288000  // 500MB in bytes
```

### D2: Zombie Process Cleanup

```rust
// crates/engine/src/chromium/launch.rs

pub struct ChromiumEngine {
    inner: Arc<Inner>,
    shutdown_token: CancellationToken,
}

impl ChromiumEngine {
    pub async fn shutdown(self) -> EngineResult<()> {
        // Graceful shutdown attempt
        let graceful = tokio::time::timeout(
            Duration::from_secs(5),
            self.graceful_shutdown()
        ).await;
        
        if graceful.is_err() {
            // Force kill after timeout
            if let Some(pid) = self.inner.browser_pid {
                #[cfg(unix)]
                unsafe {
                    libc::kill(pid as i32, libc::SIGKILL);
                }
                #[cfg(windows)]
                {
                    // Windows process termination
                }
            }
        }
        
        Ok(())
    }
}
```

### D3: Per-Engine Concurrency Limits

```rust
// Add semaphore to ChromiumEngine

pub struct ChromiumEngine {
    inner: Arc<Inner>,
    page_semaphore: Arc<Semaphore>,  // Limit concurrent pages
}

impl ChromiumEngine {
    pub async fn launch_with(config: BrowserConfig) -> EngineResult<Self> {
        // ... existing launch ...
        let page_semaphore = Arc::new(Semaphore::new(config.max_concurrent_pages));
        
        Ok(Self {
            inner,
            page_semaphore,
        })
    }
    
    pub async fn html_to_pdf(&self, ...) -> EngineResult<Vec<u8>> {
        let _permit = self.page_semaphore.acquire().await
            .map_err(|_| EngineError::Internal("Engine shutting down".into()))?;
        
        // ... actual render ...
    }
}
```

---

## Part E: Robustness Improvements

### E1: PDF Output Validation

```rust
// crates/engine/src/pdfops/validate.rs

pub fn validate_pdf_output(bytes: &[u8]) -> EngineResult<()> {
    // Check PDF header
    if !bytes.starts_with(b"%PDF-1.") {
        return Err(EngineError::Pdf(
            "Invalid PDF header".into()
        ));
    }
    
    // Check PDF trailer
    if !bytes.windows(5).any(|w| w == b"%%EOF") {
        return Err(EngineError::Pdf(
            "PDF missing EOF marker".into()
        ));
    }
    
    // Try to load with lopdf
    let doc = lopdf::Document::load_mem(bytes)
        .map_err(|e| EngineError::Pdf(format!("PDF parse error: {}", e)))?;
    
    // Check for at least one page
    if doc.get_pages().is_empty() {
        return Err(EngineError::Pdf(
            "PDF contains no pages".into()
        ));
    }
    
    // Check for corrupted content streams
    for (page_num, page_id) in doc.get_pages() {
        if let Ok(page) = doc.get_page(page_id) {
            if let Ok(contents) = page.get_contents() {
                // Validate content stream decodes properly
                if let Err(e) = contents.decode() {
                    return Err(EngineError::Pdf(format!(
                        "Page {} content stream corrupted: {}",
                        page_num, e
                    )));
                }
            }
        }
    }
    
    Ok(())
}
```

### E2: LibreOffice Output Validation

```rust
// After LibreOffice conversion
pub async fn validate_office_output(bytes: &[u8]) -> EngineResult<()> {
    // PDF validation
    validate_pdf_output(bytes)?;
    
    // Size sanity check (empty or extremely large)
    if bytes.len() < 100 {
        return Err(EngineError::Internal(
            "LibreOffice produced empty PDF".into()
        ));
    }
    if bytes.len() > 100_000_000 {
        // 100MB limit
        return Err(EngineError::Internal(
            "LibreOffice produced oversized PDF".into()
        ));
    }
    
    // Page count sanity check for input type
    // (e.g., warn if single-page input produces 1000-page output)
    
    Ok(())
}
```

### E3: Malformed PDF Recovery

```rust
// crates/engine/src/pdfops/recover.rs

pub fn try_recover_pdf(bytes: &[u8]) -> EngineResult<Vec<u8>> {
    // Try loading with different repair strategies
    
    // Strategy 1: Try as-is
    if let Ok(doc) = lopdf::Document::load_mem(bytes) {
        return doc.save_to_bytes();
    }
    
    // Strategy 2: Repair xref table
    if let Ok(doc) = repair_xref_table(bytes) {
        return doc.save_to_bytes();
    }
    
    // Strategy 3: Rebuild from objects
    if let Ok(doc) = rebuild_pdf_objects(bytes) {
        return doc.save_to_bytes();
    }
    
    Err(EngineError::Pdf(
        "PDF too corrupted to repair".into()
    ))
}
```

---

## Part F: Server Robustness

### F1: Multipart Security Limits

```rust
// crates/server/src/multipart.rs

pub struct MultipartConfig {
    pub max_body_size: usize,        // 50 MiB default
    pub max_field_name_len: usize,   // 256 chars
    pub max_field_value_len: usize,    // 1 MiB
    pub max_file_count: usize,        // 100 files
    pub max_file_name_len: usize,     // 255 chars
}

pub async fn parse_multipart(
    mut multipart: Multipart,
    config: &MultipartConfig,
) -> Result<ParsedForm, ApiError> {
    let mut file_count = 0;
    
    while let Some(field) = multipart.next_field().await? {
        let name = field.name()
            .ok_or(ApiError::BadMultipart("Field missing name".into()))?;
        
        // Field name length check
        if name.len() > config.max_field_name_len {
            return Err(ApiError::BadMultipart(format!(
                "Field name too long: {} chars (max {})",
                name.len(), config.max_field_name_len
            )));
        }
        
        // File count limit
        if field.file_name().is_some() {
            file_count += 1;
            if file_count > config.max_file_count {
                return Err(ApiError::BadMultipart(format!(
                    "Too many files: {} (max {})",
                    file_count, config.max_file_count
                )));
            }
        }
        
        // ... rest of parsing
    }
    
    Ok(ParsedForm { ... })
}
```

### F2: Webhook Circuit Breaker

```rust
// crates/server/src/webhook/circuit_breaker.rs

pub struct CircuitBreaker {
    failures: AtomicU32,
    last_failure: AtomicU64, // Unix timestamp
    threshold: u32,
    reset_timeout: Duration,
    state: RwLock<CircuitState>,
}

pub enum CircuitState {
    Closed,      // Normal operation
    Open,       // Failing, reject fast
    HalfOpen,   // Testing if recovered
}

impl CircuitBreaker {
    pub async fn call<F, Fut>(&self, f: F) -> Result<WebhookResult, WebhookError>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<WebhookResult, WebhookError>>,
    {
        // Check state
        let state = *self.state.read().await;
        match state {
            CircuitState::Open => {
                // Check if should try half-open
                let last = self.last_failure.load(Ordering::Relaxed);
                let now = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs();
                
                if now - last > self.reset_timeout.as_secs() {
                    *self.state.write().await = CircuitState::HalfOpen;
                } else {
                    return Err(WebhookError::CircuitOpen);
                }
            }
            CircuitState::HalfOpen | CircuitState::Closed => {}
        }
        
        // Attempt call
        match f().await {
            Ok(result) => {
                self.on_success().await;
                Ok(result)
            }
            Err(e) => {
                self.on_failure().await;
                Err(e)
            }
        }
    }
}
```

### F3: Batch Per-Item Timeout

```rust
// crates/server/src/batch/mod.rs

pub struct BatchConfig {
    /// Timeout per individual item
    pub per_item_timeout: Duration,
    /// Global batch timeout
    pub global_timeout: Duration,
    /// Continue if individual items fail
    pub continue_on_error: bool,
}

pub async fn process_batch_item(
    item: &BatchItem,
    config: &BatchConfig,
) -> BatchItemResult {
    let result = tokio::time::timeout(
        config.per_item_timeout,
        process_single_item(item)
    ).await;
    
    match result {
        Ok(Ok(pdf)) => BatchItemResult::Success(pdf),
        Ok(Err(e)) => {
            if config.continue_on_error {
                BatchItemResult::Failed(e.to_string())
            } else {
                // Fail entire batch
                BatchItemResult::Abort(e.to_string())
            }
        }
        Err(_) => BatchItemResult::Timeout,
    }
}
```

### F4: Graceful Shutdown Guarantee

```rust
// crates/server/src/shutdown.rs

pub struct GracefulShutdown {
    active_requests: AtomicUsize,
    shutdown_signal: watch::Sender<bool>,
    completion_notify: Notify,
}

impl GracefulShutdown {
    pub async fn shutdown(&self, timeout: Duration) {
        // Signal shutdown
        let _ = self.shutdown_signal.send(true);
        
        // Wait for active requests with timeout
        let start = Instant::now();
        while self.active_requests.load(Ordering::Relaxed) > 0 {
            let elapsed = start.elapsed();
            if elapsed >= timeout {
                tracing::warn!(
                    "Shutdown timeout reached with {} active requests",
                    self.active_requests.load(Ordering::Relaxed)
                );
                break;
            }
            
            tokio::time::timeout(
                Duration::from_millis(100),
                self.completion_notify.notified()
            ).await.ok();
        }
        
        // Force close engines
        // ... engine shutdown ...
    }
}
```

---

## Test Plan

### Unit Tests

```rust
// tests for ULID
#[test]
fn ulid_generation_is_lowercase() {
    let id = generate_ulid();
    assert!(id.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit()));
    assert_eq!(id.len(), 26);
}

#[test]
fn ulid_sorting_is_chronological() {
    let id1 = generate_ulid();
    std::thread::sleep(Duration::from_millis(2));
    let id2 = generate_ulid();
    assert!(id1 < id2);
}

// tests for SSRF
#[tokio::test]
async fn blocks_localhost_url() {
    let config = UrlValidationConfig::default();
    assert!(validate_url("http://localhost/", &config).await.is_err());
    assert!(validate_url("http://127.0.0.1/", &config).await.is_err());
}

#[tokio::test]
async fn blocks_private_ip_ranges() {
    let config = UrlValidationConfig::default();
    let blocked = vec![
        "http://10.0.0.1/",
        "http://192.168.1.1/",
        "http://172.16.0.1/",
    ];
    for url in blocked {
        assert!(validate_url(url, &config).await.is_err(), "Should block {}", url);
    }
}

// tests for header injection
#[test]
fn blocks_crlf_in_header_name() {
    assert!(validate_header("X-Evil\r\nHost", "value").is_err());
}

#[test]
fn blocks_blocked_headers() {
    assert!(validate_header("Host", "evil.com").is_err());
    assert!(validate_header("Content-Length", "100").is_err());
}

// tests for path traversal
#[test]
fn blocks_traversal_patterns() {
    assert!(sanitize_filename("../../../etc/passwd").is_err());
    assert!(sanitize_filename("..\\..\\windows\\system32").is_err());
    assert!(sanitize_filename("/etc/passwd").is_err());
}

// tests for timeout classification
#[test]
fn timeout_type_in_response() {
    let err = EngineError::NavigationTimeout { url: "...".into() };
    let response = ApiError::from(err).to_response();
    assert_eq!(response.1.code, "NAVIGATION_TIMEOUT");
}

// tests for partial success
#[test]
fn partial_success_with_warnings() {
    let result = ConversionResult {
        pdf_bytes: vec![...],
        warnings: vec![ResourceWarning { ... }],
    };
    assert!(!result.warnings.is_empty());
}

// tests for PDF validation
#[test]
fn rejects_invalid_pdf_header() {
    assert!(validate_pdf_output(b"NOTPDF").is_err());
}

#[test]
fn rejects_missing_eof() {
    assert!(validate_pdf_output(b"%PDF-1.4\n1 0 obj").is_err());
}
```

### Integration Tests

```rust
// tests/security_ssrf.rs
#[tokio::test]
#[ignore = "requires server"]
async fn test_ssrf_protection_active() {
    // Start server with SSRF config
    let resp = client
        .post("/forms/chromium/convert/url")
        .form(&[("url", "http://localhost:3000/admin")])
        .send()
        .await;
    
    assert_eq!(resp.status(), 400);
    let body: Value = resp.json().await;
    assert_eq!(body["code"], "INVALID_OPTION");
}

// tests/graceful_shutdown.rs
#[tokio::test]
#[ignore = "requires server"]
async fn test_graceful_shutdown_completes_requests() {
    // Start long-running request
    let req = client.post("...").send();
    
    // Trigger shutdown
    send_sigterm();
    
    // Request should complete
    let result = tokio::time::timeout(Duration::from_secs(10), req).await;
    assert!(result.is_ok());
}
```

---

## Acceptance

- [ ] All UUID dependencies removed, ULID crate added
- [ ] All identifiers use lowercase 26-char ULID format
- [ ] ULID generation produces sortable, collision-resistant IDs
- [ ] SSRF validator blocks localhost, private IPs, link-local
- [ ] URL allowlist mode available for strict deployments
- [ ] Header injection validator rejects CRLF in all headers
- [ ] Dangerous headers (Host, Content-Length) blocked
- [ ] Path traversal validator covers Unix + Windows patterns
- [ ] LibreOffice macro execution disabled
- [ ] Timeout types classified (navigation, render, idle, resource)
- [ ] Partial success mode allows PDFs with resource warnings
- [ ] Multiple validation errors returned in single response
- [ ] Memory limits enforced for Chrome rendering
- [ ] Zombie Chrome processes cleaned up on shutdown
- [ ] Per-engine concurrency limits enforced
- [ ] PDF output validation catches corrupted/malformed output
- [ ] LibreOffice output validated for size and page count
- [ ] Multipart parser enforces field name, file count limits
- [ ] Webhook circuit breaker prevents retry storms
- [ ] Batch per-item timeout prevents poison pills
- [ ] Graceful shutdown waits for active requests
- [ ] All tests pass: `cargo test -p server -- --ignored`

---

## Migration Guide

### For API Consumers

| Change | Before | After |
|--------|--------|-------|
| Request ID | `550e8400-e29b-41d4-a716-446655440000` | `01hqrqhp6qw2v3c5x7z9abcd8e` |
| Batch ID | `batch_550e8400e29b41d4a716446655440000` | `batch_01hqrqhp6qw2v3c5x7z9abcd8e` |
| Error codes | `TIMEOUT` | `NAVIGATION_TIMEOUT`, `RENDER_TIMEOUT` |

### For Operators

New configuration options:
```yaml
security:
  url_validation:
    allowlist_only: false
    blocked_cidrs:
      - "127.0.0.0/8"
      - "10.0.0.0/8"
  multipart:
    max_field_name_len: 256
    max_file_count: 100
  webhooks:
    circuit_breaker_threshold: 5
    circuit_breaker_reset: 60s
```

---

## References

- ULID Spec: https://github.com/ulid/spec
- SSRF Prevention: https://cheatsheetseries.owasp.org/cheatsheets/Server_Side_Request_Forgery_Prevention_Cheat_Sheet.html
- Header Injection: https://owasp.org/www-community/attacks/HTTP_Response_Splitting
- Path Traversal: https://owasp.org/www-community/attacks/Path_Traversal
