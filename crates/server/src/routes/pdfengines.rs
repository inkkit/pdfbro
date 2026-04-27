//! `/forms/pdfengines/*` route handlers (merge / split / flatten / metadata).

use std::collections::{BTreeMap, HashMap};

use axum::extract::{Multipart, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use engine::PageRanges;
use engine::pdfops::{self, Metadata, SplitMode};

use crate::error::{ApiError, ApiResult};
use crate::multipart::{FormFields, UploadedFile};
use crate::routes::chromium::{pdf_response, zip_response};
use crate::routes::libreoffice::build_zip;
use crate::state::AppState;

const SPAWN_BLOCKING_THRESHOLD: usize = 1024 * 1024;

// ---------------------------------------------------------------------------
// /forms/pdfengines/merge
// ---------------------------------------------------------------------------

/// `POST /forms/pdfengines/merge`.
pub async fn pdfengines_merge(State(state): State<AppState>, mp: Multipart) -> ApiResult<Response> {
    let _permit = acquire_permit(&state).await?;
    let form = FormFields::from_multipart(mp).await?;
    let files = form.files_by_field("files");
    if files.len() < 2 {
        return Err(ApiError::InvalidField {
            field: "files",
            message: "merge requires at least two files".to_string(),
        });
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

    Ok(pdf_response(merged, "result.pdf"))
}

fn merge_blobs(blobs: &[Vec<u8>]) -> engine::EngineResult<Vec<u8>> {
    let refs: Vec<&[u8]> = blobs.iter().map(|v| v.as_slice()).collect();
    pdfops::merge(&refs)
}

// ---------------------------------------------------------------------------
// /forms/pdfengines/split
// ---------------------------------------------------------------------------

/// `POST /forms/pdfengines/split`.
pub async fn pdfengines_split(State(state): State<AppState>, mp: Multipart) -> ApiResult<Response> {
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

    let mode = parse_split_mode(&form.map)?;
    let unify = parse_bool_field(&form.map, "splitUnify")?.unwrap_or(false);
    let mode_was_pages =
        matches!(mode, SplitMode::ByRanges(_)) && form.map.contains_key("splitPages");

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
        return Ok(pdf_response(only, "result.pdf"));
    }

    if unify && mode_was_pages {
        // Reassemble all chunks into a single PDF (Gotenberg quirk).
        let merged = tokio::task::spawn_blocking(move || merge_blobs(&chunks))
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))??;
        return Ok(pdf_response(merged, "result.pdf"));
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
                .get("splitPages")
                .ok_or(ApiError::MissingField("splitPages"))?;
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
                    field: "splitPages",
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
        return Ok(pdf_response(only, "result.pdf"));
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
        return Ok(pdf_response(only, "result.pdf"));
    }
    let names: Vec<String> = files.iter().map(|f| f.filename.clone()).collect();
    let zip = build_zip(&names, &outputs)?;
    Ok(zip_response(zip, "result.zip"))
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
            parse_split_mode(&fm(&[("splitMode", "pages"), ("splitPages", "1-2,5-7,9")])).unwrap();
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
}
