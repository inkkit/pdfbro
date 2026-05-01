//! Batch API request/response types and validation.
//!
//! Implements the batch conversion API allowing multiple files
//! of mixed types (HTML, Office, URLs, screenshots) to be
//! processed in a single request.

use std::time::SystemTime;

use serde::{Deserialize, Serialize};

use crate::error::ApiError;
use crate::multipart::UploadedFile;

/// Maximum items per batch (configurable, default).
pub const DEFAULT_MAX_ITEMS: usize = 50;

/// Default maximum output size per batch (500MB).
pub const DEFAULT_MAX_OUTPUT_BYTES: u64 = 500 * 1024 * 1024;

/// Default batch retention time (1 hour).
pub const DEFAULT_RETENTION_MINUTES: u64 = 60;

/// Unique identifier for a batch.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct BatchId(String);

impl BatchId {
    /// Generate a new unique batch ID using ULID.
    ///
    /// ULID provides lexicographic sorting (chronological order),
    /// 26 lowercase characters, and collision resistance.
    pub fn new() -> Self {
        Self(format!("batch_{}", ulid::Ulid::new().to_string().to_lowercase()))
    }

    /// Create a BatchId from an existing string (for parsing URLs).
    pub fn from_raw(s: String) -> Self {
        Self(s)
    }
}

impl Default for BatchId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for BatchId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl AsRef<str> for BatchId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

/// Batch submission request.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchRequest {
    /// How to package the output.
    #[serde(default)]
    pub output_mode: OutputMode,

    /// Global options applied to all items unless overridden.
    #[serde(default)]
    pub global_options: GlobalOptions,

    /// Individual conversion items.
    pub items: Vec<BatchItem>,

    /// Options for merge output mode.
    #[serde(default)]
    pub merge_options: MergeOptions,
}

impl BatchRequest {
    /// Validate the entire batch request.
    pub fn validate(&self, files: &[UploadedFile]) -> Result<(), ApiError> {
        // Check item count
        if self.items.is_empty() {
            return Err(ApiError::InvalidField {
                field: "items",
                message: "batch must contain at least one item".into(),
            });
        }
        if self.items.len() > DEFAULT_MAX_ITEMS {
            return Err(ApiError::InvalidField {
                field: "items",
                message: format!("batch exceeds maximum of {} items", DEFAULT_MAX_ITEMS),
            });
        }

        // Build a set of uploaded file names for quick lookup
        let uploaded_files: std::collections::HashSet<&str> =
            files.iter().map(|f| f.filename.as_str()).collect();

        // Validate each item
        for (idx, item) in self.items.iter().enumerate() {
            item.validate(idx, &uploaded_files, &self.global_options)?;
        }

        // If merge mode, ensure all items produce PDF output
        if self.output_mode == OutputMode::Merge {
            for (idx, item) in self.items.iter().enumerate() {
                if !item.produces_pdf() {
                    return Err(ApiError::InvalidField {
                        field: "items",
                        message: format!(
                            "item[{}]: outputMode 'merge' requires PDF output, but '{}' produces images",
                            idx, item.file
                        ),
                    });
                }
            }
        }

        Ok(())
    }
}

/// How to package batch output.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OutputMode {
    /// ZIP archive containing all outputs (default).
    #[default]
    Zip,
    /// Single merged PDF (all items must produce PDF).
    Merge,
}

/// Global options applied to batch items.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GlobalOptions {
    /// PDF/Chromium options.
    #[serde(default)]
    pub pdf: Option<serde_json::Value>,
    /// LibreOffice options.
    #[serde(default)]
    pub office: Option<serde_json::Value>,
    /// Screenshot options.
    #[serde(default)]
    pub screenshot: Option<serde_json::Value>,
}

/// Options for merge output mode.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MergeOptions {
    /// How to handle bookmarks from source PDFs.
    #[serde(default = "default_bookmark_source")]
    pub bookmark_source: BookmarkSource,
}

fn default_bookmark_source() -> BookmarkSource {
    BookmarkSource::All
}

/// Bookmark handling for merged output.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BookmarkSource {
    /// Include bookmarks from all source PDFs.
    #[default]
    All,
    /// Only include bookmarks from first PDF.
    First,
    /// Strip all bookmarks.
    None,
}

/// Single item in a batch.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchItem {
    /// File name (must match uploaded file) or URL.
    pub file: String,

    /// Conversion type.
    #[serde(rename = "type")]
    pub item_type: BatchItemType,

    /// Per-item options that override global options.
    #[serde(default)]
    pub options: ItemOptions,
}

impl BatchItem {
    /// Validate this item.
    fn validate(
        &self,
        idx: usize,
        uploaded_files: &std::collections::HashSet<&str>,
        _globals: &GlobalOptions,
    ) -> Result<(), ApiError> {
        // Check if file exists in upload (URLs don't need file upload)
        if !self.item_type.is_url_type() && !uploaded_files.contains(self.file.as_str()) {
            return Err(ApiError::InvalidField {
                field: "items",
                message: format!("item[{}]: file '{}' not found in upload", idx, self.file),
            });
        }

        // Validate URL format for URL types
        if self.item_type.is_url_type() {
            if url::Url::parse(&self.file).is_err() {
                return Err(ApiError::InvalidField {
                    field: "items",
                    message: format!("item[{}]: '{}' is not a valid URL", idx, self.file),
                });
            }
        }

        Ok(())
    }

    /// Returns true if this item produces PDF output.
    fn produces_pdf(&self) -> bool {
        matches!(
            self.item_type,
            BatchItemType::ChromiumHtml
                | BatchItemType::ChromiumUrl
                | BatchItemType::ChromiumMarkdown
                | BatchItemType::LibreOffice
        )
    }

    /// Get the expected output extension for this item.
    pub fn output_extension(&self) -> &'static str {
        match self.item_type {
            BatchItemType::ChromiumHtml
            | BatchItemType::ChromiumUrl
            | BatchItemType::ChromiumMarkdown
            | BatchItemType::LibreOffice => "pdf",
            BatchItemType::ChromiumScreenshotHtml
            | BatchItemType::ChromiumScreenshotUrl
            | BatchItemType::ChromiumScreenshotMarkdown => {
                // Default to png, actual format determined by options
                "png"
            }
        }
    }
}

/// Conversion type for a batch item.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BatchItemType {
    /// HTML to PDF via Chromium.
    ChromiumHtml,
    /// URL to PDF via Chromium.
    ChromiumUrl,
    /// Markdown to PDF via Chromium.
    ChromiumMarkdown,
    /// HTML to screenshot via Chromium.
    ChromiumScreenshotHtml,
    /// URL to screenshot via Chromium.
    ChromiumScreenshotUrl,
    /// Markdown to screenshot via Chromium.
    ChromiumScreenshotMarkdown,
    /// Office document to PDF via LibreOffice.
    LibreOffice,
}

impl BatchItemType {
    /// Returns true if this type requires a URL input.
    fn is_url_type(&self) -> bool {
        matches!(self, BatchItemType::ChromiumUrl | BatchItemType::ChromiumScreenshotUrl)
    }

    /// Returns true if this is a screenshot type.
    pub fn is_screenshot(&self) -> bool {
        matches!(
            self,
            BatchItemType::ChromiumScreenshotHtml
                | BatchItemType::ChromiumScreenshotUrl
                | BatchItemType::ChromiumScreenshotMarkdown
        )
    }

    /// Returns true if this uses Chromium engine.
    pub fn uses_chromium(&self) -> bool {
        !matches!(self, BatchItemType::LibreOffice)
    }

    /// Returns true if this uses LibreOffice engine.
    pub fn uses_libreoffice(&self) -> bool {
        matches!(self, BatchItemType::LibreOffice)
    }

    /// Returns true if this type produces PDF output.
    pub fn produces_pdf(&self) -> bool {
        matches!(
            self,
            BatchItemType::ChromiumHtml
                | BatchItemType::ChromiumUrl
                | BatchItemType::ChromiumMarkdown
                | BatchItemType::LibreOffice
        )
    }
}

/// Per-item override options.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemOptions {
    /// PDF options override.
    #[serde(default)]
    pub pdf: Option<serde_json::Value>,
    /// Office options override.
    #[serde(default)]
    pub office: Option<serde_json::Value>,
    /// Screenshot options override.
    #[serde(default)]
    pub screenshot: Option<serde_json::Value>,
}

/// Batch submission response.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchSubmitResponse {
    /// Unique batch identifier.
    pub batch_id: BatchId,
    /// Current status.
    pub status: BatchStatus,
    /// When the batch will expire if not downloaded.
    pub expires_at: SystemTime,
}

/// Batch processing status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BatchStatus {
    /// Queued waiting for processing.
    Queued,
    /// Currently processing.
    Processing,
    /// Completed successfully (may have partial failures).
    Completed,
    /// Failed entirely (e.g., storage error, not item failures).
    Failed,
}

/// Detailed batch status response.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchStatusResponse {
    /// Batch identifier.
    pub batch_id: BatchId,
    /// Current status.
    pub status: BatchStatus,
    /// When batch was submitted.
    pub submitted_at: SystemTime,
    /// When processing started (if applicable).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub started_at: Option<SystemTime>,
    /// When processing completed (if applicable).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<SystemTime>,
    /// Progress information.
    pub progress: BatchProgress,
    /// Individual item results.
    pub items: Vec<ItemResult>,
    /// Summary of results (only present when complete).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub results: Option<BatchResultsSummary>,
    /// Download URL (only when complete with output).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub download_url: Option<String>,
}

/// Progress counters.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchProgress {
    /// Total items in batch.
    pub total: usize,
    /// Items successfully completed.
    pub completed: usize,
    /// Items that failed.
    pub failed: usize,
    /// Items still pending.
    pub pending: usize,
}

/// Result for a single batch item.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemResult {
    /// Item index in original request.
    pub index: usize,
    /// Original file name or URL.
    pub file: String,
    /// Current status.
    pub status: ItemStatus,
    /// Output type (extension).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_type: Option<String>,
    /// Number of pages (for PDFs, if known).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pages: Option<u32>,
    /// Output size in bytes (if successful).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bytes: Option<u64>,
    /// Error message (if failed).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// Error code for programmatic handling.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_code: Option<ErrorCode>,
}

/// Item processing status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ItemStatus {
    /// Waiting to start.
    Pending,
    /// Currently processing.
    Processing,
    /// Successfully completed.
    Success,
    /// Failed.
    Error,
}

impl ItemStatus {
    /// Returns true if status is Success.
    pub fn is_success(&self) -> bool {
        matches!(self, ItemStatus::Success)
    }
}

/// Batch results summary (present when complete).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchResultsSummary {
    /// Number of successful items.
    pub succeeded: usize,
    /// Number of failed items.
    pub failed: usize,
    /// Total output bytes.
    pub total_bytes: u64,
    /// Whether output is ready for download.
    pub output_ready: bool,
}

/// Error codes for programmatic handling.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ErrorCode {
    /// File not found in upload.
    FileNotFound,
    /// Invalid URL format.
    InvalidUrl,
    /// Options failed validation.
    InvalidOptions,
    /// Unknown conversion type.
    UnsupportedType,
    /// Engine error during conversion.
    ConversionFailed,
    /// PDF merge operation failed.
    MergeFailed,
    /// Operation timed out.
    Timeout,
    /// Storage error.
    StorageError,
    /// Internal server error.
    InternalError,
}

impl ErrorCode {
    /// Convert from an error string.
    pub fn from_error(err: &str) -> Self {
        match err {
            _ if err.contains("not found") => Self::FileNotFound,
            _ if err.contains("timeout") => Self::Timeout,
            _ if err.contains("merge") => Self::MergeFailed,
            _ if err.contains("storage") || err.contains("io") => Self::StorageError,
            _ => Self::InternalError,
        }
    }
}

/// Batch configuration.
#[derive(Debug, Clone)]
pub struct BatchConfig {
    /// Maximum items per batch.
    pub max_items: usize,
    /// Maximum request body size in bytes.
    pub max_body_bytes: usize,
    /// Maximum output size per batch in bytes.
    pub max_output_bytes: u64,
    /// How long to retain batch results.
    pub retention_minutes: u64,
    /// Concurrent conversions per batch.
    pub concurrency_per_batch: usize,
    /// Maximum concurrent batches server-wide.
    pub max_active_batches: usize,
    /// Storage directory path.
    pub storage_path: std::path::PathBuf,
}

impl Default for BatchConfig {
    fn default() -> Self {
        Self {
            max_items: DEFAULT_MAX_ITEMS,
            max_body_bytes: 100 * 1024 * 1024, // 100MB
            max_output_bytes: DEFAULT_MAX_OUTPUT_BYTES,
            retention_minutes: DEFAULT_RETENTION_MINUTES,
            concurrency_per_batch: 4,
            max_active_batches: 10,
            storage_path: std::path::PathBuf::from("/tmp/folio-batches"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn batch_id_generation() {
        let id1 = BatchId::new();
        let id2 = BatchId::new();
        assert_ne!(id1.to_string(), id2.to_string());
        assert!(id1.to_string().starts_with("batch_"));
    }

    #[test]
    fn output_mode_default_is_zip() {
        let mode: OutputMode = Default::default();
        assert_eq!(mode, OutputMode::Zip);
    }

    #[test]
    fn batch_item_type_pdf_detection() {
        assert!(BatchItemType::ChromiumHtml.produces_pdf());
        assert!(BatchItemType::ChromiumUrl.produces_pdf());
        assert!(BatchItemType::ChromiumMarkdown.produces_pdf());
        assert!(BatchItemType::LibreOffice.produces_pdf());
        assert!(!BatchItemType::ChromiumScreenshotHtml.produces_pdf());
        assert!(!BatchItemType::ChromiumScreenshotUrl.produces_pdf());
        assert!(!BatchItemType::ChromiumScreenshotMarkdown.produces_pdf());
    }

    #[test]
    fn batch_item_type_is_url() {
        assert!(BatchItemType::ChromiumUrl.is_url_type());
        assert!(BatchItemType::ChromiumScreenshotUrl.is_url_type());
        assert!(!BatchItemType::ChromiumHtml.is_url_type());
        assert!(!BatchItemType::LibreOffice.is_url_type());
    }

    #[test]
    fn batch_item_type_engines() {
        assert!(BatchItemType::ChromiumHtml.uses_chromium());
        assert!(BatchItemType::ChromiumUrl.uses_chromium());
        assert!(BatchItemType::ChromiumScreenshotHtml.uses_chromium());
        assert!(!BatchItemType::LibreOffice.uses_chromium());
        assert!(BatchItemType::LibreOffice.uses_libreoffice());
    }

    #[test]
    fn batch_item_produces_pdf() {
        let item = BatchItem {
            item_type: BatchItemType::ChromiumHtml,
            file: "test.html".to_string(),
            options: ItemOptions::default(),
        };
        assert!(item.produces_pdf());

        let screenshot = BatchItem {
            item_type: BatchItemType::ChromiumScreenshotHtml,
            file: "test.html".to_string(),
            options: ItemOptions::default(),
        };
        assert!(!screenshot.produces_pdf());
    }

    #[test]
    fn batch_item_output_extension() {
        let pdf_item = BatchItem {
            item_type: BatchItemType::ChromiumHtml,
            file: "test.html".to_string(),
            options: ItemOptions::default(),
        };
        assert_eq!(pdf_item.output_extension(), "pdf");

        let png_item = BatchItem {
            item_type: BatchItemType::ChromiumScreenshotHtml,
            file: "test.html".to_string(),
            options: ItemOptions::default(),
        };
        assert_eq!(png_item.output_extension(), "png");
    }

    #[test]
    fn batch_request_validation_empty_items() {
        let request = BatchRequest {
            items: vec![],
            output_mode: OutputMode::Zip,
            global_options: GlobalOptions::default(),
            merge_options: MergeOptions::default(),
        };
        let result = request.validate(&[]);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("at least one item"));
    }

    #[test]
    fn batch_request_validation_too_many_items() {
        let items: Vec<BatchItem> = (0..60)
            .map(|i| BatchItem {
                item_type: BatchItemType::ChromiumHtml,
                file: format!("test{}.html", i),
                options: ItemOptions::default(),
            })
            .collect();
        let request = BatchRequest {
            items,
            output_mode: OutputMode::Zip,
            global_options: GlobalOptions::default(),
            merge_options: MergeOptions::default(),
        };
        let result = request.validate(&[]);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("exceeds maximum"));
    }

    #[test]
    fn batch_request_validation_missing_file() {
        let request = BatchRequest {
            items: vec![BatchItem {
                item_type: BatchItemType::ChromiumHtml,
                file: "missing.html".to_string(),
                options: ItemOptions::default(),
            }],
            output_mode: OutputMode::Zip,
            global_options: GlobalOptions::default(),
            merge_options: MergeOptions::default(),
        };
        // No uploaded files provided
        let result = request.validate(&[]);
        assert!(result.is_err());
    }

    #[test]
    fn batch_request_validation_merge_requires_pdf() {
        let request = BatchRequest {
            items: vec![BatchItem {
                item_type: BatchItemType::ChromiumScreenshotHtml, // Screenshot produces PNG
                file: "test.html".to_string(),
                options: ItemOptions::default(),
            }],
            output_mode: OutputMode::Merge,
            global_options: GlobalOptions::default(),
            merge_options: MergeOptions::default(),
        };
        let uploaded = crate::multipart::UploadedFile {
            filename: "test.html".to_string(),
            field_name: "files".to_string(),
            path: std::path::PathBuf::from("/tmp/test.html"),
            content_type: None,
            size: 0,
        };
        let result = request.validate(&[uploaded]);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("merge"));
    }

    #[test]
    fn batch_request_validation_url_type_no_file_needed() {
        let request = BatchRequest {
            items: vec![BatchItem {
                item_type: BatchItemType::ChromiumUrl,
                file: "https://example.com".to_string(),
                options: ItemOptions::default(),
            }],
            output_mode: OutputMode::Zip,
            global_options: GlobalOptions::default(),
            merge_options: MergeOptions::default(),
        };
        // URLs don't need uploaded files
        let result = request.validate(&[]);
        assert!(result.is_ok());
    }

    #[test]
    fn batch_request_validation_invalid_url() {
        let request = BatchRequest {
            items: vec![BatchItem {
                item_type: BatchItemType::ChromiumUrl,
                file: "not-a-valid-url".to_string(),
                options: ItemOptions::default(),
            }],
            output_mode: OutputMode::Zip,
            global_options: GlobalOptions::default(),
            merge_options: MergeOptions::default(),
        };
        let result = request.validate(&[]);
        assert!(result.is_err());
    }

    #[test]
    fn batch_submit_response_serialization() {
        use std::time::SystemTime;
        let response = BatchSubmitResponse {
            batch_id: BatchId::new(),
            status: BatchStatus::Queued,
            expires_at: SystemTime::now(),
        };
        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("batchId"));
        assert!(json.contains("queued"));
    }

    #[test]
    fn batch_status_response_serialization_success() {
        use std::time::SystemTime;
        let response = BatchStatusResponse {
            batch_id: BatchId::new(),
            status: BatchStatus::Completed,
            submitted_at: SystemTime::now(),
            started_at: Some(SystemTime::now()),
            completed_at: Some(SystemTime::now()),
            progress: BatchProgress {
                total: 3,
                completed: 3,
                failed: 0,
                pending: 0,
            },
            items: vec![
                ItemResult {
                    index: 0,
                    file: "test.html".to_string(),
                    status: ItemStatus::Success,
                    output_type: Some("pdf".to_string()),
                    pages: Some(2),
                    bytes: Some(1024),
                    error: None,
                    error_code: None,
                }
            ],
            results: Some(BatchResultsSummary {
                succeeded: 3,
                failed: 0,
                total_bytes: 3072,
                output_ready: true,
            }),
            download_url: Some("/forms/batch/batch_123/download".to_string()),
        };
        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("completed"));
        assert!(json.contains("batchId"));
    }

    #[test]
    fn batch_status_response_serialization_failed() {
        use std::time::SystemTime;
        let response = BatchStatusResponse {
            batch_id: BatchId::new(),
            status: BatchStatus::Failed,
            submitted_at: SystemTime::now(),
            started_at: Some(SystemTime::now()),
            completed_at: Some(SystemTime::now()),
            progress: BatchProgress {
                total: 1,
                completed: 0,
                failed: 1,
                pending: 0,
            },
            items: vec![
                ItemResult {
                    index: 0,
                    file: "test.html".to_string(),
                    status: ItemStatus::Error,
                    output_type: None,
                    pages: None,
                    bytes: None,
                    error: Some("Conversion failed".to_string()),
                    error_code: Some(ErrorCode::ConversionFailed),
                }
            ],
            results: None,
            download_url: None,
        };
        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("failed"));
    }

    #[test]
    fn error_code_serialization() {
        let code = ErrorCode::InvalidOptions;
        let json = serde_json::to_string(&code).unwrap();
        assert_eq!(json, "\"INVALID_OPTIONS\"");
    }

    #[test]
    fn batch_id_from_raw() {
        let raw = "batch_abc123".to_string();
        let id = BatchId::from_raw(raw.clone());
        assert_eq!(id.to_string(), raw);
    }

    #[test]
    fn item_status_is_success() {
        assert!(ItemStatus::Success.is_success());
        assert!(!ItemStatus::Error.is_success());
        assert!(!ItemStatus::Pending.is_success());
        assert!(!ItemStatus::Processing.is_success());
    }
}
