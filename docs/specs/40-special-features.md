# Spec 40 — Special Features

> Advanced features that Gotenberg supports but Folio is missing:
> downloading files from remote URLs, Basic Authentication, TLS,
> Cloud Run/Lambda support, and URL allow/deny lists.

## Goal

Implement special features that enable Folio to be deployed
in production environments with security, cloud integration,
and remote file access capabilities.

## Scope

**In:**

### 1. Download from Remote URLs

- Download files from HTTP/HTTPS URLs for conversion
- Support S3, GCS, Azure Blob URLs
- Timeout and retry logic
- Size limit for downloads

### 2. Basic Authentication

- HTTP basic auth for API endpoints
- Configurable username/password
- Exempt health/version endpoints

### 3. TLS Support

- HTTPS listener with cert/key
- Auto-redirect HTTP to HTTPS
- TLS version configuration

### 4. Cloud Deployment

- Cloud Run (GCP) configuration
- AWS Lambda handler
- Health check endpoints for load balancers

### 5. URL Allow/Deny Lists (Security)

- Regex-based URL filtering
- Separate allow and deny lists
- Deny list takes precedence

**Out:**

- OAuth2/OpenID Connect (complex, separate feature)
- mTLS client certificates (nice to have)
- Rate limiting (separate feature)

## 1. Download from Remote URLs

### Gotenberg Implementation

| Field | Gotenberg Source | Description |
|-------|------------------|-------------|
| Download from URL | `pkg/modules/chromium/chromium.go:~L500-600` | Uses `download.FromURL()` |

### Implementation

#### New Endpoint: `POST /forms/chromium/convert/url` (extend existing)

Already accepts `url` field. Need to:
1. Download URL content to temp file
2. Convert downloaded file

#### New Feature: Download Files from URLs in Multipart

```rust
// crates/server/src/routes/chromium.rs

use reqwest::Client;

async fn download_url(url: &str, max_size: u64) -> Result<Vec<u8>, EngineError> {
    let client = Client::new();
    let response = client.get(url)
        .send()
        .await
        .map_err(|e| EngineError::Navigation {
            url: url.into(),
            reason: format!("Download failed: {}", e),
        })?;

    // Check content length
    if let Some(len) = response.content_length() {
        if len > max_size {
            return Err(EngineError::InvalidOption(
                format!("File too large: {} bytes", len)
            ));
        }
    }

    let bytes = response.bytes()
        .await
        .map_err(|e| EngineError::Navigation {
            url: url.into(),
            reason: format!("Download failed: {}", e),
        })?;

    Ok(bytes.to_vec())
}
```

#### Form Field: `downloadFiles`

| Field | Type | Description |
|-------|------|-------------|
| `downloadFiles` | JSON array | URLs to download and include in conversion |

Example:
```json
[
  "https://example.com/image.png",
  "https://s3.amazonaws.com/bucket/document.pdf"
]
```

## 2. Basic Authentication

### Gotenberg Implementation

| Flag | Gotenberg Source | Description |
|------|------------------|-------------|
| `--api-basic-auth-username` | `pkg/modules/api/config.go:BasicAuthUsername` | Username |
| `--api-basic-auth-password` | `pkg/modules/api/config.go:BasicAuthPassword` | Password |

### Implementation

#### Middleware for Axum

```rust
// crates/server/src/auth.rs

use axum::middleware::Next;
use axum::http::{Request, StatusCode};
use base64::{engine::general_purpose, Engine as _};

pub async fn basic_auth_middleware(
    request: Request,
    next: Next,
    username: Option<String>,
    password: Option<String>,
) -> Result<(), StatusCode> {
    // Skip auth for health/version endpoints
    if request.uri().path() == "/health" || request.uri().path() == "/version" {
        return Ok(());
    }

    let Some(auth_header) = request.headers().get("Authorization") else {
        return Err(StatusCode::UNAUTHORIZED);
    };

    let Some(auth_str) = auth_header.to_str().ok() else {
        return Err(StatusCode::UNAUTHORIZED);
    };

    if !auth_str.starts_with("Basic ") {
        return Err(StatusCode::UNAUTHORIZED);
    }

    let encoded = &auth_str[6..];
    let Ok(decoded) = general_purpose::STANDARD.decode(encoded) else {
        return Err(StatusCode::UNAUTHORIZED);
    };

    let Ok(credentials) = String::from_utf8(decoded) else {
        return Err(StatusCode::UNAUTHORIZED);
    };

    let Some((user, pass)) = credentials.split_once(':') else {
        return Err(StatusCode::UNAUTHORIZED);
    };

    if Some(user.to_string()) == username && Some(pass.to_string()) == password {
        Ok(())
    } else {
        Err(StatusCode::UNAUTHORIZED)
    }
}
```

## 3. TLS Support

### Gotenberg Implementation

| Flag | Gotenberg Source | Description |
|------|------------------|-------------|
| `--api-tls-cert-file` | `pkg/modules/api/config.go:TlsCertFile` | TLS certificate |
| `--api-tls-key-file` | `pkg/modules/api/config.go:TlsKeyFile` | TLS private key |

### Implementation

#### TLS in Axum with `tokio-rustls`

```rust
// crates/server/src/tls.rs

use tokio_rustls::TlsAcceptor;
use rustls::{Certificate, PrivateKey, ServerConfig};
use std::fs::File;
use std::io::Read;

pub fn load_tls_config(cert_path: &Path, key_path: &Path) -> Result<ServerConfig, Box<dyn std::error::Error>> {
    // Load certificate
    let mut cert_file = File::open(cert_path)?;
    let mut cert_buf = Vec::new();
    cert_file.read_to_end(&mut cert_buf)?;
    let cert = Certificate(cert_buf);

    // Load private key
    let mut key_file = File::open(key_path)?;
    let mut key_buf = Vec::new();
    key_file.read_to_end(&mut key_buf)?;
    let key = PrivateKey(key_buf);

    let config = ServerConfig::builder()
        .with_safe_defaults()
        .with_no_client_auth()
        .with_single_cert(vec![cert], key)?;

    Ok(config)
}
```

#### Server Startup with TLS

```rust
// crates/server/src/main.rs

if let (Some(cert), Some(key)) = (&config.tls_cert_file, &config.tls_key_file) {
    // TLS mode
    let tls_config = load_tls_config(cert, key)?;
    // Bind with TLS
} else {
    // Plain HTTP mode (existing)
}
```

## 4. Cloud Deployment

### Cloud Run (GCP)

#### Gotenberg Reference

Gotenberg has pre-built Docker images for Cloud Run:
- `gcr.io/gotenberg/gotenberg:latest`
- Health check endpoint: `/health`

#### Folio Implementation

```dockerfile
# Dockerfile.cloudrun
FROM rust:1.75 as builder
WORKDIR /app
COPY . .
RUN cargo build --release -p server

FROM debian:bullseye
COPY --from=builder /app/target/release/folio-server /usr/local/bin/
RUN apt-get update && apt-get install -y chromium libreoffice
EXPOSE 8080
CMD ["folio-server", "--port", "8080"]
```

Environment variables for Cloud Run:
- `PORT=8080` (Cloud Run sets this automatically)

### AWS Lambda

#### Gotenberg Reference

Gotenberg has Lambda runtime support via `github.com/aws/aws-lambda-go`.

#### Folio Implementation (Future)

Use `lambda_runtime` crate for Rust Lambda support.

## 5. URL Allow/Deny Lists

### Gotenberg Implementation

| Flag | Gotenberg Source | Description |
|------|------------------|-------------|
| `--chromium-allow-list` | `pkg/modules/chromium/config.go:AllowList` | Allowed URL patterns |
| `--chromium-deny-list` | `pkg/modules/chromium/config.go:DenyList` | Denied URL patterns |

### Implementation

#### URL Validation

```rust
// crates/server/src/url_filter.rs

use regex::Regex;

pub struct UrlFilter {
    allow_list: Vec<Regex>,
    deny_list: Vec<Regex>,
}

impl UrlFilter {
    pub fn new(allow: &[String], deny: &[String]) -> Result<Self, regex::Error> {
        let allow_list = allow.iter()
            .map(|p| Regex::new(p))
            .collect::<Result<Vec<_>, _>>()?;

        let deny_list = deny.iter()
            .map(|p| Regex::new(p))
            .collect::<Result<Vec<_>, _>>()?;

        Ok(Self { allow_list, deny_list })
    }

    pub fn is_allowed(&self, url: &str) -> bool {
        // Check deny list first (takes precedence)
        if self.deny_list.iter().any(|re| re.is_match(url)) {
            return false;
        }

        // If allow list is empty, allow all (that aren't denied)
        if self.allow_list.is_empty() {
            return true;
        }

        // Otherwise, must be in allow list
        self.allow_list.iter().any(|re| re.is_match(url))
    }
}
```

## References to Gotenberg Source

| Feature | Gotenberg File | Line Numbers |
|---------|------------------|-------------|
| Download URLs | `pkg/modules/chromium/chromium.go` | ~L500-600 |
| Basic auth | `pkg/modules/api/api.go` | ~L100-150 |
| TLS support | `pkg/modules/api/api.go` | ~L150-200 |
| URL filter | `pkg/modules/chromium/chromium.go` | ~L600-700 |
| Cloud Run | `Dockerfile` | Full file |

To read Gotenberg source:
```bash
cd /Users/__deesh_reddy__/projects/personal_git/rust_builds/folio/gotenberg
cat pkg/modules/chromium/chromium.go | grep -A10 "FromURL"
```

## Expected Behavior

### Download from URLs
- Accept HTTP/HTTPS URLs in `downloadFiles` field
- Download to temp directory
- Apply size limit (default 50 MiB)
- Return error if download fails

### Basic Auth
- Return `401 Unauthorized` if no credentials
- Return `401` if wrong credentials
- Skip auth for `/health` and `/version`

### TLS
- Load cert/key from files
- Accept HTTPS connections
- Reject non-TLS connections (or redirect)

### URL Filtering
- Deny list checked first (higher priority)
- Allow list empty = allow all (except denied)
- Regex patterns matched against full URL

## Test Plan

### Unit Tests

- `download_url_returns_bytes`
- `download_url_exceeds_size_limit`
- `basic_auth_validates_credentials`
- `basic_auth_exempts_health_endpoint`
- `url_filter_deny_list_blocks`
- `url_filter_allow_list_permits`

### Integration Tests

- `download_and_convert_remote_html`
- `basic_auth_rejects_unauthorized_request`
- `tls_accepts_https_connections`
- `url_deny_list_blocks_navigation`

## Acceptance

- [ ] Download from remote URLs in multipart
- [ ] Basic auth middleware with exemption list
- [ ] TLS support with cert/key loading
- [ ] URL allow/deny lists with regex
- [ ] Cloud Run Dockerfile
- [ ] Unit tests for all features
- [ ] Integration tests for key scenarios
- [ ] `cargo clippy -p server -- -D warnings` clean

## References

- Gotenberg source: `/Users/__deesh_reddy__/projects/personal_git/rust_builds/folio/gotenberg/pkg/modules/`
- reqwest crate: https://docs.rs/reqwest/
- Axum TLS: https://docs.rs/axum/latest/axum/#tls
- Cloud Run: https://cloud.google.com/run/docs
- AWS Lambda Rust: https://github.com/awslabs/aws-lambda-rust-runtime
