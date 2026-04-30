//! `/forms/batch/*` route handlers for batch conversion API.
//!
//! Three endpoints:
//! - `POST /forms/batch/submit` - Submit a new batch
//! - `GET /forms/batch/{id}/status` - Query batch status
//! - `GET /forms/batch/{id}/download` - Download batch results

use axum::body::Body;
use axum::extract::{Multipart, Path, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::Response;

use crate::error::{ApiError, ApiResult};
use crate::multipart::FormFields;
use crate::routes::batch_types::*;
use crate::state::AppState;

/// `POST /forms/batch/submit` - Submit a new batch for processing.
///
/// Request: multipart/form-data with:
/// - `batch.json` field containing the batch request JSON
/// - Additional files referenced in the request
///
/// Response: `BatchSubmitResponse` with batch ID for polling.
pub async fn batch_submit(
    State(state): State<AppState>,
    _headers: HeaderMap,
    mp: Multipart,
) -> ApiResult<Response> {
    // Check max active batches
    let batch_manager = state
        .batch_manager
        .as_ref()
        .ok_or_else(|| ApiError::Internal("batch system not initialized".into()))?;

    let active_count = batch_manager.active_count().await;
    if active_count >= state.config.batch_max_active {
        return Err(ApiError::InvalidField {
            field: "server",
            message: format!(
                "server at capacity ({} active batches), try again later",
                active_count
            ),
        });
    }

    // Parse multipart
    let form = FormFields::from_multipart(mp).await?;

    // Extract batch.json
    let batch_json = form
        .map
        .get("batch.json")
        .ok_or_else(|| ApiError::MissingField("batch.json"))?;

    let request: BatchRequest = serde_json::from_str(batch_json).map_err(|e| {
        ApiError::InvalidField {
            field: "batch.json",
            message: format!("invalid JSON: {e}"),
        }
    })?;

    // Validate request
    request.validate(&form.files)?;

    // Create batch
    let batch_state = batch_manager.create_batch(request.clone()).await;
    let batch_id = batch_state.id.clone();

    // Start background processing
    let state_manager = batch_manager.clone();
    let app_state = state.clone();
    tokio::spawn(async move {
        crate::batch_worker::process_batch(batch_id.clone(), state_manager, app_state).await;
    });

    // Build response
    let response = BatchSubmitResponse {
        batch_id: batch_state.id.clone(),
        status: BatchStatus::Queued,
        expires_at: batch_state.expires_at,
    };

    let body = serde_json::to_vec(&response).map_err(|e| {
        ApiError::Internal(format!("failed to serialize response: {e}"))
    })?;

    Ok(Response::builder()
        .status(StatusCode::ACCEPTED)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body))
        .unwrap())
}

/// `GET /forms/batch/{id}/status` - Query batch processing status.
///
/// Response: `BatchStatusResponse` with current progress and results.
pub async fn batch_status(
    State(state): State<AppState>,
    Path(batch_id): Path<String>,
) -> ApiResult<Response> {
    let batch_manager = state
        .batch_manager
        .as_ref()
        .ok_or_else(|| ApiError::Internal("batch system not initialized".into()))?;

    if !is_valid_batch_id(&batch_id) {
        return Err(ApiError::NotFound);
    }
    let batch_id = BatchId::from_raw(batch_id);

    let batch_state = batch_manager
        .get_batch(&batch_id)
        .await
        .ok_or(ApiError::NotFound)?;

    // Build base URL for download link
    let base_url = format!("http://localhost:{}", state.config.port);

    let response = batch_state.to_status_response(&base_url);

    let body = serde_json::to_vec(&response).map_err(|e| {
        ApiError::Internal(format!("failed to serialize response: {e}"))
    })?;

    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body))
        .unwrap())
}

/// `GET /forms/batch/{id}/download` - Download batch results.
///
/// Returns the ZIP or merged PDF file. Returns 410 Gone if already
/// downloaded or expired.
pub async fn batch_download(
    State(state): State<AppState>,
    Path(batch_id): Path<String>,
) -> ApiResult<Response> {
    let batch_manager = state
        .batch_manager
        .as_ref()
        .ok_or_else(|| ApiError::Internal("batch system not initialized".into()))?;

    if !is_valid_batch_id(&batch_id) {
        return Err(ApiError::NotFound);
    }
    let batch_id = BatchId::from_raw(batch_id);

    let batch_state = batch_manager
        .get_batch(&batch_id)
        .await
        .ok_or(ApiError::NotFound)?;

    // Check batch is complete
    if batch_state.status != BatchStatus::Completed {
        return Err(ApiError::InvalidField {
            field: "status",
            message: format!("batch not complete (status: {:?})", batch_state.status),
        });
    }

    // Get output file path
    let output_path = batch_state
        .output_path
        .as_ref()
        .ok_or_else(|| ApiError::Internal("batch has no output".into()))?;

    // Check file exists
    if !tokio::fs::try_exists(output_path).await.unwrap_or(false) {
        return Err(ApiError::Gone);
    }

    // Read file
    let contents = tokio::fs::read(output_path).await.map_err(|e| {
        ApiError::Internal(format!("failed to read output: {e}"))
    })?;

    // Determine content type
    let content_type = match batch_state.request.output_mode {
        OutputMode::Zip => "application/zip",
        OutputMode::Merge => "application/pdf",
    };

    // Determine filename
    let filename = format!("{}.{}", batch_id, match batch_state.request.output_mode {
        OutputMode::Zip => "zip",
        OutputMode::Merge => "pdf",
    });

    // Remove batch after download (one-time download)
    batch_manager.remove_batch(&batch_id).await;

    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, content_type)
        .header(
            header::CONTENT_DISPOSITION,
            format!(r#"attachment; filename="{}""#, filename),
        )
        .body(Body::from(contents))
        .unwrap())
}

/// Validate a batch ID string format.
fn is_valid_batch_id(s: &str) -> bool {
    s.starts_with("batch_") && s.len() > 7
}

// We need a way to construct BatchId from string - add to batch_types.rs
// For now, implement directly

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_batch_id_validation() {
        // Valid IDs start with batch_
        assert!(is_valid_batch_id("batch_abc123"));
        // Invalid IDs
        assert!(!is_valid_batch_id("invalid"));
        assert!(!is_valid_batch_id(""));
        assert!(!is_valid_batch_id("batch_")); // too short
    }
}
