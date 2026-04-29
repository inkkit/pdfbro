//! Batch state management - in-memory tracking and filesystem persistence.
//!
//! Batches are stored in memory with periodic writes to disk for recovery.
//! Output files are stored on disk with automatic cleanup.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use tokio::fs;
use tokio::sync::RwLock;
use tracing::info;

use crate::routes::batch_types::*;

/// In-memory state for a single batch.
#[derive(Debug, Clone)]
pub struct BatchState {
    /// Batch identifier.
    pub id: BatchId,
    /// Original request.
    pub request: BatchRequest,
    /// Current status.
    pub status: BatchStatus,
    /// When batch was created.
    pub submitted_at: SystemTime,
    /// When processing started.
    pub started_at: Option<SystemTime>,
    /// When processing completed.
    pub completed_at: Option<SystemTime>,
    /// When batch expires and can be cleaned up.
    pub expires_at: SystemTime,
    /// Results for each item.
    pub item_results: Vec<ItemResult>,
    /// Path to output file (ZIP or merged PDF).
    pub output_path: Option<PathBuf>,
    /// Size of output file in bytes.
    pub output_size: u64,
    /// Error message if batch failed entirely.
    pub error: Option<String>,
}

impl BatchState {
    /// Create a new batch state from a request.
    pub fn new(id: BatchId, request: BatchRequest, retention_minutes: u64) -> Self {
        let now = SystemTime::now();
        let expires_at = now + Duration::from_secs(retention_minutes * 60);

        // Initialize pending results for all items
        let item_results: Vec<ItemResult> = request
            .items
            .iter()
            .enumerate()
            .map(|(index, item)| ItemResult {
                index,
                file: item.file.clone(),
                status: ItemStatus::Pending,
                output_type: None,
                pages: None,
                bytes: None,
                error: None,
                error_code: None,
            })
            .collect();

        Self {
            id,
            request,
            status: BatchStatus::Queued,
            submitted_at: now,
            started_at: None,
            completed_at: None,
            expires_at,
            item_results,
            output_path: None,
            output_size: 0,
            error: None,
        }
    }

    /// Mark batch as processing.
    pub fn mark_processing(&mut self) {
        self.status = BatchStatus::Processing;
        self.started_at = Some(SystemTime::now());
    }

    /// Mark an item as processing.
    pub fn mark_item_processing(&mut self, index: usize) {
        if let Some(item) = self.item_results.get_mut(index) {
            item.status = ItemStatus::Processing;
        }
    }

    /// Mark an item as completed successfully.
    pub fn mark_item_success(
        &mut self,
        index: usize,
        output_type: String,
        pages: Option<u32>,
        bytes: u64,
    ) {
        if let Some(item) = self.item_results.get_mut(index) {
            item.status = ItemStatus::Success;
            item.output_type = Some(output_type);
            item.pages = pages;
            item.bytes = Some(bytes);
        }
    }

    /// Mark an item as failed.
    pub fn mark_item_error(&mut self, index: usize, error: String, error_code: ErrorCode) {
        if let Some(item) = self.item_results.get_mut(index) {
            item.status = ItemStatus::Error;
            item.error = Some(error);
            item.error_code = Some(error_code);
        }
    }

    /// Mark batch as completed.
    pub fn mark_completed(&mut self, output_path: Option<PathBuf>, output_size: u64) {
        self.status = BatchStatus::Completed;
        self.completed_at = Some(SystemTime::now());
        self.output_path = output_path;
        self.output_size = output_size;
    }

    /// Mark batch as failed.
    pub fn mark_failed(&mut self, error: String) {
        self.status = BatchStatus::Failed;
        self.completed_at = Some(SystemTime::now());
        self.error = Some(error);
    }

    /// Check if batch has expired.
    pub fn is_expired(&self) -> bool {
        SystemTime::now() > self.expires_at
    }

    /// Get progress summary.
    pub fn progress(&self) -> BatchProgress {
        let total = self.item_results.len();
        let completed = self
            .item_results
            .iter()
            .filter(|r| r.status == ItemStatus::Success)
            .count();
        let failed = self
            .item_results
            .iter()
            .filter(|r| r.status == ItemStatus::Error)
            .count();
        let pending = total - completed - failed;

        BatchProgress {
            total,
            completed,
            failed,
            pending,
        }
    }

    /// Get results summary (only valid when complete).
    pub fn results_summary(&self) -> Option<BatchResultsSummary> {
        if self.status != BatchStatus::Completed {
            return None;
        }

        let succeeded = self
            .item_results
            .iter()
            .filter(|r| r.status == ItemStatus::Success)
            .count();
        let failed = self
            .item_results
            .iter()
            .filter(|r| r.status == ItemStatus::Error)
            .count();

        Some(BatchResultsSummary {
            succeeded,
            failed,
            total_bytes: self.output_size,
            output_ready: self.output_path.is_some(),
        })
    }

    /// Build status response.
    pub fn to_status_response(&self, base_url: &str) -> BatchStatusResponse {
        let download_url = if self.status == BatchStatus::Completed && self.output_path.is_some() {
            Some(format!("{}/forms/batch/{}/download", base_url, self.id))
        } else {
            None
        };

        BatchStatusResponse {
            batch_id: self.id.clone(),
            status: self.status,
            submitted_at: self.submitted_at,
            started_at: self.started_at,
            completed_at: self.completed_at,
            progress: self.progress(),
            items: self.item_results.clone(),
            results: self.results_summary(),
            download_url,
        }
    }
}

/// Thread-safe batch state manager.
#[derive(Debug, Clone)]
pub struct BatchStateManager {
    inner: Arc<RwLock<BatchStateInner>>,
}

#[derive(Debug)]
struct BatchStateInner {
    /// Active batches by ID.
    batches: HashMap<BatchId, BatchState>,
    /// Storage directory path.
    storage_path: PathBuf,
    /// Retention duration.
    retention: Duration,
}

impl BatchStateManager {
    /// Create a new state manager.
    pub async fn new(storage_path: PathBuf, retention_minutes: u64) -> std::io::Result<Self> {
        // Ensure storage directory exists
        fs::create_dir_all(&storage_path).await?;
        fs::create_dir_all(storage_path.join("outputs")).await?;

        let retention = Duration::from_secs(retention_minutes * 60);

        let inner = BatchStateInner {
            batches: HashMap::new(),
            storage_path,
            retention,
        };

        Ok(Self {
            inner: Arc::new(RwLock::new(inner)),
        })
    }

    /// Create a new batch.
    pub async fn create_batch(&self, request: BatchRequest) -> BatchState {
        let id = BatchId::new();
        let retention_minutes = self.inner.read().await.retention.as_secs() / 60;
        let state = BatchState::new(id.clone(), request, retention_minutes);

        let mut inner = self.inner.write().await;
        inner.batches.insert(id.clone(), state.clone());

        info!(batch_id = %id, "created new batch");
        state
    }

    /// Get a batch by ID.
    pub async fn get_batch(&self, id: &BatchId) -> Option<BatchState> {
        let inner = self.inner.read().await;
        inner.batches.get(id).cloned()
    }

    /// Update a batch state.
    pub async fn update_batch(&self, state: BatchState) {
        let mut inner = self.inner.write().await;
        inner.batches.insert(state.id.clone(), state);
    }

    /// Remove a batch.
    pub async fn remove_batch(&self, id: &BatchId) {
        let mut inner = self.inner.write().await;
        if let Some(state) = inner.batches.remove(id) {
            // Clean up output file if present
            if let Some(path) = state.output_path {
                let _ = fs::remove_file(&path).await;
            }
            info!(batch_id = %id, "removed batch");
        }
    }

    /// Get storage path for outputs.
    pub async fn output_path(&self) -> PathBuf {
        let inner = self.inner.read().await;
        inner.storage_path.join("outputs")
    }

    /// Get output path for a specific batch.
    pub async fn batch_output_path(&self, id: &BatchId, extension: &str) -> PathBuf {
        let inner = self.inner.read().await;
        inner
            .storage_path
            .join("outputs")
            .join(format!("{}.{}", id, extension))
    }

    /// Run cleanup of expired batches.
    pub async fn cleanup_expired(&self) {
        let expired_ids: Vec<BatchId> = {
            let inner = self.inner.read().await;
            inner
                .batches
                .iter()
                .filter(|(_, state)| state.is_expired())
                .map(|(id, _)| id.clone())
                .collect()
        };

        for id in expired_ids {
            self.remove_batch(&id).await;
            info!(batch_id = %id, "cleaned up expired batch");
        }
    }

    /// Get count of active batches.
    pub async fn active_count(&self) -> usize {
        let inner = self.inner.read().await;
        inner
            .batches
            .values()
            .filter(|b| b.status == BatchStatus::Queued || b.status == BatchStatus::Processing)
            .count()
    }

    /// List all batch IDs.
    pub async fn list_batches(&self) -> Vec<BatchId> {
        let inner = self.inner.read().await;
        inner.batches.keys().cloned().collect()
    }
}

/// Spawn cleanup background task.
pub fn spawn_cleanup_task(manager: BatchStateManager, interval_minutes: u64) {
    let interval = Duration::from_secs(interval_minutes * 60);

    tokio::spawn(async move {
        loop {
            tokio::time::sleep(interval).await;
            manager.cleanup_expired().await;
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_test_request() -> BatchRequest {
        BatchRequest {
            output_mode: OutputMode::Zip,
            global_options: GlobalOptions::default(),
            items: vec![BatchItem {
                file: "test.html".into(),
                item_type: BatchItemType::ChromiumHtml,
                options: ItemOptions::default(),
            }],
            merge_options: MergeOptions::default(),
        }
    }

    #[tokio::test]
    async fn test_batch_state_lifecycle() {
        let request = create_test_request();
        let mut state = BatchState::new(BatchId::new(), request, 60);

        assert_eq!(state.status, BatchStatus::Queued);
        assert_eq!(state.progress().pending, 1);

        state.mark_processing();
        assert_eq!(state.status, BatchStatus::Processing);
        assert!(state.started_at.is_some());

        state.mark_item_processing(0);
        assert_eq!(state.item_results[0].status, ItemStatus::Processing);

        state.mark_item_success(0, "pdf".into(), Some(5), 1024);
        assert_eq!(state.item_results[0].status, ItemStatus::Success);
        assert_eq!(state.item_results[0].pages, Some(5));

        state.mark_completed(None, 1024);
        assert_eq!(state.status, BatchStatus::Completed);
        assert!(state.completed_at.is_some());
        assert_eq!(state.progress().completed, 1);
    }

    #[tokio::test]
    async fn test_batch_state_manager() {
        let temp_dir = TempDir::new().unwrap();
        let manager = BatchStateManager::new(temp_dir.path().into(), 60)
            .await
            .unwrap();

        let request = create_test_request();
        let state = manager.create_batch(request).await;

        let retrieved = manager.get_batch(&state.id).await;
        assert!(retrieved.is_some());

        manager.remove_batch(&state.id).await;
        let removed = manager.get_batch(&state.id).await;
        assert!(removed.is_none());
    }

    #[test]
    fn test_progress_calculation() {
        let request = BatchRequest {
            output_mode: OutputMode::Zip,
            global_options: GlobalOptions::default(),
            items: vec![
                BatchItem {
                    file: "a.html".into(),
                    item_type: BatchItemType::ChromiumHtml,
                    options: ItemOptions::default(),
                },
                BatchItem {
                    file: "b.html".into(),
                    item_type: BatchItemType::ChromiumHtml,
                    options: ItemOptions::default(),
                },
                BatchItem {
                    file: "c.html".into(),
                    item_type: BatchItemType::ChromiumHtml,
                    options: ItemOptions::default(),
                },
            ],
            merge_options: MergeOptions::default(),
        };

        let mut state = BatchState::new(BatchId::new(), request, 60);

        // Initial state
        let progress = state.progress();
        assert_eq!(progress.total, 3);
        assert_eq!(progress.pending, 3);
        assert_eq!(progress.completed, 0);
        assert_eq!(progress.failed, 0);

        // One success, one error
        state.mark_item_success(0, "pdf".into(), None, 100);
        state.mark_item_error(1, "error".into(), ErrorCode::ConversionFailed);

        let progress = state.progress();
        assert_eq!(progress.completed, 1);
        assert_eq!(progress.failed, 1);
        assert_eq!(progress.pending, 1);
    }
}
