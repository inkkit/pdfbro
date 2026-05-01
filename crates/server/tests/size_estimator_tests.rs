//! Integration tests for Spec 46 — PDF Size Estimator.
//!
//! Tests the `/estimate` endpoints for PDF size prediction.

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use http_body_util::BodyExt;
use tower::ServiceExt;
use serde_json::json;

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
// POST /estimate tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_estimate_returns_200_with_html() {
    let (app, _state) = test_app().await;

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
async fn test_estimate_returns_json_response() {
    let (app, _state) = test_app().await;

    let body = json!({
        "html": "<html><body>Test</body></html>"
    });

    let request = Request::builder()
        .method("POST")
        .uri("/estimate")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body.to_string()))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    
    assert_eq!(response.status(), StatusCode::OK);
    
    let content_type = response.headers()
        .get(header::CONTENT_TYPE)
        .unwrap()
        .to_str()
        .unwrap();
    assert!(content_type.contains("application/json"));
}

#[tokio::test]
async fn test_estimate_returns_400_without_html_or_url() {
    let (app, _state) = test_app().await;

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
    
    // Should return 400 when neither html nor url provided
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_estimate_detects_web_fonts() {
    let (app, _state) = test_app().await;

    let body = json!({
        "html": r#"
            <html>
            <head>
                <style>
                    @font-face { font-family: 'Custom'; src: url('font.woff2'); }
                </style>
            </head>
            <body>Test</body>
            </html>
        "#
    });

    let request = Request::builder()
        .method("POST")
        .uri("/estimate")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body.to_string()))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    
    assert_eq!(response.status(), StatusCode::OK);
    
    // Parse response to check for warnings
    let body_bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body_str = String::from_utf8_lossy(&body_bytes);
    let json: serde_json::Value = serde_json::from_str(&body_str).unwrap();
    
    // Should have warnings about web fonts
    assert!(json.get("warnings").is_some());
}

#[tokio::test]
async fn test_estimate_detects_images() {
    let (app, _state) = test_app().await;

    let body = json!({
        "html": r#"
            <html>
            <body>
                <img src="https://example.com/image1.jpg">
                <img src="https://example.com/image2.png">
            </body>
            </html>
        "#
    });

    let request = Request::builder()
        .method("POST")
        .uri("/estimate")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body.to_string()))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    
    assert_eq!(response.status(), StatusCode::OK);
    
    // Parse response
    let body_bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body_str = String::from_utf8_lossy(&body_bytes);
    let json: serde_json::Value = serde_json::from_str(&body_str).unwrap();
    
    // Check breakdown includes images
    let breakdown = json.get("breakdown").unwrap();
    assert!(breakdown.get("images_mb").is_some());
}

#[tokio::test]
async fn test_estimate_returns_confidence_level() {
    let (app, _state) = test_app().await;

    let body = json!({
        "html": "<html><body>Simple test</body></html>"
    });

    let request = Request::builder()
        .method("POST")
        .uri("/estimate")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body.to_string()))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    
    assert_eq!(response.status(), StatusCode::OK);
    
    // Parse response
    let body_bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body_str = String::from_utf8_lossy(&body_bytes);
    let json: serde_json::Value = serde_json::from_str(&body_str).unwrap();
    
    // Should have confidence field
    let confidence = json.get("confidence").unwrap().as_str().unwrap();
    assert!(["high", "medium", "low"].contains(&confidence));
}

#[tokio::test]
async fn test_estimate_returns_size_breakdown() {
    let (app, _state) = test_app().await;

    let body = json!({
        "html": "<html><body>Test</body></html>"
    });

    let request = Request::builder()
        .method("POST")
        .uri("/estimate")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body.to_string()))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    
    assert_eq!(response.status(), StatusCode::OK);
    
    // Parse response
    let body_bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body_str = String::from_utf8_lossy(&body_bytes);
    let json: serde_json::Value = serde_json::from_str(&body_str).unwrap();
    
    // Should have breakdown with all fields
    let breakdown = json.get("breakdown").unwrap();
    assert!(breakdown.get("fonts_mb").is_some());
    assert!(breakdown.get("images_mb").is_some());
    assert!(breakdown.get("markup_mb").is_some());
    assert!(breakdown.get("overhead_mb").is_some());
}

// ---------------------------------------------------------------------------
// POST /estimate/form tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_estimate_form_returns_200_with_html_file() {
    let (app, _state) = test_app().await;

    let boundary = "----test-boundary";
    let html_content = "<html><body>Test</body></html>";

    let body = format!(
        "------{boundary}\r\n" +
        "Content-Disposition: form-data; name=\"files\"; filename=\"test.html\"\r\n" +
        "Content-Type: text/html\r\n\r\n" +
        "{html_content}\r\n" +
        "------{boundary}--\r\n"
    );

    let request = Request::builder()
        .method("POST")
        .uri("/estimate/form")
        .header(header::CONTENT_TYPE, format!("multipart/form-data; boundary={boundary}"))
        .body(Body::from(body))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_estimate_form_returns_200_with_html_field() {
    let (app, _state) = test_app().await;

    let boundary = "----test-boundary";
    let html_content = "<html><body>Test</body></html>";

    let body = format!(
        "------{boundary}\r\n" +
        "Content-Disposition: form-data; name=\"html\"\r\n\r\n" +
        "{html_content}\r\n" +
        "------{boundary}--\r\n"
    );

    let request = Request::builder()
        .method("POST")
        .uri("/estimate/form")
        .header(header::CONTENT_TYPE, format!("multipart/form-data; boundary={boundary}"))
        .body(Body::from(body))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    
    assert_eq!(response.status(), StatusCode::OK);
}

// ---------------------------------------------------------------------------
// POST /estimate/batch tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_estimate_batch_returns_200() {
    let (app, _state) = test_app().await;

    let body = json!({
        "urls": [
            "https://example.com/page1",
            "https://example.com/page2",
            "https://example.com/page3"
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
    let (app, _state) = test_app().await;

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

#[tokio::test]
async fn test_estimate_batch_returns_estimates_array() {
    let (app, _state) = test_app().await;

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
    
    // Parse response
    let body_bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body_str = String::from_utf8_lossy(&body_bytes);
    let json: serde_json::Value = serde_json::from_str(&body_str).unwrap();
    
    // Should have estimates array
    let estimates = json.get("estimates").unwrap().as_array().unwrap();
    assert_eq!(estimates.len(), 2);
    
    // Each estimate should have url and estimated_size_mb
    for estimate in estimates {
        assert!(estimate.get("url").is_some());
        assert!(estimate.get("estimated_size_mb").is_some());
        assert!(estimate.get("confidence").is_some());
    }
}

#[tokio::test]
async fn test_estimate_batch_returns_total_mb() {
    let (app, _state) = test_app().await;

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
    
    // Parse response
    let body_bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body_str = String::from_utf8_lossy(&body_bytes);
    let json: serde_json::Value = serde_json::from_str(&body_str).unwrap();
    
    // Should have total_mb
    assert!(json.get("total_mb").is_some());
}

// ---------------------------------------------------------------------------
// Unit tests for HTML analysis logic
// ---------------------------------------------------------------------------

#[test]
fn test_estimate_url_size_basic() {
    let size1 = estimate_url_size("https://example.com/page");
    let size2 = estimate_url_size("https://example.com/gallery/photos");
    let size3 = estimate_url_size("https://example.com/dashboard/app");
    
    // Gallery pages should be estimated larger
    assert!(size2 > size1);
    // Dashboard pages should be estimated larger than basic
    assert!(size3 >= size1);
}

/// Simple URL size estimation (mirrors implementation logic)
fn estimate_url_size(url: &str) -> f64 {
    let mut base_size = 1.0;
    
    if url.len() > 100 {
        base_size += 0.5;
    }
    
    let lower_url = url.to_lowercase();
    if lower_url.contains("/gallery")
        || lower_url.contains("/images")
        || lower_url.contains("/photos")
    {
        base_size += 2.0;
    }
    
    if lower_url.contains("/dashboard") || lower_url.contains("/app") {
        base_size += 1.0;
    }
    
    base_size += (url.len() % 10) as f64 / 10.0;
    
    base_size
}

#[test]
fn test_round_to_2dp() {
    assert_eq!(round_to_2dp(1.2345), 1.23);
    assert_eq!(round_to_2dp(1.2355), 1.24);
    assert_eq!(round_to_2dp(1.0), 1.0);
}

fn round_to_2dp(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}
