//! Integration tests for Scalar API Documentation.

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
async fn test_openapi_spec_returns_200() {
    let (app, _state) = test_app();

    let request = Request::builder()
        .method("GET")
        .uri("/openapi.json")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_openapi_spec_returns_json() {
    let (app, _state) = test_app();

    let request = Request::builder()
        .method("GET")
        .uri("/openapi.json")
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
async fn test_docs_endpoint_returns_200() {
    let (app, _state) = test_app();

    let request = Request::builder()
        .method("GET")
        .uri("/docs")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_docs_endpoint_returns_html() {
    let (app, _state) = test_app();

    let request = Request::builder()
        .method("GET")
        .uri("/docs")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    
    let content_type = response.headers()
        .get(header::CONTENT_TYPE)
        .unwrap()
        .to_str()
        .unwrap();
    assert!(content_type.contains("text/html"));
}
