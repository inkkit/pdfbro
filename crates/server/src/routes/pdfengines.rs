//! `/forms/pdfengines/*` route handlers (merge / split / flatten / metadata).

use std::collections::{BTreeMap, HashMap};

use axum::extract::{Multipart, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use engine::PageRanges;
use engine::pdfops::{self, Metadata, SplitMode};

use crate::error::{ApiError, ApiResult};
use crate::multipart::{FormFields, UploadedFile};
use crate::routes::util::{build_zip, pdf_response, zip_response};
use crate::state::AppState;
use engine::Bookmark;
use engine::PdfAProfile;
use engine::bookmarks::{read_bookmarks, write_bookmarks};
use engine::pdfa::convert_to_pdfa;
use engine::pdfops::{WatermarkKind, WatermarkOptions, Position as WatermarkPosition, watermark};
use engine::encrypt::{EncryptionAlgorithm, Permissions, encrypt_pdf, decrypt_pdf};

const SPAWN_BLOCKING_THRESHOLD: usize = 1024 * 1024;

// ---------------------------------------------------------------------------
// /forms/pdfengines/merge
// ---------------------------------------------------------------------------

/// `POST /forms/pdfengines/merge`.
pub async fn pdfengines_merge(
    State(state): State<AppState>,
    headers: HeaderMap,
    mp: Multipart,
) -> ApiResult<Response> {
    let _permit = acquire_permit(&state).await?;
    let form = FormFields::from_multipart(mp).await?;
    let files = form.files_by_field("files");
    if files.len() < 2 {
        return Err(ApiError::InvalidField {
            field: "files",
            message: "merge requires at least two files".to_string(),
        });
    }

    // Check for async webhook mode before processing.
    tracing::info!("pdfengines_merge: extracting webhook config from headers");
    match crate::webhook::extract_webhook_config(&headers) {
        Ok(Some(config)) => {
            tracing::info!("pdfengines_merge: webhook config found, sync_mode={}", config.sync_mode);
            if !config.sync_mode {
                let blobs = read_all(&files).await?;
                if let Some(queue) = &state.webhook_queue {
                    let job_id = crate::webhook::spawn_job(
                        queue,
                        crate::webhook::WebhookOperation::PdfMerge,
                        config,
                        crate::webhook::JobData::PdfMerge { files: blobs },
                    )
                    .await?;

                    let body = serde_json::json!({ "job_id": job_id });
                    let mut resp_headers = HeaderMap::new();
                    resp_headers.insert(
                        header::CONTENT_TYPE,
                        HeaderValue::from_static("application/json"),
                    );
                    return Ok((StatusCode::ACCEPTED, resp_headers, axum::body::Body::from(body.to_string())).into_response());
                }
                tracing::warn!("pdfengines_merge: webhook config but no queue available");
            }
        }
        Ok(None) => {
            tracing::info!("pdfengines_merge: no webhook config");
        }
        Err(e) => {
            tracing::warn!("pdfengines_merge: webhook config extraction failed: {}", e);
            return Err(ApiError::Webhook(e.to_string()));
        }
    }

    let blobs = read_all(&files).await?;
    let total: usize = blobs.iter().map(Vec::len).sum();
    let merged = if total > SPAWN_BLOCKING_THRESHOLD {
        tokio::task::spawn_blocking(move || merge_blobs(&blobs))
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))??
    } else {
        merge_blobs(&blobs)?
    };

    let merged = if let Some(meta_str) = form.map.get("metadata").filter(|s| !s.is_empty()) {
        let meta: Metadata =
            serde_json::from_str(meta_str).map_err(|e| ApiError::InvalidField {
                field: "metadata",
                message: e.to_string(),
            })?;
        let bytes = merged;
        tokio::task::spawn_blocking(move || pdfops::write_metadata(&bytes, &meta))
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))??
    } else {
        merged
    };

    let filename = output_filename(&headers, "result");
    Ok(pdf_response(merged, &filename))
}

fn merge_blobs(blobs: &[Vec<u8>]) -> engine::EngineResult<Vec<u8>> {
    let refs: Vec<&[u8]> = blobs.iter().map(|v| v.as_slice()).collect();
    pdfops::merge(&refs)
}

// ---------------------------------------------------------------------------
// /forms/pdfengines/split
// ---------------------------------------------------------------------------

/// `POST /forms/pdfengines/split`.
pub async fn pdfengines_split(State(state): State<AppState>, headers: HeaderMap, mp: Multipart) -> ApiResult<Response> {
    let _permit = acquire_permit(&state).await?;
    let form = FormFields::from_multipart(mp).await?;
    let files = form.files_by_field("files");
    if files.len() != 1 {
        return Err(ApiError::InvalidField {
            field: "files",
            message: "split expects exactly one file".to_string(),
        });
    }
    let bytes = read_one(files[0]).await?;

    let mut mode = parse_split_mode(&form.map)?;
    let unify = parse_bool_field(&form.map, "splitUnify")?.unwrap_or(false);
    let mode_was_pages =
        matches!(mode, SplitMode::ByRanges(_)) && form.map.contains_key("splitSpan");

    // In Gotenberg's "pages" mode, each page in the specified range becomes
    // its own output file (rather than one file per range chunk).
    if mode_was_pages {
        let total = lopdf::Document::load_mem(&bytes)
            .map_err(|e| ApiError::Internal(e.to_string()))?
            .get_pages()
            .len() as u32;
        if let SplitMode::ByRanges(ranges) = &mode {
            let mut pages: Vec<u32> = Vec::new();
            for r in ranges {
                pages.extend(r.expand(total));
            }
            pages.sort_unstable();
            pages.dedup();
            mode = SplitMode::Pages(pages);
        }
    }

    let mut chunks = if bytes.len() > SPAWN_BLOCKING_THRESHOLD {
        let mode_clone = mode.clone();
        tokio::task::spawn_blocking(move || pdfops::split(&bytes, &mode_clone))
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))??
    } else {
        pdfops::split(&bytes, &mode)?
    };

    if chunks.is_empty() {
        return Err(ApiError::InvalidField {
            field: "splitPages",
            message: "no pages selected by split request".to_string(),
        });
    }

    if chunks.len() == 1
        && let Some(only) = chunks.pop()
    {
        let filename = output_filename(&headers, "result");
        return Ok(pdf_response(only, &filename));
    }

    if unify && mode_was_pages {
        // Reassemble all chunks into a single PDF (Gotenberg quirk).
        let merged = tokio::task::spawn_blocking(move || merge_blobs(&chunks))
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))??;
        let filename = output_filename(&headers, "result");
        return Ok(pdf_response(merged, &filename));
    }

    let names: Vec<String> = (1..=chunks.len())
        .map(|i| format!("result-{i:03}.pdf"))
        .collect();
    let zip = build_zip(&names, &chunks)?;
    Ok(zip_response(zip, "result.zip"))
}

fn parse_split_mode(map: &HashMap<String, String>) -> ApiResult<SplitMode> {
    // Accept both `splitMode` (Folio) and `mode` (Gotenberg).
    let raw = map
        .get("splitMode")
        .or_else(|| map.get("mode"))
        .map(String::as_str)
        .unwrap_or("intervals");
    match raw.trim().to_ascii_lowercase().as_str() {
        "intervals" => {
            let span_raw = map
                .get("splitSpan")
                .ok_or(ApiError::MissingField("splitSpan"))?;
            let span: u32 =
                span_raw
                    .parse()
                    .map_err(|e: std::num::ParseIntError| ApiError::InvalidField {
                        field: "splitSpan",
                        message: e.to_string(),
                    })?;
            Ok(SplitMode::EveryN(span))
        }
        "pages" => {
            let pages_raw = map
                .get("splitSpan")
                .ok_or(ApiError::MissingField("splitSpan"))?;
            let mut ranges: Vec<PageRanges> = Vec::new();
            for chunk in pages_raw
                .split(',')
                .map(str::trim)
                .filter(|s| !s.is_empty())
            {
                ranges.push(PageRanges::parse(chunk)?);
            }
            if ranges.is_empty() {
                return Err(ApiError::InvalidField {
                    field: "splitSpan",
                    message: "expected one or more page-range chunks".to_string(),
                });
            }
            Ok(SplitMode::ByRanges(ranges))
        }
        other => Err(ApiError::InvalidField {
            field: "splitMode",
            message: format!("expected `intervals` or `pages`, got `{other}`"),
        }),
    }
}

// ---------------------------------------------------------------------------
// /forms/pdfengines/flatten
// ---------------------------------------------------------------------------

/// `POST /forms/pdfengines/flatten`.
pub async fn pdfengines_flatten(
    State(state): State<AppState>,
    headers: HeaderMap,
    mp: Multipart,
) -> ApiResult<Response> {
    let _permit = acquire_permit(&state).await?;
    let form = FormFields::from_multipart(mp).await?;
    let files = form.files_by_field("files");
    if files.is_empty() {
        return Err(ApiError::MissingFile("files".to_string()));
    }

    let mut outputs: Vec<Vec<u8>> = Vec::with_capacity(files.len());
    for f in &files {
        let bytes = read_one(f).await?;
        let flat = if bytes.len() > SPAWN_BLOCKING_THRESHOLD {
            tokio::task::spawn_blocking(move || pdfops::flatten(&bytes))
                .await
                .map_err(|e| ApiError::Internal(e.to_string()))??
        } else {
            pdfops::flatten(&bytes)?
        };
        outputs.push(flat);
    }

    if outputs.len() == 1
        && let Some(only) = outputs.pop()
    {
        let filename = output_filename(&headers, "result");
        return Ok(pdf_response(only, &filename));
    }
    let names: Vec<String> = files.iter().map(|f| f.filename.clone()).collect();
    let zip = build_zip(&names, &outputs)?;
    Ok(zip_response(zip, "result.zip"))
}

// ---------------------------------------------------------------------------
// /forms/pdfengines/metadata/{read,write}
// ---------------------------------------------------------------------------

/// `POST /forms/pdfengines/metadata/read`.
pub async fn pdfengines_metadata_read(
    State(state): State<AppState>,
    mp: Multipart,
) -> ApiResult<Response> {
    let _permit = acquire_permit(&state).await?;
    let form = FormFields::from_multipart(mp).await?;
    let files = form.files_by_field("files");
    if files.is_empty() {
        return Err(ApiError::MissingFile("files".to_string()));
    }

    let mut out: BTreeMap<String, Metadata> = BTreeMap::new();
    for f in &files {
        let bytes = read_one(f).await?;
        let name = f.filename.clone();
        let meta = if bytes.len() > SPAWN_BLOCKING_THRESHOLD {
            tokio::task::spawn_blocking(move || pdfops::read_metadata(&bytes))
                .await
                .map_err(|e| ApiError::Internal(e.to_string()))??
        } else {
            pdfops::read_metadata(&bytes)?
        };
        out.insert(name, meta);
    }

    let body = serde_json::to_vec(&out).map_err(|e| ApiError::Internal(e.to_string()))?;
    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/json"),
    );
    headers.insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_static("attachment; filename=\"result.json\""),
    );
    Ok((StatusCode::OK, headers, body).into_response())
}

/// `POST /forms/pdfengines/metadata/write`.
pub async fn pdfengines_metadata_write(
    State(state): State<AppState>,
    headers: HeaderMap,
    mp: Multipart,
) -> ApiResult<Response> {
    let _permit = acquire_permit(&state).await?;
    let form = FormFields::from_multipart(mp).await?;
    let files = form.files_by_field("files");
    if files.is_empty() {
        return Err(ApiError::MissingFile("files".to_string()));
    }

    let meta_str = form
        .map
        .get("metadata")
        .filter(|s| !s.is_empty())
        .ok_or(ApiError::MissingField("metadata"))?;
    let meta: Metadata = serde_json::from_str(meta_str).map_err(|e| ApiError::InvalidField {
        field: "metadata",
        message: e.to_string(),
    })?;

    let mut outputs: Vec<Vec<u8>> = Vec::with_capacity(files.len());
    for f in &files {
        let bytes = read_one(f).await?;
        let meta_clone = meta.clone();
        let written = if bytes.len() > SPAWN_BLOCKING_THRESHOLD {
            tokio::task::spawn_blocking(move || pdfops::write_metadata(&bytes, &meta_clone))
                .await
                .map_err(|e| ApiError::Internal(e.to_string()))??
        } else {
            pdfops::write_metadata(&bytes, &meta_clone)?
        };
        outputs.push(written);
    }

    if outputs.len() == 1
        && let Some(only) = outputs.pop()
    {
        let filename = output_filename(&headers, "result");
        return Ok(pdf_response(only, &filename));
    }
    let names: Vec<String> = files.iter().map(|f| f.filename.clone()).collect();
    let zip = build_zip(&names, &outputs)?;
    Ok(zip_response(zip, "result.zip"))
}

// ---------------------------------------------------------------------------
// /forms/pdfengines/rotate
// ---------------------------------------------------------------------------

/// `POST /forms/pdfengines/rotate`.
/// Rotates selected pages by 90° increments.
pub async fn pdfengines_rotate(
    State(state): State<AppState>,
    headers: HeaderMap,
    mp: Multipart,
) -> ApiResult<Response> {
    let _permit = acquire_permit(&state).await?;
    let form = FormFields::from_multipart(mp).await?;
    let files = form.files_by_field("files");
    if files.len() != 1 {
        return Err(ApiError::InvalidField {
            field: "files",
            message: "rotate expects exactly one PDF file".to_string(),
        });
    }
    let bytes = read_one(files[0]).await?;

    let angle_str = form.map.get("rotate").ok_or(ApiError::MissingField("rotate"))?;
    let angle: i32 = angle_str.parse().map_err(|e| ApiError::InvalidField {
        field: "rotate",
        message: format!("expected integer angle: {e}"),
    })?;

    // Default to all pages ("1-" is open-ended, covers every page after clamping).
    let pages = PageRanges::parse("1-").map_err(|e| ApiError::Internal(e.to_string()))?;
    let rotated = tokio::task::spawn_blocking(move || {
        pdfops::rotate(&bytes, &pages, angle)
    })
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    let filename = output_filename(&headers, "rotated");

    Ok(pdf_response(rotated, &filename))
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

async fn read_all(files: &[&UploadedFile]) -> ApiResult<Vec<Vec<u8>>> {
    let mut out = Vec::with_capacity(files.len());
    for f in files {
        out.push(read_one(f).await?);
    }
    Ok(out)
}

async fn read_one(file: &UploadedFile) -> ApiResult<Vec<u8>> {
    tokio::fs::read(&file.path)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))
}

/// Extract `Gotenberg-Output-Filename` from HTTP headers.
/// Strips any trailing `.pdf` and re-adds it.
fn output_filename(headers: &HeaderMap, default: &str) -> String {
    let raw = headers
        .get("Gotenberg-Output-Filename")
        .and_then(|v| v.to_str().ok());
    match raw {
        Some(s) => format!("{}.pdf", s.trim_end_matches(".pdf")),
        None => format!("{}.pdf", default.trim_end_matches(".pdf")),
    }
}

async fn acquire_permit(state: &AppState) -> ApiResult<tokio::sync::OwnedSemaphorePermit> {
    state
        .sem
        .clone()
        .acquire_owned()
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))
}

fn parse_bool_field(map: &HashMap<String, String>, key: &'static str) -> ApiResult<Option<bool>> {
    match map.get(key) {
        None => Ok(None),
        Some(s) if s.is_empty() => Ok(None),
        Some(s) => match s.trim().to_ascii_lowercase().as_str() {
            "1" | "true" | "yes" | "on" => Ok(Some(true)),
            "0" | "false" | "no" | "off" => Ok(Some(false)),
            other => Err(ApiError::InvalidField {
                field: key,
                message: format!("expected boolean, got `{other}`"),
            }),
        },
    }
}

// ---------------------------------------------------------------------------
// /forms/pdfengines/convert (PDF/A conversion)
// ---------------------------------------------------------------------------

/// `POST /forms/pdfengines/convert`.
/// Converts PDF to PDF/A conformance (PDF/A-1b, PDF/A-2b, PDF/A-3b).
pub async fn pdfengines_convert(
    State(state): State<AppState>,
    headers: HeaderMap,
    mp: Multipart,
) -> ApiResult<Response> {
    let _permit = acquire_permit(&state).await?;
    let form = FormFields::from_multipart(mp).await?;
    let files = form.files_by_field("files");
    if files.len() != 1 {
        return Err(ApiError::InvalidField {
            field: "files",
            message: "convert expects exactly one PDF file".to_string(),
        });
    }
    let bytes = read_one(files[0]).await?;

    // Parse PDF/A profile
    let profile_str = form.map.get("pdfa").ok_or(ApiError::MissingField("pdfa"))?;
    let profile: PdfAProfile = profile_str.parse().map_err(|e: String| ApiError::InvalidField {
        field: "pdfa",
        message: e,
    })?;

    // Run conversion
    let converted = convert_to_pdfa(&bytes, profile).await.map_err(|e| ApiError::Internal(e.to_string()))?;

    let filename = output_filename(&headers, "converted");

    Ok(pdf_response(converted, &filename))
}

// ---------------------------------------------------------------------------
// /forms/pdfengines/bookmarks/read
// ---------------------------------------------------------------------------

/// `POST /forms/pdfengines/bookmarks/read`.
/// Reads bookmarks from a PDF and returns them as JSON.
pub async fn pdfengines_bookmarks_read(
    State(state): State<AppState>,
    headers: HeaderMap,
    mp: Multipart,
) -> ApiResult<Response> {
    let _permit = acquire_permit(&state).await?;
    let form = FormFields::from_multipart(mp).await?;
    let files = form.files_by_field("files");
    if files.len() != 1 {
        return Err(ApiError::InvalidField {
            field: "files",
            message: "bookmarks read expects exactly one file".to_string(),
        });
    }
    let bytes = read_one(files[0]).await?;

    let bookmarks = tokio::task::spawn_blocking(move || read_bookmarks(&bytes))
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    let filename = files[0].filename.as_str();
    let result = serde_json::json!({ filename: bookmarks });

    let body = serde_json::to_string(&result).map_err(|e| ApiError::Internal(e.to_string()))?;

    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/json")
        .body(axum::body::Body::from(body))
        .unwrap())
}

// ---------------------------------------------------------------------------
// /forms/pdfengines/bookmarks/write
// ---------------------------------------------------------------------------

/// `POST /forms/pdfengines/bookmarks/write`.
/// Writes bookmarks to a PDF.
pub async fn pdfengines_bookmarks_write(
    State(state): State<AppState>,
    headers: HeaderMap,
    mp: Multipart,
) -> ApiResult<Response> {
    let _permit = acquire_permit(&state).await?;
    let form = FormFields::from_multipart(mp).await?;
    let files = form.files_by_field("files");
    if files.len() != 1 {
        return Err(ApiError::InvalidField {
            field: "files",
            message: "bookmarks write expects exactly one file".to_string(),
        });
    }
    let bytes = read_one(files[0]).await?;

    // Parse bookmarks JSON
    let bookmarks_json = form.map.get("bookmarks").ok_or(ApiError::MissingField("bookmarks"))?;
    let bookmarks: Vec<Bookmark> = serde_json::from_str(bookmarks_json).map_err(|e| ApiError::InvalidField {
        field: "bookmarks",
        message: format!("Invalid bookmarks JSON: {}", e),
    })?;

    let output = tokio::task::spawn_blocking(move || write_bookmarks(&bytes, &bookmarks))
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    let filename = output_filename(&headers, "document");

    Ok(pdf_response(output, &filename))
}

// ---------------------------------------------------------------------------
// /forms/pdfengines/watermark and /stamp
// ---------------------------------------------------------------------------

/// `POST /forms/pdfengines/watermark` - Apply watermark (behind content).
pub async fn pdfengines_watermark(
    State(state): State<AppState>,
    headers: HeaderMap,
    mp: Multipart,
) -> ApiResult<Response> {
    let _permit = acquire_permit(&state).await?;
    let form = FormFields::from_multipart(mp).await?;
    let files = form.files_by_field("files");
    if files.len() != 1 {
        return Err(ApiError::InvalidField {
            field: "files",
            message: "watermark expects exactly one file".to_string(),
        });
    }
    let pdf_bytes = read_one(files[0]).await?;

    // Parse watermark text
    let text = form.map.get("watermark")
        .or_else(|| form.map.get("text"))
        .ok_or(ApiError::MissingField("watermark"))?;

    let opts = parse_watermark_options(&form.map, WatermarkKind::Text {
        text: text.clone(),
        font: None, // Use default Helvetica
        font_size: 48.0,
        color: [0.5, 0.5, 0.5, 0.5],
    })?;

    let output = tokio::task::spawn_blocking(move || watermark(&pdf_bytes, &opts))
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    let filename = output_filename(&headers, "result");

    Ok(pdf_response(output, &filename))
}

/// `POST /forms/pdfengines/stamp` - Apply stamp (in front of content).
pub async fn pdfengines_stamp(
    State(state): State<AppState>,
    headers: HeaderMap,
    mp: Multipart,
) -> ApiResult<Response> {
    let _permit = acquire_permit(&state).await?;
    let form = FormFields::from_multipart(mp).await?;
    let files = form.files_by_field("files");
    if files.len() != 1 {
        return Err(ApiError::InvalidField {
            field: "files",
            message: "stamp expects exactly one file".to_string(),
        });
    }
    let pdf_bytes = read_one(files[0]).await?;

    // Parse stamp text
    let text = form.map.get("stamp")
        .or_else(|| form.map.get("text"))
        .ok_or(ApiError::MissingField("stamp"))?;

    let opts = parse_watermark_options(&form.map, WatermarkKind::Text {
        text: text.clone(),
        font: None, // Use default Helvetica
        font_size: 48.0,
        color: [0.5, 0.5, 0.5, 0.5],
    })?;

    let output = tokio::task::spawn_blocking(move || watermark(&pdf_bytes, &opts))
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    let filename = output_filename(&headers, "result");

    Ok(pdf_response(output, &filename))
}

fn parse_watermark_options(
    form: &HashMap<String, String>,
    kind: WatermarkKind,
) -> Result<WatermarkOptions, ApiError> {
    let opacity = form.get("opacity")
        .and_then(|s| s.parse::<f32>().ok())
        .unwrap_or(0.5);

    let rotation = form.get("rotation")
        .and_then(|s| s.parse::<f32>().ok())
        .unwrap_or(0.0);

    let position = form.get("position")
        .and_then(|s| match s.as_str() {
            "center" => Some(WatermarkPosition::Center),
            "top-left" => Some(WatermarkPosition::TopLeft),
            "top-center" => Some(WatermarkPosition::TopCenter),
            "top-right" => Some(WatermarkPosition::TopRight),
            "middle-left" => Some(WatermarkPosition::MiddleLeft),
            "middle-right" => Some(WatermarkPosition::MiddleRight),
            "bottom-left" => Some(WatermarkPosition::BottomLeft),
            "bottom-center" => Some(WatermarkPosition::BottomCenter),
            "bottom-right" => Some(WatermarkPosition::BottomRight),
            _ => None,
        })
        .unwrap_or(WatermarkPosition::Center);

    let all_pages = form.get("pages")
        .map(|s| s == "all")
        .unwrap_or(true);

    let tiled = form.get("tiled")
        .map(|s| s == "true")
        .unwrap_or(false);

    Ok(WatermarkOptions {
        kind,
        opacity,
        rotation_deg: rotation,
        position,
        all_pages,
        tiled,
    })
}

// ---------------------------------------------------------------------------
// /forms/pdfengines/encrypt and /decrypt
// ---------------------------------------------------------------------------

/// `POST /forms/pdfengines/encrypt` - Encrypt PDF with password.
pub async fn pdfengines_encrypt(
    State(state): State<AppState>,
    headers: HeaderMap,
    mp: Multipart,
) -> ApiResult<Response> {
    let _permit = acquire_permit(&state).await?;
    let form = FormFields::from_multipart(mp).await?;
    let files = form.files_by_field("files");
    if files.len() != 1 {
        return Err(ApiError::InvalidField {
            field: "files",
            message: "encrypt expects exactly one file".to_string(),
        });
    }
    let pdf_bytes = read_one(files[0]).await?;

    // Get passwords
    let user_password = form.map.get("userPassword").map(|s| s.as_str());
    let owner_password = form.map.get("ownerPassword").map(|s| s.as_str());

    // Parse algorithm
    let algorithm = match form.map.get("algorithm").map(|s| s.as_str()) {
        Some("aes128") => EncryptionAlgorithm::Aes128,
        _ => EncryptionAlgorithm::Aes256,
    };

    // Parse permissions
    let permissions = form.map.get("permissions")
        .map(|s| Permissions::from_string(s))
        .unwrap_or_else(Permissions::allow_all);

    let output = encrypt_pdf(&pdf_bytes, user_password, owner_password, algorithm, permissions)
        .await
        .map_err(ApiError::from)?;

    let filename = output_filename(&headers, "encrypted");

    Ok(pdf_response(output, &filename))
}

/// `POST /forms/pdfengines/decrypt` - Remove encryption from PDF.
pub async fn pdfengines_decrypt(
    State(state): State<AppState>,
    headers: HeaderMap,
    mp: Multipart,
) -> ApiResult<Response> {
    let _permit = acquire_permit(&state).await?;
    let form = FormFields::from_multipart(mp).await?;
    let files = form.files_by_field("files");
    if files.len() != 1 {
        return Err(ApiError::InvalidField {
            field: "files",
            message: "decrypt expects exactly one file".to_string(),
        });
    }
    let pdf_bytes = read_one(files[0]).await?;

    // Get password
    let password = form.map.get("password").ok_or(ApiError::MissingField("password"))?;

    let output = decrypt_pdf(&pdf_bytes, password)
        .await
        .map_err(ApiError::from)?;

    let filename = output_filename(&headers, "decrypted");

    Ok(pdf_response(output, &filename))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fm(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect()
    }

    #[test]
    fn split_mode_intervals_parses() {
        let m = parse_split_mode(&fm(&[("splitMode", "intervals"), ("splitSpan", "3")])).unwrap();
        match m {
            SplitMode::EveryN(n) => assert_eq!(n, 3),
            _ => panic!("expected EveryN"),
        }
    }

    #[test]
    fn split_mode_pages_parses_multiple_chunks() {
        let m =
            parse_split_mode(&fm(&[("splitMode", "pages"), ("splitSpan", "1-2,5-7,9")])).unwrap();
        match m {
            SplitMode::ByRanges(rs) => assert_eq!(rs.len(), 3),
            _ => panic!("expected ByRanges"),
        }
    }

    #[test]
    fn split_mode_accepts_legacy_mode_field() {
        let m = parse_split_mode(&fm(&[("mode", "intervals"), ("splitSpan", "1")])).unwrap();
        match m {
            SplitMode::EveryN(n) => assert_eq!(n, 1),
            _ => panic!("expected EveryN"),
        }
    }

    #[test]
    fn split_mode_unknown_rejected() {
        assert!(parse_split_mode(&fm(&[("splitMode", "junk"), ("splitSpan", "1")])).is_err());
    }

    #[test]
    fn split_mode_intervals_missing_span_rejected() {
        let err = parse_split_mode(&fm(&[("splitMode", "intervals")])).unwrap_err();
        match err {
            ApiError::MissingField("splitSpan") => {}
            other => panic!("expected MissingField, got {other:?}"),
        }
    }

    // -----------------------------------------------------------------------
    // output_filename helper
    // -----------------------------------------------------------------------

    #[test]
    fn output_filename_reads_from_header() {
        let mut headers = HeaderMap::new();
        headers.insert("Gotenberg-Output-Filename", HeaderValue::from_static("report"));
        assert_eq!(output_filename(&headers, "default"), "report.pdf");
    }

    #[test]
    fn output_filename_falls_back_to_default() {
        let headers = HeaderMap::new();
        assert_eq!(output_filename(&headers, "default"), "default.pdf");
    }

    #[test]
    fn output_filename_strips_trailing_pdf() {
        let mut headers = HeaderMap::new();
        headers.insert("Gotenberg-Output-Filename", HeaderValue::from_static("report.pdf"));
        assert_eq!(output_filename(&headers, "default"), "report.pdf");
    }

    // -----------------------------------------------------------------------
    // Encryption algorithm parsing
    // -----------------------------------------------------------------------

    #[test]
    fn encryption_algorithm_from_form_field() {
        let m = fm(&[("algorithm", "128")]);
        let algo = m.get("algorithm").unwrap();
        assert_eq!(algo.as_str(), "128");
    }

    // -----------------------------------------------------------------------
    // Permissions parsing
    // -----------------------------------------------------------------------

    #[test]
    fn permissions_all_grants_everything() {
        let p = Permissions::from_string("all");
        assert!(p.print);
        assert!(p.modify_content);
        assert!(p.annotate);
    }

    #[test]
    fn permissions_view_only_alias() {
        let p = Permissions::from_string("view-only");
        assert!(!p.print);
        assert!(!p.annotate);
        assert!(!p.extract_content);
    }
}
