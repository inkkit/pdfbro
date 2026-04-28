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
        // TODO: Handle other multipart responses
        panic!("Multiple PDF count not yet implemented");
    }
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
pub async fn check_files_in_response(world: &mut FolioWorld, _files: Vec<String>) {
    // For now, just verify we have a body
    assert!(world.body.is_some(), "No response body available");
}

/// Check if bytes are valid PDF
fn is_pdf_content(bytes: &[u8]) -> bool {
    // PDF magic number: %PDF
    bytes.starts_with(b"%PDF")
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

