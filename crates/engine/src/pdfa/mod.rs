//! PDF/A and PDF/UA conformance conversion.
//!
//! Implements spec 14 — `engine::pdfa`.

use std::time::Duration;

use crate::types::{EngineError, EngineResult};

/// PDF/A conformance levels for archival compliance.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PdfAProfile {
    /// PDF/A-1b: Basic conformance (Level B) for PDF 1.4.
    PdfA1b,
    /// PDF/A-2b: Basic conformance (Level B) for PDF 1.7.
    PdfA2b,
    /// PDF/A-3b: Basic conformance (Level B) with embedded files support.
    PdfA3b,
}

impl PdfAProfile {
    /// Ghostscript PDFACompatibilityPolicy value.
    fn ghostscript_policy(&self) -> &'static str {
        match self {
            PdfAProfile::PdfA1b => "1",
            PdfAProfile::PdfA2b => "2",
            PdfAProfile::PdfA3b => "3",
        }
    }

    /// Ghostscript ProcessColorModel.
    fn color_model(&self) -> &'static str {
        // PDF/A requires specific color models
        match self {
            PdfAProfile::PdfA1b => "DeviceRGB",
            PdfAProfile::PdfA2b => "DeviceRGB",
            PdfAProfile::PdfA3b => "DeviceRGB",
        }
    }

    /// Human-readable profile name.
    pub fn as_str(&self) -> &'static str {
        match self {
            PdfAProfile::PdfA1b => "PDF/A-1b",
            PdfAProfile::PdfA2b => "PDF/A-2b",
            PdfAProfile::PdfA3b => "PDF/A-3b",
        }
    }
}

impl std::str::FromStr for PdfAProfile {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "PDF/A-1b" | "pdf/a-1b" | "PDFA1b" => Ok(PdfAProfile::PdfA1b),
            "PDF/A-2b" | "pdf/a-2b" | "PDFA2b" => Ok(PdfAProfile::PdfA2b),
            "PDF/A-3b" | "pdf/a-3b" | "PDFA3b" => Ok(PdfAProfile::PdfA3b),
            _ => Err(format!("Unknown PDF/A profile: {}", s)),
        }
    }
}

/// Convert a PDF to PDF/A conformance using Ghostscript.
///
/// This function shells out to Ghostscript's pdfwrite device which handles:
/// - Color model conversion
/// - Font embedding verification
/// - Structure tagging for compliance
pub async fn convert_to_pdfa(pdf: &[u8], profile: PdfAProfile) -> EngineResult<Vec<u8>> {
    let timeout = Duration::from_secs(120);

    // Create temp files for input/output
    let tmp_dir = tempfile::tempdir()?;
    let input_path = tmp_dir.path().join("input.pdf");
    let output_path = tmp_dir.path().join("output.pdf");

    // Write input
    tokio::fs::write(&input_path, pdf).await?;

    // Run Ghostscript conversion
    let result = run_ghostscript(&input_path, &output_path, profile, timeout).await;

    match result {
        Ok(()) => {
            let output = tokio::fs::read(&output_path).await?;
            Ok(output)
        }
        Err(e) => {
            // Try fallback to qpdf if Ghostscript fails
            tracing::warn!(error = %e, "Ghostscript failed, trying qpdf fallback");
            run_qpdf_fallback(&input_path, &output_path, timeout).await?;
            let output = tokio::fs::read(&output_path).await?;
            Ok(output)
        }
    }
}

async fn run_ghostscript(
    input: &std::path::Path,
    output: &std::path::Path,
    profile: PdfAProfile,
    timeout: Duration,
) -> EngineResult<()> {
    let policy = profile.ghostscript_policy();
    let color_model = profile.color_model();

    let mut cmd = tokio::process::Command::new("gs");
    cmd.arg("-dPDFA=".to_string() + policy)
        .arg("-dBATCH")
        .arg("-dNOPAUSE")
        .arg("-dNOOUTERSAVE")
        .arg(format!("-sProcessColorModel={}", color_model))
        .arg("-sDEVICE=pdfwrite")
        .arg("-sPDFACompatibilityPolicy=1")
        .arg(format!("-sOutputFile={}", output.display()))
        .arg("-c")
        .arg(".setpdfwrite")
        .arg("-f")
        .arg(input)
        .kill_on_drop(true)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());

    let output_result = match tokio::time::timeout(timeout, cmd.output()).await {
        Err(_) => return Err(EngineError::Timeout(timeout)),
        Ok(Err(e)) => {
            return Err(EngineError::Internal(format!(
                "Ghostscript spawn failed: {}",
                e
            )));
        }
        Ok(Ok(o)) => o,
    };

    if !output_result.status.success() {
        let stderr = String::from_utf8_lossy(&output_result.stderr);
        let stdout = String::from_utf8_lossy(&output_result.stdout);
        return Err(EngineError::Internal(format!(
            "Ghostscript conversion failed. stderr: {}, stdout: {}",
            stderr, stdout
        )));
    }

    // Verify output was created
    if !output.exists() {
        return Err(EngineError::Internal(
            "Ghostscript did not produce output file".into(),
        ));
    }

    Ok(())
}

async fn run_qpdf_fallback(
    input: &std::path::Path,
    output: &std::path::Path,
    timeout: Duration,
) -> EngineResult<()> {
    let mut cmd = tokio::process::Command::new("qpdf");
    cmd.arg("--qpdf")
        .arg("--set-pdf-a")
        .arg(input)
        .arg(output)
        .kill_on_drop(true)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());

    let output_result = match tokio::time::timeout(timeout, cmd.output()).await {
        Err(_) => return Err(EngineError::Timeout(timeout)),
        Ok(Err(e)) => {
            return Err(EngineError::Internal(format!(
                "qpdf fallback spawn failed: {}",
                e
            )));
        }
        Ok(Ok(o)) => o,
    };

    if !output_result.status.success() {
        let stderr = String::from_utf8_lossy(&output_result.stderr);
        return Err(EngineError::Internal(format!(
            "qpdf fallback failed: {}",
            stderr
        )));
    }

    Ok(())
}

/// Check if Ghostscript is available.
pub fn ghostscript_available() -> bool {
    which::which("gs").is_ok()
}

/// Check if qpdf is available.
pub fn qpdf_available() -> bool {
    which::which("qpdf").is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pdfa_profile_parsing() {
        assert_eq!(
            "PDF/A-1b".parse::<PdfAProfile>().unwrap(),
            PdfAProfile::PdfA1b
        );
        assert_eq!(
            "PDF/A-2b".parse::<PdfAProfile>().unwrap(),
            PdfAProfile::PdfA2b
        );
        assert_eq!(
            "PDF/A-3b".parse::<PdfAProfile>().unwrap(),
            PdfAProfile::PdfA3b
        );
        assert!("invalid".parse::<PdfAProfile>().is_err());
    }

    #[test]
    fn pdfa_profile_as_str() {
        assert_eq!(PdfAProfile::PdfA1b.as_str(), "PDF/A-1b");
        assert_eq!(PdfAProfile::PdfA2b.as_str(), "PDF/A-2b");
        assert_eq!(PdfAProfile::PdfA3b.as_str(), "PDF/A-3b");
    }
}
