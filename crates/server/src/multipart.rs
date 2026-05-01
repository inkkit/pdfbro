//! Multipart parsing and form-map → request-struct deserialisation helpers.
//!
//! All POST routes in this server share the same shape:
//!   1. Parse a `multipart/form-data` body.
//!   2. Stream every file part into a per-request [`tempfile::TempDir`],
//!      rejecting any path-traversal attempts.
//!   3. Collect non-file fields into a `HashMap<String, String>`
//!      (last-write-wins on duplicates), exposing them via
//!      [`FormFields::map`].
//!   4. Optionally re-encode the map back to `application/x-www-form-urlencoded`
//!      and feed it through `serde_urlencoded::from_str` to recover a
//!      typed request struct (this gives camelCase mapping for free,
//!      via the existing `#[serde(rename_all = "camelCase")]` on the
//!      engine option types).

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use axum::extract::Multipart;
use axum::extract::multipart::MultipartError;
use serde::de::DeserializeOwned;
use tempfile::TempDir;
use tokio::io::AsyncWriteExt;

use crate::error::ApiError;

/// Security limits for multipart parsing.
#[derive(Debug, Clone)]
pub struct MultipartSecurityConfig {
    /// Maximum length of a field name (default: 256).
    pub max_field_name_len: usize,
    /// Maximum length of a non-file field value (default: 1 MB).
    pub max_field_value_len: usize,
    /// Maximum number of files in a single request (default: 100).
    pub max_file_count: usize,
    /// Maximum length of a filename (default: 255).
    pub max_filename_len: usize,
}

impl Default for MultipartSecurityConfig {
    fn default() -> Self {
        Self {
            max_field_name_len: 256,
            max_field_value_len: 1024 * 1024, // 1 MB
            max_file_count: 100,
            max_filename_len: 255,
        }
    }
}

impl MultipartSecurityConfig {
    /// Create a strict configuration for high-security environments.
    pub fn strict() -> Self {
        Self {
            max_field_name_len: 128,
            max_field_value_len: 64 * 1024, // 64 KB
            max_file_count: 10,
            max_filename_len: 100,
        }
    }
}

/// Result of consuming a multipart body.
#[derive(Debug)]
pub struct FormFields {
    /// Named files that were saved into [`Self::tmp`]. Multiple uploads
    /// under the same form name (e.g. Gotenberg's `files`) all live here
    /// in encounter order.
    pub files: Vec<UploadedFile>,
    /// Non-file form fields, last-write-wins on duplicates.
    pub map: HashMap<String, String>,
    /// Owns the per-request scratch directory; dropped at end-of-request.
    pub tmp: TempDir,
}

/// Metadata for a single uploaded file persisted into the tempdir.
#[derive(Debug, Clone)]
pub struct UploadedFile {
    /// Multipart field name (typically `"files"`).
    pub field_name: String,
    /// Sanitised file name (no path separators, no `..` segments).
    pub filename: String,
    /// Reported `Content-Type`, if any.
    pub content_type: Option<String>,
    /// Absolute path inside the request's tempdir.
    pub path: PathBuf,
    /// Total bytes written.
    pub size: u64,
}

impl FormFields {
    /// Read the whole multipart body into a fresh [`FormFields`].
    ///
    /// Files are streamed out to disk to keep memory usage bounded; the
    /// scratch directory is auto-deleted when [`FormFields`] (and hence
    /// [`Self::tmp`]) is dropped.
    pub async fn from_multipart(mp: Multipart) -> Result<Self, ApiError> {
        Self::from_multipart_with_config(mp, MultipartSecurityConfig::default()).await
    }

    /// Read multipart body with custom security configuration.
    pub async fn from_multipart_with_config(
        mut mp: Multipart,
        config: MultipartSecurityConfig,
    ) -> Result<Self, ApiError> {
        let tmp = TempDir::new().map_err(|e| ApiError::Internal(e.to_string()))?;
        let mut files: Vec<UploadedFile> = Vec::new();
        let mut map: HashMap<String, String> = HashMap::new();

        while let Some(field) = next_field(&mut mp).await? {
            let name = field.name().unwrap_or("").to_string();

            // Field name length check
            if name.len() > config.max_field_name_len {
                return Err(ApiError::BadMultipart(format!(
                    "Field name too long: {} chars (max {})",
                    name.len(), config.max_field_name_len
                )));
            }

            // Files have an associated file_name; non-files do not.
            if let Some(raw_filename) = field.file_name().map(str::to_string) {
                // File count limit
                if files.len() >= config.max_file_count {
                    return Err(ApiError::BadMultipart(format!(
                        "Too many files: {} (max {})",
                        files.len(), config.max_file_count
                    )));
                }

                // Filename length check
                if raw_filename.len() > config.max_filename_len {
                    return Err(ApiError::BadMultipart(format!(
                        "Filename too long: {} chars (max {})",
                        raw_filename.len(), config.max_filename_len
                    )));
                }

                let filename = sanitise_filename(&raw_filename)
                    .ok_or_else(|| ApiError::UnsafeFilename(raw_filename.clone()))?;
                let content_type = field.content_type().map(str::to_string);
                let path = tmp.path().join(unique_filename(&files, &filename));

                let mut file = tokio::fs::File::create(&path)
                    .await
                    .map_err(|e| ApiError::Internal(e.to_string()))?;
                let mut bytes_written: u64 = 0;
                let mut field = field;
                loop {
                    match field.chunk().await {
                        Ok(Some(chunk)) => {
                            bytes_written += chunk.len() as u64;
                            file.write_all(&chunk)
                                .await
                                .map_err(|e| ApiError::Internal(e.to_string()))?;
                        }
                        Ok(None) => break,
                        Err(e) => return Err(multipart_to_api(e)),
                    }
                }
                file.flush()
                    .await
                    .map_err(|e| ApiError::Internal(e.to_string()))?;

                files.push(UploadedFile {
                    field_name: name,
                    filename,
                    content_type,
                    path,
                    size: bytes_written,
                });
            } else {
                // Plain text field.
                let bytes = field.bytes().await.map_err(multipart_to_api)?;

                // Field value length check
                if bytes.len() > config.max_field_value_len {
                    return Err(ApiError::BadMultipart(format!(
                        "Field '{}' value too large: {} bytes (max {})",
                        name,
                        bytes.len(),
                        config.max_field_value_len
                    )));
                }

                // Decode best-effort as UTF-8; on failure fall back to lossy.
                let value = String::from_utf8(bytes.to_vec())
                    .unwrap_or_else(|e| String::from_utf8_lossy(e.as_bytes()).into_owned());
                map.insert(name, value);
            }
        }

        Ok(Self { files, map, tmp })
    }

    /// Borrow the file at the given form-name + file-name.
    pub fn find_named(&self, field_name: &str, filename: &str) -> Option<&UploadedFile> {
        self.files
            .iter()
            .find(|f| f.field_name == field_name && f.filename == filename)
    }

    /// Borrow all files uploaded under `field_name`.
    pub fn files_by_field(&self, field_name: &str) -> Vec<&UploadedFile> {
        self.files
            .iter()
            .filter(|f| f.field_name == field_name)
            .collect()
    }

    /// Deserialise the captured form map into the requested type.
    ///
    /// Re-serialises the map as `application/x-www-form-urlencoded` and
    /// runs it through `serde_urlencoded::from_str`. This route exists so
    /// that engine option types with `#[serde(rename_all = "camelCase")]`
    /// can be consumed without a separate field-by-field parser.
    pub fn deserialise<T: DeserializeOwned>(&self) -> Result<T, ApiError> {
        let pairs: Vec<(&String, &String)> = self.map.iter().collect();
        let encoded =
            serde_urlencoded::to_string(&pairs).map_err(|e| ApiError::Internal(e.to_string()))?;
        serde_urlencoded::from_str::<T>(&encoded).map_err(|e| ApiError::InvalidField {
            field: "body",
            message: e.to_string(),
        })
    }
}

async fn next_field<'a>(
    mp: &'a mut Multipart,
) -> Result<Option<axum::extract::multipart::Field<'a>>, ApiError> {
    match mp.next_field().await {
        Ok(opt) => Ok(opt),
        Err(e) => Err(multipart_to_api(e)),
    }
}

fn multipart_to_api(e: MultipartError) -> ApiError {
    // axum's MultipartError exposes a status() to decide 413 vs 400.
    let status = e.status();
    let msg = e.body_text();
    if status == axum::http::StatusCode::PAYLOAD_TOO_LARGE {
        ApiError::BodyTooLarge
    } else {
        ApiError::BadMultipart(msg)
    }
}

/// Reject path-traversal and return the basename only.
///
/// Returns `None` for inputs that contain `..` segments or absolute paths.
pub(crate) fn sanitise_filename(name: &str) -> Option<String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return None;
    }
    // Reject any embedded NUL or path separators.
    if trimmed.contains('\0') {
        return None;
    }
    let path = Path::new(trimmed);
    if path.is_absolute() {
        return None;
    }
    if path.components().any(|c| {
        matches!(
            c,
            std::path::Component::ParentDir | std::path::Component::RootDir
        )
    }) {
        return None;
    }
    // Use only the final component.
    let basename = path.file_name()?.to_string_lossy().to_string();
    if basename.is_empty() || basename == "." || basename == ".." {
        return None;
    }
    Some(basename)
}

/// If a filename collides with an already-saved file, suffix with `-N`.
fn unique_filename(existing: &[UploadedFile], desired: &str) -> String {
    if !existing.iter().any(|f| f.filename == desired) {
        return desired.to_string();
    }
    let (stem, ext) = match desired.rsplit_once('.') {
        Some((s, e)) => (s, format!(".{e}")),
        None => (desired, String::new()),
    };
    let mut n = 1;
    loop {
        let candidate = format!("{stem}-{n}{ext}");
        if !existing.iter().any(|f| f.filename == candidate) {
            return candidate;
        }
        n += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitise_rejects_traversal() {
        assert!(sanitise_filename("../etc/passwd").is_none());
        assert!(sanitise_filename("/etc/passwd").is_none());
        assert!(sanitise_filename("..").is_none());
        assert!(sanitise_filename(".").is_none());
        assert!(sanitise_filename("").is_none());
        assert!(sanitise_filename("foo\0bar").is_none());
    }

    #[test]
    fn sanitise_keeps_basename() {
        assert_eq!(
            sanitise_filename("index.html").as_deref(),
            Some("index.html")
        );
        // Note: a leading subdir is treated as Normal+Normal which we
        // collapse to the basename.
        assert_eq!(
            sanitise_filename("sub/index.html").as_deref(),
            Some("index.html")
        );
        assert_eq!(
            sanitise_filename("  hello.pdf  ").as_deref(),
            Some("hello.pdf")
        );
    }

    #[test]
    fn unique_filename_appends_numeric_suffix() {
        let mut files = vec![UploadedFile {
            field_name: "files".into(),
            filename: "report.pdf".into(),
            content_type: None,
            path: PathBuf::new(),
            size: 0,
        }];
        let next = unique_filename(&files, "report.pdf");
        assert_eq!(next, "report-1.pdf");
        files.push(UploadedFile {
            field_name: "files".into(),
            filename: "report-1.pdf".into(),
            content_type: None,
            path: PathBuf::new(),
            size: 0,
        });
        assert_eq!(unique_filename(&files, "report.pdf"), "report-2.pdf");
    }

    #[test]
    fn multipart_security_config_defaults() {
        let config = MultipartSecurityConfig::default();
        assert_eq!(config.max_field_name_len, 256);
        assert_eq!(config.max_field_value_len, 1024 * 1024);
        assert_eq!(config.max_file_count, 100);
        assert_eq!(config.max_filename_len, 255);
    }

    #[test]
    fn multipart_security_config_strict() {
        let config = MultipartSecurityConfig::strict();
        assert_eq!(config.max_field_name_len, 128);
        assert_eq!(config.max_field_value_len, 64 * 1024);
        assert_eq!(config.max_file_count, 10);
        assert_eq!(config.max_filename_len, 100);
    }
}
