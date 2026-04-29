# Spec 50 — Batch API#

> Convert 100+ URLs/documents in one request. Gotenberg
> processes one-at-a-time. Folio's async architecture
> enables true batch processing with webhook notifications.

## Goal#

Create a batch API that processes multiple conversion
requests in parallel, with webhook notifications when
all are complete. Enables SaaS use cases and bulk
processing workflows.

## Problem Analysis#

### Current State (Gotenberg)#

**User workflow for 100 URLs:**
```bash
# User has to script this themselves
for url in $(cat urls.txt); do
  curl -X POST http://gotenberg/forms/chromium/convert/url \
    --form url=$url -o output-$(basename $url).pdf &
done
wait
```

**Problems:**
- No built-in batch support
- User has to manage parallelism
- No aggregation of results
- No webhook when all done

**User Quote (Gotenberg Discussion):**
> "I need to convert 500 URLs to PDF monthly. Wish
> Gotenberg had a batch endpoint instead of me
> writing shell scripts."
> — Gotenberg Discussion #1200

### Desired State (Folio)#

```bash
# Folio batch API
curl -X POST http://localhost:3000/batch \
  --form urls='["https://a.com", "https://b.com"]' \
  --form webhook-url=http://my-app.com/callback
```

## Scope#

**In:**

- `POST /batch` - Submit batch job
- `GET /batch/{job_id}` - Check batch status
- `GET /batch/{job_id}/download` - Download all PDFs as ZIP
- Webhook notification when batch complete
- Parallel processing (configurable concurrency)
- Job persistence (survives restart)
- Progress tracking per individual item

**Out:**

- Batch template rendering (use `/forms/templates/` instead)
- Scheduled batches (cron-like, too complex)
- Batch editing (separate feature)

## Implementation#

### 1. Batch Job Model#

```rust
// crates/server/src/batch/mod.rs#

use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchJob {
    pub id: Uuid,
    pub status: BatchStatus,
    pub created_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub items: Vec<BatchItem>,
    pub webhook_url: Option<String>,
    pub concurrency: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BatchStatus {
    Pending,
    Processing { completed: usize, total: usize },
    Completed,
    Failed { error: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchItem {
    pub id: Uuid,
    pub endpoint: String,  // e.g., "/forms/chromium/convert/url"
    pub form_data: HashMap<String, String>,
    pub status: ItemStatus,
    pub result_url: Option<String>,  // /batch/{job}/download/{item}
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ItemStatus {
    Pending,
    Processing,
    Completed,
    Failed { error: String },
}
```

### 2. Batch Submission Endpoint#

```rust
// crates/server/src/routes/batch.rs#

pub async fn submit_batch(
    State(state): State<AppState>,
    Json(req): Json<BatchRequest>,
) -> ApiResult<impl IntoResponse> {
    let job_id = Uuid::new_v4();
    
    let mut items = Vec::new();
    for (i, url) in req.urls.iter().enumerate() {
        items.push(BatchItem {
            id: Uuid::new_v4(),
            endpoint: "/forms/chromium/convert/url".into(),
            form_data: hashmap! {
                "url".into() => url.clone()
            },
            status: ItemStatus::Pending,
            result_url: Some(format!(
                "/batch/{}/download/{}/",
                job_id, i
            )),
        });
    }
    
    let job = BatchJob {
        id: job_id,
        status: BatchStatus::Pending,
        created_at: Utc::now(),
        completed_at: None,
        items,
        webhook_url: req.webhook_url,
        concurrency: req.concurrency.unwrap_or(6),
    };
    
    // Store job
    state.batch_store.store(&job).await?;
    
    // Start processing in background
    let state_clone = state.clone();
    tokio::spawn(async move {
        process_batch(state_clone, job_id).await;
    });
    
    Ok((
        StatusCode::ACCEPTED,
        Json(BatchResponse {
            job_id,
            status_url: format!("/batch/{}", job_id),
            download_url: format!("/batch/{}/download", job_id),
        }),
    ))
}
```

### 3. Batch Processing#

```rust
// crates/server/src/batch/processor.rs#

pub async fn process_batch(state: AppState, job_id: Uuid) {
    let mut job = match state.batch_store.get(job_id).await {
        Ok(j) => j,
        Err(_) => return,
    };
    
    job.status = BatchStatus::Processing {
        completed: 0,
        total: job.items.len(),
    };
    state.batch_store.store(&job).await.ok();
    
    // Process items with concurrency limit
    let sem = Arc::new(Semaphore::new(job.concurrency));
    let mut handles = Vec::new();
    
    for (i, item) in job.items.iter_mut().enumerate() {
        let permit = sem.clone().acquire_owned().await.unwrap();
        let state_clone = state.clone();
        let mut item = item.clone();
        
        let handle = tokio::spawn(async move {
            let _permit = permit;  // Hold permit until done
            item.status = ItemStatus::Processing;
            
            // Process based on endpoint
            let result = process_item(&state_clone, &item).await;
            
            match result {
                Ok(pdf_bytes) => {
                    // Store result
                    state_clone.batch_store
                        .store_result(item.id, pdf_bytes)
                        .await
                        .ok();
                    item.status = ItemStatus::Completed;
                }
                Err(e) => {
                    item.status = ItemStatus::Failed {
                        error: e.to_string(),
                    };
                }
            }
            
            (i, item)
        });
        
        handles.push(handle);
    }
    
    // Wait for all
    let mut results = Vec::new();
    for handle in handles {
        results.push(handle.await.unwrap());
    }
    
    // Update items
    for (i, item) in results {
        job.items[i] = item;
    }
    
    // Mark complete
    job.status = BatchStatus::Completed;
    job.completed_at = Some(Utc::now());
    state.batch_store.store(&job).await.ok();
    
    // Call webhook if configured
    if let Some(webhook_url) = &job.webhook_url {
        call_webhook(webhook_url, &job).await;
    }
}
```

### 4. Batch Status Endpoint#

```rust
/// Check batch job status.
pub async fn batch_status(
    Path(job_id): Path<Uuid>,
    State(state): State<AppState>,
) -> ApiResult<impl IntoResponse> {
    let job = state.batch_store.get(job_id).await
        .map_err(|_| ApiError::InvalidOption("Job not found".into()))?;
    
    Ok(Json(BatchStatusResponse {
        job_id,
        status: job.status.clone(),
        created_at: job.created_at,
        completed_at: job.completed_at,
        items: job.items.iter().map(|i| ItemSummary {
            id: i.id,
            status: i.status.clone(),
            result_url: i.result_url.clone(),
        }).collect(),
    }))
}
```

### 5. Batch Download (ZIP)#

```rust
/// Download all results as ZIP.
pub async fn batch_download(
    Path(job_id): Path<Uuid>,
    State(state): State<AppState>,
) -> ApiResult<impl IntoResponse> {
    let job = state.batch_store.get(job_id).await
        .map_err(|_| ApiError::InvalidOption("Job not found".into()))?;
    
    if !matches!(job.status, BatchStatus::Completed) {
        return Err(ApiError::InvalidOption("Job not completed".into()));
    }
    
    // Create ZIP with all PDFs
    let mut zip_data = Vec::new();
    let mut zip = zip::ZipWriter::new(std::io::Cursor::new(&mut zip_data));
    
    for (i, item) in job.items.iter().enumerate() {
        if let Some(result) = state.batch_store.get_result(item.id).await.ok() {
            let filename = format!("result-{}.pdf", i);
            zip.start_file(filename, Default::default())?;
            zip.write_all(&result)?;
        }
    }
    
    zip.finish()?;
    
    Ok((
        [(header::CONTENT_TYPE, HeaderValue::from_static("application/zip"))],
        zip_data,
    ))
}
```

## Form Fields#

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `urls` | JSON array | Yes | URLs to convert |
| `webhook_url` | string | No | Callback URL |
| `concurrency` | int | No | Max parallel (default: 6) |

## Expected Behaviour#

### Submit Batch#

```bash
curl -X POST http://localhost:3000/batch \
  -H "Content-Type: application/json" \
  -d '{
    "urls": [
      "https://example.com",
      "https://google.com"
    ],
    "webhook_url": "http://my-app.com/callback",
    "concurrency": 10
  }'
```

Response:
```json
{
  "job_id": "550e8400-e29b-41d4-a716-446655440000",
  "status_url": "/batch/550e8400-e29b-41d4-a716-446655440000",
  "download_url": "/batch/550e8400-e29b-41d4-a716-446655440000/download"
}
```

### Check Status#

```bash
curl http://localhost:3000/batch/550e8400-e29b-41d4-a716-446655440000
```

```json
{
  "job_id": "550e8400-e29b-41d4-a716-446655440000",
  "status": {
    "type": "Processing",
    "completed": 5,
    "total": 10
  },
  "items": [
    {
      "id": "...",
      "status": "Completed",
      "result_url": "/batch/.../download/0/"
    }
  ]
}
```

### Webhook Payload#

```json
POST http://my-app.com/callback
{
  "job_id": "550e8400-e29b-41d4-a716-446655440000",
  "status": "Completed",
  "download_url": "/batch/550e8400-e29b-41d4-a716-446655440000/download",
  "summary": {
    "total": 10,
    "completed": 10,
    "failed": 0
  }
}
```

## Test Plan#

### Unit Tests#

- `submit_batch_creates_job`
- `process_batch_completes_all`
- `webhook_called_on_complete`

### Integration Tests#

- `batch_100_urls_completes`
- `batch_download_returns_zip`
- `batch_status_shows_progress`
- `webhook_receives_notification`

## Acceptance#

- [ ] `POST /batch` submits batch job
- [ ] `GET /batch/{job_id}` checks status
- [ ] `GET /batch/{job_id}/download` returns ZIP
- [ ] Parallel processing with concurrency limit
- [ ] Webhook notification on complete
- [ ] Job persistence (survives restart)
- [ ] Unit tests for batch processor
- [ ] Integration tests for full workflow
- [ ] `cargo clippy -p server -- -D warnings` clean

## References#

- Gotenberg discussion #1200: https://github.com/gotenberg/gotenberg/discussions/1200
- ZIP crate: https://docs.rs/zip/
- UUID crate: https://docs.rs/uuid/
