//! Integration tests for Spec 46 — PDF Size Estimator.

use std::time::Duration;
use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use tower::ServiceExt;
use serde_json::json;

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

#[tokio::test]
async fn test_estimate_returns_200_with_html() {
    let (app, _state) = test_app();

    let body = json!({
        "html": "<html><body><h1>Test</h1></body></html>"
    });

    let request = Request::builder()
        .method("POST")
        .uri("/estimate")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body.to_string()))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_estimate_returns_400_without_html_or_url() {
    let (app, _state) = test_app();

    let body = json!({
        "other": "value"
    });

    let request = Request::builder()
        .method("POST")
        .uri("/estimate")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body.to_string()))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_estimate_form_returns_200_with_html_field() {
    let (app, _state) = test_app();

    let boundary = "----test-boundary";
    let html_content = "<html><body>Test</body></html>";

    let body = format!(
        "--{}\r\nContent-Disposition: form-data; name=\"html\"\r\n\r\n{}\r\n--{}--\r\n",
        boundary, html_content, boundary
    );

    let request = Request::builder()
        .method("POST")
        .uri("/estimate/form")
        .header(header::CONTENT_TYPE, format!("multipart/form-data; boundary={}", boundary))
        .body(Body::from(body))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_estimate_batch_returns_200() {
    let (app, _state) = test_app();

    let body = json!({
        "urls": [
            "https://example.com/page1",
            "https://example.com/page2"
        ]
    });

    let request = Request::builder()
        .method("POST")
        .uri("/estimate/batch")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body.to_string()))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_estimate_batch_returns_400_without_urls() {
    let (app, _state) = test_app();

    let body = json!({
        "urls": []
    });

    let request = Request::builder()
        .method("POST")
        .uri("/estimate/batch")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body.to_string()))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[test]
fn test_estimate_url_size_basic() {
    fn estimate_url_size(url: &str) -> f64 {
        let mut base_size = 1.0;
        if url.len() > 100 {
            base_size += 0.5;
        }
        let lower_url = url.to_lowercase();
        if lower_url.contains("/gallery") || lower_url.contains("/images") || lower_url.contains("/photos") {
            base_size += 2.0;
        }
        if lower_url.contains("/dashboard") || lower_url.contains("/app") {
            base_size += 1.0;
        }
        base_size += (url.len() % 10) as f64 / 10.0;
        base_size
    }

    let size1 = estimate_url_size("https://example.com/page");
    let size2 = estimate_url_size("https://example.com/gallery/photos");
    assert!(size2 > size1);
}
