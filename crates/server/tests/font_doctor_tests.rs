//! Integration tests for Spec 43 — Font Doctor.

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
        batch_storage_path: std::path::PathBuf::from("/tmp/pdfbro-batches"),
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

#[tokio::test]
async fn test_list_fonts_returns_200() {
    let (app, _state) = test_app();

    let request = Request::builder()
        .method("GET")
        .uri("/debug/fonts")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_list_fonts_returns_json() {
    let (app, _state) = test_app();

    let request = Request::builder()
        .method("GET")
        .uri("/debug/fonts")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    
    let content_type = response.headers()
        .get(header::CONTENT_TYPE)
        .unwrap()
        .to_str()
        .unwrap();
    assert!(content_type.contains("application/json"));
}

#[tokio::test]
async fn test_validate_fonts_returns_200_with_html() {
    let (app, _state) = test_app();

    let boundary = "----test-boundary";
    let html_content = r#"<style>body { font-family: 'Arial', sans-serif; }</style><body>Test</body>"#;

    let body = format!(
        "--{}\r\nContent-Disposition: form-data; name=\"html\"\r\n\r\n{}\r\n--{}--\r\n",
        boundary, html_content, boundary
    );

    let request = Request::builder()
        .method("POST")
        .uri("/debug/validate-fonts")
        .header(header::CONTENT_TYPE, format!("multipart/form-data; boundary={}", boundary))
        .body(Body::from(body))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_validate_fonts_returns_400_without_input() {
    let (app, _state) = test_app();

    let boundary = "----test-boundary";
    let body = format!(
        "--{}\r\nContent-Disposition: form-data; name=\"other\"\r\n\r\nvalue\r\n--{}--\r\n",
        boundary, boundary
    );

    let request = Request::builder()
        .method("POST")
        .uri("/debug/validate-fonts")
        .header(header::CONTENT_TYPE, format!("multipart/form-data; boundary={}", boundary))
        .body(Body::from(body))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_diagnose_html_returns_200() {
    let (app, _state) = test_app();

    let boundary = "----test-boundary";
    let html_content = r#"<!DOCTYPE html><html><head><style>body { font-family: 'Arial'; }</style></head><body><h1>Test</h1></body></html>"#;

    let body = format!(
        "--{}\r\nContent-Disposition: form-data; name=\"html\"\r\n\r\n{}\r\n--{}--\r\n",
        boundary, html_content, boundary
    );

    let request = Request::builder()
        .method("POST")
        .uri("/debug/diagnose-html")
        .header(header::CONTENT_TYPE, format!("multipart/form-data; boundary={}", boundary))
        .body(Body::from(body))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_diagnose_html_returns_400_without_html() {
    let (app, _state) = test_app();

    let boundary = "----test-boundary";
    let body = format!(
        "--{}\r\nContent-Disposition: form-data; name=\"other\"\r\n\r\nvalue\r\n--{}--\r\n",
        boundary, boundary
    );

    let request = Request::builder()
        .method("POST")
        .uri("/debug/diagnose-html")
        .header(header::CONTENT_TYPE, format!("multipart/form-data; boundary={}", boundary))
        .body(Body::from(body))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}
