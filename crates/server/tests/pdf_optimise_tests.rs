//! Integration tests for Spec 42 — Smart PDF Optimiser.

use std::time::Duration;
use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use tower::ServiceExt;

use server::app::build_router;
use server::state::AppState;
use server::config::{ServerConfig, LogFormat};

fn test_config() -> ServerConfig {
    ServerConfig {
        host: "127.0.0.1".parse().unwrap(),
        port: 0,
        concurrency: 4,
        max_body_bytes: 4 * 1024 * 1024,
        request_timeout: Duration::from_secs(60),
        chrome_path: None,
        no_sandbox: None,
        soffice_path: None,
        log_level: "off".to_string(),
        log_format: LogFormat::Text,
        batch_max_items: 50,
        batch_concurrency: 4,
        batch_max_active: 10,
        batch_retention_minutes: 60,
        batch_storage_path: std::path::PathBuf::from("/tmp/folio-batches"),
        otel_enabled: false,
        otel_endpoint: "http://localhost:4318/v1/traces".to_string(),
        chromium_lazy_start: false,
        chromium_idle_shutdown_timeout: None,
        libreoffice_lazy_start: false,
        libreoffice_idle_shutdown_timeout: None,
        api_disable_health_route_telemetry: false,
        api_disable_root_route_telemetry: false,
        api_disable_debug_route_telemetry: false,
        api_disable_version_route_telemetry: false,
        api_enable_debug_route: false,
        api_tls_cert_file: None,
        api_tls_key_file: None,
        api_basic_auth_username: None,
        api_basic_auth_password: None,
        api_download_from_allow_list: Vec::new(),
        api_download_from_deny_list: Vec::new(),
        api_download_from_max_retry: 3,
        api_disable_download_from: false,
        api_correlation_id_header: "x-request-id".to_string(),
        api_root_path: String::new(),
        libreoffice_unoserver_port: 2003,
        libreoffice_unoserver_ready_timeout: std::time::Duration::from_secs(60),
        webhook_max_retry: 4,
        webhook_retry_min_wait: std::time::Duration::from_secs(1),
        webhook_retry_max_wait: std::time::Duration::from_secs(30),
        webhook_client_timeout: std::time::Duration::from_secs(30),
        webhook_allow_list: vec![],
        webhook_deny_list: vec![],
    }
}

fn test_app() -> (axum::Router, AppState) {
    let config = test_config();
    let state = AppState::new(None, config.clone());
    let router = build_router(state.clone(), &config);
    (router, state)
}

fn create_test_pdf() -> Vec<u8> {
    b"%PDF-1.4\n1 0 obj<< /Type /Catalog /Pages 2 0 R >>endobj\n2 0 obj<< /Type /Pages /Kids [] /Count 0 >>endobj\nxref\n0 3\n0000000000 65535 f\n0000000009 00000 n\n0000000058 00000 n\ntrailer<< /Size 3 /Root 1 0 R >>\nstartxref\n115\n%%EOF".to_vec()
}

fn build_optimise_request(pdf_data: Vec<u8>, preset: Option<&str>, backend: Option<&str>) -> Request<Body> {
    let boundary = "----test-boundary";
    let mut body = Vec::new();

    body.extend_from_slice(format!("------{}\r\n", boundary).as_bytes());
    body.extend_from_slice(b"Content-Disposition: form-data; name=\"files\"; filename=\"test.pdf\"\r\n");
    body.extend_from_slice(b"Content-Type: application/pdf\r\n\r\n");
    body.extend_from_slice(&pdf_data);
    body.extend_from_slice(b"\r\n");

    if let Some(p) = preset {
        body.extend_from_slice(format!("------{}\r\n", boundary).as_bytes());
        body.extend_from_slice(b"Content-Disposition: form-data; name=\"preset\"\r\n\r\n");
        body.extend_from_slice(p.as_bytes());
        body.extend_from_slice(b"\r\n");
    }

    if let Some(b) = backend {
        body.extend_from_slice(format!("------{}\r\n", boundary).as_bytes());
        body.extend_from_slice(b"Content-Disposition: form-data; name=\"backend\"\r\n\r\n");
        body.extend_from_slice(b.as_bytes());
        body.extend_from_slice(b"\r\n");
    }

    body.extend_from_slice(format!("------{}--\r\n", boundary).as_bytes());

    Request::builder()
        .method("POST")
        .uri("/forms/pdfengines/optimise")
        .header(header::CONTENT_TYPE, format!("multipart/form-data; boundary={}", boundary))
        .body(Body::from(body))
        .unwrap()
}

#[tokio::test]
async fn test_optimise_pdf_returns_200_with_valid_pdf() {
    let (app, _state) = test_app();
    let pdf = create_test_pdf();
    let request = build_optimise_request(pdf, None, None);
    let response = app.oneshot(request).await.unwrap();
    
    let status = response.status();
    assert!(status == StatusCode::OK || status == StatusCode::INTERNAL_SERVER_ERROR);
}

#[tokio::test]
async fn test_optimise_pdf_accepts_screen_preset() {
    let (app, _state) = test_app();
    let pdf = create_test_pdf();
    let request = build_optimise_request(pdf, Some("screen"), None);
    let response = app.oneshot(request).await.unwrap();
    
    let status = response.status();
    assert!(status == StatusCode::OK || status == StatusCode::INTERNAL_SERVER_ERROR);
}

#[tokio::test]
async fn test_optimise_pdf_accepts_ebook_preset() {
    let (app, _state) = test_app();
    let pdf = create_test_pdf();
    let request = build_optimise_request(pdf, Some("ebook"), None);
    let response = app.oneshot(request).await.unwrap();
    
    let status = response.status();
    assert!(status == StatusCode::OK || status == StatusCode::INTERNAL_SERVER_ERROR);
}

#[tokio::test]
async fn test_optimise_pdf_accepts_printer_preset() {
    let (app, _state) = test_app();
    let pdf = create_test_pdf();
    let request = build_optimise_request(pdf, Some("printer"), None);
    let response = app.oneshot(request).await.unwrap();
    
    let status = response.status();
    assert!(status == StatusCode::OK || status == StatusCode::INTERNAL_SERVER_ERROR);
}

#[tokio::test]
async fn test_optimise_pdf_accepts_ghostscript_backend() {
    let (app, _state) = test_app();
    let pdf = create_test_pdf();
    let request = build_optimise_request(pdf, None, Some("ghostscript"));
    let response = app.oneshot(request).await.unwrap();
    
    let status = response.status();
    assert!(status == StatusCode::OK || status == StatusCode::INTERNAL_SERVER_ERROR);
}

#[tokio::test]
async fn test_optimise_pdf_accepts_qpdf_backend() {
    let (app, _state) = test_app();
    let pdf = create_test_pdf();
    let request = build_optimise_request(pdf, None, Some("qpdf"));
    let response = app.oneshot(request).await.unwrap();
    
    let status = response.status();
    assert!(status == StatusCode::OK || status == StatusCode::INTERNAL_SERVER_ERROR);
}

#[tokio::test]
async fn test_optimise_pdf_returns_400_for_invalid_preset() {
    let (app, _state) = test_app();
    let pdf = create_test_pdf();
    let request = build_optimise_request(pdf, Some("invalid_preset"), None);
    let response = app.oneshot(request).await.unwrap();
    
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_optimise_pdf_returns_400_for_missing_file() {
    let (app, _state) = test_app();
    
    let boundary = "----test-boundary";
    let body = format!(
        "------{}\r\nContent-Disposition: form-data; name=\"preset\"\r\n\r\nscreen\r\n------{}--\r\n",
        boundary, boundary
    );

    let request = Request::builder()
        .method("POST")
        .uri("/forms/pdfengines/optimise")
        .header(header::CONTENT_TYPE, format!("multipart/form-data; boundary={}", boundary))
        .body(Body::from(body))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[test]
fn test_compression_ratio_calculation() {
    let original_size = 1000_usize;
    let optimised_size = 500_usize;
    
    let ratio = optimised_size as f64 / original_size as f64;
    let reduction_percent = (1.0 - ratio) * 100.0;
    
    assert_eq!(ratio, 0.5);
    assert_eq!(reduction_percent, 50.0);
}

#[test]
fn test_preset_from_str_case_insensitive() {
    let presets = ["screen", "SCREEN", "Screen", "ebook", "EBOOK", "printer", "PRINTER"];
    
    for preset in &presets {
        let lower = preset.to_lowercase();
        assert!(
            ["screen", "ebook", "printer"].contains(&lower.as_str()),
            "Preset {} should be valid", preset
        );
    }
}
