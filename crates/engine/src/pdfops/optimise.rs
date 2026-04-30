//! PDF optimisation/compression using external backends.
//!
//! Supports Ghostscript, qpdf, and pdfcpu with auto-selection.
//! Implementation of `docs/specs/42-smart-pdf-optimiser.md`.

use std::path::Path;
use std::process::Command;
use std::time::Instant;

use crate::types::{EngineError, EngineResult};

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Compression quality preset.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptimisePreset {
    /// Low quality, 72 DPI, heavy compression (smallest file)
    Screen,
    /// Medium quality, 150 DPI (balanced)
    Ebook,
    /// High quality, 300 DPI, light compression (largest file)
    Printer,
}

impl OptimisePreset {
    /// Parse preset from string.
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "screen" => Some(Self::Screen),
            "ebook" => Some(Self::Ebook),
            "printer" => Some(Self::Printer),
            _ => None,
        }
    }

    /// Ghostscript PDFSETTINGS parameter.
    pub fn ghostscript_settings(&self) -> &'static str {
        match self {
            Self::Screen => "/screen",
            Self::Ebook => "/ebook",
            Self::Printer => "/printer",
        }
    }
}

/// Backend for PDF optimisation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptimiseBackend {
    /// Ghostscript - best compression, slowest
    Ghostscript,
    /// qpdf - medium compression, faster
    Qpdf,
}

impl OptimiseBackend {
    /// Check if backend is available on system.
    pub fn is_available(&self) -> bool {
        let binary = match self {
            Self::Ghostscript => "gs",
            Self::Qpdf => "qpdf",
        };
        Command::new("which")
            .arg(binary)
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    /// Priority order (best compression first).
    pub fn priority_order() -> Vec<Self> {
        vec![Self::Ghostscript, Self::Qpdf]
    }
}

/// Optimisation result with statistics.
#[derive(Debug, Clone)]
pub struct OptimiseResult {
    /// Optimised PDF bytes.
    pub data: Vec<u8>,
    /// Original size in bytes.
    pub original_size: usize,
    /// Optimised size in bytes.
    pub optimised_size: usize,
    /// Backend used.
    pub backend: OptimiseBackend,
    /// Preset applied.
    pub preset: OptimisePreset,
    /// Duration in milliseconds.
    pub duration_ms: u64,
}

impl OptimiseResult {
    /// Compression ratio (0.0-1.0, lower is better).
    pub fn compression_ratio(&self) -> f64 {
        if self.original_size == 0 {
            return 0.0;
        }
        self.optimised_size as f64 / self.original_size as f64
    }

    /// Size reduction percentage.
    pub fn reduction_percent(&self) -> f64 {
        (1.0 - self.compression_ratio()) * 100.0
    }
}

// ---------------------------------------------------------------------------
// Main Optimise Function
// ---------------------------------------------------------------------------

/// Optimise a PDF using the best available backend.
pub async fn optimise_pdf(
    input: &[u8],
    preset: OptimisePreset,
    preferred_backend: Option<OptimiseBackend>,
) -> EngineResult<OptimiseResult> {
    let start = Instant::now();
    let original_size = input.len();

    // Create temp directory
    let temp_dir = tempfile::tempdir()
        .map_err(|e| EngineError::Internal(format!("Failed to create temp dir: {e}")))?;

    let input_path = temp_dir.path().join("input.pdf");
    let output_path = temp_dir.path().join("output.pdf");

    // Write input to temp file
    std::fs::write(&input_path, input)
        .map_err(|e| EngineError::Internal(format!("Failed to write input: {e}")))?;

    // Determine backend
    let backends: Vec<OptimiseBackend> = match preferred_backend {
        Some(b) if b.is_available() => vec![b],
        _ => OptimiseBackend::priority_order()
            .into_iter()
            .filter(|b| b.is_available())
            .collect(),
    };

    if backends.is_empty() {
        return Err(EngineError::Internal(
            "No PDF optimisation backend available. Install ghostscript or qpdf.".into(),
        ));
    }

    let mut last_error = None;

    for backend in backends {
        tracing::info!(?backend, "Attempting PDF optimisation");

        let result = match backend {
            OptimiseBackend::Ghostscript => {
                optimise_with_ghostscript(&input_path, &output_path, preset)
            }
            OptimiseBackend::Qpdf => optimise_with_qpdf(&input_path, &output_path, preset),
        };

        match result {
            Ok(()) => {
                // Read output
                let data = std::fs::read(&output_path)
                    .map_err(|e| EngineError::Internal(format!("Failed to read output: {e}")))?;

                let duration_ms = start.elapsed().as_millis() as u64;

                tracing::info!(
                    original_size,
                    optimised_size = data.len(),
                    backend = ?backend,
                    preset = ?preset,
                    duration_ms,
                    ratio = data.len() as f64 / original_size as f64,
                    "PDF optimisation complete"
                );

                return Ok(OptimiseResult {
                    data,
                    original_size,
                    optimised_size: data.len(),
                    backend,
                    preset,
                    duration_ms,
                });
            }
            Err(e) => {
                tracing::warn!(?backend, error = %e, "Backend failed, trying next");
                last_error = Some(e);
            }
        }
    }

    // All backends failed
    Err(last_error.unwrap_or_else(|| {
        EngineError::Internal("All optimisation backends failed".into())
    }))
}

// ---------------------------------------------------------------------------
// Backend Implementations
// ---------------------------------------------------------------------------

fn optimise_with_ghostscript(
    input_path: &Path,
    output_path: &Path,
    preset: OptimisePreset,
) -> EngineResult<()> {
    let mut cmd = Command::new("gs");

    // Base arguments
    cmd.arg("-sDEVICE=pdfwrite")
        .arg("-dNOPAUSE")
        .arg("-dQUIET")
        .arg("-dBATCH")
        .arg(format!("-sOutputFile={}", output_path.display()));

    // Preset-specific settings
    match preset {
        OptimisePreset::Screen => {
            cmd.arg("-dPDFSETTINGS=/screen")
                .arg("-dCompatibilityLevel=1.4")
                .arg("-dDownsampleColorImages=true")
                .arg("-dColorImageResolution=72")
                .arg("-dAutoFilterColorImages=false")
                .arg("-dColorImageFilter=/DCTEncode")
                .arg("-dDownsampleGrayImages=true")
                .arg("-dGrayImageResolution=72")
                .arg("-dAutoFilterGrayImages=false")
                .arg("-dGrayImageFilter=/DCTEncode")
                .arg("-dDownsampleMonoImages=true")
                .arg("-dMonoImageResolution=72")
                .arg("-dAutoFilterMonoImages=false")
                .arg("-dMonoImageFilter=/CCITTFaxEncode");
        }
        OptimisePreset::Ebook => {
            cmd.arg("-dPDFSETTINGS=/ebook")
                .arg("-dCompatibilityLevel=1.5")
                .arg("-dDownsampleColorImages=true")
                .arg("-dColorImageResolution=150")
                .arg("-dDownsampleGrayImages=true")
                .arg("-dGrayImageResolution=150");
        }
        OptimisePreset::Printer => {
            cmd.arg("-dPDFSETTINGS=/printer")
                .arg("-dCompatibilityLevel=1.6")
                .arg("-dColorImageResolution=300")
                .arg("-dGrayImageResolution=300");
        }
    }

    // Input file (must be last)
    cmd.arg(input_path.display());

    tracing::info!(
        input = %input_path.display(),
        output = %output_path.display(),
        preset = ?preset,
        "Running Ghostscript optimisation"
    );

    let output = cmd.output().map_err(|e| {
        EngineError::Internal(format!("Failed to execute Ghostscript: {e}"))
    })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        tracing::error!(stderr = %stderr, "Ghostscript optimisation failed");
        return Err(EngineError::Internal(format!(
            "Ghostscript optimisation failed: {}",
            stderr
        )));
    }

    Ok(())
}

fn optimise_with_qpdf(
    input_path: &Path,
    output_path: &Path,
    _preset: OptimisePreset,
) -> EngineResult<()> {
    let mut cmd = Command::new("qpdf");

    cmd.arg("--linearize")
        .arg("--object-streams=generate")
        .arg("--compress-streams=y");

    cmd.arg(input_path.display()).arg(output_path.display());

    tracing::info!(
        input = %input_path.display(),
        output = %output_path.display(),
        "Running qpdf optimisation"
    );

    let output = cmd.output().map_err(|e| {
        EngineError::Internal(format!("Failed to execute qpdf: {e}"))
    })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        tracing::error!(stderr = %stderr, "qpdf optimisation failed");
        return Err(EngineError::Internal(format!(
            "qpdf optimisation failed: {}",
            stderr
        )));
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preset_from_str() {
        assert_eq!(
            OptimisePreset::from_str("screen"),
            Some(OptimisePreset::Screen)
        );
        assert_eq!(
            OptimisePreset::from_str("ebook"),
            Some(OptimisePreset::Ebook)
        );
        assert_eq!(
            OptimisePreset::from_str("printer"),
            Some(OptimisePreset::Printer)
        );
        assert_eq!(
            OptimisePreset::from_str("SCREEN"),
            Some(OptimisePreset::Screen)
        );
        assert_eq!(OptimisePreset::from_str("invalid"), None);
    }

    #[test]
    fn preset_ghostscript_settings() {
        assert_eq!(OptimisePreset::Screen.ghostscript_settings(), "/screen");
        assert_eq!(OptimisePreset::Ebook.ghostscript_settings(), "/ebook");
        assert_eq!(OptimisePreset::Printer.ghostscript_settings(), "/printer");
    }

    #[test]
    fn optimise_result_calculations() {
        let result = OptimiseResult {
            data: vec![0; 500],
            original_size: 1000,
            optimised_size: 500,
            backend: OptimiseBackend::Ghostscript,
            preset: OptimisePreset::Screen,
            duration_ms: 100,
        };

        assert_eq!(result.compression_ratio(), 0.5);
        assert_eq!(result.reduction_percent(), 50.0);
    }

    #[test]
    fn optimise_result_zero_size() {
        let result = OptimiseResult {
            data: vec![],
            original_size: 0,
            optimised_size: 0,
            backend: OptimiseBackend::Qpdf,
            preset: OptimisePreset::Ebook,
            duration_ms: 0,
        };

        assert_eq!(result.compression_ratio(), 0.0);
    }

    #[test]
    fn backend_priority_order() {
        let order = OptimiseBackend::priority_order();
        assert_eq!(order.len(), 2);
        assert_eq!(order[0], OptimiseBackend::Ghostscript);
        assert_eq!(order[1], OptimiseBackend::Qpdf);
    }
}
