//! `soffice` executable discovery and version probing.
//!
//! The discovery search order is documented in spec 12 § *Executable
//! discovery*. The probe step runs `soffice --headless --version` under a
//! caller-supplied timeout to confirm the binary actually starts.

use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::types::{EngineError, EngineResult};

/// Public entry point: locate `soffice` using env var, `$PATH`, and platform defaults.
pub(super) fn find_soffice() -> EngineResult<PathBuf> {
    find_in(&candidate_paths())
}

/// Search the supplied list of candidate paths in order, returning the first
/// path that exists and is executable.
///
/// Factored out of [`find_soffice`] so unit tests can exercise the lookup
/// without mutating the process environment (which is `unsafe` on edition 2024).
pub(super) fn find_in(searched: &[PathBuf]) -> EngineResult<PathBuf> {
    for path in searched {
        if path.exists() && is_executable(path) {
            return Ok(path.clone());
        }
    }
    Err(EngineError::Internal(format!(
        "LibreOffice not found: searched {searched:?}"
    )))
}

/// Build the platform-specific list of candidate locations.
pub(super) fn candidate_paths() -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = Vec::new();

    // 1. $LIBREOFFICE_PATH (env var)
    if let Some(p) = std::env::var_os("LIBREOFFICE_PATH") {
        out.push(PathBuf::from(p));
    }

    // 2. `which soffice`, `which libreoffice`
    if let Some(p) = which_in_path("soffice") {
        out.push(p);
    }
    if let Some(p) = which_in_path("libreoffice") {
        out.push(p);
    }

    // 3-5. Platform defaults.
    #[cfg(target_os = "macos")]
    {
        out.push(PathBuf::from(
            "/Applications/LibreOffice.app/Contents/MacOS/soffice",
        ));
        // Homebrew cask `--appdir=~/Applications` install location.
        if let Some(home) = std::env::var_os("HOME") {
            let mut p = PathBuf::from(home);
            p.push("Applications/LibreOffice.app/Contents/MacOS/soffice");
            out.push(p);
        }
    }
    #[cfg(target_os = "linux")]
    {
        out.push(PathBuf::from("/usr/bin/soffice"));
        out.push(PathBuf::from("/usr/bin/libreoffice"));
        out.push(PathBuf::from("/usr/lib/libreoffice/program/soffice"));
        out.push(PathBuf::from("/snap/bin/libreoffice"));
        out.push(PathBuf::from(
            "/var/lib/flatpak/exports/bin/org.libreoffice.LibreOffice",
        ));
    }
    #[cfg(target_os = "windows")]
    {
        out.push(PathBuf::from(
            r"C:\Program Files\LibreOffice\program\soffice.exe",
        ));
        out.push(PathBuf::from(
            r"C:\Program Files (x86)\LibreOffice\program\soffice.exe",
        ));
    }

    out
}

fn which_in_path(name: &str) -> Option<PathBuf> {
    let path_var = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path_var) {
        let candidate = dir.join(name);
        if candidate.is_file() && is_executable(&candidate) {
            return Some(candidate);
        }
        #[cfg(target_os = "windows")]
        {
            let exe = dir.join(format!("{name}.exe"));
            if exe.is_file() {
                return Some(exe);
            }
        }
    }
    None
}

#[cfg(unix)]
fn is_executable(p: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    p.metadata()
        .map(|m| m.is_file() && (m.permissions().mode() & 0o111) != 0)
        .unwrap_or(false)
}

#[cfg(not(unix))]
fn is_executable(p: &Path) -> bool {
    p.is_file()
}

/// Run `soffice --headless --version` under `timeout`. Empty stdout or a
/// non-zero exit status is reported as `EngineError::Internal`.
pub(super) async fn probe(exe: &Path, timeout: Duration) -> EngineResult<()> {
    let mut cmd = tokio::process::Command::new(exe);
    cmd.arg("--headless")
        .arg("--version")
        .kill_on_drop(true)
        .stdin(std::process::Stdio::null());

    let out = match tokio::time::timeout(timeout, cmd.output()).await {
        Err(_) => return Err(EngineError::Timeout(timeout)),
        Ok(Err(e)) => {
            return Err(EngineError::Internal(format!(
                "LibreOffice probe failed: {e}"
            )));
        }
        Ok(Ok(o)) => o,
    };

    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        return Err(EngineError::Internal(format!(
            "LibreOffice probe failed (exit {:?}): {}",
            out.status.code(),
            stderr.trim()
        )));
    }
    if out.stdout.is_empty() {
        return Err(EngineError::Internal(
            "LibreOffice probe failed: empty stdout".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discover_returns_searched_list_when_missing() {
        let paths = vec![
            PathBuf::from("/nonexistent/__folio_no_soffice_a"),
            PathBuf::from("/nonexistent/__folio_no_soffice_b"),
        ];
        let err = find_in(&paths).expect_err("should fail for missing paths");
        let msg = format!("{err}");
        assert!(
            msg.contains("__folio_no_soffice_a"),
            "missing first path in: {msg}"
        );
        assert!(
            msg.contains("__folio_no_soffice_b"),
            "missing second path in: {msg}"
        );
        assert!(matches!(err, EngineError::Internal(_)));
    }

    #[test]
    fn candidate_paths_is_non_empty() {
        // At minimum each platform contributes at least one default location.
        let paths = candidate_paths();
        assert!(!paths.is_empty());
    }
}
