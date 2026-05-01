//! Webhook system for async processing.
//!
//! Implements spec 15 — Webhook System.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::http::{HeaderMap, HeaderValue, header};
use engine::{OptimiseBackend, OptimisePreset};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tracing::{error, info, warn};

mod config;
mod queue;
mod validate;

pub use config::{WebhookConfig, extract_webhook_config};
pub use queue::{WebhookQueue, spawn_job, start_workers};
pub use validate::{validate_webhook_url, ValidationError};

/// If async webhook mode is requested, spawn a job and return a 202 response.
/// Returns `Ok(None)` if no webhook or sync mode — the caller should proceed
/// with normal synchronous processing.
pub async fn maybe_spawn_webhook(
    headers: &HeaderMap,
    state: &crate::state::AppState,
    operation: WebhookOperation,
    data: JobData,
) -> crate::error::ApiResult<Option<axum::response::Response>> {
    match extract_webhook_config(headers) {
        Ok(Some(config)) => {
            tracing::info!("webhook config found, sync_mode={}", config.sync_mode);
            if !config.sync_mode {
                if let Some(queue) = &state.webhook_queue {
                    let job_id = spawn_job(queue, operation, config, data).await
                        .map_err(|e| crate::error::ApiError::Webhook(e.to_string()))?;
                    let body = serde_json::json!({ "job_id": job_id });
                    let resp = axum::response::Response::builder()
                        .status(axum::http::StatusCode::ACCEPTED)
                        .header(axum::http::header::CONTENT_TYPE, "application/json")
                        .body(axum::body::Body::from(body.to_string()))
                        .unwrap();
                    return Ok(Some(resp));
                }
                tracing::warn!("webhook config but no queue available");
            }
            Ok(None)
        }
        Ok(None) => Ok(None),
        Err(e) => Err(crate::error::ApiError::Webhook(e.to_string())),
    }
}

/// Engine references needed by webhook workers to execute jobs.
#[derive(Clone)]
pub struct WebhookEngineContext {
    /// Chromium PDF backend (if feature enabled).
    #[cfg(feature = "chromium")]
    pub chromium: Option<Arc<dyn crate::backend::PdfBackend>>,
    #[cfg(not(feature = "chromium"))]
    pub chromium: Option<Arc<()>>,
    /// LibreOffice engine (if feature enabled).
    #[cfg(feature = "libreoffice")]
    pub libreoffice: Option<Arc<crate::supervised_engine::SupervisedLibreOfficeEngine>>,
    #[cfg(not(feature = "libreoffice"))]
    pub libreoffice: Option<Arc<()>>,
}

/// Webhook operation types.
#[derive(Debug, Clone)]
pub enum WebhookOperation {
    /// Chromium HTML to PDF conversion.
    ChromiumConvertHtml,
    /// Chromium URL to PDF conversion.
    ChromiumConvertUrl,
    /// Chromium Markdown to PDF conversion.
    ChromiumConvertMarkdown,
    /// Chromium HTML to screenshot.
    ChromiumScreenshotHtml,
    /// Chromium URL to screenshot.
    ChromiumScreenshotUrl,
    /// Chromium Markdown to screenshot.
    ChromiumScreenshotMarkdown,
    /// LibreOffice document conversion.
    LibreOfficeConvert,
    /// PDF merge operation.
    PdfMerge,
    /// PDF split operation.
    PdfSplit,
    /// PDF flatten operation.
    PdfFlatten,
    /// PDF metadata read.
    PdfMetadataRead,
    /// PDF metadata write.
    PdfMetadataWrite,
    /// PDF/A conversion.
    PdfConvert,
    /// PDF rotate operation.
    PdfRotate,
    /// PDF watermark.
    PdfWatermark,
    /// PDF stamp.
    PdfStamp,
    /// PDF encrypt.
    PdfEncrypt,
    /// PDF decrypt.
    PdfDecrypt,
    /// PDF optimise.
    PdfOptimise,
    /// PDF bookmarks read.
    PdfBookmarksRead,
    /// PDF bookmarks write.
    PdfBookmarksWrite,
}

impl WebhookOperation {
    fn as_str(&self) -> &'static str {
        match self {
            WebhookOperation::ChromiumConvertHtml => "chromium_convert_html",
            WebhookOperation::ChromiumConvertUrl => "chromium_convert_url",
            WebhookOperation::ChromiumConvertMarkdown => "chromium_convert_markdown",
            WebhookOperation::ChromiumScreenshotHtml => "chromium_screenshot_html",
            WebhookOperation::ChromiumScreenshotUrl => "chromium_screenshot_url",
            WebhookOperation::ChromiumScreenshotMarkdown => "chromium_screenshot_markdown",
            WebhookOperation::LibreOfficeConvert => "libreoffice_convert",
            WebhookOperation::PdfMerge => "pdf_merge",
            WebhookOperation::PdfSplit => "pdf_split",
            WebhookOperation::PdfFlatten => "pdf_flatten",
            WebhookOperation::PdfMetadataRead => "pdf_metadata_read",
            WebhookOperation::PdfMetadataWrite => "pdf_metadata_write",
            WebhookOperation::PdfConvert => "pdf_convert",
            WebhookOperation::PdfRotate => "pdf_rotate",
            WebhookOperation::PdfWatermark => "pdf_watermark",
            WebhookOperation::PdfStamp => "pdf_stamp",
            WebhookOperation::PdfEncrypt => "pdf_encrypt",
            WebhookOperation::PdfDecrypt => "pdf_decrypt",
            WebhookOperation::PdfOptimise => "pdf_optimise",
            WebhookOperation::PdfBookmarksRead => "pdf_bookmarks_read",
            WebhookOperation::PdfBookmarksWrite => "pdf_bookmarks_write",
        }
    }
}

/// Job status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum JobStatus {
    /// Job completed successfully.
    Success,
    /// Job failed with error.
    Error,
}

/// Webhook result payload sent to webhook URL.
#[derive(Debug, Clone, Serialize)]
pub struct WebhookResult {
    /// Unique job ID.
    pub job_id: String,
    /// Job status (success/error).
    pub status: JobStatus,
    /// Operation type.
    pub operation: String,
    /// Output filename if applicable.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filename: Option<String>,
    /// Error message if status is error.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// Processing duration in milliseconds.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
    /// JSON output for read operations (metadata, bookmarks).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<serde_json::Value>,
}

/// Webhook job definition.
#[derive(Debug, Clone)]
pub struct WebhookJob {
    /// Unique job ID.
    pub id: String,
    /// Operation type.
    pub operation: WebhookOperation,
    /// Webhook configuration.
    pub config: WebhookConfig,
    /// Processing data (serialized to pass between workers).
    pub data: JobData,
}

/// Job data types.
#[derive(Debug, Clone)]
pub enum JobData {
    /// Chromium HTML conversion job.
    ChromiumHtml {
        /// Raw HTML bytes.
        html: Vec<u8>,
        /// Conversion options.
        options: engine::PdfOptions,
        /// Request context.
        ctx: engine::RequestContext,
    },
    /// Chromium URL conversion job.
    ChromiumUrl {
        /// Target URL.
        url: String,
        /// Conversion options.
        options: engine::PdfOptions,
        /// Request context.
        ctx: engine::RequestContext,
    },
    /// Chromium Markdown conversion job.
    ChromiumMarkdown {
        /// Pre-rendered HTML bytes (markdown already converted).
        html: Vec<u8>,
        /// Conversion options.
        options: engine::PdfOptions,
        /// Request context.
        ctx: engine::RequestContext,
    },
    /// LibreOffice conversion job.
    LibreOffice {
        /// Source file bytes.
        files: Vec<Vec<u8>>,
        /// Conversion options.
        options: engine::OfficeOptions,
        /// Merge outputs into single PDF.
        merge: bool,
    },
    /// PDF merge job.
    PdfMerge {
        /// Files to merge.
        files: Vec<Vec<u8>>,
    },
    /// PDF split job.
    PdfSplit {
        /// Source file bytes.
        file: Vec<u8>,
        /// Split mode.
        mode: engine::pdfops::SplitMode,
    },
    /// PDF flatten job.
    PdfFlatten {
        /// Source file bytes.
        file: Vec<u8>,
    },
    /// PDF metadata read job.
    PdfMetadataRead {
        /// Source file bytes.
        file: Vec<u8>,
    },
    /// PDF metadata write job.
    PdfMetadataWrite {
        /// Source file bytes.
        file: Vec<u8>,
        /// Metadata to write.
        metadata: engine::pdfops::Metadata,
    },
    /// PDF/A conversion job.
    PdfConvert {
        /// Source file bytes.
        file: Vec<u8>,
        /// Target PDF/A profile.
        profile: engine::pdfa::PdfAProfile,
    },
    /// PDF rotate job.
    PdfRotate {
        /// Source file bytes.
        file: Vec<u8>,
        /// Rotation angle in degrees (90, 180, 270).
        angle: u16,
        /// Page ranges to rotate (None = all pages).
        pages: Option<engine::PageRanges>,
    },
    /// PDF watermark job.
    PdfWatermark {
        /// Source file bytes.
        file: Vec<u8>,
        /// Watermark options.
        options: engine::pdfops::WatermarkOptions,
    },
    /// PDF stamp job.
    PdfStamp {
        /// Source file bytes.
        file: Vec<u8>,
        /// Watermark options.
        options: engine::pdfops::WatermarkOptions,
    },
    /// PDF encrypt job.
    PdfEncrypt {
        /// Source file bytes.
        file: Vec<u8>,
        /// Password.
        password: String,
        /// Encryption algorithm.
        algorithm: engine::encrypt::EncryptionAlgorithm,
        /// Permissions.
        permissions: engine::encrypt::Permissions,
    },
    /// PDF decrypt job.
    PdfDecrypt {
        /// Source file bytes.
        file: Vec<u8>,
        /// Password.
        password: String,
    },
    /// PDF optimise job.
    PdfOptimise {
        /// Source file bytes.
        file: Vec<u8>,
        /// Compression preset.
        preset: String,
        /// Optional forced backend.
        backend: Option<String>,
    },
    /// PDF bookmarks read job.
    PdfBookmarksRead {
        /// Source file bytes.
        file: Vec<u8>,
    },
    /// PDF bookmarks write job.
    PdfBookmarksWrite {
        /// Source file bytes.
        file: Vec<u8>,
        /// Bookmarks to write.
        bookmarks: Vec<engine::Bookmark>,
    },
    /// Chromium HTML screenshot job.
    ChromiumScreenshotHtml {
        /// Raw HTML bytes.
        html: Vec<u8>,
        /// Screenshot options.
        options: engine::ScreenshotOptions,
    },
    /// Chromium URL screenshot job.
    ChromiumScreenshotUrl {
        /// Target URL.
        url: String,
        /// Screenshot options.
        options: engine::ScreenshotOptions,
    },
    /// Chromium Markdown screenshot job.
    ChromiumScreenshotMarkdown {
        /// Pre-rendered HTML bytes.
        html: Vec<u8>,
        /// Screenshot options.
        options: engine::ScreenshotOptions,
    },
}

/// Webhook delivery client.
pub struct WebhookClient {
    http: Client,
    max_retries: u32,
    retry_delay: Duration,
}

impl Default for WebhookClient {
    fn default() -> Self {
        Self {
            http: Client::builder()
                .timeout(Duration::from_secs(30))
                .build()
                .expect("valid client config"),
            max_retries: 3,
            retry_delay: Duration::from_secs(5),
        }
    }
}

/// Webhook payload types for delivery.
#[derive(Debug, Clone)]
pub enum WebhookPayload {
    /// Single PDF file attachment.
    Pdf {
        /// PDF file bytes.
        data: Vec<u8>,
        /// Output filename.
        filename: String,
    },
    /// ZIP archive attachment.
    Zip {
        /// ZIP archive bytes.
        data: Vec<u8>,
        /// Output filename.
        filename: String,
    },
    /// JSON-only payload (no attachment).
    Json {
        /// JSON response data.
        data: serde_json::Value,
    },
}

impl WebhookPayload {
    fn mime_type(&self) -> &'static str {
        match self {
            WebhookPayload::Pdf { .. } => "application/pdf",
            WebhookPayload::Zip { .. } => "application/zip",
            WebhookPayload::Json { .. } => "application/json",
        }
    }
}

impl WebhookClient {
    /// Deliver webhook with result.
    pub async fn deliver(
        &self,
        url: &str,
        result: &WebhookResult,
        extra_headers: &HeaderMap,
        payload: &WebhookPayload,
    ) -> Result<(), WebhookError> {
        let mut last_error = None;

        for attempt in 1..=self.max_retries {
            match self.try_deliver(url, result, extra_headers, payload).await {
                Ok(()) => {
                    info!(job_id = %result.job_id, url = %url, attempt, "Webhook delivered successfully");
                    return Ok(());
                }
                Err(e) => {
                    warn!(job_id = %result.job_id, url = %url, attempt, error = %e, "Webhook delivery failed, retrying");
                    last_error = Some(e);
                    if attempt < self.max_retries {
                        tokio::time::sleep(self.retry_delay).await;
                    }
                }
            }
        }

        Err(last_error.unwrap_or_else(|| {
            WebhookError::Delivery("Max retries exceeded".into())
        }))
    }

    async fn try_deliver(
        &self,
        url: &str,
        result: &WebhookResult,
        extra_headers: &HeaderMap,
        payload: &WebhookPayload,
    ) -> Result<(), WebhookError> {
        let mut request = self.http.post(url);

        // Add headers
        let mut headers = HeaderMap::new();
        headers.insert(header::CONTENT_TYPE, HeaderValue::from_static("application/json"));
        headers.insert(
            "gotenberg-trace",
            HeaderValue::from_str(&result.job_id)?,
        );

        // Add user extra headers
        for (key, value) in extra_headers {
            headers.insert(key.clone(), value.clone());
        }

        request = request.headers(headers);

        // Build body
        match payload {
            WebhookPayload::Pdf { data, filename } => {
                let form = reqwest::multipart::Form::new()
                    .text("metadata", serde_json::to_string(result)?)
                    .part(
                        "file",
                        reqwest::multipart::Part::bytes(data.clone())
                            .file_name(filename.clone())
                            .mime_str(payload.mime_type())?,
                    );
                request = request.multipart(form);
            }
            WebhookPayload::Zip { data, filename } => {
                let form = reqwest::multipart::Form::new()
                    .text("metadata", serde_json::to_string(result)?)
                    .part(
                        "file",
                        reqwest::multipart::Part::bytes(data.clone())
                            .file_name(filename.clone())
                            .mime_str(payload.mime_type())?,
                    );
                request = request.multipart(form);
            }
            WebhookPayload::Json { .. } => {
                request = request.json(result);
            }
        }

        let response = request.send().await.map_err(|e| WebhookError::Http(e))?;

        let status = response.status();
        if status.is_success() {
            Ok(())
        } else {
            let body = response.text().await.unwrap_or_default();
            Err(WebhookError::HttpStatus { status, body })
        }
    }
}

/// Webhook errors.
#[derive(Debug, thiserror::Error)]
pub enum WebhookError {
    /// Invalid webhook URL.
    #[error("Invalid webhook URL: {0}")]
    InvalidUrl(String),
    /// SSRF protection blocked the URL.
    #[error("SSRF protection: URL not allowed: {0}")]
    SsrfProtection(String),
    /// HTTP client error.
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),
    /// Non-success HTTP status.
    #[error("HTTP error status {status}: {body}")]
    HttpStatus {
        /// HTTP status code.
        status: reqwest::StatusCode,
        /// Response body.
        body: String,
    },
    /// Delivery failed after retries.
    #[error("Delivery failed: {0}")]
    Delivery(String),
    /// Invalid HTTP header value.
    #[error("Invalid header value: {0}")]
    InvalidHeader(#[from] axum::http::header::InvalidHeaderValue),
    /// JSON serialization error.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

/// Process webhook job and deliver result.
pub async fn process_webhook_job(
    job: WebhookJob,
    client: &WebhookClient,
    ctx: &WebhookEngineContext,
    start_time: Instant,
) -> Result<(), WebhookError> {
    let operation_str = job.operation.as_str().to_string();

    let result = execute_job(&job, ctx).await;

    let duration_ms = start_time.elapsed().as_millis() as u64;

    let (status, payload, error, output, filename) = match result {
        Ok(payload) => {
            let (filename, output) = match &payload {
                WebhookPayload::Pdf { filename, .. } => (Some(filename.clone()), None),
                WebhookPayload::Zip { filename, .. } => (Some(filename.clone()), None),
                WebhookPayload::Json { data } => (Some("result.json".into()), Some(data.clone())),
            };
            (JobStatus::Success, Some(payload), None, output, filename)
        }
        Err(e) => (JobStatus::Error, None, Some(e), None, None),
    };

    let webhook_result = WebhookResult {
        job_id: job.id.clone(),
        status,
        operation: operation_str,
        filename,
        error,
        duration_ms: Some(duration_ms),
        output,
    };

    // Determine webhook URL (error URL for errors if configured)
    let webhook_url = if webhook_result.status == JobStatus::Error && job.config.error_url.is_some() {
        job.config.error_url.as_ref().unwrap()
    } else {
        &job.config.webhook_url
    };

    let deliver_payload = payload.unwrap_or(WebhookPayload::Json { data: serde_json::Value::Null });

    // Deliver webhook
    client.deliver(
        webhook_url,
        &webhook_result,
        &job.config.extra_headers,
        &deliver_payload,
    ).await
}

/// Execute the actual job operation using engine references.
async fn execute_job(job: &WebhookJob, ctx: &WebhookEngineContext) -> Result<WebhookPayload, String> {
    match &job.operation {
        // ── Chromium PDF conversions ──
        WebhookOperation::ChromiumConvertHtml => {
            let (html, options, req_ctx) = match &job.data {
                JobData::ChromiumHtml { html, options, ctx } => (html, options, ctx),
                _ => return Err("Invalid job data for ChromiumConvertHtml".into()),
            };
            let backend = ctx.chromium.as_ref()
                .ok_or("Chromium backend not available")?;
            let html_str = String::from_utf8(html.clone()).map_err(|e| e.to_string())?;
            let pdf = backend.html_to_pdf(&html_str, None, options, req_ctx).await
                .map_err(|e| e.to_string())?;
            Ok(WebhookPayload::Pdf { data: pdf, filename: "result.pdf".into() })
        }
        WebhookOperation::ChromiumConvertUrl => {
            let (url, options, req_ctx) = match &job.data {
                JobData::ChromiumUrl { url, options, ctx } => (url, options, ctx),
                _ => return Err("Invalid job data for ChromiumConvertUrl".into()),
            };
            let backend = ctx.chromium.as_ref()
                .ok_or("Chromium backend not available")?;
            let pdf = backend.url_to_pdf(url, options, req_ctx).await
                .map_err(|e| e.to_string())?;
            Ok(WebhookPayload::Pdf { data: pdf, filename: "result.pdf".into() })
        }
        WebhookOperation::ChromiumConvertMarkdown => {
            let (html, options, req_ctx) = match &job.data {
                JobData::ChromiumMarkdown { html, options, ctx } => (html, options, ctx),
                _ => return Err("Invalid job data for ChromiumConvertMarkdown".into()),
            };
            let backend = ctx.chromium.as_ref()
                .ok_or("Chromium backend not available")?;
            let html_str = String::from_utf8(html.clone()).map_err(|e| e.to_string())?;
            let pdf = backend.html_to_pdf(&html_str, None, options, req_ctx).await
                .map_err(|e| e.to_string())?;
            Ok(WebhookPayload::Pdf { data: pdf, filename: "result.pdf".into() })
        }
        // ── Chromium screenshots ──
        WebhookOperation::ChromiumScreenshotHtml => {
            let (html, options) = match &job.data {
                JobData::ChromiumScreenshotHtml { html, options } => (html, options),
                _ => return Err("Invalid job data for ChromiumScreenshotHtml".into()),
            };
            let backend = ctx.chromium.as_ref()
                .ok_or("Chromium backend not available")?;
            let html_str = String::from_utf8(html.clone()).map_err(|e| e.to_string())?;
            let img = backend.html_to_screenshot(&html_str, options).await
                .map_err(|e| e.to_string())?;
            let ext = options.format.extension();
            Ok(WebhookPayload::Pdf { data: img, filename: format!("result.{ext}") })
        }
        WebhookOperation::ChromiumScreenshotUrl => {
            let (url, options) = match &job.data {
                JobData::ChromiumScreenshotUrl { url, options } => (url, options),
                _ => return Err("Invalid job data for ChromiumScreenshotUrl".into()),
            };
            let backend = ctx.chromium.as_ref()
                .ok_or("Chromium backend not available")?;
            let img = backend.url_to_screenshot(url, options).await
                .map_err(|e| e.to_string())?;
            let ext = options.format.extension();
            Ok(WebhookPayload::Pdf { data: img, filename: format!("result.{ext}") })
        }
        WebhookOperation::ChromiumScreenshotMarkdown => {
            let (html, options) = match &job.data {
                JobData::ChromiumScreenshotMarkdown { html, options } => (html, options),
                _ => return Err("Invalid job data for ChromiumScreenshotMarkdown".into()),
            };
            let backend = ctx.chromium.as_ref()
                .ok_or("Chromium backend not available")?;
            let html_str = String::from_utf8(html.clone()).map_err(|e| e.to_string())?;
            let img = backend.html_to_screenshot(&html_str, options).await
                .map_err(|e| e.to_string())?;
            let ext = options.format.extension();
            Ok(WebhookPayload::Pdf { data: img, filename: format!("result.{ext}") })
        }
        // ── LibreOffice ──
        WebhookOperation::LibreOfficeConvert => {
            let (files, options, merge) = match &job.data {
                JobData::LibreOffice { files, options, merge } => (files, options, *merge),
                _ => return Err("Invalid job data for LibreOfficeConvert".into()),
            };
            let engine = ctx.libreoffice.as_ref()
                .ok_or("LibreOffice engine not available")?;
            // Write files to temp dir
            let tmp_dir = tempfile::tempdir().map_err(|e| e.to_string())?;
            let mut paths = Vec::new();
            for (i, file) in files.iter().enumerate() {
                let path = tmp_dir.path().join(format!("input_{i}"));
                tokio::fs::write(&path, file).await.map_err(|e| e.to_string())?;
                paths.push(path);
            }
            let path_refs: Vec<PathBuf> = paths.iter().cloned().collect();
            if paths.len() == 1 || merge {
                let pdf = engine.convert(&paths[0], options).await
                    .map_err(|e| e.to_string())?;
                Ok(WebhookPayload::Pdf { data: pdf, filename: "result.pdf".into() })
            } else {
                let outputs = engine.convert_many(&path_refs, options).await
                    .map_err(|e| e.to_string())?;
                let names: Vec<String> = (0..outputs.len()).map(|i| format!("output_{i}.pdf")).collect();
                let zip = tokio::task::spawn_blocking(move || {
                    crate::routes::util::build_zip(&names, &outputs)
                }).await.map_err(|e| e.to_string())?
                    .map_err(|e| e.to_string())?;
                Ok(WebhookPayload::Zip { data: zip, filename: "result.zip".into() })
            }
        }
        // ── PDF merge ──
        WebhookOperation::PdfMerge => {
            let files = match &job.data {
                JobData::PdfMerge { files } => files.clone(),
                _ => return Err("Invalid job data for PdfMerge".into()),
            };
            let out = tokio::task::spawn_blocking(move || {
                let refs: Vec<&[u8]> = files.iter().map(|v| v.as_slice()).collect();
                engine::pdfops::merge(&refs)
            })
                .await.map_err(|e| e.to_string())?
                .map_err(|e| e.to_string())?;
            Ok(WebhookPayload::Pdf { data: out, filename: "result.pdf".into() })
        }
        // ── PDF split ──
        WebhookOperation::PdfSplit => {
            let (file, mode) = match &job.data {
                JobData::PdfSplit { file, mode } => (file, mode),
                _ => return Err("Invalid job data for PdfSplit".into()),
            };
            let file = file.clone();
            let mode = mode.clone();
            let outputs = tokio::task::spawn_blocking(move || engine::pdfops::split(&file, &mode))
                .await.map_err(|e| e.to_string())?
                .map_err(|e| e.to_string())?;
            let names: Vec<String> = (0..outputs.len()).map(|i| format!("output_{i}.pdf")).collect();
            let zip = tokio::task::spawn_blocking(move || {
                crate::routes::util::build_zip(&names, &outputs)
            }).await.map_err(|e| e.to_string())?
                .map_err(|e| e.to_string())?;
            Ok(WebhookPayload::Zip { data: zip, filename: "result.zip".into() })
        }
        // ── PDF flatten ──
        WebhookOperation::PdfFlatten => {
            let file = match &job.data {
                JobData::PdfFlatten { file } => file,
                _ => return Err("Invalid job data for PdfFlatten".into()),
            };
            let file = file.clone();
            let out = tokio::task::spawn_blocking(move || engine::pdfops::flatten(&file))
                .await.map_err(|e| e.to_string())?
                .map_err(|e| e.to_string())?;
            Ok(WebhookPayload::Pdf { data: out, filename: "result.pdf".into() })
        }
        // ── PDF metadata read ──
        WebhookOperation::PdfMetadataRead => {
            let file = match &job.data {
                JobData::PdfMetadataRead { file } => file,
                _ => return Err("Invalid job data for PdfMetadataRead".into()),
            };
            let file = file.clone();
            let meta = tokio::task::spawn_blocking(move || engine::pdfops::read_metadata(&file))
                .await.map_err(|e| e.to_string())?
                .map_err(|e| e.to_string())?;
            let json = serde_json::to_value(&meta).map_err(|e| e.to_string())?;
            Ok(WebhookPayload::Json { data: json })
        }
        // ── PDF metadata write ──
        WebhookOperation::PdfMetadataWrite => {
            let (file, metadata) = match &job.data {
                JobData::PdfMetadataWrite { file, metadata } => (file, metadata),
                _ => return Err("Invalid job data for PdfMetadataWrite".into()),
            };
            let file = file.clone();
            let metadata = metadata.clone();
            let out = tokio::task::spawn_blocking(move || engine::pdfops::write_metadata(&file, &metadata))
                .await.map_err(|e| e.to_string())?
                .map_err(|e| e.to_string())?;
            Ok(WebhookPayload::Pdf { data: out, filename: "result.pdf".into() })
        }
        // ── PDF/A convert ──
        WebhookOperation::PdfConvert => {
            let (file, profile) = match &job.data {
                JobData::PdfConvert { file, profile } => (file, profile),
                _ => return Err("Invalid job data for PdfConvert".into()),
            };
            let file = file.clone();
            let profile = *profile;
            let out = engine::pdfa::convert_to_pdfa(&file, profile).await
                .map_err(|e| e.to_string())?;
            Ok(WebhookPayload::Pdf { data: out, filename: "result.pdf".into() })
        }
        // ── PDF rotate ──
        WebhookOperation::PdfRotate => {
            let (file, angle, pages) = match &job.data {
                JobData::PdfRotate { file, angle, pages } => (file, angle, pages),
                _ => return Err("Invalid job data for PdfRotate".into()),
            };
            let file = file.clone();
            let angle = *angle;
            let pages = pages.clone().unwrap_or_else(|| engine::PageRanges::parse("1-").expect("1- is valid"));
            let out = tokio::task::spawn_blocking(move || engine::pdfops::rotate(&file, &pages, angle as i32))
                .await.map_err(|e| e.to_string())?
                .map_err(|e| e.to_string())?;
            Ok(WebhookPayload::Pdf { data: out, filename: "result.pdf".into() })
        }
        // ── PDF watermark ──
        WebhookOperation::PdfWatermark => {
            let (file, options) = match &job.data {
                JobData::PdfWatermark { file, options } => (file, options),
                _ => return Err("Invalid job data for PdfWatermark".into()),
            };
            let file = file.clone();
            let options = options.clone();
            let out = tokio::task::spawn_blocking(move || engine::pdfops::watermark(&file, &options))
                .await.map_err(|e| e.to_string())?
                .map_err(|e| e.to_string())?;
            Ok(WebhookPayload::Pdf { data: out, filename: "result.pdf".into() })
        }
        // ── PDF stamp ──
        WebhookOperation::PdfStamp => {
            let (file, options) = match &job.data {
                JobData::PdfStamp { file, options } => (file, options),
                _ => return Err("Invalid job data for PdfStamp".into()),
            };
            let file = file.clone();
            let options = options.clone();
            let out = tokio::task::spawn_blocking(move || engine::pdfops::watermark(&file, &options))
                .await.map_err(|e| e.to_string())?
                .map_err(|e| e.to_string())?;
            Ok(WebhookPayload::Pdf { data: out, filename: "result.pdf".into() })
        }
        // ── PDF encrypt ──
        WebhookOperation::PdfEncrypt => {
            let (file, password, algorithm, permissions) = match &job.data {
                JobData::PdfEncrypt { file, password, algorithm, permissions } => (file, password, algorithm, permissions),
                _ => return Err("Invalid job data for PdfEncrypt".into()),
            };
            let file = file.clone();
            let password = password.clone();
            let algorithm = *algorithm;
            let permissions = *permissions;
            let out = engine::encrypt::encrypt_pdf(&file, Some(&password), None, algorithm, permissions).await
                .map_err(|e| e.to_string())?;
            Ok(WebhookPayload::Pdf { data: out, filename: "result.pdf".into() })
        }
        // ── PDF decrypt ──
        WebhookOperation::PdfDecrypt => {
            let (file, password) = match &job.data {
                JobData::PdfDecrypt { file, password } => (file, password),
                _ => return Err("Invalid job data for PdfDecrypt".into()),
            };
            let file = file.clone();
            let password = password.clone();
            let out = engine::encrypt::decrypt_pdf(&file, &password).await
                .map_err(|e| e.to_string())?;
            Ok(WebhookPayload::Pdf { data: out, filename: "result.pdf".into() })
        }
        // ── PDF optimise ──
        WebhookOperation::PdfOptimise => {
            let (file, preset_str, backend_str) = match &job.data {
                JobData::PdfOptimise { file, preset, backend } => (file, preset, backend),
                _ => return Err("Invalid job data for PdfOptimise".into()),
            };
            let file = file.clone();
            let preset = OptimisePreset::from_str(preset_str)
                .ok_or_else(|| format!("Invalid preset: {}", preset_str))?;
            let preferred_backend = backend_str.as_ref().and_then(|b| match b.as_str() {
                "ghostscript" => Some(OptimiseBackend::Ghostscript),
                "qpdf" => Some(OptimiseBackend::Qpdf),
                _ => None,
            });
            let out = engine::optimise_pdf(&file, preset, preferred_backend).await
                .map_err(|e| e.to_string())?;
            Ok(WebhookPayload::Pdf { data: out.data, filename: "optimised.pdf".into() })
        }
        // ── PDF bookmarks read ──
        WebhookOperation::PdfBookmarksRead => {
            let file = match &job.data {
                JobData::PdfBookmarksRead { file } => file,
                _ => return Err("Invalid job data for PdfBookmarksRead".into()),
            };
            let file = file.clone();
            let bms = tokio::task::spawn_blocking(move || engine::read_bookmarks(&file))
                .await.map_err(|e| e.to_string())?
                .map_err(|e| e.to_string())?;
            let json = serde_json::to_value(&bms).map_err(|e| e.to_string())?;
            Ok(WebhookPayload::Json { data: json })
        }
        // ── PDF bookmarks write ──
        WebhookOperation::PdfBookmarksWrite => {
            let (file, bookmarks) = match &job.data {
                JobData::PdfBookmarksWrite { file, bookmarks } => (file, bookmarks),
                _ => return Err("Invalid job data for PdfBookmarksWrite".into()),
            };
            let file = file.clone();
            let bookmarks = bookmarks.clone();
            let out = tokio::task::spawn_blocking(move || engine::write_bookmarks(&file, &bookmarks))
                .await.map_err(|e| e.to_string())?
                .map_err(|e| e.to_string())?;
            Ok(WebhookPayload::Pdf { data: out, filename: "result.pdf".into() })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn webhook_operation_as_str() {
        assert_eq!(WebhookOperation::PdfMerge.as_str(), "pdf_merge");
        assert_eq!(WebhookOperation::ChromiumConvertHtml.as_str(), "chromium_convert_html");
    }

    #[test]
    fn job_status_serialization() {
        let success = JobStatus::Success;
        let error = JobStatus::Error;
        assert_eq!(serde_json::to_string(&success).unwrap(), "\"success\"");
        assert_eq!(serde_json::to_string(&error).unwrap(), "\"error\"");
    }
}
