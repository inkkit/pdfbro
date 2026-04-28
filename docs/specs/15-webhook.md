# Spec 15 — Webhook System

> Asynchronous processing with HTTP callbacks.
> Enables non-blocking PDF operations with webhook notifications.

## Goal

Provide async request processing where Folio calls a user-provided webhook
URL when processing completes (success or error). Mirrors Gotenberg's
webhook functionality for long-running operations.

## Scope

**In:**

- Async mode via `Gotenberg-Async: true` header.
- Webhook callback via `Gotenberg-Webhook-Url` header.
- Error webhook via `Gotenberg-Webhook-Error-Url` header (optional).
- Extra HTTP headers for webhook requests.
- JSON event payload with result metadata.
- In-memory job queue (phase 1) → persistent queue (phase 2).

**Out:**

- Webhook signature verification (HMAC) — follow-up security spec.
- Webhook retry with exponential backoff — basic retry only.
- Event sourcing / webhook events endpoint — basic callback only.

## Public API (Internal)

Module path: `server::webhook`. Internal to server crate.

```rust
use axum::http::HeaderMap;

/// Webhook configuration extracted from request headers.
#[derive(Debug, Clone)]
pub struct WebhookConfig {
    /// Primary webhook URL for success notifications.
    pub webhook_url: String,
    /// Optional separate URL for error notifications.
    pub error_url: Option<String>,
    /// Extra headers to include in webhook requests.
    pub extra_headers: HeaderMap,
    /// Run synchronously even if webhooks configured (sync mode override).
    pub sync_mode: bool,
}

/// Extract webhook config from request headers.
pub fn extract_webhook_config(headers: &HeaderMap) -> Option<WebhookConfig>;

/// Job handle for async processing.
pub struct WebhookJob {
    pub id: String,
    pub operation: Operation,
    pub config: WebhookConfig,
}

/// Operations that support async/webhooks.
#[derive(Debug, Clone)]
pub enum Operation {
    ChromiumConvertHtml { html: Vec<u8>, opts: PdfOptions },
    ChromiumConvertUrl { url: String, opts: PdfOptions },
    LibreOfficeConvert { file: Vec<u8>, opts: OfficeOptions, filename: String },
    PdfMerge { files: Vec<Vec<u8>> },
    PdfSplit { file: Vec<u8>, mode: SplitMode },
    PdfConvert { file: Vec<u8>, profile: PdfAProfile },
}

/// Spawn async job and return job ID immediately.
pub async fn spawn_webhook_job(
    job: WebhookJob,
    state: AppState,
) -> Result<String, WebhookError>;

/// Deliver webhook callback with result.
pub async fn deliver_webhook(
    url: &str,
    result: &WebhookResult,
    extra_headers: &HeaderMap,
) -> Result<(), WebhookError>;

/// Webhook result payload.
#[derive(Debug, Clone, Serialize)]
pub struct WebhookResult {
    pub job_id: String,
    pub status: JobStatus,
    pub operation: String,
    pub filename: Option<String>,
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum JobStatus {
    Success,
    Error,
}
```

## HTTP API

### Headers (Request)

| Header | Required | Description |
|--------|----------|-------------|
| `Gotenberg-Async` | No | `true` to enable async mode |
| `Gotenberg-Webhook-Url` | Yes* | Webhook URL for success |
| `Gotenberg-Webhook-Error-Url` | No | Separate URL for errors |
| `Gotenberg-Webhook-Extra-Http-Headers` | No | JSON object of extra headers |

*Required if `Gotenberg-Async: true`

### Headers (Webhook Request)

Folio sends POST to webhook URL with:

| Header | Value |
|--------|-------|
| `Content-Type` | `application/json` or `application/pdf` |
| `Gotenberg-Trace` | Correlation ID from original request |
| `X-Request-Id` | Folio's request ID |
| User's extra headers | As specified |

### Response (Async Mode)

When async mode enabled, immediate response:

```http
HTTP/1.1 202 Accepted
Gotenberg-Trace: <correlation-id>

{"job_id": "uuid", "status": "pending"}
```

### Webhook Payload (Success)

```json
{
  "job_id": "uuid",
  "status": "success",
  "operation": "chromium_convert_html",
  "filename": "result.pdf",
  "duration_ms": 1234
}
```

With PDF attached as binary body, or download URL if configured for storage.

### Webhook Payload (Error)

```json
{
  "job_id": "uuid",
  "status": "error",
  "operation": "pdf_merge",
  "error": "Failed to parse PDF: invalid xref",
  "duration_ms": 500
}
```

## Implementation Strategy

### Option 1: In-Memory Queue (Phase 1)

Use `tokio::task::spawn` + `tokio::sync::mpsc` channel:

```rust
pub struct WebhookQueue {
    sender: mpsc::Sender<WebhookJob>,
    receiver: Arc<Mutex<mpsc::Receiver<WebhookJob>>>,
}
```

Pros:
- Simple, no external dependencies
- Fast for moderate load

Cons:
- Jobs lost on restart
- No horizontal scaling

### Option 2: Persistent Queue (Phase 2)

SQLite or Redis-backed queue:

```rust
pub struct PersistentQueue {
    db: SqlitePool,
}
```

Pros:
- Survives restarts
- Can scale horizontally

Cons:
- Additional dependency

### Decision

**Phase 1:** In-memory queue with optional SQLite persistence.

## Architecture

```
Request → Extract Webhook Config
       → If async: Queue Job → Return 202
       → Worker processes job
       → POST result to webhook URL
```

Worker pool:
- 4 concurrent webhook processors (configurable)
- Timeout: 30s for webhook delivery
- Retry: 3 attempts with 5s delay

## Error Handling

| Error | Action |
|-------|--------|
| Invalid webhook URL | 400 Bad Request |
| Webhook timeout | Retry 2x, then fail |
| Webhook 4xx/5xx | Retry 2x, then fail |
| Job processing error | Send to error webhook |

## Security Considerations

1. **URL validation** - Reject private IPs, localhost (configurable)
2. **SSRF protection** - DNS rebinding checks
3. **HMAC signatures** - Optional webhook signing (follow-up)
4. **Rate limiting** - Per-webhook rate limits

## Testing

Unit tests:
- Webhook config extraction from headers
- URL validation (allow/block lists)
- Job serialization/deserialization

Integration tests:
- End-to-end async conversion with webhook
- Error webhook delivery
- Retry behavior

## Dependencies

```toml
[dependencies]
# HTTP client for webhook delivery
reqwest = { version = "0.12", features = ["json"] }
# Job queue (in-memory)
tokio = { version = "1", features = ["sync", "rt"] }
# URL validation
url = "2"
```

## Open Questions

1. Should we support webhook body in sync mode too?
2. File storage for large outputs vs streaming?
3. Webhook signature verification (HMAC) priority?
4. Should we add webhook events API (list/deliveries)?

## References

- Gotenberg webhook docs: https://gotenberg.dev/docs/webhook
- CloudEvents spec for webhook payload structure
