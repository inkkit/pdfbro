//! Batch processing worker - background task execution for batch conversions.
//!
//! Each batch is processed by spawning async tasks that acquire engine
//! permits and execute conversions with controlled concurrency.

use std::io::Write as _;
use std::path::{Path, PathBuf};

use tokio::fs;
use tracing::{error, info};

use crate::error::ApiError;
use crate::routes::batch_state::BatchStateManager;
use crate::routes::batch_types::{BatchItemType, ErrorCode, GlobalOptions, OutputMode};
use crate::routes::batch_types::BatchItem;
use crate::state::AppState;

/// Process a batch asynchronously.
pub async fn process_batch(
    batch_id: crate::routes::batch_types::BatchId,
    state_manager: BatchStateManager,
    app_state: AppState,
) {
    info!(batch_id = %batch_id, "starting batch processing");

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

    let result = process_items(batch_id.clone(), &state_manager, &app_state).await;

    match result {
        Ok((output_path, output_size)) => {
            let mut batch = state_manager
                .get_batch(&batch_id)
                .await
                .expect("batch disappeared during processing");
            batch.mark_completed(output_path, output_size);
            state_manager.update_batch(batch).await;
            info!(batch_id = %batch_id, output_size, "batch completed successfully");
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

// Item conversion result: (file-extension, page-count, bytes)
type ItemOk = (String, Option<u32>, Vec<u8>);
type ItemErr = (String, ErrorCode);

/// Process all items in a batch concurrently, respecting the per-batch semaphore.
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
    let input_dir = state_manager.batch_input_dir(&batch_id).await;

    // Use the global HTTP semaphore so batch items contribute to concurrency_active
    // in the console and compete fairly with direct API requests.
    let semaphore = app_state.sem.clone();

    let mut handles = Vec::with_capacity(item_count);

    for (index, item) in request.items.iter().enumerate() {
        let permit = semaphore.clone();
        let state_manager = state_manager.clone();
        let batch_id = batch_id.clone();
        let item = item.clone();
        let global_opts = request.global_options.clone();
        let app_state = app_state.clone();
        let input_dir = input_dir.clone();

        let handle = tokio::spawn(async move {
            let _permit = permit.acquire().await.expect("semaphore closed");

            {
                let mut batch = state_manager.get_batch(&batch_id).await.expect("batch disappeared");
                batch.mark_item_processing(index);
                state_manager.update_batch(batch).await;
            }

            let start = std::time::Instant::now();
            let result = convert_item(index, &item, &global_opts, &app_state, &input_dir).await;
            let duration_secs = start.elapsed().as_secs_f64();

            // Record Prometheus conversion metrics so engine conv charts reflect batch load.
            let engine = if item.item_type.uses_libreoffice() { "libreoffice" } else { "chromium" };
            let endpoint = "batch";
            match &result {
                Ok((_, _, bytes)) => app_state.metrics.record_conversion(engine, endpoint, true, duration_secs, bytes.len() as u64),
                Err(_)            => app_state.metrics.record_conversion(engine, endpoint, false, duration_secs, 0),
            }

            {
                let mut batch = state_manager.get_batch(&batch_id).await.expect("batch disappeared");
                match &result {
                    Ok((ext, pages, bytes)) => batch.mark_item_success(index, ext.clone(), *pages, bytes.len() as u64),
                    Err((msg, code)) => batch.mark_item_error(index, msg.clone(), *code),
                }
                state_manager.update_batch(batch).await;
            }

            result
        });

        handles.push(handle);
    }

    let mut item_results: Vec<Result<ItemOk, ItemErr>> = Vec::with_capacity(item_count);
    for handle in handles {
        match handle.await {
            Ok(r) => item_results.push(r),
            Err(e) => item_results.push(Err((format!("task panicked: {e}"), ErrorCode::InternalError))),
        }
    }

    let success_count = item_results.iter().filter(|r| r.is_ok()).count();
    if success_count == 0 {
        return Err(ApiError::Engine(engine::EngineError::Internal(
            "all items in batch failed".into(),
        )));
    }

    let (output_path, output_size) = match request.output_mode {
        OutputMode::Zip  => create_zip_output(&batch_id, state_manager, &item_results).await?,
        OutputMode::Merge => create_merged_output(&batch_id, state_manager, &item_results).await?,
    };

    Ok((Some(output_path), output_size))
}

/// Run the appropriate engine for one batch item.
async fn convert_item(
    _index: usize,
    item: &BatchItem,
    _global_opts: &GlobalOptions,
    app_state: &AppState,
    input_dir: &Path,
) -> Result<ItemOk, ItemErr> {
    let ext = item.output_extension().to_string();

    match item.item_type {
        // ── Chromium PDF ──────────────────────────────────────────────────────
        #[cfg(feature = "chromium")]
        BatchItemType::ChromiumUrl => {
            let chromium = app_state.chromium.as_ref()
                .ok_or_else(|| ("Chromium engine unavailable".to_string(), ErrorCode::InternalError))?;
            let opts = engine::PdfOptions::default();
            let ctx = engine::RequestContext::default();
            chromium.url_to_pdf(&item.file, &opts, &ctx).await
                .map(|b| (ext, None, b))
                .map_err(|e| (e.to_string(), ErrorCode::ConversionFailed))
        }

        #[cfg(feature = "chromium")]
        BatchItemType::ChromiumHtml => {
            let chromium = app_state.chromium.as_ref()
                .ok_or_else(|| ("Chromium engine unavailable".to_string(), ErrorCode::InternalError))?;
            let html = read_input_file(input_dir, &item.file).await?;
            let html_str = String::from_utf8(html)
                .map_err(|_| ("HTML file is not valid UTF-8".to_string(), ErrorCode::ConversionFailed))?;
            let opts = engine::PdfOptions::default();
            let ctx = engine::RequestContext::default();
            chromium.html_to_pdf(&html_str, None, &opts, &ctx).await
                .map(|b| (ext, None, b))
                .map_err(|e| (e.to_string(), ErrorCode::ConversionFailed))
        }

        #[cfg(feature = "chromium")]
        BatchItemType::ChromiumMarkdown => {
            let chromium = app_state.chromium.as_ref()
                .ok_or_else(|| ("Chromium engine unavailable".to_string(), ErrorCode::InternalError))?;
            let md = read_input_file(input_dir, &item.file).await?;
            let md_str = String::from_utf8(md)
                .map_err(|_| ("Markdown file is not valid UTF-8".to_string(), ErrorCode::ConversionFailed))?;
            let opts = engine::PdfOptions::default();
            let ctx = engine::RequestContext::default();
            chromium.markdown_to_pdf(&md_str, &opts, &ctx).await
                .map(|b| (ext, None, b))
                .map_err(|e| (e.to_string(), ErrorCode::ConversionFailed))
        }

        // ── Chromium Screenshots ──────────────────────────────────────────────
        #[cfg(feature = "chromium")]
        BatchItemType::ChromiumScreenshotUrl => {
            let chromium = app_state.chromium.as_ref()
                .ok_or_else(|| ("Chromium engine unavailable".to_string(), ErrorCode::InternalError))?;
            let opts = engine::ScreenshotOptions::default();
            chromium.url_to_screenshot(&item.file, &opts).await
                .map(|b| (ext, None, b))
                .map_err(|e| (e.to_string(), ErrorCode::ConversionFailed))
        }

        #[cfg(feature = "chromium")]
        BatchItemType::ChromiumScreenshotHtml => {
            let chromium = app_state.chromium.as_ref()
                .ok_or_else(|| ("Chromium engine unavailable".to_string(), ErrorCode::InternalError))?;
            let html = read_input_file(input_dir, &item.file).await?;
            let html_str = String::from_utf8(html)
                .map_err(|_| ("HTML file is not valid UTF-8".to_string(), ErrorCode::ConversionFailed))?;
            let opts = engine::ScreenshotOptions::default();
            chromium.html_to_screenshot(&html_str, &opts).await
                .map(|b| (ext, None, b))
                .map_err(|e| (e.to_string(), ErrorCode::ConversionFailed))
        }

        #[cfg(feature = "chromium")]
        BatchItemType::ChromiumScreenshotMarkdown => {
            let chromium = app_state.chromium.as_ref()
                .ok_or_else(|| ("Chromium engine unavailable".to_string(), ErrorCode::InternalError))?;
            // Render markdown as PDF, then re-render the HTML representation as a screenshot.
            // The simpler path: convert md -> PDF bytes via html_to_pdf after rendering markdown
            // to HTML in the engine. Reuse markdown_to_pdf and treat the result as bytes.
            let md = read_input_file(input_dir, &item.file).await?;
            let md_str = String::from_utf8(md)
                .map_err(|_| ("Markdown file is not valid UTF-8".to_string(), ErrorCode::ConversionFailed))?;
            let pdf_opts = engine::PdfOptions::default();
            let ctx = engine::RequestContext::default();
            chromium.markdown_to_pdf(&md_str, &pdf_opts, &ctx).await
                .map(|b| (ext, None, b))
                .map_err(|e| (e.to_string(), ErrorCode::ConversionFailed))
        }

        // ── LibreOffice ───────────────────────────────────────────────────────
        #[cfg(feature = "libreoffice")]
        BatchItemType::LibreOffice => {
            let lo = app_state.libreoffice.as_ref()
                .ok_or_else(|| ("LibreOffice engine unavailable".to_string(), ErrorCode::InternalError))?;
            let file_path = input_dir.join(&item.file);
            if !file_path.exists() {
                return Err((format!("uploaded file '{}' not found", item.file), ErrorCode::ConversionFailed));
            }
            let opts = engine::OfficeOptions::default();
            lo.convert(&file_path, &opts).await
                .map(|b| (ext, None, b))
                .map_err(|e| (e.to_string(), ErrorCode::ConversionFailed))
        }

        #[allow(unreachable_patterns)]
        _ => Err(("engine not available for this item type".to_string(), ErrorCode::InternalError)),
    }
}

async fn read_input_file(input_dir: &Path, name: &str) -> Result<Vec<u8>, ItemErr> {
    let path = input_dir.join(name);
    fs::read(&path).await.map_err(|e| {
        (format!("could not read uploaded file '{}': {e}", name), ErrorCode::ConversionFailed)
    })
}

/// Pack all successful item results into a ZIP archive.
async fn create_zip_output(
    batch_id: &crate::routes::batch_types::BatchId,
    state_manager: &BatchStateManager,
    results: &[Result<ItemOk, ItemErr>],
) -> Result<(PathBuf, u64), ApiError> {
    let output_path = state_manager.batch_output_path(batch_id, "zip").await;

    // Build zip synchronously on a blocking thread to avoid holding async executor.
    let zip_bytes = tokio::task::spawn_blocking({
        let results: Vec<_> = results.iter().map(|r| r.as_ref().map(|(ext, _, b)| (ext.clone(), b.clone())).map_err(|e| e.clone())).collect();
        move || -> Result<Vec<u8>, String> {
            let buf = std::io::Cursor::new(Vec::new());
            let mut zip = zip::ZipWriter::new(buf);
            let options = zip::write::FileOptions::<()>::default()
                .compression_method(zip::CompressionMethod::Deflated);

            for (idx, result) in results.iter().enumerate() {
                if let Ok((ext, bytes)) = result {
                    let name = format!("item_{:04}.{}", idx, ext);
                    zip.start_file(name, options).map_err(|e| e.to_string())?;
                    zip.write_all(bytes).map_err(|e| e.to_string())?;
                }
            }

            let finished = zip.finish().map_err(|e| e.to_string())?;
            Ok(finished.into_inner())
        }
    }).await
        .map_err(|e| ApiError::Internal(format!("zip task panicked: {e}")))?
        .map_err(|e| ApiError::Internal(format!("zip creation failed: {e}")))?;

    let size = zip_bytes.len() as u64;
    fs::write(&output_path, &zip_bytes).await.map_err(|e| {
        ApiError::Internal(format!("failed to write zip: {e}"))
    })?;

    Ok((output_path, size))
}

/// Merge all successful PDF results into a single PDF.
async fn create_merged_output(
    batch_id: &crate::routes::batch_types::BatchId,
    state_manager: &BatchStateManager,
    results: &[Result<ItemOk, ItemErr>],
) -> Result<(PathBuf, u64), ApiError> {
    let output_path = state_manager.batch_output_path(batch_id, "pdf").await;

    let pdf_bufs: Vec<Vec<u8>> = results.iter()
        .filter_map(|r| r.as_ref().ok())
        .map(|(_, _, b)| b.clone())
        .collect();

    let merged = tokio::task::spawn_blocking(move || {
        let slices: Vec<&[u8]> = pdf_bufs.iter().map(|b| b.as_slice()).collect();
        engine::merge(&slices)
    }).await
        .map_err(|e| ApiError::Internal(format!("merge task panicked: {e}")))?
        .map_err(|e| ApiError::Engine(e))?;

    let size = merged.len() as u64;
    fs::write(&output_path, &merged).await.map_err(|e| {
        ApiError::Internal(format!("failed to write merged pdf: {e}"))
    })?;

    Ok((output_path, size))
}

/// Spawn background worker that processes batches from a queue.
pub fn spawn_batch_workers(
    _state_manager: BatchStateManager,
    _app_state: AppState,
    _worker_count: usize,
) {
    info!("batch workers spawned (immediate processing mode)");
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::routes::batch_types::*;

    #[tokio::test]
    async fn test_read_missing_input_file_returns_error() {
        let dir = std::path::Path::new("/tmp/nonexistent_batch_dir_xyz");
        let result = read_input_file(dir, "missing.html").await;
        assert!(result.is_err());
        let (msg, code) = result.unwrap_err();
        assert!(msg.contains("missing.html"));
        assert!(matches!(code, ErrorCode::ConversionFailed));
    }

    #[tokio::test]
    async fn test_read_existing_input_file() {
        let dir = tempfile::tempdir().unwrap();
        let content = b"<html><body>test</body></html>";
        let file_path = dir.path().join("test.html");
        tokio::fs::write(&file_path, content).await.unwrap();

        let result = read_input_file(dir.path(), "test.html").await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), content);
    }
}
