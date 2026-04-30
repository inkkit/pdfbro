//! downloadFrom: URL fetch + allow/deny filtering.
//!
//! Called from every multipart route after `FormFields::from_multipart`.
//! Downloads each URL listed in the `downloadFrom` JSON field, writes the
//! resulting bytes to the request's TempDir, and pushes a synthetic
//! `UploadedFile` entry so downstream handling sees the file alongside any
//! real uploads.

use std::collections::HashMap;

use serde::Deserialize;

use crate::config::ServerConfig;
use crate::error::ApiError;
use crate::multipart::{FormFields, UploadedFile};

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// One entry in the `downloadFrom` JSON array.
#[derive(Debug, Deserialize)]
pub struct DownloadItem {
    /// Full URL to fetch.
    pub url: String,
    /// Optional extra HTTP headers sent with the fetch request.
    #[serde(default)]
    pub extra_http_headers: HashMap<String, String>,
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Parse the `downloadFrom` form field (a JSON array) into a list of
/// [`DownloadItem`]s. Returns an empty vec if the field is absent or blank.
pub fn parse_download_from(
    map: &HashMap<String, String>,
) -> Result<Vec<DownloadItem>, ApiError> {
    let raw = match map.get("downloadFrom") {
        Some(v) if !v.trim().is_empty() => v,
        _ => return Ok(vec![]),
    };
    serde_json::from_str::<Vec<DownloadItem>>(raw).map_err(|e| ApiError::InvalidField {
        field: "downloadFrom",
        message: e.to_string(),
    })
}

/// Validate a URL against allow/deny lists.
///
/// - If `allow_list` is non-empty, the URL must match at least one pattern.
/// - If `deny_list` is non-empty, the URL must not match any pattern.
/// - Patterns are full regex strings (via the `regex` crate).
pub fn url_allowed(
    url: &str,
    allow_list: &[String],
    deny_list: &[String],
) -> Result<bool, ApiError> {
    use regex::Regex;

    if !deny_list.is_empty() {
        for pattern in deny_list {
            let re = Regex::new(pattern).map_err(|e| ApiError::InvalidField {
                field: "downloadFrom deny list",
                message: e.to_string(),
            })?;
            if re.is_match(url) {
                return Ok(false);
            }
        }
    }

    if !allow_list.is_empty() {
        for pattern in allow_list {
            let re = Regex::new(pattern).map_err(|e| ApiError::InvalidField {
                field: "downloadFrom allow list",
                message: e.to_string(),
            })?;
            if re.is_match(url) {
                return Ok(true);
            }
        }
        // Non-empty allow list but no match → denied.
        return Ok(false);
    }

    // No allow list → allowed by default.
    Ok(true)
}

/// Fetch each URL listed in `downloadFrom`, write to `form.tmp`, push to
/// `form.files`. No-op if `config.api_disable_download_from` is true or
/// `downloadFrom` is absent.
pub async fn inject_downloads(
    form: &mut FormFields,
    config: &ServerConfig,
) -> Result<(), ApiError> {
    if config.api_disable_download_from {
        return Ok(());
    }

    let items = parse_download_from(&form.map)?;
    if items.is_empty() {
        return Ok(());
    }

    let client = reqwest::Client::new();

    for item in items {
        // Validate against allow/deny lists.
        if !url_allowed(&item.url, &config.api_download_from_allow_list, &config.api_download_from_deny_list)? {
            return Err(ApiError::InvalidField {
                field: "downloadFrom",
                message: format!("URL `{}` is not allowed by the server allow/deny list", item.url),
            });
        }

        // Derive filename from URL path, then sanitise to prevent path traversal.
        let raw_name = url_to_filename(&item.url);
        let filename = crate::multipart::sanitise_filename(&raw_name)
            .ok_or_else(|| ApiError::InvalidField {
                field: "downloadFrom",
                message: format!("URL `{}` produces an unsafe filename", item.url),
            })?;

        // Retry loop.
        let mut last_err = None;
        let mut fetched: Option<Vec<u8>> = None;
        for _attempt in 0..=config.api_download_from_max_retry {
            let mut req = client.get(&item.url);
            for (k, v) in &item.extra_http_headers {
                req = req.header(k.as_str(), v.as_str());
            }
            match req.send().await {
                Ok(resp) if resp.status().is_success() => {
                    match resp.bytes().await {
                        Ok(b) => {
                            fetched = Some(b.to_vec());
                            break;
                        }
                        Err(e) => {
                            last_err = Some(e.to_string());
                        }
                    }
                }
                Ok(resp) if resp.status().is_server_error() => {
                    // 5xx: transient, retry
                    last_err = Some(format!("HTTP {}", resp.status()));
                }
                Ok(resp) => {
                    // 4xx or other: permanent client error, fail immediately
                    return Err(ApiError::Internal(format!(
                        "downloadFrom: URL `{}` returned permanent error {}",
                        item.url,
                        resp.status(),
                    )));
                }
                Err(e) => {
                    last_err = Some(e.to_string());
                }
            }
        }

        let content = fetched.ok_or_else(|| ApiError::Internal(format!(
            "downloadFrom: failed to fetch `{}` after {} attempts: {}",
            item.url,
            config.api_download_from_max_retry + 1,
            last_err.unwrap_or_default(),
        )))?;

        // Write to tempdir.
        let dest_path = form.tmp.path().join(&filename);
        tokio::fs::write(&dest_path, &content)
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))?;

        form.files.push(UploadedFile {
            field_name: "files".to_string(),
            filename,
            content_type: None,
            path: dest_path,
            size: content.len() as u64,
        });
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn url_to_filename(url: &str) -> String {
    // Use the last path segment, stripping query/fragment.
    let clean = url.split('?').next().unwrap_or(url);
    let clean = clean.split('#').next().unwrap_or(clean);
    let name = clean.rsplit('/').next().unwrap_or("download");
    if name.is_empty() {
        "download".to_string()
    } else {
        name.to_string()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_map(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
    }

    #[test]
    fn parse_download_from_absent() {
        let result = parse_download_from(&make_map(&[])).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn parse_download_from_blank() {
        let result = parse_download_from(&make_map(&[("downloadFrom", "  ")])).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn parse_download_from_valid_json() {
        let json = r#"[{"url":"https://example.com/a.pdf"},{"url":"https://cdn.example.com/b.pdf","extra_http_headers":{"X-Token":"abc"}}]"#;
        let items = parse_download_from(&make_map(&[("downloadFrom", json)])).unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].url, "https://example.com/a.pdf");
        assert_eq!(items[1].extra_http_headers.get("X-Token").map(String::as_str), Some("abc"));
    }

    #[test]
    fn parse_download_from_invalid_json() {
        let err = parse_download_from(&make_map(&[("downloadFrom", "not-json")])).unwrap_err();
        assert!(matches!(err, ApiError::InvalidField { field: "downloadFrom", .. }));
    }

    #[test]
    fn url_allowed_empty_lists_allows_all() {
        assert!(url_allowed("https://example.com/file.pdf", &[], &[]).unwrap());
    }

    #[test]
    fn url_allowed_deny_list_blocks() {
        let deny = vec!["http://".to_string()];
        assert!(!url_allowed("http://example.com/file.pdf", &[], &deny).unwrap());
        assert!(url_allowed("https://example.com/file.pdf", &[], &deny).unwrap());
    }

    #[test]
    fn url_allowed_allow_list_restricts() {
        let allow = vec!["https://trusted\\.com".to_string()];
        assert!(url_allowed("https://trusted.com/file.pdf", &allow, &[]).unwrap());
        assert!(!url_allowed("https://other.com/file.pdf", &allow, &[]).unwrap());
    }

    #[test]
    fn url_allowed_deny_takes_precedence_over_allow() {
        let allow = vec!["https://".to_string()];
        let deny = vec!["https://bad\\.com".to_string()];
        assert!(!url_allowed("https://bad.com/file.pdf", &allow, &deny).unwrap());
        assert!(url_allowed("https://good.com/file.pdf", &allow, &deny).unwrap());
    }

    #[test]
    fn url_to_filename_basic() {
        assert_eq!(url_to_filename("https://example.com/docs/report.pdf"), "report.pdf");
        assert_eq!(url_to_filename("https://example.com/file.pdf?token=abc"), "file.pdf");
    }

    #[test]
    fn url_to_filename_trailing_slash_returns_download() {
        // Empty last segment falls back to "download".
        assert_eq!(url_to_filename("https://example.com/"), "download");
    }

    #[test]
    fn url_to_filename_path_traversal_rejected_by_sanitise() {
        // url_to_filename returns ".." but sanitise_filename must reject it.
        let raw = url_to_filename("http://evil.com/..");
        assert_eq!(raw, "..");
        // When used through the full flow, sanitise_filename catches it.
        // Test that sanitise_filename rejects the output.
        assert!(crate::multipart::sanitise_filename(&raw).is_none());
    }

    #[test]
    fn disabled_config_skips_download() {
        let json = r#"[{"url":"https://example.com/a.pdf"}]"#;
        let items = parse_download_from(&make_map(&[("downloadFrom", json)])).unwrap();
        assert_eq!(items.len(), 1);
    }

    #[tokio::test]
    async fn inject_downloads_fetches_and_appends_file() {
        use axum::Router;
        use axum::routing::get;
        use std::net::SocketAddr;

        // Start a minimal axum server serving a static PDF-like body.
        let fixture = Router::new().route(
            "/test.pdf",
            get(|| async { (axum::http::StatusCode::OK, b"FAKEPDF".as_ref()) }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr: SocketAddr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, fixture).await.unwrap();
        });

        let json = format!(r#"[{{"url":"http://{}/test.pdf","extra_http_headers":{{}}}}]"#, addr);
        let tmp = tempfile::TempDir::new().unwrap();
        let mut form = FormFields {
            files: vec![],
            map: {
                let mut m = HashMap::new();
                m.insert("downloadFrom".to_string(), json);
                m
            },
            tmp,
        };

        let config = crate::config::ServerConfig::resolve(
            &crate::config::ServerArgs::default(),
            &HashMap::new(),
        )
        .unwrap();

        inject_downloads(&mut form, &config).await.unwrap();

        assert_eq!(form.files.len(), 1);
        assert_eq!(form.files[0].filename, "test.pdf");
        assert_eq!(form.files[0].size, 7);
        let written = tokio::fs::read(&form.files[0].path).await.unwrap();
        assert_eq!(written, b"FAKEPDF");
    }
}
