# Spec 14 — `engine::pdfa`

> PDF/A and PDF/UA conformance conversion via Ghostscript or qpdf.
> Stateless free functions on in-memory PDF byte streams.

## Goal

Provide PDF/A-1b, PDF/A-2b, PDF/A-3b, and PDF/UA conformance conversion
for existing PDF documents. This enables enterprise archival compliance
and accessibility standards.

## Scope

**In:**

- PDF/A-1b, PDF/A-2b, PDF/A-3b conversion (archival compliance).
- PDF/UA-1, PDF/UA-2 conversion (accessibility).
- Validation of output against veraPDF or similar.
- Shell-out to `gs` (Ghostscript) or `qpdf` for actual conversion.
- Server endpoint `/forms/pdfengines/convert` with `pdfa` form field.

**Out:**

- Creating PDF/A from scratch (convert from HTML/Office via Chromium/LibreOffice).
- PDF/A-1a, PDF/A-2a, PDF/A-3a (full conformance with logical structure).
- Repairing malformed PDFs that cannot be parsed.

## Public API

Module path: `engine::pdfa`. Stateless free functions.

```rust
use crate::types::{EngineError, EngineResult};

/// PDF/A conformance levels for archival compliance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PdfAProfile {
    /// PDF/A-1b: Basic conformance (Level B) for PDF 1.4.
    PdfA1b,
    /// PDF/A-2b: Basic conformance (Level B) for PDF 1.7.
    PdfA2b,
    /// PDF/A-3b: Basic conformance (Level B) with embedded files support.
    PdfA3b,
}

/// PDF/UA conformance levels for accessibility.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PdfUaProfile {
    /// PDF/UA-1: Universal Accessibility (ISO 14289-1).
    PdfUa1,
    /// PDF/UA-2: Updated accessibility standard (ISO 14289-2).
    PdfUa2,
}

/// Convert a PDF to PDF/A conformance.
/// 
/// Uses Ghostscript's pdfwrite device with PDF/A settings.
/// Falls back to qpdf if Ghostscript is unavailable.
pub fn convert_to_pdfa(pdf: &[u8], profile: PdfAProfile) -> EngineResult<Vec<u8>>;

/// Convert a PDF to PDF/UA accessibility conformance.
/// 
/// Adds accessibility features and validates logical structure.
pub fn convert_to_pdfua(pdf: &[u8], profile: PdfUaProfile) -> EngineResult<Vec<u8>>;

/// Validate a PDF against a PDF/A or PDF/UA profile.
/// 
/// Returns validation report with passed/failed rules.
/// Requires external tool (veraPDF or qpdf validation).
pub fn validate(pdf: &[u8], profile: PdfAValidationProfile) -> EngineResult<ValidationReport>;

#[derive(Debug, Clone)]
pub struct ValidationReport {
    pub compliant: bool,
    pub profile: String,
    pub failed_rules: Vec<RuleViolation>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct RuleViolation {
    pub rule_id: String,
    pub description: String,
    pub severity: Severity,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
}
```

## Implementation Strategy

### Option 1: Ghostscript (Primary)

Ghostscript's `pdfwrite` device has built-in PDF/A conversion:

```bash
gs -dPDFA=1 -dBATCH -dNOPAUSE -sProcessColorModel=DeviceRGB \
   -sDEVICE=pdfwrite -sPDFACompatibilityPolicy=1 \
   -sOutputFile=output.pdf input.pdf
```

Pros:
- Industry standard, widely tested
- Handles color model conversion
- Built-in font embedding checks

Cons:
- Large dependency (~50MB)
- Slower than pure-Rust alternatives

### Option 2: qpdf (Fallback)

qpdf has limited PDF/A support via `--qpdf` and `--set-pdf-a`:

```bash
qpdf --qpdf --set-pdf-a input.pdf output.pdf
```

Pros:
- Already in our Docker images
- Fast, pure transformation

Cons:
- Limited profile support
- No color model conversion

### Decision

**Primary:** Ghostscript for full PDF/A-1b/2b/3b support
**Fallback:** qpdf for basic compliance marking

## Server API

New endpoint mirroring Gotenberg:

```
POST /forms/pdfengines/convert
```

Form fields:
- `files` - Input PDF file(s)
- `pdfa` - Profile: `PDF/A-1b`, `PDF/A-2b`, `PDF/A-3b`
- `pdfua` - Profile: `PDF/UA-1`, `PDF/UA-2` (mutually exclusive with `pdfa`)

Response:
- Converted PDF with proper `Content-Type: application/pdf`
- `Content-Disposition` with `.pdf` suffix

## Error Handling

| Error | Condition |
|-------|-----------|
| `EngineError::InvalidInput` | Input not a valid PDF |
| `EngineError::ConversionFailed` | Ghostscript/qpdf error |
| `EngineError::ProfileUnsupported` | Profile not available |
| `EngineError::Timeout` | Conversion exceeded limit |

## Testing

Unit tests:
- Convert sample PDFs to each profile
- Verify output opens without error
- Check PDF version header changed appropriately

Integration tests (BDD):
- Gotenberg feature parity: `pdfengines_convert.feature`
- veraPDF validation of output
- Binary size not exploded

## Dependencies

```toml
[dependencies]
# Shell execution
tokio = { version = "1", features = ["process"] }

[dev-dependencies]
# PDF parsing for verification
pdf-extract = "0.8"
```

Runtime requirements:
- `gs` (Ghostscript 9.50+) OR `qpdf` (10.6+)
- veraPDF (optional, for validation testing)

## Open Questions

1. Should we embed Ghostscript in Docker or make it optional?
2. Do we need PDF/A-3b file embedding support?
3. Should validation be a separate endpoint?

## References

- ISO 19005-1 (PDF/A-1)
- ISO 19005-2 (PDF/A-2)
- ISO 19005-3 (PDF/A-3)
- ISO 14289 (PDF/UA)
- Ghostscript PDF/A docs: https://ghostscript.com/doc/VectorDevices.htm#PDFA
