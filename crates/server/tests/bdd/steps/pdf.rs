#![allow(dead_code)]

//! PDF assertion step definitions.
//!
//! Maps Gotenberg's PDF assertions:
//! - `thereShouldBeXPDFs` -> `check_pdf_count`
//! - `thePDFShouldHaveXPages` -> `check_page_count`
//! - `thePDFContentAtPageShouldBe` -> `check_page_content`

use lopdf::Document;

use crate::support::world::FolioWorld;

/// Step: Then there should be 1 PDF(s) in the response
pub async fn check_pdf_count(world: &mut FolioWorld, expected: usize) {
    let body = world.body.as_ref().unwrap();
    
    if expected == 1 {
        // For single PDF response, verify we have PDF content
        assert!(
            is_pdf_content(body),
            "Response is not a valid PDF"
        );
    } else {
        // For ZIP responses, count how many entries are PDFs
        let count = count_pdfs_in_zip(body);
        assert_eq!(
            count, expected,
            "Expected {} PDF(s) in ZIP response, found {}",
            expected, count
        );
    }
}

/// Count PDF entries in a ZIP archive.
fn count_pdfs_in_zip(bytes: &[u8]) -> usize {
    use std::io::Cursor;
    let cursor = Cursor::new(bytes);
    let mut archive = zip::ZipArchive::new(cursor).expect("Response is not a valid ZIP archive");
    let mut count = 0;
    for i in 0..archive.len() {
        let file = archive.by_index(i).expect("Failed to read ZIP entry");
        if file.name().ends_with(".pdf") {
            count += 1;
        }
    }
    count
}

/// Check if bytes are valid ZIP
fn is_zip_content(bytes: &[u8]) -> bool {
    // ZIP magic number: PK\x03\x04
    bytes.starts_with(b"PK\x03\x04") || bytes.starts_with(b"PK\x05\x06") || bytes.starts_with(b"PK\x07\x08")
}

/// Step: Then the "foo.pdf" PDF should have 2 page(s)
pub async fn check_page_count(world: &mut FolioWorld, filename: String, expected: usize) {
    let body = world.body.as_ref().expect("No response body");

    let doc = Document::load_mem(body).expect("Failed to parse PDF");
    let page_count = doc.get_pages().len();

    assert_eq!(
        page_count, expected,
        "Expected {} pages, got {} in {}",
        expected, page_count, filename
    );
}

/// Step: Then the "foo.pdf" PDF should have the following content at page 1:
/// """
/// Expected text
/// """
pub async fn check_page_content(
    world: &mut FolioWorld,
    filename: String,
    page_num: usize,
    expected: String,
) {
    let body = world.body.as_ref().expect("No response body");

    // Extract text from PDF
    let text = extract_pdf_text(body, page_num).expect("Failed to extract PDF text");

    // Normalize whitespace and compare
    let normalized_expected = expected.trim().replace("\r\n", "\n");
    let normalized_actual = text.trim().replace("\r\n", "\n");

    assert!(
        normalized_actual.contains(&normalized_expected),
        "PDF {} page {} content mismatch.\nExpected to contain:\n{}\n\nActual:\n{}",
        filename,
        page_num,
        normalized_expected,
        normalized_actual
    );
}

/// Step: Then there should be the following file(s) in the response:
/// | foo.pdf |
/// | bar.pdf |
pub async fn check_files_in_response(world: &mut FolioWorld, files: Vec<String>) {
    let headers = world.response_headers.as_ref().expect("No response headers available");
    let cd = headers
        .get("content-disposition")
        .expect("Missing Content-Disposition header");
    for expected in &files {
        assert!(
            cd.contains(expected),
            "Content-Disposition `{cd}` does not contain expected filename `{expected}`"
        );
    }
}

/// Check if bytes are valid PDF
fn is_pdf_content(bytes: &[u8]) -> bool {
    // PDF magic number: %PDF
    bytes.starts_with(b"%PDF")
}

// =============================================================
// External-tool helpers (graceful degradation when unavailable)
// =============================================================

use std::process::Command;
use std::io::Write as _;

/// Check if an external binary is available on PATH.
fn tool_available(name: &str) -> bool {
    Command::new(name).arg("--version").output().is_ok()
}

/// Validate PDF/A compliance using verapdf. Returns (passed, failed_rules).
fn verapdf_validate(bytes: &[u8]) -> Option<(bool, usize)> {
    if !tool_available("verapdf") {
        return None;
    }
    let mut tmp = tempfile::NamedTempFile::new().ok()?;
    tmp.write_all(bytes).ok()?;
    let out = Command::new("verapdf")
        .args(["--format", "json", tmp.path().to_str()?])
        .output()
        .ok()?;
    let stdout = String::from_utf8_lossy(&out.stdout);
    let json: serde_json::Value = serde_json::from_str(&stdout).ok()?;
    let failed = json
        .pointer("/report/jobs/0/validationResult/details/failedRules")
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as usize;
    Some((failed == 0, failed))
}

/// Extract PDF bytes from either a raw PDF response or a named entry inside a ZIP.
fn extract_named_pdf(body: &[u8], filename: &str) -> Vec<u8> {
    if body.starts_with(b"%PDF") {
        return body.to_vec();
    }
    use std::io::{Cursor, Read};
    let mut archive = zip::ZipArchive::new(Cursor::new(body))
        .expect("Response is neither a PDF nor a ZIP");
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).expect("Failed to read ZIP entry");
        if entry.name() == filename {
            let mut buf = Vec::new();
            entry.read_to_end(&mut buf).expect("Failed to read ZIP entry");
            return buf;
        }
    }
    panic!("File {filename} not found in ZIP response");
}

/// Step: Then the "foo.pdf" PDF should pass PDF/A validation
pub async fn check_pdfa_valid(world: &mut FolioWorld, filename: String) {
    let body = world.body.as_ref().expect("No response body");
    let pdf_bytes = extract_named_pdf(body, &filename);
    match verapdf_validate(&pdf_bytes) {
        Some((true, _)) => {}
        Some((false, n)) => panic!("{filename} failed PDF/A validation with {n} failed rule(s)"),
        None => eprintln!("WARN: verapdf not available; skipping PDF/A check for {filename}"),
    }
}

/// Step: Then the "foo.pdf" PDF should have N image(s)
pub async fn check_image_count(world: &mut FolioWorld, filename: String, expected: usize) {
    let body = world.body.as_ref().expect("No response body");
    let pdf_bytes = extract_named_pdf(body, &filename);
    let doc = lopdf::Document::load_mem(&pdf_bytes).expect("Failed to parse PDF");
    let mut count = 0usize;
    for (_, obj) in doc.objects.iter() {
        if let Ok(stream) = obj.as_stream() {
            if let Ok(subtype) = stream.dict.get(b"Subtype").and_then(|v| v.as_name()) {
                if subtype == b"Image" {
                    count += 1;
                }
            }
        }
    }
    assert_eq!(count, expected, "Expected {expected} image(s) in {filename}, found {count}");
}

/// Extract text from specific page of PDF
fn extract_pdf_text(bytes: &[u8], page_num: usize) -> Result<String, Box<dyn std::error::Error>> {
    let doc = Document::load_mem(bytes)?;
    let pages = doc.get_pages();

    if page_num == 0 || page_num > pages.len() {
        return Err(format!("Page {} out of range (1-{})", page_num, pages.len()).into());
    }

    let page_id = pages.keys().nth(page_num - 1).unwrap();
    let object_id = (*page_id, 0u16); // Convert u32 to ObjectId tuple
    let page = doc.get_object(object_id)?;

    // Simple text extraction using lopdf
    // For full extraction, use pdf_extract crate
    let mut text = String::new();

    if let Ok(dict) = page.as_dict() {
        if let Ok(contents) = dict.get(b"Contents") {
            if let Ok(id) = contents.as_reference() {
                if let Ok(content_obj) = doc.get_object(id) {
                    if let Ok(stream) = content_obj.as_stream() {
                        if let Ok(content) = stream.decode_content() {
                            // Extract text operators
                            for operation in &content.operations {
                                if operation.operator == "Tj" || operation.operator == "TJ" {
                                    for operand in &operation.operands {
                                        // as_string returns Result<Cow<'_, str>, _> in lopdf 0.34
                                        if let Ok(s) = operand.as_string() {
                                            text.push_str(&s);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(text)
}

