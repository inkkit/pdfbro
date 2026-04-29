# Spec 42 — Smart PDF Optimiser

> Automatically detect and reduce oversized PDFs generated from
> HTML/URL conversions. Solves the #1 complaint: "PDFs 8x larger
> than expected" (Gotenberg issues #521, #1056, #1067).

## Goal

Create an intelligent PDF optimisation system that automatically
detects bloated PDFs and offers one-click compression
with multiple quality presets. This directly addresses the
top user complaint across all PDF generation tools.

## Problem Analysis

### Gotenberg Issues (Real User Quotes)

> "We recently switched from AthenaPDF to Gotenberg... noticed a
> significant increase of file size... broke our integration with
> other tools which enforce a file size limit."
> — Issue #521

> "Generated PDF sizes with v8.x are ~2-3x larger than
> same generated PDF on v7.x... 286kb vs 795kb"
> — Issue #1067

> "With Google web font: 264 KB. With locally installed
> version of that font: 131 KB... Ghostscript can reduce
> even more... 27 MB → 12 MB → 1.1 MB"
> — Issue #521

### Root Causes Identified

| Cause | Impact | Solution |
|------|--------|----------|
| Web fonts embedded in PDF | +200% size | Detect & warn, suggest local install |
| White background paths (Chromium bug) | +50% size | Strip background paths |
| No compression applied | +300% size | Apply Ghostscript/qpdf compression |
| Duplicate images (Chromium bug #1077) | +100% size | Deduplicate images |
| Unused fonts subset not applied | +150% size | Proper font subsetting |

## Scope

**In:**

- `POST /forms/pdfengines/optimise` endpoint
- Auto-detection of bloated PDFs (>5MB threshold)
- Three quality presets: `screen`, `ebook`, `printer`
- Backend selection: Ghostscript (best), qpdf, pdfcpu
- Pre-conversion size estimation endpoint
- Size warning headers in responses
- Image deduplication (Chromium bug #1077)
- Font subsetting verification

**Out:**

- Automatic optimisation without user request (too magic)
- PDF/A compliance breaking (document in spec-22)
- Lossy image compression (separate feature)

## Implementation

### 1. New Endpoint: `POST /forms/pdfengines/optimise`

```rust
// crates/server/src/routes/pdfengines.rs

/// Optimise PDF file size.
pub async fn optimise(
    State(state): State<AppState>,
    mp: Multipart,
) -> ApiResult<impl IntoResponse> {
    let start = Instant::now();
    let form = parse_multipart(mp).await?;

    // Extract options
    let preset = form.get("preset").unwrap_or("screen").to_string();
    let files = extract_files(&form)?;

    if files.len() != 1 {
        return Err(ApiError::InvalidOption(
            "optimise requires exactly one PDF file".into()
        ));
    }

    // Optimise
    let result = state
        .pdfops
        .as_ref()
        .unwrap()
        .optimise(&files[0], &preset)
        .await?;

    let duration = start.elapsed().as_secs_f64();

    // Log optimisation stats
    tracing::info!(
        bytes_in = files[0].len(),
        bytes_out = result.len(),
        ratio = result.len() as f64 / files[0].len() as f64,
        duration_ms = duration * 1000.0,
        "PDF optimised"
    );

    pdf_response(result, "result.pdf")
}
```

### 2. PDF Ops Implementation

```rust
// crates/engine/src/pdfops/optimise.rs

use std::process::{Command, Stdio};

pub struct OptimiseOptions {
    pub preset: OptimisePreset,
    pub backend: OptimiseBackend,
}

#[derive(Debug, Clone, Copy)]
pub enum OptimisePreset {
    Screen,    // Low quality, 72 DPI, heavy compression
    Ebook,     // Medium quality, 150 DPI
    Printer,   // High quality, 300 DPI, light compression
}

#[derive(Debug, Clone, Copy)]
pub enum OptimiseBackend {
    Ghostscript,  // Best compression, slow
    Qpdf,        // Medium compression, fast
    PdfCpu,      // Light compression, fastest
}

impl PdfOps {
    pub async fn optimise(
        &self,
        input: &[u8],
        preset: &str,
    ) -> Result<Vec<u8>, EngineError> {
        let preset = match preset.to_lowercase().as_str() {
            "screen" => OptimisePreset::Screen,
            "ebook" => OptimisePreset::Ebook,
            "printer" => OptimisePreset::Printer,
            _ => return Err(EngineError::InvalidOption(
                format!("Unknown preset: {}, use screen/ebook/printer", preset)
            )),
        };

        // Try backends in order of compression quality
        let backends: Vec<OptimiseBackend> = vec![
            OptimiseBackend::Ghostscript,
            OptimiseBackend::Qpdf,
            OptimiseBackend::PdfCpu,
        ];

        for backend in backends {
            if backend.is_available() {
                tracing::info!(?backend, "Using backend for optimisation");
                return self.optimise_with_backend(input, &preset, backend).await;
            }
        }

        Err(EngineError::Internal(
            "No optimisation backend available (install ghostscript/qpdf/pdfcpu)".into()
        ))
    }

    async fn optimise_with_backend(
        &self,
        input: &[u8],
        preset: &OptimisePreset,
        backend: OptimiseBackend,
    ) -> Result<Vec<u8>, EngineError> {
        match backend {
            OptimiseBackend::Ghostscript => self.optimise_ghostscript(input, preset).await,
            OptimiseBackend::Qpdf => self.optimise_qpdf(input, preset).await,
            OptimiseBackend::PdfCpu => self.optimise_pdfcpu(input, preset).await,
        }
    }

    async fn optimise_ghostscript(
        &self,
        input: &[u8],
        preset: &OptimisePreset,
    ) -> Result<Vec<u8>, EngineError> {
        let preset_args = match preset {
            OptimisePreset::Screen => vec![
                "-dPDFSETTINGS=/screen",
                "-dCompatibilityLevel=1.4",
                "-dDownsampleColorImages=true",
                "-dColorImageResolution=72",
                "-dAutoFilterColorImages=false",
                "-dColorImageFilter=/DCTEncode",
            ],
            OptimisePreset::Ebook => vec![
                "-dPDFSETTINGS=/ebook",
                "-dCompatibilityLevel=1.5",
                "-dDownsampleColorImages=true",
                "-dColorImageResolution=150",
            ],
            OptimisePreset::Printer => vec![
                "-dPDFSETTINGS=/printer",
                "-dCompatibilityLevel=1.6",
                "-dColorImageResolution=300",
            ],
        };

        let mut cmd = Command::new("gs");
        cmd.arg("-sDEVICE=pdfwrite")
            .arg("-dNOPAUSE")
            .arg("-dQUIET")
            .arg(format!("-sOutputFile={}", output_path.display()))
            .args(&preset_args)
            .arg(input_path.display());

        let output = cmd.output()
            .map_err(|e| EngineError::Internal(
                format!("Ghostscript failed: {}", e)
            ))?;

        if !output.status.success() {
            return Err(EngineError::Internal(
                format!("Ghostscript error: {}", String::from_utf8_lossy(&output.stderr))
            ));
        }

        tokio::fs::read(&output_path).await
            .map_err(|e| EngineError::Internal(e.to_string()))
    }
}
```

### 3. Size Estimation Endpoint

```rust
// New endpoint: POST /estimate

pub async fn estimate_size(
    State(state): State<AppState>,
    mp: Multipart,
) -> ApiResult<impl IntoResponse> {
    let form = parse_multipart(mp).await?;

    // Parse the conversion request
    let options = parse_chromium_options(&form)?;

    // Estimate size based on inputs
    let estimate = SizeEstimate {
        estimated_mb: calculate_estimate(&form).await?,
        warnings: vec![],
    };

    // Check for web fonts
    if has_web_fonts(&form) {
        estimate.warnings.push(
            "Uses web fonts - may increase size by 200%".into()
        );
    }

    // Check for images
    if has_large_images(&form) {
        estimate.warnings.push(
            "Contains large images - consider optimisation".into()
        );
    }

    Ok(Json(estimate))
}

#[derive(Serialize)]
struct SizeEstimate {
    estimated_mb: f64,
    warnings: Vec<String>,
}
```

### 4. Response Headers (Size Warnings)

```rust
// Add to all PDF conversion responses

if let Some(ref response) = result {
    let size_mb = response.body().len() as f64 / 1_000_000.0;

    if size_mb > 5.0 {
        response.headers_mut().insert(
            HeaderName::from_static("X-Size-Warning"),
            HeaderValue::from_str(&format!(
                "PDF size {:.1} MB exceeds recommended 5 MB. Consider POST /forms/pdfengines/optimise",
                size_mb
            )).unwrap(),
        );
    }
}
```

## Form Fields

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `files` | file | required | PDF file to optimise |
| `preset` | string | "screen" | Compression preset: screen/ebook/printer |
| `backend` | string | "auto" | Force backend: ghostscript/qpdf/pdfcpu |

## Expected Behaviour

### Optimise Endpoint

1. Accept PDF file + preset
2. Detect best available backend (Ghostscript > qpdf > pdfcpu)
3. Apply compression based on preset
4. Return optimised PDF
5. Include compression stats in response headers

### Size Estimation

1. Accept same form data as conversion endpoints
2. Analyse inputs (HTML, CSS, images, fonts)
3. Return estimated output size
4. Warn about web fonts, large images

### Response Headers

```
X-Original-Size: 10240  (10 MB)
X-Optimised-Size: 2048   (2 MB)
X-Compression-Ratio: 20%      (80% reduction)
X-Warnings: Uses web fonts
```

## Test Plan

### Unit Tests

- `optimise_ghostscript_screen_preset`
- `optimise_qpdf_fallback_when_ghostscript_missing`
- `estimate_size_with_web_fonts`
- `parse_preset_from_form`

### Integration Tests

- `optimise_10mb_pdf_to_2mb` - Real compression
- `optimise_presets_produce_different_sizes`
- `estimate_warns_about_web_fonts`
- `response_header_includes_size_warning`

### Performance Tests

- `optimise_100mb_pdf_completes_in_30s`

## Acceptance

- [ ] `POST /forms/pdfengines/optimise` endpoint
- [ ] Three presets: screen/ebook/printer
- [ ] Auto backend selection (Ghostscript first)
- [ ] `POST /estimate` endpoint for size estimation
- [ ] Response headers with size warnings
- [ ] Unit tests for all functions
- [ ] Integration tests with real PDFs
- [ ] `cargo clippy -p engine -- -D warnings` clean

## References

- Gotenberg issue #521: https://github.com/gotenberg/gotenberg/issues/521
- Gotenberg issue #1056: https://github.com/gotenberg/gotenberg/issues/1056
- Ghostscript documentation: https://www.ghostscript.com/doc/9.56.1/Use.htm
- qpdf documentation: https://qpdf.readthedocs.io/
