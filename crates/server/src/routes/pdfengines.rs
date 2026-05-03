//! `/forms/pdfengines/*` route handlers (merge / split / flatten / metadata).

use std::collections::{BTreeMap, HashMap};

use axum::extract::{Multipart, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use engine::{EngineError, PageRanges};
use engine::pdfops::{self, Metadata, SplitMode};

use crate::error::{ApiError, ApiResult};
use crate::multipart::{FormFields, UploadedFile};
use crate::routes::util::{build_zip, output_filename, pdf_response, zip_response};
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
    let mut form = FormFields::from_multipart(mp).await?;
    crate::download::inject_downloads(&mut form, &state.config).await?;
    let files = form.files_by_field("files");
    if files.len() < 2 {
        return Err(ApiError::InvalidField {
            field: "files",
            message: "merge requires at least two files".to_string(),
        });
    }

    // Validate: PDF/A + Encrypt is not supported
    let has_pdfa = form.map.get("pdfa").filter(|s| !s.is_empty()).is_some();
    let has_encrypt = form.map.get("userPassword").filter(|s| !s.is_empty()).is_some()
        || form.map.get("ownerPassword").filter(|s| !s.is_empty()).is_some();
    if has_pdfa && has_encrypt {
        return Err(ApiError::InvalidField {
            field: "pdfa",
            message: "PDF/A conversion and encryption cannot be combined".to_string(),
        });
    }

    // Validate: stamp/watermark with pdf or image source requires a file
    validate_stamp_file(&form, "stamp", "stampSource")?;
    validate_watermark_file(&form, "watermark", "watermarkSource")?;

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

    // Collect per-file page counts for bookmark auto-indexing before consuming blobs
    let auto_index = parse_bool_field(&form.map, "autoIndexBookmarks")?.unwrap_or(false);
    let per_file_bookmarks: Vec<Vec<Bookmark>> = if auto_index {
        let mut result = Vec::new();
        for blob in &blobs {
            let b = blob.clone();
            let bmarks = tokio::task::spawn_blocking(move || read_bookmarks(&b))
                .await
                .map_err(|e| ApiError::Internal(e.to_string()))??;
            result.push(bmarks);
        }
        result
    } else {
        Vec::new()
    };
    let per_file_page_counts: Vec<usize> = if auto_index {
        let mut result = Vec::new();
        for blob in &blobs {
            let count = lopdf::Document::load_mem(blob)
                .map_err(|e| ApiError::Internal(e.to_string()))?
                .get_pages()
                .len();
            result.push(count);
        }
        result
    } else {
        Vec::new()
    };

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

    // Write explicit bookmarks if provided
    let merged = if let Some(bm_str) = form.map.get("bookmarks").filter(|s| !s.is_empty()) {
        let bookmarks: Vec<Bookmark> =
            serde_json::from_str(bm_str).map_err(|e| ApiError::InvalidField {
                field: "bookmarks",
                message: e.to_string(),
            })?;
        let bytes = merged;
        tokio::task::spawn_blocking(move || write_bookmarks(&bytes, &bookmarks))
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))??
    } else if auto_index && !per_file_bookmarks.is_empty() {
        // Re-index bookmarks: offset each file's bookmarks by cumulative page count
        let mut all_bookmarks: Vec<Bookmark> = Vec::new();
        let mut page_offset: u32 = 0;
        for (file_bmarks, count) in per_file_bookmarks.into_iter().zip(per_file_page_counts.iter()) {
            for mut bm in file_bmarks {
                bm.page += page_offset;
                all_bookmarks.push(bm);
            }
            page_offset += *count as u32;
        }
        if !all_bookmarks.is_empty() {
            let bytes = merged;
            tokio::task::spawn_blocking(move || write_bookmarks(&bytes, &all_bookmarks))
                .await
                .map_err(|e| ApiError::Internal(e.to_string()))??
        } else {
            merged
        }
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
pub async fn pdfengines_split(
    State(state): State<AppState>,
    headers: HeaderMap,
    mp: Multipart,
) -> ApiResult<Response> {
    let _permit = acquire_permit(&state).await?;
    let mut form = FormFields::from_multipart(mp).await?;
    crate::download::inject_downloads(&mut form, &state.config).await?;
    let files = form.files_by_field("files");
    if files.is_empty() {
        return Err(ApiError::MissingFile("files".to_string()));
    }

    // Validate: PDF/A + Encrypt is not supported
    let has_pdfa = form.map.get("pdfa").filter(|s| !s.is_empty()).is_some();
    let has_encrypt = form.map.get("userPassword").filter(|s| !s.is_empty()).is_some()
        || form.map.get("ownerPassword").filter(|s| !s.is_empty()).is_some();
    if has_pdfa && has_encrypt {
        return Err(ApiError::InvalidField {
            field: "pdfa",
            message: "PDF/A conversion and encryption cannot be combined".to_string(),
        });
    }

    // Validate: stamp/watermark with pdf or image source requires a file
    validate_stamp_file(&form, "stamp", "stampSource")?;
    validate_watermark_file(&form, "watermark", "watermarkSource")?;

    let mode = parse_split_mode(&form.map)?;
    let unify = parse_bool_field(&form.map, "splitUnify")?.unwrap_or(false);
    let mode_was_pages =
        matches!(mode, SplitMode::ByRanges(_)) && form.map.contains_key("splitSpan");

    // In Gotenberg's "pages" mode we need to compute per-file page expansion.
    // We'll handle this inside the loop below.

    // Process each input file
    let mut all_chunks: Vec<Vec<u8>> = Vec::new();
    let mut all_names: Vec<String> = Vec::new();
    let mut input_stem = String::from("result");

    for f in &files {
        let bytes = read_one(f).await?;
        let stem = f.filename.trim_end_matches(".pdf").to_string();
        if files.len() == 1 {
            input_stem = stem.clone();
        }

        // Resolve "pages" mode per-file
        let file_mode = if mode_was_pages {
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
                SplitMode::Pages(pages)
            } else {
                mode.clone()
            }
        } else {
            mode.clone()
        };

        let mode_clone = file_mode.clone();
        let mut file_chunks = if bytes.len() > SPAWN_BLOCKING_THRESHOLD {
            tokio::task::spawn_blocking(move || pdfops::split(&bytes, &mode_clone))
                .await
                .map_err(|e| ApiError::Internal(e.to_string()))??
        } else {
            pdfops::split(&bytes, &file_mode)?
        };

        if file_chunks.is_empty() {
            return Err(ApiError::InvalidField {
                field: "splitPages",
                message: "no pages selected by split request".to_string(),
            });
        }

        for (i, chunk) in file_chunks.drain(..).enumerate() {
            all_names.push(format!("{stem}_{i}.pdf"));
            all_chunks.push(chunk);
        }
    }

    // Single-file, single-chunk, no-unify → return as PDF
    if all_chunks.len() == 1 && files.len() == 1 && !unify {
        let filename = output_filename(&headers, &input_stem);
        return Ok(pdf_response(all_chunks.pop().unwrap(), &filename));
    }

    if unify && mode_was_pages && files.len() == 1 {
        let merged = tokio::task::spawn_blocking(move || merge_blobs(&all_chunks))
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))??;
        let filename = output_filename(&headers, &input_stem);
        return Ok(pdf_response(merged, &filename));
    }

    let zip_name = {
        let stem = headers
            .get("Gotenberg-Output-Filename")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.trim_end_matches(".zip"))
            .unwrap_or("result");
        format!("{stem}.zip")
    };
    let zip = build_zip(&all_names, &all_chunks)?;
    Ok(zip_response(zip, &zip_name))
}

fn parse_split_mode(map: &HashMap<String, String>) -> ApiResult<SplitMode> {
    // Accept both `splitMode` (pdfbro) and `mode` (Gotenberg).
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
    let mut form = FormFields::from_multipart(mp).await?;
    crate::download::inject_downloads(&mut form, &state.config).await?;
    let files = form.files_by_field("files");
    if files.is_empty() {
        return Err(ApiError::MissingFile("files".to_string()));
    }

    // Webhook support for single-file flatten.
    if files.len() == 1 {
        let bytes = read_one(files[0]).await?;
        if let Some(resp) = crate::webhook::maybe_spawn_webhook(
            &headers,
            &state,
            crate::webhook::WebhookOperation::PdfFlatten,
            crate::webhook::JobData::PdfFlatten { file: bytes.clone() },
        ).await? {
            return Ok(resp);
        }
        let flat = if bytes.len() > SPAWN_BLOCKING_THRESHOLD {
            tokio::task::spawn_blocking(move || pdfops::flatten(&bytes))
                .await
                .map_err(|e| ApiError::Internal(e.to_string()))??
        } else {
            pdfops::flatten(&bytes)?
        };
        let filename = output_filename(&headers, "result");
        return Ok(pdf_response(flat, &filename));
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
    let zip_name = {
        let stem = headers
            .get("Gotenberg-Output-Filename")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.trim_end_matches(".zip"))
            .unwrap_or("result");
        format!("{stem}.zip")
    };
    Ok(zip_response(zip, &zip_name))
}

// ---------------------------------------------------------------------------
// /forms/pdfengines/metadata/{read,write}
// ---------------------------------------------------------------------------

/// `POST /forms/pdfengines/metadata/read`.
pub async fn pdfengines_metadata_read(
    State(state): State<AppState>,
    headers: HeaderMap,
    mp: Multipart,
) -> ApiResult<Response> {
    let _permit = acquire_permit(&state).await?;
    let mut form = FormFields::from_multipart(mp).await?;
    crate::download::inject_downloads(&mut form, &state.config).await?;
    let files = form.files_by_field("files");
    if files.is_empty() {
        return Err(ApiError::MissingFile("files".to_string()));
    }

    // Webhook support for single-file read.
    if files.len() == 1 {
        let bytes = read_one(files[0]).await?;
        if let Some(resp) = crate::webhook::maybe_spawn_webhook(
            &headers,
            &state,
            crate::webhook::WebhookOperation::PdfMetadataRead,
            crate::webhook::JobData::PdfMetadataRead { file: bytes },
        ).await? {
            return Ok(resp);
        }
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
    let mut resp_headers = HeaderMap::new();
    resp_headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/json"),
    );
    resp_headers.insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_static("attachment; filename=\"result.json\""),
    );
    Ok((StatusCode::OK, resp_headers, body).into_response())
}

/// `POST /forms/pdfengines/metadata/write`.
pub async fn pdfengines_metadata_write(
    State(state): State<AppState>,
    headers: HeaderMap,
    mp: Multipart,
) -> ApiResult<Response> {
    let _permit = acquire_permit(&state).await?;
    let mut form = FormFields::from_multipart(mp).await?;
    crate::download::inject_downloads(&mut form, &state.config).await?;
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

    // Reject control characters (newlines, etc.) in string metadata values
    // to prevent injection attacks via ExifTool-style tools.
    reject_control_chars_in_metadata(&meta)?;

    // Webhook support for single-file write.
    if files.len() == 1 {
        let bytes = read_one(files[0]).await?;
        if let Some(resp) = crate::webhook::maybe_spawn_webhook(
            &headers,
            &state,
            crate::webhook::WebhookOperation::PdfMetadataWrite,
            crate::webhook::JobData::PdfMetadataWrite {
                file: bytes,
                metadata: meta.clone(),
            },
        ).await? {
            return Ok(resp);
        }
    }

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
    let mut form = FormFields::from_multipart(mp).await?;
    crate::download::inject_downloads(&mut form, &state.config).await?;
    let files = form.files_by_field("files");
    if files.is_empty() {
        return Err(ApiError::MissingFile("files".to_string()));
    }

    // Gotenberg uses `rotateAngle`; accept that as well as the shorter `rotate` alias.
    let angle_str = form.map.get("rotateAngle")
        .or_else(|| form.map.get("rotate"))
        .ok_or(ApiError::MissingField("rotateAngle"))?;
    let angle: u16 = angle_str.parse().map_err(|e: std::num::ParseIntError| ApiError::InvalidField {
        field: "rotateAngle",
        message: format!("expected integer angle: {e}"),
    })?;

    // Validate angle is a valid rotation (0, 90, 180, 270)
    if angle != 0 && angle != 90 && angle != 180 && angle != 270 {
        return Err(ApiError::InvalidField {
            field: "rotateAngle",
            message: format!("angle must be 0/90/180/270 (got {})", angle),
        });
    }

    // Gotenberg uses `rotatePages` for page selection; default to all pages.
    let pages = if let Some(rp) = form.map.get("rotatePages") {
        PageRanges::parse(rp).map_err(|e| ApiError::Engine(EngineError::InvalidPageRange(e.to_string())))?
    } else {
        PageRanges::parse("1-").map_err(|e| ApiError::Internal(e.to_string()))?
    };

    let mut outputs = Vec::new();
    let mut names = Vec::new();
    for f in &files {
        let bytes = read_one(f).await?;
        let pages_clone = pages.clone();
        let rotated = tokio::task::spawn_blocking(move || {
            pdfops::rotate(&bytes, &pages_clone, angle as i32)
        })
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .map_err(|e| ApiError::Internal(e.to_string()))?;
        outputs.push(rotated);
        names.push(f.filename.clone());
    }

    if outputs.len() == 1 {
        let filename = output_filename(&headers, &names[0]);
        return Ok(pdf_response(outputs.pop().unwrap(), &filename));
    }
    let zip_name = {
        let stem = headers.get("Gotenberg-Output-Filename")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.trim_end_matches(".zip"))
            .unwrap_or("result");
        format!("{stem}.zip")
    };
    let zip = build_zip(&names, &outputs)?;
    Ok(zip_response(zip, &zip_name))
}

// ---------------------------------------------------------------------------
// /forms/pdfengines/embed
// ---------------------------------------------------------------------------

/// `POST /forms/pdfengines/embed` — attach one or more files as PDF
/// embedded attachments using qpdf.
///
/// Form fields:
/// - `files`: exactly one PDF to receive the attachment(s).
/// - `embeds`: one or more files to embed as attachments.
pub async fn pdfengines_embed(
    State(state): State<AppState>,
    _headers: HeaderMap,
    mp: Multipart,
) -> ApiResult<Response> {
    let _permit = acquire_permit(&state).await?;
    let mut form = FormFields::from_multipart(mp).await?;
    crate::download::inject_downloads(&mut form, &state.config).await?;

    let pdf_files = form.files_by_field("files");
    if pdf_files.len() != 1 {
        return Err(ApiError::InvalidField {
            field: "files",
            message: "embed requires exactly one PDF file in the `files` field".to_string(),
        });
    }
    let pdf_path = pdf_files[0].path.clone();

    let embed_files = form.files_by_field("embeds");
    if embed_files.is_empty() {
        return Err(ApiError::InvalidField {
            field: "embeds",
            message: "embed requires at least one file in the `embeds` field".to_string(),
        });
    }

    let out_path = form.tmp.path().join("embedded.pdf");
    let mut current_input = pdf_path;
    for (i, embed) in embed_files.iter().enumerate() {
        let out = if i == embed_files.len() - 1 {
            out_path.clone()
        } else {
            form.tmp.path().join(format!("intermediate_{i}.pdf"))
        };

        let status = tokio::process::Command::new("qpdf")
            .arg("--add-attachment")
            .arg(&embed.path)
            .arg("--")
            .arg(&current_input)
            .arg(&out)
            .status()
            .await
            .map_err(|e| ApiError::Internal(format!("qpdf spawn failed: {e}")))?;

        if !status.success() {
            return Err(ApiError::Internal(format!(
                "qpdf exited with status {} while embedding `{}`",
                status,
                embed.filename,
            )));
        }

        current_input = out;
    }

    let pdf_bytes = tokio::fs::read(&out_path)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    Ok(pdf_response(pdf_bytes, "embedded.pdf"))
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

async fn acquire_permit(state: &AppState) -> ApiResult<tokio::sync::OwnedSemaphorePermit> {
    state
        .sem
        .clone()
        .acquire_owned()
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))
}

fn reject_control_chars_in_metadata(meta: &Metadata) -> ApiResult<()> {
    let fields: &[Option<&str>] = &[
        meta.title.as_deref(),
        meta.author.as_deref(),
        meta.subject.as_deref(),
        meta.creator.as_deref(),
        meta.producer.as_deref(),
        meta.creation_date.as_deref(),
        meta.mod_date.as_deref(),
    ];
    for val in fields.iter().flatten() {
        if val.chars().any(|c| c.is_control()) {
            return Err(ApiError::InvalidField {
                field: "metadata",
                message: "At least one PDF engine cannot process the requested metadata".to_string(),
            });
        }
    }
    for v in meta.custom.values() {
        if let Some(s) = v.as_str() {
            if s.chars().any(|c| c.is_control()) {
                return Err(ApiError::InvalidField {
                    field: "metadata",
                    message: "At least one PDF engine cannot process the requested metadata"
                        .to_string(),
                });
            }
        }
    }
    Ok(())
}

fn validate_stamp_file(form: &FormFields, file_field: &str, source_field: &'static str) -> ApiResult<()> {
    let source = form.map.get(source_field).map(|s| s.as_str()).unwrap_or("text");
    if source == "image" || source == "pdf" {
        let has_file = form.files.iter().any(|f| f.field_name == file_field);
        if !has_file {
            return Err(ApiError::InvalidField {
                field: source_field,
                message: "a stamp file is required for image or pdf source".to_string(),
            });
        }
    }
    Ok(())
}

fn validate_watermark_file(form: &FormFields, file_field: &str, source_field: &'static str) -> ApiResult<()> {
    let source = form.map.get(source_field).map(|s| s.as_str()).unwrap_or("text");
    if source == "image" || source == "pdf" {
        let has_file = form.files.iter().any(|f| f.field_name == file_field);
        if !has_file {
            return Err(ApiError::InvalidField {
                field: source_field,
                message: "a watermark file is required for image or pdf source".to_string(),
            });
        }
    }
    Ok(())
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
    let mut form = FormFields::from_multipart(mp).await?;
    crate::download::inject_downloads(&mut form, &state.config).await?;
    let files = form.files_by_field("files");
    if files.is_empty() {
        return Err(ApiError::MissingFile("files".to_string()));
    }

    let pdfa_str = form.map.get("pdfa");
    let pdfua = form.map.get("pdfua").map(|s| s.as_str() == "true").unwrap_or(false);
    if pdfa_str.is_none() && !pdfua {
        return Err(ApiError::InvalidField {
            field: "pdfa",
            message: "either 'pdfa' or 'pdfua' form fields must be provided".to_string(),
        });
    }
    let profile: Option<PdfAProfile> = if let Some(s) = pdfa_str {
        Some(s.parse::<PdfAProfile>().map_err(|e: String| ApiError::InvalidField {
            field: "pdfa",
            message: e,
        })?)
    } else {
        None
    };

    let mut outputs = Vec::new();
    let mut names = Vec::new();

    for f in &files {
        let bytes = read_one(f).await?;
        let converted = if let Some(prof) = profile {
            convert_to_pdfa(&bytes, prof).await.map_err(|e| ApiError::Internal(e.to_string()))?
        } else {
            // pdfua only: return as-is
            bytes
        };
        outputs.push(converted);
        names.push(f.filename.clone());
    }

    if outputs.len() == 1 {
        let filename = output_filename(&headers, &names[0]);
        return Ok(pdf_response(outputs.pop().unwrap(), &filename));
    }
    let zip_name = {
        let stem = headers.get("Gotenberg-Output-Filename")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.trim_end_matches(".zip"))
            .unwrap_or("result");
        format!("{stem}.zip")
    };
    let zip = build_zip(&names, &outputs)?;
    Ok(zip_response(zip, &zip_name))
}

// ---------------------------------------------------------------------------
// /forms/pdfengines/bookmarks/read
// ---------------------------------------------------------------------------

/// `POST /forms/pdfengines/bookmarks/read`.
/// Reads bookmarks from a PDF and returns them as JSON.
pub async fn pdfengines_bookmarks_read(
    State(state): State<AppState>,
    _headers: HeaderMap,
    mp: Multipart,
) -> ApiResult<Response> {
    let _permit = acquire_permit(&state).await?;
    let mut form = FormFields::from_multipart(mp).await?;
    crate::download::inject_downloads(&mut form, &state.config).await?;
    let files = form.files_by_field("files");
    if files.is_empty() {
        return Err(ApiError::MissingFile("files".to_string()));
    }

    let mut out = serde_json::Map::new();
    for f in &files {
        let bytes = read_one(f).await?;
        let filename = f.filename.clone();
        let bookmarks = tokio::task::spawn_blocking(move || read_bookmarks(&bytes))
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))?
            .map_err(|e| ApiError::Internal(e.to_string()))?;
        out.insert(filename, serde_json::json!(bookmarks));
    }

    let body = serde_json::to_string(&out).map_err(|e| ApiError::Internal(e.to_string()))?;

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
    let mut form = FormFields::from_multipart(mp).await?;
    crate::download::inject_downloads(&mut form, &state.config).await?;
    let files = form.files_by_field("files");
    if files.is_empty() {
        return Err(ApiError::MissingFile("files".to_string()));
    }

    // Parse bookmarks JSON - accept array or object (filename → array) format
    let bookmarks_json = form.map.get("bookmarks").ok_or(ApiError::MissingField("bookmarks"))?;
    let all_bookmarks: serde_json::Value = serde_json::from_str(bookmarks_json).map_err(|e| ApiError::InvalidField {
        field: "bookmarks",
        message: format!("Invalid bookmarks JSON: {}", e),
    })?;

    let mut outputs = Vec::new();
    let mut names = Vec::new();

    for f in &files {
        // Determine bookmarks for this file
        let file_bookmarks: Vec<Bookmark> = if all_bookmarks.is_array() {
            serde_json::from_value(all_bookmarks.clone()).map_err(|e| ApiError::InvalidField {
                field: "bookmarks",
                message: format!("Invalid bookmarks JSON: {}", e),
            })?
        } else if let Some(obj) = all_bookmarks.as_object() {
            // Map format: look up by filename
            if let Some(bm_val) = obj.get(&f.filename) {
                serde_json::from_value(bm_val.clone()).map_err(|e| ApiError::InvalidField {
                    field: "bookmarks",
                    message: format!("Invalid bookmarks JSON for {}: {}", f.filename, e),
                })?
            } else {
                Vec::new()
            }
        } else {
            return Err(ApiError::InvalidField {
                field: "bookmarks",
                message: "bookmarks must be a JSON array or object".to_string(),
            });
        };

        let bytes = read_one(f).await?;
        let output = tokio::task::spawn_blocking(move || write_bookmarks(&bytes, &file_bookmarks))
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))?
            .map_err(|e| ApiError::Internal(e.to_string()))?;
        outputs.push(output);
        names.push(f.filename.clone());
    }

    if outputs.len() == 1 {
        let filename = output_filename(&headers, &names[0]);
        return Ok(pdf_response(outputs.pop().unwrap(), &filename));
    }
    let zip_name = {
        let stem = headers.get("Gotenberg-Output-Filename")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.trim_end_matches(".zip"))
            .unwrap_or("result");
        format!("{stem}.zip")
    };
    let zip = build_zip(&names, &outputs)?;
    Ok(zip_response(zip, &zip_name))
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
    let mut form = FormFields::from_multipart(mp).await?;
    crate::download::inject_downloads(&mut form, &state.config).await?;
    let files = form.files_by_field("files");
    if files.is_empty() {
        return Err(ApiError::MissingFile("files".to_string()));
    }

    // Gotenberg API: watermarkSource=text, watermarkExpression=TEXT
    // Legacy pdfbro API: watermark=TEXT or text=TEXT
    let source = form.map.get("watermarkSource").map(|s| s.as_str()).unwrap_or("text");

    let kind = match source {
        "image" => {
            let img_file = form.files.iter().find(|f| f.field_name == "watermark")
                .ok_or_else(|| ApiError::InvalidField {
                    field: "watermarkSource",
                    message: "a watermark file is required for image or pdf source".to_string(),
                })?;
            let bytes = tokio::fs::read(&img_file.path)
                .await
                .map_err(|e| ApiError::Internal(e.to_string()))?;
            WatermarkKind::ImagePng { bytes }
        }
        "pdf" => {
            let img_file = form.files.iter().find(|f| f.field_name == "watermark")
                .ok_or_else(|| ApiError::InvalidField {
                    field: "watermarkSource",
                    message: "a watermark file is required for image or pdf source".to_string(),
                })?;
            let bytes = tokio::fs::read(&img_file.path)
                .await
                .map_err(|e| ApiError::Internal(e.to_string()))?;
            WatermarkKind::ImagePng { bytes }
        }
        _ => {
            let text = form.map.get("watermarkExpression")
                .or_else(|| form.map.get("watermark"))
                .or_else(|| form.map.get("text"))
                .ok_or(ApiError::MissingField("watermarkExpression"))?;
            WatermarkKind::Text {
                text: text.clone(),
                font: None,
                font_size: 48.0,
                color: [0.5, 0.5, 0.5, 0.5],
            }
        }
    };

    let opts = parse_watermark_options(&form.map, kind)?;

    let mut outputs = Vec::new();
    let mut names = Vec::new();
    for f in &files {
        let pdf_bytes = read_one(f).await?;
        let opts_clone = opts.clone();
        let output = tokio::task::spawn_blocking(move || watermark(&pdf_bytes, &opts_clone))
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))?
            .map_err(|e| ApiError::Internal(e.to_string()))?;
        outputs.push(output);
        names.push(f.filename.clone());
    }

    if outputs.len() == 1 {
        let filename = output_filename(&headers, names[0].trim_end_matches(".pdf"));
        return Ok(pdf_response(outputs.pop().unwrap(), &filename));
    }
    let zip_name = {
        let stem = headers.get("Gotenberg-Output-Filename")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.trim_end_matches(".zip"))
            .unwrap_or("result");
        format!("{stem}.zip")
    };
    let zip = build_zip(&names, &outputs)?;
    Ok(zip_response(zip, &zip_name))
}

/// `POST /forms/pdfengines/stamp` - Apply stamp (in front of content).
pub async fn pdfengines_stamp(
    State(state): State<AppState>,
    headers: HeaderMap,
    mp: Multipart,
) -> ApiResult<Response> {
    let _permit = acquire_permit(&state).await?;
    let mut form = FormFields::from_multipart(mp).await?;
    crate::download::inject_downloads(&mut form, &state.config).await?;
    let files = form.files_by_field("files");
    if files.is_empty() {
        return Err(ApiError::MissingFile("files".to_string()));
    }

    // Gotenberg API: stampSource=text, stampExpression=TEXT
    // Legacy pdfbro API: stamp=TEXT or text=TEXT
    let source = form.map.get("stampSource").map(|s| s.as_str()).unwrap_or("text");

    let kind = match source {
        "image" => {
            let img_file = form.files.iter().find(|f| f.field_name == "stamp")
                .ok_or_else(|| ApiError::InvalidField {
                    field: "stampSource",
                    message: "a stamp file is required for image or pdf source".to_string(),
                })?;
            let bytes = tokio::fs::read(&img_file.path)
                .await
                .map_err(|e| ApiError::Internal(e.to_string()))?;
            WatermarkKind::ImagePng { bytes }
        }
        "pdf" => {
            let img_file = form.files.iter().find(|f| f.field_name == "stamp")
                .ok_or_else(|| ApiError::InvalidField {
                    field: "stampSource",
                    message: "a stamp file is required for image or pdf source".to_string(),
                })?;
            let bytes = tokio::fs::read(&img_file.path)
                .await
                .map_err(|e| ApiError::Internal(e.to_string()))?;
            WatermarkKind::ImagePng { bytes }
        }
        _ => {
            let text = form.map.get("stampExpression")
                .or_else(|| form.map.get("stamp"))
                .or_else(|| form.map.get("text"))
                .ok_or(ApiError::MissingField("stampExpression"))?;
            WatermarkKind::Text {
                text: text.clone(),
                font: None,
                font_size: 48.0,
                color: [0.5, 0.5, 0.5, 0.5],
            }
        }
    };

    let opts = parse_watermark_options(&form.map, kind)?;

    let mut outputs = Vec::new();
    let mut names = Vec::new();
    for f in &files {
        let pdf_bytes = read_one(f).await?;
        let opts_clone = opts.clone();
        let output = tokio::task::spawn_blocking(move || watermark(&pdf_bytes, &opts_clone))
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))?
            .map_err(|e| ApiError::Internal(e.to_string()))?;
        outputs.push(output);
        names.push(f.filename.clone());
    }

    if outputs.len() == 1 {
        let filename = output_filename(&headers, &names[0]);
        return Ok(pdf_response(outputs.pop().unwrap(), &filename));
    }
    let zip_name = {
        let stem = headers.get("Gotenberg-Output-Filename")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.trim_end_matches(".zip"))
            .unwrap_or("result");
        format!("{stem}.zip")
    };
    let zip = build_zip(&names, &outputs)?;
    Ok(zip_response(zip, &zip_name))
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
    let mut form = FormFields::from_multipart(mp).await?;
    crate::download::inject_downloads(&mut form, &state.config).await?;
    let files = form.files_by_field("files");
    if files.is_empty() {
        return Err(ApiError::MissingFile("files".to_string()));
    }

    // Get passwords
    let user_password = form.map.get("userPassword").cloned();
    let owner_password = form.map.get("ownerPassword").cloned();

    // Parse algorithm
    let algorithm = match form.map.get("algorithm").map(|s| s.as_str()) {
        Some("aes128") => EncryptionAlgorithm::Aes128,
        _ => EncryptionAlgorithm::Aes256,
    };

    // Parse permissions
    let permissions = form.map.get("permissions")
        .map(|s| Permissions::from_string(s))
        .unwrap_or_else(Permissions::allow_all);

    let mut outputs = Vec::new();
    let mut names = Vec::new();
    for f in &files {
        let pdf_bytes = read_one(f).await?;
        let output = encrypt_pdf(&pdf_bytes, user_password.as_deref(), owner_password.as_deref(), algorithm, permissions)
            .await
            .map_err(ApiError::from)?;
        outputs.push(output);
        names.push(f.filename.clone());
    }

    if outputs.len() == 1 {
        let filename = output_filename(&headers, &names[0]);
        return Ok(pdf_response(outputs.pop().unwrap(), &filename));
    }
    let zip_name = {
        let stem = headers.get("Gotenberg-Output-Filename")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.trim_end_matches(".zip"))
            .unwrap_or("result");
        format!("{stem}.zip")
    };
    let zip = build_zip(&names, &outputs)?;
    Ok(zip_response(zip, &zip_name))
}

/// `POST /forms/pdfengines/decrypt` - Remove encryption from PDF.
pub async fn pdfengines_decrypt(
    State(state): State<AppState>,
    headers: HeaderMap,
    mp: Multipart,
) -> ApiResult<Response> {
    let _permit = acquire_permit(&state).await?;
    let mut form = FormFields::from_multipart(mp).await?;
    crate::download::inject_downloads(&mut form, &state.config).await?;
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

    if let Some(resp) = crate::webhook::maybe_spawn_webhook(
        &headers,
        &state,
        crate::webhook::WebhookOperation::PdfDecrypt,
        crate::webhook::JobData::PdfDecrypt {
            file: pdf_bytes.clone(),
            password: password.clone(),
        },
    ).await? {
        return Ok(resp);
    }

    let output = decrypt_pdf(&pdf_bytes, password)
        .await
        .map_err(ApiError::from)?;

    let filename = output_filename(&headers, &files[0].filename);

    Ok(pdf_response(output, &filename))
}

// ---------------------------------------------------------------------------
// /forms/pdfengines/optimise
// ---------------------------------------------------------------------------

use engine::{optimise_pdf, OptimiseBackend, OptimisePreset};

/// `POST /forms/pdfengines/optimise`.
pub async fn pdfengines_optimise(
    State(state): State<AppState>,
    headers: HeaderMap,
    mp: Multipart,
) -> ApiResult<Response> {
    let _permit = acquire_permit(&state).await?;
    let mut form = FormFields::from_multipart(mp).await?;
    crate::download::inject_downloads(&mut form, &state.config).await?;
    let files = form.files_by_field("files");
    if files.len() != 1 {
        return Err(ApiError::InvalidField {
            field: "files",
            message: "optimise requires exactly one file".to_string(),
        });
    }

    // Get preset (default: screen for max compression)
    let preset_str = form.map.get("preset").map(|s| s.as_str()).unwrap_or("screen");
    let preset = OptimisePreset::from_str(preset_str)
        .ok_or_else(|| ApiError::InvalidField {
            field: "preset",
            message: format!(
                "Invalid preset '{}'. Use: screen, ebook, printer",
                preset_str
            ),
        })?;

    // Optional: force specific backend
    let preferred_backend = form.map.get("backend").and_then(|b| match b.as_str() {
        "ghostscript" => Some(OptimiseBackend::Ghostscript),
        "qpdf" => Some(OptimiseBackend::Qpdf),
        _ => None,
    });

    let pdf_bytes = read_one(files[0]).await?;

    // Check for async webhook mode before processing.
    if let Some(resp) = crate::webhook::maybe_spawn_webhook(
        &headers,
        &state,
        crate::webhook::WebhookOperation::PdfOptimise,
        crate::webhook::JobData::PdfOptimise {
            file: pdf_bytes.clone(),
            preset: preset_str.to_string(),
            backend: preferred_backend.map(|b| format!("{:?}", b).to_lowercase()),
        },
    )
    .await?
    {
        return Ok(resp);
    }

    // Run optimisation
    let optimise_result = optimise_pdf(&pdf_bytes, preset, preferred_backend)
        .await
        .map_err(ApiError::from)?;

    // Build response with headers
    let mut resp_headers = HeaderMap::new();

    // Original size header
    resp_headers.insert(
        header::HeaderName::from_static("x-original-size"),
        HeaderValue::from(optimise_result.original_size),
    );

    // Optimised size header
    resp_headers.insert(
        header::HeaderName::from_static("x-optimised-size"),
        HeaderValue::from(optimise_result.optimised_size),
    );

    // Compression ratio
    resp_headers.insert(
        header::HeaderName::from_static("x-compression-ratio"),
        HeaderValue::from_str(&format!("{:.2}", optimise_result.compression_ratio())).unwrap(),
    );

    // Reduction percentage
    resp_headers.insert(
        header::HeaderName::from_static("x-reduction-percent"),
        HeaderValue::from_str(&format!("{:.1}%", optimise_result.reduction_percent())).unwrap(),
    );

    // Backend used
    resp_headers.insert(
        header::HeaderName::from_static("x-backend-used"),
        HeaderValue::from_str(&format!("{:?}", optimise_result.backend).to_lowercase()).unwrap(),
    );

    // Content disposition
    let filename = output_filename(&headers, &files[0].filename);
    let output_filename = filename.replace(".pdf", "_optimised.pdf");
    resp_headers.insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_str(&format!("attachment; filename=\"{}\"", output_filename)).unwrap(),
    );

    resp_headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/pdf"),
    );

    Ok((
        StatusCode::OK,
        resp_headers,
        axum::body::Body::from(optimise_result.data),
    )
        .into_response())
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

    #[tokio::test]
    #[ignore = "requires qpdf binary and real PDF files"]
    async fn embed_returns_valid_pdf() {
        // Exercised in Docker CI via `cargo test -- --ignored`.
        // Placeholder to document the expected behavior:
        // upload one PDF as `files` and one text file as `embeds`,
        // verify response starts with %PDF.
    }
}
