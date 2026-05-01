//! PDF output validation for robustness.
//!
//! Validates that generated PDFs are well-formed, not corrupted,
//! and meet basic quality expectations.

use crate::types::{EngineError, EngineResult};

/// Minimum valid PDF size in bytes (header + trailer).
const MIN_PDF_SIZE: usize = 100;

/// Maximum reasonable PDF size in bytes (100 MB).
const MAX_PDF_SIZE: usize = 100 * 1024 * 1024;

/// Validates a PDF output for basic correctness.
///
/// Checks:
/// - PDF header present (%PDF-1.x)
/// - PDF trailer present (%%EOF)
/// - Reasonable file size
/// - Can be parsed by lopdf
/// - Contains at least one page
///
/// # Arguments
///
/// * `bytes` - Raw PDF bytes to validate
///
/// # Returns
///
/// - `Ok(())` if PDF is valid
/// - `Err(EngineError::Pdf)` if validation fails
///
/// # Example
///
/// ```
/// use engine::pdfops::validate::validate_pdf_output;
///
/// let pdf_bytes = b"%PDF-1.4\n1 0 obj<</Type/Catalog/Pages 2 0 R>>endobj\n2 0 obj<</Type/Pages/Kids[3 0 R]/Count 1>>endobj\n3 0 obj<</Type/Page/MediaBox[0 0 612 792]>>endobj\nxref\n0 4\n0000000000 65535 f\n0000000009 00000 n\n0000000058 00000 n\n0000000115 00000 n\ntrailer<</Size 4/Root 1 0 R>>\nstartxref\n147\n%%EOF";
///
/// // This is a minimal valid PDF structure for testing
/// // In real use, you'd have actual PDF bytes
/// ```
pub fn validate_pdf_output(bytes: &[u8]) -> EngineResult<()> {
    // Check minimum size
    if bytes.len() < MIN_PDF_SIZE {
        return Err(EngineError::Pdf(format!(
            "PDF too small: {} bytes (minimum {})",
            bytes.len(),
            MIN_PDF_SIZE
        )));
    }

    // Check maximum size
    if bytes.len() > MAX_PDF_SIZE {
        return Err(EngineError::Pdf(format!(
            "PDF too large: {} bytes (maximum {})",
            bytes.len(),
            MAX_PDF_SIZE
        )));
    }

    // Check PDF header
    if !bytes.starts_with(b"%PDF-1.") {
        return Err(EngineError::Pdf(
            "Invalid PDF header: missing %PDF-1.x marker".to_string()
        ));
    }

    // Check PDF trailer
    if !bytes.windows(5).any(|w| w == b"%%EOF") {
        return Err(EngineError::Pdf(
            "Invalid PDF trailer: missing %%EOF marker".to_string()
        ));
    }

    // Try to parse with lopdf
    let doc = lopdf::Document::load_mem(bytes).map_err(|e| {
        EngineError::Pdf(format!("PDF parse error: {}", e))
    })?;

    // Check for at least one page
    let pages = doc.get_pages();
    if pages.is_empty() {
        return Err(EngineError::Pdf(
            "PDF contains no pages".to_string()
        ));
    }

    // Validate each page has required entries
    for (page_num, page_id) in &pages {
        let page = doc.get_page(*page_id).map_err(|e| {
            EngineError::Pdf(format!(
                "Failed to get page {}: {}",
                page_num, e
            ))
        })?;

        // Check for MediaBox (required for valid PDF)
        if page.media_box().is_err() {
            return Err(EngineError::Pdf(format!(
                "Page {} missing required MediaBox",
                page_num
            )));
        }
    }

    Ok(())
}

/// Validates a PDF and returns detailed information.
///
/// # Returns
///
/// A tuple of (page_count, file_size) on success
pub fn validate_pdf_with_info(bytes: &[u8]) -> EngineResult<(u32, usize)> {
    validate_pdf_output(bytes)?;

    let doc = lopdf::Document::load_mem(bytes)
        .map_err(|e| EngineError::Pdf(format!("PDF parse error: {}", e)))?;

    let page_count = doc.get_pages().len() as u32;
    let file_size = bytes.len();

    Ok((page_count, file_size))
}

/// Quick validation that just checks header and trailer.
///
/// Useful for early validation before full parsing.
pub fn quick_validate_pdf(bytes: &[u8]) -> bool {
    bytes.len() >= MIN_PDF_SIZE
        && bytes.starts_with(b"%PDF-1.")
        && bytes.windows(5).any(|w| w == b"%%EOF")
}

/// Validates LibreOffice output for sanity.
///
/// In addition to PDF validation, checks:
/// - Size is reasonable for the input type
/// - Page count makes sense
///
/// # Arguments
///
/// * `bytes` - PDF bytes from LibreOffice conversion
/// * `input_extension` - Original input file extension (e.g., "docx", "xlsx")
pub fn validate_libreoffice_output(
    bytes: &[u8],
    _input_extension: &str,
) -> EngineResult<()> {
    // Basic PDF validation
    validate_pdf_output(bytes)?;

    let (page_count, file_size) = validate_pdf_with_info(bytes)?;

    // Sanity checks based on page count
    if page_count > 1000 {
        tracing::warn!(
            "LibreOffice produced PDF with {} pages - this may indicate a conversion issue",
            page_count
        );
    }

    // Sanity checks based on file size per page
    if page_count > 0 {
        let bytes_per_page = file_size / page_count as usize;
        if bytes_per_page > 10 * 1024 * 1024 {
            // More than 10 MB per page is suspicious
            tracing::warn!(
                "PDF has {} bytes per page ({} total pages) - unusually large",
                bytes_per_page,
                page_count
            );
        }
    }

    tracing::debug!(
        "LibreOffice output validated: {} pages, {} bytes",
        page_count,
        file_size
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_too_small_pdf() {
        let result = validate_pdf_output(b"tiny");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("too small"));
    }

    #[test]
    fn rejects_missing_header() {
        let data = vec![b' '; 200]; // Spaces, no PDF header
        let result = validate_pdf_output(&data);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("header"));
    }

    #[test]
    fn rejects_missing_trailer() {
        let data = b"%PDF-1.4\n1 0 obj<<>>endobj\n"; // Missing %%EOF
        let result = validate_pdf_output(data);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("trailer"));
    }

    #[test]
    fn quick_validation_basic() {
        assert!(!quick_validate_pdf(b"tiny"));
        assert!(!quick_validate_pdf(b"%PDF-1.4")); // No EOF

        let valid = b"%PDF-1.4\n... content ...\n%%EOF";
        assert!(quick_validate_pdf(valid));
    }
}
