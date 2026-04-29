//! Batch processing worker - background task execution for batch conversions.
//!
//! Each batch is processed by spawning async tasks that acquire engine
//! permits and execute conversions with controlled concurrency.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use tokio::fs;
use tokio::sync::Semaphore;
use tracing::{error, info};

use crate::error::ApiError;
use crate::routes::batch_state::BatchStateManager;
use crate::routes::batch_types::{ErrorCode, OutputMode};
use crate::state::AppState;

/// Process a batch asynchronously.
pub async fn process_batch(
    batch_id: crate::routes::batch_types::BatchId,
    state_manager: BatchStateManager,
    app_state: AppState,
) {
    info!(batch_id = %batch_id, "starting batch processing");

    // Mark batch as processing
    {
        let mut batch = match state_manager.get_batch(&batch_id).await {
            Some(b) => b,
            None => {
                error!(batch_id = %batch_id, "batch not found");
                return;
            }
        };
        batch.mark_processing();
        state_manager.update_batch(batch).await;
    }

    // Process items
    let result = process_items(batch_id.clone(), &state_manager, &app_state).await;

    // Finalize batch
    match result {
        Ok((output_path, output_size)) => {
            let mut batch = state_manager
                .get_batch(&batch_id)
                .await
                .expect("batch disappeared during processing");
            batch.mark_completed(output_path, output_size);
            state_manager.update_batch(batch).await;
            info!(
                batch_id = %batch_id,
                output_size,
                "batch completed successfully"
            );
        }
        Err(e) => {
            let mut batch = state_manager
                .get_batch(&batch_id)
                .await
                .expect("batch disappeared during processing");
            batch.mark_failed(e.to_string());
            state_manager.update_batch(batch).await;
            error!(batch_id = %batch_id, error = %e, "batch failed");
        }
    }
}

/// Process all items in a batch.
async fn process_items(
    batch_id: crate::routes::batch_types::BatchId,
    state_manager: &BatchStateManager,
    app_state: &AppState,
) -> Result<(Option<PathBuf>, u64), ApiError> {
    let batch = state_manager
        .get_batch(&batch_id)
        .await
        .ok_or_else(|| ApiError::Internal("batch not found".into()))?;

    let request = &batch.request;
    let item_count = request.items.len();

    // Create semaphore for per-batch concurrency
    let concurrency = app_state.config.batch_concurrency;
    let semaphore = Arc::new(Semaphore::new(concurrency));

    // Spawn tasks for each item
    let mut handles = Vec::with_capacity(item_count);

    for (index, item) in request.items.iter().enumerate() {
        let permit = semaphore.clone();
        let state_manager = state_manager.clone();
        let batch_id = batch_id.clone();
        let item = item.clone();
        let global_opts = request.global_options.clone();

        let handle = tokio::spawn(async move {
            // Wait for permit
            let _permit = permit.acquire().await.expect("semaphore closed");

            // Mark item as processing
            {
                let mut batch = state_manager
                    .get_batch(&batch_id)
                    .await
                    .expect("batch disappeared");
                batch.mark_item_processing(index);
                state_manager.update_batch(batch).await;
            }

            // Process the item
            let result = process_single_item(index, &item, &global_opts).await;

            // Update item result
            let update_result = {
                let mut batch = state_manager
                    .get_batch(&batch_id)
                    .await
                    .expect("batch disappeared");

                match &result {
                    Ok((ext, pages, bytes)) => {
                        batch.mark_item_success(index, ext.clone(), *pages, *bytes);
                    }
                    Err((error, code)) => {
                        batch.mark_item_error(index, error.clone(), *code);
                    }
                }
                state_manager.update_batch(batch).await;
                result
            };

            update_result
        });

        handles.push(handle);
    }

    // Wait for all items to complete
    let mut item_results = Vec::with_capacity(item_count);
    for handle in handles {
        match handle.await {
            Ok(result) => item_results.push(result),
            Err(e) => {
                item_results.push(Err((format!("task panicked: {e}"), ErrorCode::InternalError)));
            }
        }
    }

    // Check if any items succeeded
    let success_count = item_results.iter().filter(|r| r.is_ok()).count();
    if success_count == 0 {
        return Err(ApiError::Engine(engine::EngineError::Internal(
            "all items in batch failed".into(),
        )));
    }

    // Generate output based on mode
    let (output_path, output_size) = match request.output_mode {
        OutputMode::Zip => {
            create_zip_output(&batch_id, state_manager, &item_results).await?
        }
        OutputMode::Merge => {
            create_merged_output(&batch_id, state_manager, &item_results).await?
        }
    };

    Ok((Some(output_path), output_size))
}

/// Process a single batch item.
async fn process_single_item(
    _index: usize,
    item: &crate::routes::batch_types::BatchItem,
    _global_opts: &crate::routes::batch_types::GlobalOptions,
) -> Result<(String, Option<u32>, u64), (String, ErrorCode)> {
    let start = Instant::now();

    // This is a placeholder implementation
    // Actual implementation would:
    // 1. Parse merged options (global + per-item overrides)
    // 2. Acquire appropriate engine
    // 3. Execute conversion
    // 4. Return output file info

    info!(
        file = %item.file,
        item_type = ?item.item_type,
        "processing batch item"
    );

    // Simulate processing delay
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // Placeholder: pretend success
    let duration = start.elapsed();
    info!(
        file = %item.file,
        duration_ms = duration.as_millis() as u64,
        "item processed"
    );

    // Return dummy result
    let ext = item.output_extension().to_string();
    let pages = if item.item_type.is_screenshot() {
        None
    } else {
        Some(5) // Dummy page count
    };
    let bytes = 1024; // Dummy size

    Ok((ext, pages, bytes))
}

/// Create ZIP output from individual item results.
async fn create_zip_output(
    batch_id: &crate::routes::batch_types::BatchId,
    state_manager: &BatchStateManager,
    _results: &[Result<(String, Option<u32>, u64), (String, ErrorCode)>],
) -> Result<(PathBuf, u64), ApiError> {
    let output_path = state_manager.batch_output_path(batch_id, "zip").await;

    // TODO: Create actual ZIP from item output files
    // For now, create empty file as placeholder
    fs::write(&output_path, b"PK").await.map_err(|e| {
        ApiError::Internal(format!("failed to create zip: {e}"))
    })?;

    let metadata = fs::metadata(&output_path).await.map_err(|e| {
        ApiError::Internal(format!("failed to read zip metadata: {e}"))
    })?;

    Ok((output_path, metadata.len()))
}

/// Create merged PDF output from individual PDF results.
async fn create_merged_output(
    batch_id: &crate::routes::batch_types::BatchId,
    state_manager: &BatchStateManager,
    _results: &[Result<(String, Option<u32>, u64), (String, ErrorCode)>],
) -> Result<(PathBuf, u64), ApiError> {
    let output_path = state_manager.batch_output_path(batch_id, "pdf").await;

    // TODO: Use engine::pdfops::merge to combine PDFs
    // For now, create empty file as placeholder
    fs::write(&output_path, b"%PDF-1.4").await.map_err(|e| {
        ApiError::Internal(format!("failed to create merged pdf: {e}"))
    })?;

    let metadata = fs::metadata(&output_path).await.map_err(|e| {
        ApiError::Internal(format!("failed to read pdf metadata: {e}"))
    })?;

    Ok((output_path, metadata.len()))
}

/// Spawn background worker that processes batches from a queue.
pub fn spawn_batch_workers(
    _state_manager: BatchStateManager,
    _app_state: AppState,
    _worker_count: usize,
) {
    // TODO: Implement queue-based batch processing
    // For now, batches are processed immediately upon submission
    info!("batch workers spawned (immediate processing mode)");
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::routes::batch_types::*;

    #[tokio::test]
    async fn test_process_single_item_placeholder() {
        let item = BatchItem {
            file: "test.html".into(),
            item_type: BatchItemType::ChromiumHtml,
            options: ItemOptions::default(),
        };
        let globals = GlobalOptions::default();

        let result = process_single_item(0, &item, &globals).await;
        assert!(result.is_ok());

        let (ext, pages, bytes) = result.unwrap();
        assert_eq!(ext, "pdf");
        assert!(pages.is_some());
        assert!(bytes > 0);
    }

    #[tokio::test]
    async fn test_screenshot_item_no_pages() {
        let item = BatchItem {
            file: "test.html".into(),
            item_type: BatchItemType::ChromiumScreenshotHtml,
            options: ItemOptions::default(),
        };
        let globals = GlobalOptions::default();

        let result = process_single_item(0, &item, &globals).await;
        assert!(result.is_ok());

        let (ext, pages, _) = result.unwrap();
        assert_eq!(ext, "png");
        assert!(pages.is_none()); // Screenshots don't have pages
    }
}
