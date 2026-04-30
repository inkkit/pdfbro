#![cfg(feature = "chromium")]

//! Router-level integration tests (no real Chrome required).
//!
//! These tests drive the full router via `tower::ServiceExt::oneshot`
//! against a mock [`PdfBackend`]. They cover the wire contract for the
//! chromium routes (success, missing-field, mapped engine errors), plus
//! the cross-cutting middleware (body limit, 404).

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use async_trait::async_trait;
use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use engine::{EngineError, EngineResult, PdfOptions, RequestContext};
use serde_json::Value;
use server::backend::PdfBackend;
use server::config::{LogFormat, ServerConfig};
use server::{AppState, build_router};
use tower::ServiceExt;

/// Multipart boundary used by the helper builder.
const BOUNDARY: &str = "----folio-test-boundary";

enum BackendBehavior {
    Pdf(Vec<u8>),
    Error(EngineError),
}

struct MockBackend {
    behavior: BackendBehavior,
    calls: AtomicUsize,
    healthy: bool,
}

impl MockBackend {
    fn pdf(bytes: Vec<u8>) -> Self {
        Self {
            behavior: BackendBehavior::Pdf(bytes),
            calls: AtomicUsize::new(0),
            healthy: true,
        }
    }

    fn err(err: EngineError) -> Self {
        Self {
            behavior: BackendBehavior::Error(err),
            calls: AtomicUsize::new(0),
            healthy: true,
        }
    }

    fn record(&self) {
        self.calls.fetch_add(1, Ordering::Relaxed);
    }

    fn produce(&self) -> EngineResult<Vec<u8>> {
        match &self.behavior {
            BackendBehavior::Pdf(b) => Ok(b.clone()),
            BackendBehavior::Error(e) => Err(clone_engine_error(e)),
        }
    }
}

#[async_trait]
impl PdfBackend for MockBackend {
    async fn html_to_pdf(
        &self,
        _html: &str,
        _base_url: Option<&str>,
        _opts: &PdfOptions,
        _ctx: &RequestContext,
    ) -> EngineResult<Vec<u8>> {
        self.record();
        self.produce()
    }

    async fn url_to_pdf(
        &self,
        _url: &str,
        _opts: &PdfOptions,
        _ctx: &RequestContext,
    ) -> EngineResult<Vec<u8>> {
        self.record();
        self.produce()
    }

    async fn markdown_to_pdf(
        &self,
        _markdown: &str,
        _opts: &PdfOptions,
        _ctx: &RequestContext,
    ) -> EngineResult<Vec<u8>> {
        self.record();
        self.produce()
    }

    async fn healthy(&self) -> bool {
        self.healthy
    }

    async fn html_to_screenshot(
        &self,
        _html: &str,
        _opts: &engine::ScreenshotOptions,
    ) -> EngineResult<Vec<u8>> {
        self.record();
        self.produce()
    }

    async fn url_to_screenshot(
        &self,
        _url: &str,
        _opts: &engine::ScreenshotOptions,
    ) -> EngineResult<Vec<u8>> {
        self.record();
        self.produce()
    }
}

fn clone_engine_error(e: &EngineError) -> EngineError {
    match e {
        EngineError::InvalidOption(s) => EngineError::InvalidOption(s.clone()),
        EngineError::InvalidPageRange(s) => EngineError::InvalidPageRange(s.clone()),
        EngineError::ChromeNotFound { searched } => EngineError::ChromeNotFound {
            searched: searched.clone(),
        },
        EngineError::ChromeLaunch(s) => EngineError::ChromeLaunch(s.clone()),
        EngineError::Cdp(s) => EngineError::Cdp(s.clone()),
        EngineError::Navigation { url, reason } => EngineError::Navigation {
            url: url.clone(),
            reason: reason.clone(),
        },
        EngineError::Timeout(d) => EngineError::Timeout(*d),
        EngineError::Io(e) => EngineError::Io(std::io::Error::new(e.kind(), e.to_string())),
        EngineError::Internal(s) => EngineError::Internal(s.clone()),
        EngineError::Pdf(s) => EngineError::Pdf(s.clone()),
    }
}

fn test_config() -> ServerConfig {
    ServerConfig {
        host: "127.0.0.1".parse().unwrap(),
        port: 0,
        concurrency: 4,
        max_body_bytes: 4 * 1024,
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

fn build_app(backend: MockBackend) -> axum::Router {
    let config = test_config();
    let state = AppState::new(Some(Arc::new(backend)), config.clone());
    build_router(state, &config)
}

fn multipart_body(parts: &[Part<'_>]) -> Vec<u8> {
    let mut out = Vec::new();
    for p in parts {
        out.extend_from_slice(b"--");
        out.extend_from_slice(BOUNDARY.as_bytes());
        out.extend_from_slice(b"\r\n");
        match p {
            Part::Text { name, value } => {
                out.extend_from_slice(
                    format!("Content-Disposition: form-data; name=\"{name}\"\r\n\r\n").as_bytes(),
                );
                out.extend_from_slice(value.as_bytes());
            }
            Part::File {
                name,
                filename,
                content_type,
                bytes,
            } => {
                out.extend_from_slice(
                    format!(
                        "Content-Disposition: form-data; name=\"{name}\"; filename=\"{filename}\"\r\nContent-Type: {content_type}\r\n\r\n",
                    )
                    .as_bytes(),
                );
                out.extend_from_slice(bytes);
            }
        }
        out.extend_from_slice(b"\r\n");
    }
    out.extend_from_slice(b"--");
    out.extend_from_slice(BOUNDARY.as_bytes());
    out.extend_from_slice(b"--\r\n");
    out
}

enum Part<'a> {
    Text {
        name: &'a str,
        value: &'a str,
    },
    File {
        name: &'a str,
        filename: &'a str,
        content_type: &'a str,
        bytes: &'a [u8],
    },
}

fn multipart_request(uri: &str, body: Vec<u8>) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri(uri)
        .header(
            header::CONTENT_TYPE,
            format!("multipart/form-data; boundary={BOUNDARY}"),
        )
        .body(Body::from(body))
        .unwrap()
}

async fn read_body(resp: axum::response::Response) -> Vec<u8> {
    axum::body::to_bytes(resp.into_body(), 1 << 20)
        .await
        .unwrap()
        .to_vec()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn health_returns_200_when_engines_up() {
    let app = build_app(MockBackend::pdf(b"%PDF-1.7\n".to_vec()));
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body: Value = serde_json::from_slice(&read_body(resp).await).unwrap();
    assert_eq!(body["status"], "up");
    assert_eq!(body["chromium"], "up");
}

#[tokio::test]
async fn version_returns_pkg_version() {
    let app = build_app(MockBackend::pdf(vec![]));
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/version")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = String::from_utf8(read_body(resp).await).unwrap();
    assert_eq!(body, env!("CARGO_PKG_VERSION"));
}

#[tokio::test]
async fn chromium_html_returns_pdf_bytes_on_success() {
    let app = build_app(MockBackend::pdf(b"%PDF-1.7\nfake".to_vec()));
    let body = multipart_body(&[Part::File {
        name: "files",
        filename: "index.html",
        content_type: "text/html",
        bytes: b"<html><body>hi</body></html>",
    }]);
    let resp = app
        .oneshot(multipart_request("/forms/chromium/convert/html", body))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(
        resp.headers()
            .get(header::CONTENT_TYPE)
            .unwrap()
            .to_str()
            .unwrap(),
        "application/pdf"
    );
    let cd = resp
        .headers()
        .get(header::CONTENT_DISPOSITION)
        .unwrap()
        .to_str()
        .unwrap();
    assert!(cd.contains("attachment"));
    assert!(cd.contains("result.pdf"));
    let bytes = read_body(resp).await;
    assert!(bytes.starts_with(b"%PDF-"));
}

#[tokio::test]
async fn chromium_html_400_on_missing_index_html() {
    let app = build_app(MockBackend::pdf(b"%PDF-1.7\n".to_vec()));
    let body = multipart_body(&[Part::Text {
        name: "scale",
        value: "1.0",
    }]);
    let resp = app
        .oneshot(multipart_request("/forms/chromium/convert/html", body))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let json: Value = serde_json::from_slice(&read_body(resp).await).unwrap();
    assert_eq!(json["code"], "MISSING_FILE");
    assert!(json["error"].as_str().unwrap().contains("index.html"));
}

#[tokio::test]
async fn chromium_url_400_on_missing_url_field() {
    let app = build_app(MockBackend::pdf(b"%PDF-1.7\n".to_vec()));
    let body = multipart_body(&[Part::Text {
        name: "scale",
        value: "1.0",
    }]);
    let resp = app
        .oneshot(multipart_request("/forms/chromium/convert/url", body))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let json: Value = serde_json::from_slice(&read_body(resp).await).unwrap();
    assert_eq!(json["code"], "MISSING_FIELD");
}

#[tokio::test]
async fn chromium_html_504_when_backend_returns_timeout() {
    let app = build_app(MockBackend::err(EngineError::Timeout(Duration::from_secs(
        30,
    ))));
    let body = multipart_body(&[Part::File {
        name: "files",
        filename: "index.html",
        content_type: "text/html",
        bytes: b"<html></html>",
    }]);
    let resp = app
        .oneshot(multipart_request("/forms/chromium/convert/html", body))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::GATEWAY_TIMEOUT);
    let json: Value = serde_json::from_slice(&read_body(resp).await).unwrap();
    assert_eq!(json["code"], "TIMEOUT");
}

#[tokio::test]
async fn chromium_html_502_when_backend_returns_navigation_error() {
    let app = build_app(MockBackend::err(EngineError::Navigation {
        url: "https://example.com".to_string(),
        reason: "net::ERR_NAME_NOT_RESOLVED".to_string(),
    }));
    let body = multipart_body(&[Part::File {
        name: "files",
        filename: "index.html",
        content_type: "text/html",
        bytes: b"<html></html>",
    }]);
    let resp = app
        .oneshot(multipart_request("/forms/chromium/convert/html", body))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_GATEWAY);
    let json: Value = serde_json::from_slice(&read_body(resp).await).unwrap();
    assert_eq!(json["code"], "NAVIGATION");
    assert_eq!(json["details"]["url"], "https://example.com");
}

#[tokio::test]
async fn body_too_large_returns_413() {
    // test_config sets max_body_bytes = 4 KiB; send a part well over.
    let app = build_app(MockBackend::pdf(b"%PDF-1.7\n".to_vec()));
    let huge = vec![b'a'; 16 * 1024];
    let body = multipart_body(&[Part::File {
        name: "files",
        filename: "index.html",
        content_type: "text/html",
        bytes: &huge,
    }]);
    let resp = app
        .oneshot(multipart_request("/forms/chromium/convert/html", body))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::PAYLOAD_TOO_LARGE);
}

#[tokio::test]
async fn nonexistent_route_returns_404() {
    let app = build_app(MockBackend::pdf(vec![]));
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/forms/does-not-exist")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn x_request_id_header_echoed() {
    let app = build_app(MockBackend::pdf(b"%PDF-1.7\n".to_vec()));
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/version")
                .header("x-request-id", "deadbeef")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        resp.headers()
            .get("x-request-id")
            .unwrap()
            .to_str()
            .unwrap(),
        "deadbeef"
    );
}
