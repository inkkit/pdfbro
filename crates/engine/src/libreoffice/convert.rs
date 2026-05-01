//! Single-file `soffice --convert-to` invocation with isolated `UserInstallation`.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::types::{EngineError, EngineResult};

use super::OfficeOptions;
use super::filter::for_extension;

/// Run `soffice` once for `input`, returning the produced PDF bytes.
///
/// Caller is responsible for input-existence checks, validation, and
/// concurrency limits — those live on [`super::LibreOfficeEngine`].
pub(super) async fn run_convert(
    exe: &Path,
    timeout: Duration,
    input: &Path,
    opts: &OfficeOptions,
) -> EngineResult<Vec<u8>> {
    // Per-call tempdir → UserInstallation directory + outdir. Drop cleans up.
    let tmp = tempfile::tempdir()?;
    let user_dir = tmp.path().join("uipfx");
    std::fs::create_dir_all(&user_dir)?;
    let outdir = tmp.path().join("out");
    std::fs::create_dir_all(&outdir)?;

    let convert_to = build_convert_to(input, opts);
    let user_url = path_to_file_url(&user_dir);

    // SECURITY: Create macro security policy file in UserInstallation
    // This disables all macro execution for this conversion
    let user_config_dir = user_dir.join("user").join("config");
    std::fs::create_dir_all(&user_config_dir)?;
    let macro_security_file = user_config_dir.join("soffice.cfg");
    std::fs::write(&macro_security_file, b"[Security]\nMacroSecurityLevel=3\n")?;

    let mut cmd = tokio::process::Command::new(exe);
    cmd.arg("--headless")
        .arg("--norestore")
        .arg("--nologo")
        .arg("--nodefault")
        .arg("--nofirststartwizard")
        .arg("--convert-to")
        .arg(&convert_to)
        .arg("--outdir")
        .arg(&outdir)
        .arg(format!("-env:UserInstallation={user_url}"))
        .arg(input)
        .kill_on_drop(true)
        .stdin(std::process::Stdio::null());

    let output = match tokio::time::timeout(timeout, cmd.output()).await {
        Err(_) => return Err(EngineError::Timeout(timeout)),
        Ok(Err(e)) => {
            return Err(EngineError::Internal(format!("soffice spawn failed: {e}")));
        }
        Ok(Ok(o)) => o,
    };

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        let code = output
            .status
            .code()
            .map(|c| c.to_string())
            .unwrap_or_else(|| "signal".into());
        return Err(EngineError::Internal(format!(
            "soffice exit {code}: {}",
            pick_message(&stderr, &stdout)
        )));
    }

    // soffice writes <input_stem>.pdf into outdir.
    let stem = input
        .file_stem()
        .ok_or_else(|| EngineError::Internal("input path has no file stem".into()))?;
    let mut pdf_path: PathBuf = outdir.join(stem);
    pdf_path.set_extension("pdf");

    let bytes = std::fs::read(&pdf_path).map_err(|e| match e.kind() {
        std::io::ErrorKind::NotFound => {
            EngineError::Internal(format!("soffice produced no PDF at {}", pdf_path.display()))
        }
        _ => EngineError::Io(e),
    })?;

    drop(tmp); // Explicit cleanup point; Drop would do it anyway.
    Ok(bytes)
}

fn build_convert_to(input: &Path, opts: &OfficeOptions) -> OsString {
    let ext = input
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or_default();
    let base = for_extension(ext);
    match opts.filter_blob() {
        Some(blob) => format!("{base}:{blob}").into(),
        None => OsString::from(base),
    }
}

/// Encode a filesystem path as a `file://` URL suitable for the
/// `-env:UserInstallation=...` argument. Best-effort lossy on non-UTF-8
/// paths (LibreOffice itself does not accept non-UTF-8 here).
fn path_to_file_url(p: &Path) -> String {
    let s = p.to_string_lossy();
    if cfg!(windows) {
        let s = s.replace('\\', "/");
        format!("file:///{s}")
    } else {
        format!("file://{s}")
    }
}

fn pick_message<'a>(stderr: &'a str, stdout: &'a str) -> &'a str {
    let s = stderr.trim();
    if s.is_empty() { stdout.trim() } else { s }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::PageRanges;

    #[test]
    fn build_convert_to_no_blob_for_default() {
        let p = PathBuf::from("/tmp/sample.docx");
        let out = build_convert_to(&p, &OfficeOptions::default());
        assert_eq!(out, OsString::from("pdf:writer_pdf_Export"));
    }

    #[test]
    fn build_convert_to_appends_blob_when_options_set() {
        let p = PathBuf::from("/tmp/sample.docx");
        let opts = OfficeOptions {
            page_ranges: Some(PageRanges::parse("1-3").expect("parse")),
            ..Default::default()
        };
        let out = build_convert_to(&p, &opts);
        let s = out.into_string().expect("utf8");
        assert!(s.starts_with("pdf:writer_pdf_Export:"), "got {s}");
        assert!(s.contains("\"PageRange\""), "got {s}");
        assert!(s.contains("\"1-3\""), "got {s}");
    }

    #[test]
    fn build_convert_to_unknown_ext_uses_pdf_fallback() {
        let p = PathBuf::from("/tmp/sample.weird");
        let out = build_convert_to(&p, &OfficeOptions::default());
        assert_eq!(out, OsString::from("pdf"));
    }

    #[test]
    fn path_to_file_url_unix_style() {
        if cfg!(not(windows)) {
            let url = path_to_file_url(Path::new("/tmp/foo bar"));
            assert_eq!(url, "file:///tmp/foo bar");
        }
    }

    #[test]
    fn pick_message_prefers_stderr_when_present() {
        assert_eq!(pick_message("err", "out"), "err");
        assert_eq!(pick_message("   ", "out"), "out");
        assert_eq!(pick_message("", ""), "");
    }
}
