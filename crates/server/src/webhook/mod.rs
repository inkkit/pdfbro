//! Webhook system for async processing.
//!
//! Implements spec 15 — Webhook System.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::http::{HeaderMap, HeaderValue, header};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tokio::sync::{Mutex, mpsc};
use tracing::{error, info, warn};

mod config;
mod queue;
mod validate;

pub use config::{WebhookConfig, extract_webhook_config};
pub use queue::{WebhookQueue, spawn_job, start_workers};
pub use validate::{validate_webhook_url, ValidationError};

/// Webhook operation types.
#[derive(Debug, Clone)]
pub enum WebhookOperation {
    /// Chromium HTML to PDF conversion.
    ChromiumConvertHtml,
    /// Chromium URL to PDF conversion.
    ChromiumConvertUrl,
    /// Chromium Markdown to PDF conversion.
    ChromiumConvertMarkdown,
    /// LibreOffice document conversion.
    LibreOfficeConvert,
    /// PDF merge operation.
    PdfMerge,
    /// PDF split operation.
    PdfSplit,
    /// PDF flatten operation.
    PdfFlatten,
    /// PDF metadata read/write.
    PdfMetadata,
    /// PDF/A conversion.
    PdfConvert,
}

impl WebhookOperation {
    fn as_str(&self) -> &'static str {
        match self {
            WebhookOperation::ChromiumConvertHtml => "chromium_convert_html",
            WebhookOperation::ChromiumConvertUrl => "chromium_convert_url",
            WebhookOperation::ChromiumConvertMarkdown => "chromium_convert_markdown",
            WebhookOperation::LibreOfficeConvert => "libreoffice_convert",
            WebhookOperation::PdfMerge => "pdf_merge",
            WebhookOperation::PdfSplit => "pdf_split",
            WebhookOperation::PdfFlatten => "pdf_flatten",
            WebhookOperation::PdfMetadata => "pdf_metadata",
            WebhookOperation::PdfConvert => "pdf_convert",
        }
    }
}

/// Job status.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
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
    ChromiumHtml {
        html: Vec<u8>,
        options: serde_json::Value,
    },
    ChromiumUrl {
        url: String,
        options: serde_json::Value,
    },
    ChromiumMarkdown {
        markdown: Vec<u8>,
        options: serde_json::Value,
    },
    LibreOffice {
        file: Vec<u8>,
        options: serde_json::Value,
        filename: String,
    },
    PdfMerge {
        files: Vec<Vec<u8>>,
    },
    PdfSplit {
        file: Vec<u8>,
        mode: String,
        span: Option<String>,
    },
    PdfFlatten {
        file: Vec<u8>,
    },
    PdfMetadataRead {
        file: Vec<u8>,
    },
    PdfMetadataWrite {
        file: Vec<u8>,
        metadata: serde_json::Value,
    },
    PdfConvert {
        file: Vec<u8>,
        profile: String,
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

impl WebhookClient {
    /// Deliver webhook with result.
    pub async fn deliver(
        &self,
        url: &str,
        result: &WebhookResult,
        extra_headers: &HeaderMap,
        pdf_data: Option<&[u8]>,
    ) -> Result<(), WebhookError> {
        let mut last_error = None;

        for attempt in 1..=self.max_retries {
            match self.try_deliver(url, result, extra_headers, pdf_data).await {
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
        pdf_data: Option<&[u8]>,
    ) -> Result<(), WebhookError> {
        let mut request = self.http.post(url);

        // Add headers
        let mut headers = HeaderMap::new();
        headers.insert(header::CONTENT_TYPE, HeaderValue::from_static("application/json"));
        headers.insert(
            HeaderValue::from_static("gotenberg-trace"),
            HeaderValue::from_str(&result.job_id)?,
        );

        // Add user extra headers
        for (key, value) in extra_headers {
            headers.insert(key.clone(), value.clone());
        }

        request = request.headers(headers);

        // Build body
        let body = if let Some(pdf) = pdf_data {
            // Multipart with JSON metadata and PDF file
            let form = reqwest::multipart::Form::new()
                .text("metadata", serde_json::to_string(result)?)
                .part(
                    "file",
                    reqwest::multipart::Part::bytes(pdf.to_vec())
                        .file_name(result.filename.clone().unwrap_or_else(|| "result.pdf".into()))
                        .mime_str("application/pdf")?,
                );
            request = request.multipart(form);
        } else {
            // JSON only
            request = request.json(result);
        };

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
    #[error("Invalid webhook URL: {0}")]
    InvalidUrl(String),
    #[error("SSRF protection: URL not allowed: {0}")]
    SsrfProtection(String),
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("HTTP error status {status}: {body}")]
    HttpStatus { status: reqwest::StatusCode, body: String },
    #[error("Delivery failed: {0}")]
    Delivery(String),
    #[error("Invalid header value: {0}")]
    InvalidHeader(#[from] axum::http::header::InvalidHeaderValue),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

/// Process webhook job and deliver result.
pub async fn process_webhook_job(
    job: WebhookJob,
    client: &WebhookClient,
    start_time: Instant,
) -> Result<(), WebhookError> {
    let operation_str = job.operation.as_str().to_string();

    // Execute job (this would call the actual engine functions)
    let (result, pdf_data) = execute_job(&job).await;

    let duration_ms = start_time.elapsed().as_millis() as u64;

    let webhook_result = WebhookResult {
        job_id: job.id.clone(),
        status: if result.is_ok() { JobStatus::Success } else { JobStatus::Error },
        operation: operation_str,
        filename: pdf_data.as_ref().map(|_| "result.pdf".into()),
        error: result.err(),
        duration_ms: Some(duration_ms),
    };

    // Determine webhook URL (error URL for errors if configured)
    let webhook_url = if webhook_result.status == JobStatus::Error && job.config.error_url.is_some() {
        job.config.error_url.as_ref().unwrap()
    } else {
        &job.config.webhook_url
    };

    // Deliver webhook
    client.deliver(
        webhook_url,
        &webhook_result,
        &job.config.extra_headers,
        pdf_data.as_deref(),
    ).await
}

/// Execute the actual job operation.
async fn execute_job(job: &WebhookJob) -> (Result<(), String>, Option<Vec<u8>>) {
    // This would integrate with the engine functions
    // For now, return a placeholder that always succeeds
    (Ok(()), None)
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
