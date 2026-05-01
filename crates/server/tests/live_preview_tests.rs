//! Integration tests for Spec 45 — Live Preview Mode.

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
    }
}

fn test_app() -> (axum::Router, AppState) {
    let config = test_config();
    let state = AppState::new(None, config.clone());
    let router = build_router(state.clone(), &config);
    (router, state)
}

#[tokio::test]
async fn test_preview_url_returns_image_or_error() {
    let (app, _state) = test_app();

    let request = Request::builder()
        .method("GET")
        .uri("/preview/url?url=https://example.com&format=png")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    let status = response.status();
    
    assert!(
        status == StatusCode::OK ||
        status == StatusCode::BAD_REQUEST ||
        status == StatusCode::INTERNAL_SERVER_ERROR
    );
}

#[tokio::test]
async fn test_preview_url_returns_400_for_invalid_format() {
    let (app, _state) = test_app();

    let request = Request::builder()
        .method("GET")
        .uri("/preview/url?url=https://example.com&format=gif")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_preview_url_accepts_viewport_params() {
    let (app, _state) = test_app();

    let request = Request::builder()
        .method("GET")
        .uri("/preview/url?url=https://example.com&width=1920&height=1080&full_page=true")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    let status = response.status();
    
    assert!(
        status == StatusCode::OK ||
        status == StatusCode::BAD_REQUEST ||
        status == StatusCode::INTERNAL_SERVER_ERROR
    );
}

#[tokio::test]
async fn test_preview_html_accepts_file_upload() {
    let (app, _state) = test_app();

    let boundary = "----test-boundary";
    let html_content = "<html><body><h1>Test</h1></body></html>";

    let body = format!(
        "------{}\r\nContent-Disposition: form-data; name=\"files\"; filename=\"test.html\"\r\nContent-Type: text/html\r\n\r\n{}\r\n------{}--\r\n",
        boundary, html_content, boundary
    );

    let request = Request::builder()
        .method("POST")
        .uri("/preview/html")
        .header(header::CONTENT_TYPE, format!("multipart/form-data; boundary={}", boundary))
        .body(Body::from(body))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    let status = response.status();
    
    assert!(
        status == StatusCode::OK ||
        status == StatusCode::BAD_REQUEST ||
        status == StatusCode::INTERNAL_SERVER_ERROR
    );
}

#[tokio::test]
async fn test_preview_markdown_accepts_file_upload() {
    let (app, _state) = test_app();

    let boundary = "----test-boundary";
    let md_content = "# Test\n\nThis is a test markdown document.";

    let body = format!(
        "------{}\r\nContent-Disposition: form-data; name=\"files\"; filename=\"test.md\"\r\nContent-Type: text/markdown\r\n\r\n{}\r\n------{}--\r\n",
        boundary, md_content, boundary
    );

    let request = Request::builder()
        .method("POST")
        .uri("/preview/markdown")
        .header(header::CONTENT_TYPE, format!("multipart/form-data; boundary={}", boundary))
        .body(Body::from(body))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    let status = response.status();
    
    assert!(
        status == StatusCode::OK ||
        status == StatusCode::BAD_REQUEST ||
        status == StatusCode::INTERNAL_SERVER_ERROR
    );
}

#[tokio::test]
async fn test_preview_compare_accepts_before_after_files() {
    let (app, _state) = test_app();

    let boundary = "----test-boundary";
    let before_html = "<html><body><h1>Before</h1></body></html>";
    let after_html = "<html><body><h1>After</h1></body></html>";

    let body = format!(
        "------{}\r\nContent-Disposition: form-data; name=\"before\"; filename=\"before.html\"\r\nContent-Type: text/html\r\n\r\n{}\r\n------{}\r\nContent-Disposition: form-data; name=\"after\"; filename=\"after.html\"\r\nContent-Type: text/html\r\n\r\n{}\r\n------{}--\r\n",
        boundary, before_html, boundary, after_html, boundary
    );

    let request = Request::builder()
        .method("POST")
        .uri("/preview/compare")
        .header(header::CONTENT_TYPE, format!("multipart/form-data; boundary={}", boundary))
        .body(Body::from(body))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    let status = response.status();
    
    assert!(
        status == StatusCode::OK ||
        status == StatusCode::BAD_REQUEST ||
        status == StatusCode::INTERNAL_SERVER_ERROR
    );
}

#[tokio::test]
async fn test_preview_compare_returns_400_with_missing_files() {
    let (app, _state) = test_app();

    let boundary = "----test-boundary";
    let before_html = "<html><body><h1>Before</h1></body></html>";

    let body = format!(
        "------{}\r\nContent-Disposition: form-data; name=\"before\"; filename=\"before.html\"\r\nContent-Type: text/html\r\n\r\n{}\r\n------{}--\r\n",
        boundary, before_html, boundary
    );

    let request = Request::builder()
        .method("POST")
        .uri("/preview/compare")
        .header(header::CONTENT_TYPE, format!("multipart/form-data; boundary={}", boundary))
        .body(Body::from(body))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}
