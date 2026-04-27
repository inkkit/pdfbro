//! `/forms/libreoffice/convert` route handler.

use std::collections::HashMap;
use std::path::PathBuf;

use axum::extract::{Multipart, State};
use axum::response::Response;
use engine::{OfficeOptions, PageRanges, PdfAProfile};

use crate::error::{ApiError, ApiResult};
use crate::multipart::FormFields;
use crate::routes::chromium::{pdf_response, zip_response};
use crate::state::AppState;

/// `POST /forms/libreoffice/convert`.
pub async fn libreoffice_convert(
    State(state): State<AppState>,
    mp: Multipart,
) -> ApiResult<Response> {
    let _permit = state
        .sem
        .clone()
        .acquire_owned()
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    let form = FormFields::from_multipart(mp).await?;
    let lo = state
        .libreoffice
        .as_ref()
        .ok_or_else(|| ApiError::Internal("LibreOffice engine unavailable".to_string()))?;

    let opts = parse_office_options(&form.map)?;
    opts.validate()?;
    let merge = parse_merge_flag(&form.map)?;

    let inputs: Vec<PathBuf> = form
        .files_by_field("files")
        .iter()
        .map(|f| f.path.clone())
        .collect();
    if inputs.is_empty() {
        return Err(ApiError::MissingFile("files".to_string()));
    }

    let mut outputs = lo.convert_many(&inputs, &opts).await?;

    if merge && outputs.len() > 1 {
        let merged =
            tokio::task::spawn_blocking(move || engine::pdfops::merge(&materialise(&outputs)))
                .await
                .map_err(|e| ApiError::Internal(e.to_string()))??;
        return Ok(pdf_response(merged, "result.pdf"));
    }

    if outputs.len() == 1
        && let Some(only) = outputs.pop()
    {
        return Ok(pdf_response(only, "result.pdf"));
    }

    // Multiple outputs: zip with names mirroring the inputs.
    let filenames: Vec<String> = form
        .files_by_field("files")
        .iter()
        .map(|f| pdf_filename_for(&f.filename))
        .collect();
    let zip_bytes = build_zip(&filenames, &outputs)?;
    Ok(zip_response(zip_bytes, "result.zip"))
}

// `convert_many` consumes its outputs into a single `Vec<Vec<u8>>`, but the
// merge call wants `&[&[u8]]`. We materialise references into a fresh vec
// inside the blocking task because the lifetimes of borrows from the moved
// vec cannot be expressed here.
fn materialise(outputs: &[Vec<u8>]) -> Vec<&[u8]> {
    outputs.iter().map(|v| v.as_slice()).collect()
}

fn pdf_filename_for(input_name: &str) -> String {
    match input_name.rsplit_once('.') {
        Some((stem, _)) => format!("{stem}.pdf"),
        None => format!("{input_name}.pdf"),
    }
}

fn parse_merge_flag(map: &HashMap<String, String>) -> ApiResult<bool> {
    match map.get("merge").map(String::as_str) {
        None | Some("") => Ok(false),
        Some(s) => match s.trim().to_ascii_lowercase().as_str() {
            "1" | "true" | "yes" | "on" => Ok(true),
            "0" | "false" | "no" | "off" => Ok(false),
            other => Err(ApiError::InvalidField {
                field: "merge",
                message: format!("expected boolean, got `{other}`"),
            }),
        },
    }
}

/// Build [`OfficeOptions`] from a form map.
pub fn parse_office_options(map: &HashMap<String, String>) -> ApiResult<OfficeOptions> {
    let mut opts = OfficeOptions::default();
    if let Some(s) = nonempty(map, "landscape") {
        opts.landscape = parse_bool(&s, "landscape")?;
    }
    // Gotenberg accepts `pageRanges` and `nativePageRanges`; latter wins.
    let range_raw = nonempty(map, "nativePageRanges").or_else(|| nonempty(map, "pageRanges"));
    if let Some(s) = range_raw {
        opts.page_ranges = Some(PageRanges::parse(&s)?);
    }
    if let Some(s) = nonempty(map, "pdfa") {
        opts.pdf_a = Some(parse_pdf_a(&s)?);
    }
    if let Some(s) = nonempty(map, "pdfua") {
        opts.pdf_ua = parse_bool(&s, "pdfua")?;
    }
    if let Some(s) = nonempty(map, "quality") {
        let v: u8 = s
            .parse()
            .map_err(|e: std::num::ParseIntError| ApiError::InvalidField {
                field: "quality",
                message: e.to_string(),
            })?;
        opts.quality = Some(v);
    }
    if let Some(s) = nonempty(map, "maxImageResolution") {
        let v: u32 = s
            .parse()
            .map_err(|e: std::num::ParseIntError| ApiError::InvalidField {
                field: "maxImageResolution",
                message: e.to_string(),
            })?;
        opts.max_image_resolution = Some(v);
    }
    Ok(opts)
}

fn parse_pdf_a(s: &str) -> ApiResult<PdfAProfile> {
    let normalised: String = s
        .trim()
        .to_ascii_lowercase()
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .collect();
    match normalised.as_str() {
        "a1b" | "pdfa1b" => Ok(PdfAProfile::A1B),
        "a2b" | "pdfa2b" => Ok(PdfAProfile::A2B),
        "a3b" | "pdfa3b" => Ok(PdfAProfile::A3B),
        other => Err(ApiError::InvalidField {
            field: "pdfa",
            message: format!("expected one of A-1b/A-2b/A-3b, got `{other}`"),
        }),
    }
}

fn parse_bool(s: &str, field: &'static str) -> ApiResult<bool> {
    match s.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Ok(true),
        "0" | "false" | "no" | "off" => Ok(false),
        other => Err(ApiError::InvalidField {
            field,
            message: format!("expected boolean, got `{other}`"),
        }),
    }
}

fn nonempty(map: &HashMap<String, String>, key: &str) -> Option<String> {
    map.get(key).filter(|s| !s.is_empty()).cloned()
}

// ---------------------------------------------------------------------------
// Zip building
// ---------------------------------------------------------------------------

/// Build a minimal stored (no-compression) ZIP archive containing the
/// given PDF byte buffers.
///
/// We assemble the archive by hand to avoid pulling in the `zip` crate
/// for what is a tiny single-shot use case. Compression-method 0 (stored)
/// is universally supported.
pub(crate) fn build_zip(filenames: &[String], blobs: &[Vec<u8>]) -> ApiResult<Vec<u8>> {
    use std::io::Write;

    let mut out = Vec::with_capacity(blobs.iter().map(|b| b.len()).sum::<usize>() + 4096);
    let mut central: Vec<u8> = Vec::new();
    let mut offset: u32 = 0;

    for (name, data) in filenames.iter().zip(blobs.iter()) {
        let crc = crc32(data);
        let name_bytes = name.as_bytes();
        let local_header_offset = offset;

        // Local file header.
        out.write_all(&[0x50, 0x4b, 0x03, 0x04]).ok();
        out.write_all(&20u16.to_le_bytes()).ok(); // version needed
        out.write_all(&0u16.to_le_bytes()).ok(); // flags
        out.write_all(&0u16.to_le_bytes()).ok(); // method = stored
        out.write_all(&0u16.to_le_bytes()).ok(); // last mod time
        out.write_all(&0u16.to_le_bytes()).ok(); // last mod date
        out.write_all(&crc.to_le_bytes()).ok();
        out.write_all(&(data.len() as u32).to_le_bytes()).ok();
        out.write_all(&(data.len() as u32).to_le_bytes()).ok();
        out.write_all(&(name_bytes.len() as u16).to_le_bytes()).ok();
        out.write_all(&0u16.to_le_bytes()).ok(); // extra len
        out.write_all(name_bytes).ok();
        out.write_all(data).ok();

        // Central directory entry.
        central.write_all(&[0x50, 0x4b, 0x01, 0x02]).ok();
        central.write_all(&20u16.to_le_bytes()).ok(); // ver made by
        central.write_all(&20u16.to_le_bytes()).ok(); // ver needed
        central.write_all(&0u16.to_le_bytes()).ok(); // flags
        central.write_all(&0u16.to_le_bytes()).ok(); // method
        central.write_all(&0u16.to_le_bytes()).ok(); // mtime
        central.write_all(&0u16.to_le_bytes()).ok(); // mdate
        central.write_all(&crc.to_le_bytes()).ok();
        central.write_all(&(data.len() as u32).to_le_bytes()).ok();
        central.write_all(&(data.len() as u32).to_le_bytes()).ok();
        central
            .write_all(&(name_bytes.len() as u16).to_le_bytes())
            .ok();
        central.write_all(&0u16.to_le_bytes()).ok(); // extra
        central.write_all(&0u16.to_le_bytes()).ok(); // comment
        central.write_all(&0u16.to_le_bytes()).ok(); // disk #
        central.write_all(&0u16.to_le_bytes()).ok(); // internal attr
        central.write_all(&0u32.to_le_bytes()).ok(); // external attr
        central.write_all(&local_header_offset.to_le_bytes()).ok();
        central.write_all(name_bytes).ok();

        // Update offset (cast guards against overflow; a single archive
        // bigger than 4 GiB would need ZIP64, which we don't support).
        let entry_len: u32 = (4 + 26 + name_bytes.len() + data.len()) as u32;
        offset = offset
            .checked_add(entry_len)
            .ok_or_else(|| ApiError::Internal("zip archive too large".to_string()))?;
    }

    let central_offset = offset;
    let central_size = central.len() as u32;
    out.extend_from_slice(&central);

    // End of central directory record.
    out.write_all(&[0x50, 0x4b, 0x05, 0x06]).ok();
    out.write_all(&0u16.to_le_bytes()).ok(); // disk
    out.write_all(&0u16.to_le_bytes()).ok(); // disk start
    out.write_all(&(filenames.len() as u16).to_le_bytes()).ok();
    out.write_all(&(filenames.len() as u16).to_le_bytes()).ok();
    out.write_all(&central_size.to_le_bytes()).ok();
    out.write_all(&central_offset.to_le_bytes()).ok();
    out.write_all(&0u16.to_le_bytes()).ok(); // comment len

    Ok(out)
}

fn crc32(data: &[u8]) -> u32 {
    // Standard ZIP CRC-32 using the IEEE polynomial 0xEDB88320.
    static TABLE: std::sync::OnceLock<[u32; 256]> = std::sync::OnceLock::new();
    let table = TABLE.get_or_init(|| {
        let mut t = [0u32; 256];
        for i in 0..256u32 {
            let mut c = i;
            for _ in 0..8 {
                if c & 1 != 0 {
                    c = 0xEDB8_8320 ^ (c >> 1);
                } else {
                    c >>= 1;
                }
            }
            t[i as usize] = c;
        }
        t
    });
    let mut crc = 0xFFFF_FFFFu32;
    for &b in data {
        crc = table[((crc ^ u32::from(b)) & 0xFF) as usize] ^ (crc >> 8);
    }
    crc ^ 0xFFFF_FFFF
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
    fn merge_flag_parses() {
        assert!(!parse_merge_flag(&fm(&[])).unwrap());
        assert!(parse_merge_flag(&fm(&[("merge", "true")])).unwrap());
        assert!(parse_merge_flag(&fm(&[("merge", "1")])).unwrap());
        assert!(!parse_merge_flag(&fm(&[("merge", "false")])).unwrap());
        assert!(parse_merge_flag(&fm(&[("merge", "maybe")])).is_err());
    }

    #[test]
    fn office_options_parse_basic() {
        let map = fm(&[
            ("landscape", "true"),
            ("pageRanges", "1-3"),
            ("pdfa", "PDF/A-2b"),
            ("pdfua", "true"),
            ("quality", "75"),
            ("maxImageResolution", "300"),
        ]);
        let opts = parse_office_options(&map).unwrap();
        assert!(opts.landscape);
        assert!(opts.page_ranges.is_some());
        assert_eq!(opts.pdf_a, Some(PdfAProfile::A2B));
        assert!(opts.pdf_ua);
        assert_eq!(opts.quality, Some(75));
        assert_eq!(opts.max_image_resolution, Some(300));
    }

    #[test]
    fn native_page_ranges_alias_overrides_page_ranges() {
        let map = fm(&[("pageRanges", "1-3"), ("nativePageRanges", "5-7")]);
        let opts = parse_office_options(&map).unwrap();
        let ranges = opts.page_ranges.unwrap();
        // The string repr should reflect the native override.
        let as_str: String = ranges.into();
        assert_eq!(as_str, "5-7");
    }

    #[test]
    fn zip_builds_with_two_entries() {
        let names = vec!["a.pdf".to_string(), "b.pdf".to_string()];
        let blobs = vec![b"hello".to_vec(), b"world!".to_vec()];
        let zip = build_zip(&names, &blobs).unwrap();
        // Sanity: starts with PK\x03\x04 and ends with PK\x05\x06.
        assert_eq!(&zip[..4], b"PK\x03\x04");
        let eocd = zip.windows(4).rposition(|w| w == b"PK\x05\x06").unwrap();
        assert_eq!(&zip[eocd..eocd + 4], b"PK\x05\x06");
        // The two blobs appear verbatim in the stream (stored, not deflated).
        assert!(zip.windows(5).any(|w| w == b"hello"));
        assert!(zip.windows(6).any(|w| w == b"world!"));
    }

    #[test]
    fn pdf_filename_for_strips_extension() {
        assert_eq!(pdf_filename_for("doc.docx"), "doc.pdf");
        assert_eq!(pdf_filename_for("noext"), "noext.pdf");
        assert_eq!(pdf_filename_for("a.b.c"), "a.b.pdf");
    }
}
