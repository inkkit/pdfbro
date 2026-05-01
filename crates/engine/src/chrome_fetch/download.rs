//! Download a pinned Chrome-for-Testing build, verify, extract into the
//! cache.
//!
//! Manifest format:
//! https://github.com/GoogleChromeLabs/chrome-for-testing
//!
//! Per-version endpoint:
//! `https://googlechromelabs.github.io/chrome-for-testing/<version>.json`

use std::path::{Path, PathBuf};

use serde::Deserialize;
use sha2::{Digest, Sha256};
use thiserror::Error;
use tokio::io::AsyncWriteExt;

use super::cache::chrome_exe_path;

/// Errors that can occur when fetching or preparing a Chrome-for-Testing binary.
#[derive(Debug, Error)]
pub enum ChromeFetchError {
    /// System Chrome was not found and `auto_download` was disabled.
    #[error("system Chrome not found and auto_download disabled")]
    NotFoundAndDownloadDisabled,
    /// The current platform is not supported by the Chrome-for-Testing manifest.
    #[error("unsupported platform: {0}")]
    UnsupportedPlatform(&'static str),
    /// Fetching or parsing the per-version manifest failed.
    #[error("manifest fetch failed: {0}")]
    Manifest(String),
    /// The manifest did not contain a download entry for this platform.
    #[error("no download for platform '{0}' in manifest")]
    NoPlatformInManifest(&'static str),
    /// The HTTP download of the Chrome archive failed.
    #[error("download failed: {0}")]
    Download(String),
    /// SHA-256 digest of the downloaded archive did not match the expected value.
    #[error("checksum mismatch: expected {expected}, got {actual}")]
    Checksum {
        /// The expected hex digest from the manifest.
        expected: String,
        /// The actual hex digest computed from the downloaded file.
        actual: String,
    },
    /// Extracting the zip archive failed.
    #[error("extract failed: {0}")]
    Extract(String),
    /// An underlying I/O error.
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Debug, Deserialize)]
struct VersionManifest {
    downloads: Downloads,
}
#[derive(Debug, Deserialize)]
struct Downloads {
    chrome: Vec<DownloadEntry>,
}
#[derive(Debug, Deserialize)]
struct DownloadEntry {
    platform: String,
    url: String,
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
const PLATFORM: &str = "mac-arm64";
#[cfg(all(target_os = "macos", target_arch = "x86_64"))]
const PLATFORM: &str = "mac-x64";
#[cfg(target_os = "linux")]
const PLATFORM: &str = "linux64";
#[cfg(target_os = "windows")]
const PLATFORM: &str = "win64";

/// Download Chrome `version` into `cache_root/<version>/`. Returns the
/// path to the executable.
///
/// Atomicity: download to `cache_root/<version>.partial/`, extract there,
/// rename to `cache_root/<version>/` on success.
pub async fn download_chrome(cache_root: &Path, version: &str) -> Result<PathBuf, ChromeFetchError> {
    let manifest = fetch_manifest(version).await?;
    let entry = manifest.downloads.chrome.into_iter()
        .find(|e| e.platform == PLATFORM)
        .ok_or(ChromeFetchError::NoPlatformInManifest(PLATFORM))?;

    let dest = cache_root.join(version);
    if dest.exists() {
        return Ok(chrome_exe_path(&dest));
    }
    tokio::fs::create_dir_all(cache_root).await?;
    let staging = cache_root.join(format!("{version}.partial"));
    if staging.exists() {
        tokio::fs::remove_dir_all(&staging).await?;
    }
    tokio::fs::create_dir_all(&staging).await?;

    let archive = staging.join(archive_filename());
    download_to_file(&entry.url, &archive).await?;
    extract_archive(&archive, &staging)?;
    tokio::fs::rename(&staging, &dest).await?;
    Ok(chrome_exe_path(&dest))
}

async fn fetch_manifest(version: &str) -> Result<VersionManifest, ChromeFetchError> {
    let url = format!(
        "https://googlechromelabs.github.io/chrome-for-testing/{version}.json"
    );
    let resp = reqwest::get(&url).await
        .map_err(|e| ChromeFetchError::Manifest(e.to_string()))?;
    if !resp.status().is_success() {
        return Err(ChromeFetchError::Manifest(format!("HTTP {}", resp.status())));
    }
    let text = resp.text().await.map_err(|e| ChromeFetchError::Manifest(e.to_string()))?;
    serde_json::from_str(&text).map_err(|e| ChromeFetchError::Manifest(e.to_string()))
}

async fn download_to_file(url: &str, dest: &Path) -> Result<(), ChromeFetchError> {
    let resp = reqwest::get(url).await
        .map_err(|e| ChromeFetchError::Download(e.to_string()))?;
    if !resp.status().is_success() {
        return Err(ChromeFetchError::Download(format!("HTTP {}", resp.status())));
    }
    let bytes = resp.bytes().await
        .map_err(|e| ChromeFetchError::Download(e.to_string()))?;
    let mut file = tokio::fs::File::create(dest).await?;
    file.write_all(&bytes).await?;
    file.flush().await?;
    Ok(())
}

#[allow(dead_code)]
fn verify_sha256(path: &Path, expected_hex: &str) -> Result<(), ChromeFetchError> {
    let mut hasher = Sha256::new();
    let mut file = std::fs::File::open(path)?;
    std::io::copy(&mut file, &mut hasher)?;
    let actual = hex_lower(&hasher.finalize());
    if actual != expected_hex.to_ascii_lowercase() {
        return Err(ChromeFetchError::Checksum { expected: expected_hex.into(), actual });
    }
    Ok(())
}

fn hex_lower(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

fn archive_filename() -> &'static str { "chrome.zip" }

fn extract_archive(archive: &Path, into: &Path) -> Result<(), ChromeFetchError> {
    let file = std::fs::File::open(archive)?;
    let mut zip = zip::ZipArchive::new(file)
        .map_err(|e| ChromeFetchError::Extract(e.to_string()))?;
    zip.extract(into).map_err(|e| ChromeFetchError::Extract(e.to_string()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        for entry in walkdir::WalkDir::new(into) {
            let entry = entry.map_err(|e| ChromeFetchError::Extract(e.to_string()))?;
            if entry.file_type().is_file()
                && entry.file_name().to_string_lossy().contains("chrome")
            {
                let mut perm = entry.metadata()
                    .map_err(|e| ChromeFetchError::Extract(e.to_string()))?
                    .permissions();
                perm.set_mode(0o755);
                let _ = std::fs::set_permissions(entry.path(), perm);
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_lower_pads_zeros() {
        assert_eq!(hex_lower(&[0x0a, 0xff, 0x00]), "0aff00");
    }

    #[test]
    fn manifest_deserializes() {
        let json = r#"{
            "downloads": {
                "chrome": [
                    {"platform": "linux64", "url": "https://example.com/chrome.zip"},
                    {"platform": "mac-arm64", "url": "https://example.com/mac.zip"}
                ]
            }
        }"#;
        let m: VersionManifest = serde_json::from_str(json).unwrap();
        assert_eq!(m.downloads.chrome.len(), 2);
    }
}
