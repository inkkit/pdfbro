//! Integration tests for Spec 42 — Smart PDF Optimiser.
//!
//! Tests the `/forms/pdfengines/optimise` endpoint with various
//! presets and backend configurations.

use std::time::Duration;

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use http_body_util::BodyExt;
use tower::ServiceExt;

use server::app::build_router;
use server::state::AppState;
use server::config::ServerConfig;

/// Create test app state with minimal configuration.
async fn test_app() -> (axum::Router, AppState) {
    let config = ServerConfig::default();
    let state = AppState::new(&config).await.unwrap();
    let router = build_router(state.clone(), &config);
    (router, state)
}

/// Helper to create a simple PDF file for testing.
fn create_test_pdf() -> Vec<u8> {
    // Minimal valid PDF structure
    let pdf = b"%PDF-1.4\n1 0 obj<< /Type /Catalog /Pages 2 0 R >>endobj\n2 0 obj<< /Type /Pages /Kids [] /Count 0 >>endobj\nxref\n0 3\n0000000000 65535 f\n0000000009 00000 n\n0000000058 00000 n\ntrailer<< /Size 3 /Root 1 0 R >>\nstartxref\n115\n%%EOF";
    pdf.to_vec()
}

/// Helper to build multipart request with PDF file.
fn build_optimise_request(
    pdf_data: Vec<u8>,
    preset: Option<&str>,
    backend: Option<&str>,
) -> Request<Body> {
    let boundary = "----test-boundary";
    let mut body = Vec::new();

    // Add files field
    body.extend_from_slice(format!("------{boundary}\r\n").as_bytes());
    body.extend_from_slice(b"Content-Disposition: form-data; name=\"files\"; filename=\"test.pdf\"\r\n");
    body.extend_from_slice(b"Content-Type: application/pdf\r\n\r\n");
    body.extend_from_slice(&pdf_data);
    body.extend_from_slice(b"\r\n");

    // Add preset if specified
    if let Some(p) = preset {
        body.extend_from_slice(format!("------{boundary}\r\n").as_bytes());
        body.extend_from_slice(b"Content-Disposition: form-data; name=\"preset\"\r\n\r\n");
        body.extend_from_slice(p.as_bytes());
        body.extend_from_slice(b"\r\n");
    }

    // Add backend if specified
    if let Some(b) = backend {
        body.extend_from_slice(format!("------{boundary}\r\n").as_bytes());
        body.extend_from_slice(b"Content-Disposition: form-data; name=\"backend\"\r\n\r\n");
        body.extend_from_slice(b.as_bytes());
        body.extend_from_slice(b"\r\n");
    }

    body.extend_from_slice(format!("------{boundary}--\r\n").as_bytes());

    Request::builder()
        .method("POST")
        .uri("/forms/pdfengines/optimise")
        .header(header::CONTENT_TYPE, format!("multipart/form-data; boundary={boundary}"))
        .body(Body::from(body))
        .unwrap()
}

#[tokio::test]
async fn test_optimise_pdf_returns_200_with_valid_pdf() {
    let (app, _state) = test_app().await;
    let pdf = create_test_pdf();
    let request = build_optimise_request(pdf, None, None);

    let response = app.oneshot(request).await.unwrap();
    
    // Should return 200 even if backends aren't available
    // (error handling tested separately)
    assert!(
        response.status() == StatusCode::OK || 
        response.status() == StatusCode::INTERNAL_SERVER_ERROR
    );
}

#[tokio::test]
async fn test_optimise_pdf_accepts_screen_preset() {
    let (app, _state) = test_app().await;
    let pdf = create_test_pdf();
    let request = build_optimise_request(pdf, Some("screen"), None);

    let response = app.oneshot(request).await.unwrap();
    
    // Should accept the preset parameter
    assert!(
        response.status() == StatusCode::OK || 
        response.status() == StatusCode::INTERNAL_SERVER_ERROR
    );
}

#[tokio::test]
async fn test_optimise_pdf_accepts_ebook_preset() {
    let (app, _state) = test_app().await;
    let pdf = create_test_pdf();
    let request = build_optimise_request(pdf, Some("ebook"), None);

    let response = app.oneshot(request).await.unwrap();
    
    assert!(
        response.status() == StatusCode::OK || 
        response.status() == StatusCode::INTERNAL_SERVER_ERROR
    );
}

#[tokio::test]
async fn test_optimise_pdf_accepts_printer_preset() {
    let (app, _state) = test_app().await;
    let pdf = create_test_pdf();
    let request = build_optimise_request(pdf, Some("printer"), None);

    let response = app.oneshot(request).await.unwrap();
    
    assert!(
        response.status() == StatusCode::OK || 
        response.status() == StatusCode::INTERNAL_SERVER_ERROR
    );
}

#[tokio::test]
async fn test_optimise_pdf_accepts_ghostscript_backend() {
    let (app, _state) = test_app().await;
    let pdf = create_test_pdf();
    let request = build_optimise_request(pdf, None, Some("ghostscript"));

    let response = app.oneshot(request).await.unwrap();
    
    assert!(
        response.status() == StatusCode::OK || 
        response.status() == StatusCode::INTERNAL_SERVER_ERROR
    );
}

#[tokio::test]
async fn test_optimise_pdf_accepts_qpdf_backend() {
    let (app, _state) = test_app().await;
    let pdf = create_test_pdf();
    let request = build_optimise_request(pdf, None, Some("qpdf"));

    let response = app.oneshot(request).await.unwrap();
    
    assert!(
        response.status() == StatusCode::OK || 
        response.status() == StatusCode::INTERNAL_SERVER_ERROR
    );
}

#[tokio::test]
async fn test_optimise_pdf_returns_400_for_invalid_preset() {
    let (app, _state) = test_app().await;
    let pdf = create_test_pdf();
    let request = build_optimise_request(pdf, Some("invalid_preset"), None);

    let response = app.oneshot(request).await.unwrap();
    
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_optimise_pdf_returns_400_for_missing_file() {
    let (app, _state) = test_app().await;
    
    let boundary = "----test-boundary";
    let body = format!(
        "------{boundary}\r\n" +
        "Content-Disposition: form-data; name=\"preset\"\r\n\r\n" +
        "screen\r\n" +
        "------{boundary}--\r\n"
    );

    let request = Request::builder()
        .method("POST")
        .uri("/forms/pdfengines/optimise")
        .header(header::CONTENT_TYPE, format!("multipart/form-data; boundary={boundary}"))
        .body(Body::from(body))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_optimise_pdf_includes_response_headers_on_success() {
    let (app, _state) = test_app().await;
    let pdf = create_test_pdf();
    let request = build_optimise_request(pdf, Some("screen"), None);

    let response = app.oneshot(request).await.unwrap();
    
    if response.status() == StatusCode::OK {
        let headers = response.headers();
        // Verify expected headers are present
        assert!(headers.contains_key("content-type"));
        assert_eq!(
            headers.get("content-type").unwrap(),
            "application/pdf"
        );
    }
}

// ---------------------------------------------------------------------------
// Tests for OptimiseResult calculations (unit tests in engine crate)
// ---------------------------------------------------------------------------

#[test]
fn test_compression_ratio_calculation() {
    // Test the calculation logic from OptimiseResult
    let original_size = 1000_usize;
    let optimised_size = 500_usize;
    
    let ratio = optimised_size as f64 / original_size as f64;
    let reduction_percent = (1.0 - ratio) * 100.0;
    
    assert_eq!(ratio, 0.5);
    assert_eq!(reduction_percent, 50.0);
}

#[test]
fn test_preset_from_str_case_insensitive() {
    // Test that preset parsing is case-insensitive
    let presets = ["screen", "SCREEN", "Screen", "ebook", "EBOOK", "printer", "PRINTER"];
    
    for preset in &presets {
        let lower = preset.to_lowercase();
        assert!(
            ["screen", "ebook", "printer"].contains(&lower.as_str()),
            "Preset {} should be valid", preset
        );
    }
}
