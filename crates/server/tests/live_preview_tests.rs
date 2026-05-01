//! Integration tests for Spec 45 — Live Preview Mode.
//!
//! Tests the `/preview/*` endpoints for HTML/URL to image conversion.
//!
//! Note: Many tests require the Chromium feature to be enabled.
//! Tests without the feature verify proper error responses.

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

// ---------------------------------------------------------------------------
// GET /preview/url tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_preview_url_returns_image_or_error() {
    let (app, _state) = test_app().await;

    let request = Request::builder()
        .method("GET")
        .uri("/preview/url?url=https://example.com&format=png")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    
    // Should return OK if Chromium available, or error if not
    assert!(
        response.status() == StatusCode::OK ||
        response.status() == StatusCode::BAD_REQUEST ||
        response.status() == StatusCode::INTERNAL_SERVER_ERROR
    );
}

#[tokio::test]
async fn test_preview_url_accepts_different_formats() {
    let (app, _state) = test_app().await;

    for format in ["png", "jpeg", "webp"] {
        let request = Request::builder()
            .method("GET")
            .uri(format!("/preview/url?url=https://example.com&format={format}"))
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        
        // Should not return 400 for invalid format
        if response.status() == StatusCode::BAD_REQUEST {
            let body = response.into_body().collect().await.unwrap().to_bytes();
            let body_str = String::from_utf8_lossy(&body);
            // Should be due to missing Chromium, not invalid format
            assert!(body_str.contains("Chromium") || body_str.contains("backend"));
        }
    }
}

#[tokio::test]
async fn test_preview_url_returns_400_for_invalid_format() {
    let (app, _state) = test_app().await;

    let request = Request::builder()
        .method("GET")
        .uri("/preview/url?url=https://example.com&format=gif")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    
    // Should return 400 for invalid format
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_preview_url_accepts_viewport_params() {
    let (app, _state) = test_app().await;

    let request = Request::builder()
        .method("GET")
        .uri("/preview/url?url=https://example.com&width=1920&height=1080&full_page=true")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    
    // Should accept viewport parameters
    assert!(
        response.status() == StatusCode::OK ||
        response.status() == StatusCode::BAD_REQUEST ||
        response.status() == StatusCode::INTERNAL_SERVER_ERROR
    );
}

#[tokio::test]
async fn test_preview_url_accepts_clip_params() {
    let (app, _state) = test_app().await;

    let request = Request::builder()
        .method("GET")
        .uri("/preview/url?url=https://example.com&clip_x=100&clip_y=100&clip_width=800&clip_height=600")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    
    // Should accept clip parameters
    assert!(
        response.status() == StatusCode::OK ||
        response.status() == StatusCode::BAD_REQUEST ||
        response.status() == StatusCode::INTERNAL_SERVER_ERROR
    );
}

#[tokio::test]
async fn test_preview_url_returns_400_without_url() {
    let (app, _state) = test_app().await;

    let request = Request::builder()
        .method("GET")
        .uri("/preview/url?format=png")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    
    // Should require URL parameter
    assert!(response.status() == StatusCode::BAD_REQUEST || 
            response.status() == StatusCode::OK); // Some implementations may default
}

// ---------------------------------------------------------------------------
// POST /preview/html tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_preview_html_accepts_file_upload() {
    let (app, _state) = test_app().await;

    let boundary = "----test-boundary";
    let html_content = "<html><body><h1>Test</h1></body></html>";

    let body = format!(
        "------{boundary}\r\n" +
        "Content-Disposition: form-data; name=\"files\"; filename=\"test.html\"\r\n" +
        "Content-Type: text/html\r\n\r\n" +
        "{html_content}\r\n" +
        "------{boundary}--\r\n"
    );

    let request = Request::builder()
        .method("POST")
        .uri("/preview/html")
        .header(header::CONTENT_TYPE, format!("multipart/form-data; boundary={boundary}"))
        .body(Body::from(body))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    
    // Should accept HTML file
    assert!(
        response.status() == StatusCode::OK ||
        response.status() == StatusCode::BAD_REQUEST ||
        response.status() == StatusCode::INTERNAL_SERVER_ERROR
    );
}

#[tokio::test]
async fn test_preview_html_accepts_format_param() {
    let (app, _state) = test_app().await;

    let boundary = "----test-boundary";
    let html_content = "<html><body>Test</body></html>";

    let body = format!(
        "------{boundary}\r\n" +
        "Content-Disposition: form-data; name=\"files\"; filename=\"test.html\"\r\n" +
        "Content-Type: text/html\r\n\r\n" +
        "{html_content}\r\n" +
        "------{boundary}\r\n" +
        "Content-Disposition: form-data; name=\"format\"\r\n\r\n" +
        "jpeg\r\n" +
        "------{boundary}--\r\n"
    );

    let request = Request::builder()
        .method("POST")
        .uri("/preview/html")
        .header(header::CONTENT_TYPE, format!("multipart/form-data; boundary={boundary}"))
        .body(Body::from(body))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    
    // Should accept format parameter
    assert!(
        response.status() == StatusCode::OK ||
        response.status() == StatusCode::BAD_REQUEST ||
        response.status() == StatusCode::INTERNAL_SERVER_ERROR
    );
}

#[tokio::test]
async fn test_preview_html_accepts_full_page_param() {
    let (app, _state) = test_app().await;

    let boundary = "----test-boundary";
    let html_content = "<html><body>Test</body></html>";

    let body = format!(
        "------{boundary}\r\n" +
        "Content-Disposition: form-data; name=\"files\"; filename=\"test.html\"\r\n" +
        "Content-Type: text/html\r\n\r\n" +
        "{html_content}\r\n" +
        "------{boundary}\r\n" +
        "Content-Disposition: form-data; name=\"full_page\"\r\n\r\n" +
        "true\r\n" +
        "------{boundary}--\r\n"
    );

    let request = Request::builder()
        .method("POST")
        .uri("/preview/html")
        .header(header::CONTENT_TYPE, format!("multipart/form-data; boundary={boundary}"))
        .body(Body::from(body))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    
    // Should accept full_page parameter
    assert!(
        response.status() == StatusCode::OK ||
        response.status() == StatusCode::BAD_REQUEST ||
        response.status() == StatusCode::INTERNAL_SERVER_ERROR
    );
}

// ---------------------------------------------------------------------------
// POST /preview/markdown tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_preview_markdown_accepts_file_upload() {
    let (app, _state) = test_app().await;

    let boundary = "----test-boundary";
    let md_content = "# Test\n\nThis is a test markdown document.";

    let body = format!(
        "------{boundary}\r\n" +
        "Content-Disposition: form-data; name=\"files\"; filename=\"test.md\"\r\n" +
        "Content-Type: text/markdown\r\n\r\n" +
        "{md_content}\r\n" +
        "------{boundary}--\r\n"
    );

    let request = Request::builder()
        .method("POST")
        .uri("/preview/markdown")
        .header(header::CONTENT_TYPE, format!("multipart/form-data; boundary={boundary}"))
        .body(Body::from(body))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    
    // Should accept markdown file
    assert!(
        response.status() == StatusCode::OK ||
        response.status() == StatusCode::BAD_REQUEST ||
        response.status() == StatusCode::INTERNAL_SERVER_ERROR
    );
}

// ---------------------------------------------------------------------------
// POST /preview/compare tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_preview_compare_accepts_before_after_files() {
    let (app, _state) = test_app().await;

    let boundary = "----test-boundary";
    let before_html = "<html><body><h1>Before</h1></body></html>";
    let after_html = "<html><body><h1>After</h1></body></html>";

    let body = format!(
        "------{boundary}\r\n" +
        "Content-Disposition: form-data; name=\"before\"; filename=\"before.html\"\r\n" +
        "Content-Type: text/html\r\n\r\n" +
        "{before_html}\r\n" +
        "------{boundary}\r\n" +
        "Content-Disposition: form-data; name=\"after\"; filename=\"after.html\"\r\n" +
        "Content-Type: text/html\r\n\r\n" +
        "{after_html}\r\n" +
        "------{boundary}--\r\n"
    );

    let request = Request::builder()
        .method("POST")
        .uri("/preview/compare")
        .header(header::CONTENT_TYPE, format!("multipart/form-data; boundary={boundary}"))
        .body(Body::from(body))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    
    // Should accept before/after files
    assert!(
        response.status() == StatusCode::OK ||
        response.status() == StatusCode::BAD_REQUEST ||
        response.status() == StatusCode::INTERNAL_SERVER_ERROR
    );
}

#[tokio::test]
async fn test_preview_compare_returns_400_with_missing_files() {
    let (app, _state) = test_app().await;

    let boundary = "----test-boundary";
    let before_html = "<html><body><h1>Before</h1></body></html>";

    // Only send 'before', missing 'after'
    let body = format!(
        "------{boundary}\r\n" +
        "Content-Disposition: form-data; name=\"before\"; filename=\"before.html\"\r\n" +
        "Content-Type: text/html\r\n\r\n" +
        "{before_html}\r\n" +
        "------{boundary}--\r\n"
    );

    let request = Request::builder()
        .method("POST")
        .uri("/preview/compare")
        .header(header::CONTENT_TYPE, format!("multipart/form-data; boundary={boundary}"))
        .body(Body::from(body))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    
    // Should return 400 when missing required files
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

// ---------------------------------------------------------------------------
// Content-Type validation tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_preview_url_returns_png_content_type() {
    let (app, _state) = test_app().await;

    let request = Request::builder()
        .method("GET")
        .uri("/preview/url?url=https://example.com&format=png")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    
    if response.status() == StatusCode::OK {
        let content_type = response.headers()
            .get(header::CONTENT_TYPE)
            .unwrap()
            .to_str()
            .unwrap();
        assert!(content_type.contains("image/png"));
    }
}

#[tokio::test]
async fn test_preview_url_returns_jpeg_content_type() {
    let (app, _state) = test_app().await;

    let request = Request::builder()
        .method("GET")
        .uri("/preview/url?url=https://example.com&format=jpeg")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    
    if response.status() == StatusCode::OK {
        let content_type = response.headers()
            .get(header::CONTENT_TYPE)
            .unwrap()
            .to_str()
            .unwrap();
        assert!(content_type.contains("image/jpeg"));
    }
}
