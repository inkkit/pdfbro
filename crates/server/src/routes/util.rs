//! Shared response utilities used by multiple route modules.

use std::collections::HashMap;
use std::io::Write;

use axum::body::Bytes;
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};

use crate::error::{ApiError, ApiResult};

/// Extract `Gotenberg-Output-Filename` from HTTP request headers.
/// Strips any trailing `.pdf` and re-appends it so callers always get a `.pdf` name.
pub fn output_filename(headers: &HeaderMap, default: &str) -> String {
    let raw = headers
        .get("Gotenberg-Output-Filename")
        .and_then(|v| v.to_str().ok());
    match raw {
        Some(s) => format!("{}.pdf", s.trim_end_matches(".pdf")),
        None => format!("{}.pdf", default.trim_end_matches(".pdf")),
    }
}

/// Build a `200 OK` response carrying a single PDF.
pub fn pdf_response(bytes: Vec<u8>, filename: &str) -> Response {
    binary_response(bytes, "application/pdf", filename)
}

/// Build a `200 OK` response carrying a ZIP archive.
pub fn zip_response(bytes: Vec<u8>, filename: &str) -> Response {
    binary_response(bytes, "application/zip", filename)
}

/// Build a generic binary response with `Content-Disposition: attachment`.
pub(crate) fn binary_response(bytes: Vec<u8>, content_type: &str, filename: &str) -> Response {
    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_str(content_type)
            .unwrap_or(HeaderValue::from_static("application/octet-stream")),
    );
    headers.insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_str(&format!("attachment; filename=\"{filename}\""))
            .unwrap_or(HeaderValue::from_static("attachment")),
    );
    (StatusCode::OK, headers, Bytes::from(bytes)).into_response()
}

/// Build a minimal stored (no-compression) ZIP archive containing the
/// given byte buffers.
///
/// We assemble the archive by hand to avoid pulling in the `zip` crate
/// for what is a tiny single-shot use case. Compression-method 0 (stored)
/// is universally supported.
pub fn build_zip(filenames: &[String], blobs: &[Vec<u8>]) -> ApiResult<Vec<u8>> {
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

/// Apply password encryption to a PDF if `userPassword` or `ownerPassword` is in the form map.
/// Returns the PDF unchanged if neither password is provided.
pub async fn apply_encryption(pdf: Vec<u8>, map: &HashMap<String, String>) -> ApiResult<Vec<u8>> {
    let user_pass = map.get("userPassword").filter(|s| !s.is_empty()).map(|s| s.as_str());
    let owner_pass = map.get("ownerPassword").filter(|s| !s.is_empty()).map(|s| s.as_str());
    if user_pass.is_none() && owner_pass.is_none() {
        return Ok(pdf);
    }
    engine::encrypt::encrypt_pdf(
        &pdf,
        user_pass,
        owner_pass,
        engine::encrypt::EncryptionAlgorithm::Aes256,
        engine::encrypt::Permissions::allow_all(),
    )
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))
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
}
