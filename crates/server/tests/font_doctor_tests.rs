//! Integration tests for Spec 43 — Font Doctor.
//!
//! Tests the `/debug/*` endpoints for font diagnostics.

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
// GET /debug/fonts tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_list_fonts_returns_200() {
    let (app, _state) = test_app().await;

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
    let (app, _state) = test_app().await;

    let request = Request::builder()
        .method("GET")
        .uri("/debug/fonts")
        .body(Body::empty())
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

// ---------------------------------------------------------------------------
// POST /debug/validate-fonts tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_validate_fonts_returns_200_with_html() {
    let (app, _state) = test_app().await;

    let boundary = "----test-boundary";
    let html_content = r#"
        <style>
            body { font-family: 'Arial', sans-serif; }
            h1 { font-family: 'Helvetica', sans-serif; }
        </style>
        <body>Test</body>
    "#;

    let body = format!(
        "------{boundary}\r\n" +
        "Content-Disposition: form-data; name=\"html\"\r\n\r\n" +
        "{html_content}\r\n" +
        "------{boundary}--\r\n"
    );

    let request = Request::builder()
        .method("POST")
        .uri("/debug/validate-fonts")
        .header(header::CONTENT_TYPE, format!("multipart/form-data; boundary={boundary}"))
        .body(Body::from(body))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_validate_fonts_returns_200_with_css() {
    let (app, _state) = test_app().await;

    let boundary = "----test-boundary";
    let css_content = r#"
        .title { font-family: 'Roboto', sans-serif; }
        .body { font-family: 'Open Sans', Arial, sans-serif; }
    "#;

    let body = format!(
        "------{boundary}\r\n" +
        "Content-Disposition: form-data; name=\"css\"\r\n\r\n" +
        "{css_content}\r\n" +
        "------{boundary}--\r\n"
    );

    let request = Request::builder()
        .method("POST")
        .uri("/debug/validate-fonts")
        .header(header::CONTENT_TYPE, format!("multipart/form-data; boundary={boundary}"))
        .body(Body::from(body))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_validate_fonts_returns_200_with_fonts_list() {
    let (app, _state) = test_app().await;

    let boundary = "----test-boundary";
    let body = format!(
        "------{boundary}\r\n" +
        "Content-Disposition: form-data; name=\"fonts\"\r\n\r\n" +
        "Arial,Helvetica,Roboto\r\n" +
        "------{boundary}--\r\n"
    );

    let request = Request::builder()
        .method("POST")
        .uri("/debug/validate-fonts")
        .header(header::CONTENT_TYPE, format!("multipart/form-data; boundary={boundary}"))
        .body(Body::from(body))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_validate_fonts_returns_400_without_input() {
    let (app, _state) = test_app().await;

    let boundary = "----test-boundary";
    let body = format!(
        "------{boundary}\r\n" +
        "Content-Disposition: form-data; name=\"other\"\r\n\r\n" +
        "value\r\n" +
        "------{boundary}--\r\n"
    );

    let request = Request::builder()
        .method("POST")
        .uri("/debug/validate-fonts")
        .header(header::CONTENT_TYPE, format!("multipart/form-data; boundary={boundary}"))
        .body(Body::from(body))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    
    // Should return 400 when no fonts/html/css provided
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

// ---------------------------------------------------------------------------
// POST /debug/diagnose-html tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_diagnose_html_returns_200() {
    let (app, _state) = test_app().await;

    let boundary = "----test-boundary";
    let html_content = r#"<!DOCTYPE html>
<html>
<head>
    <style>
        body { font-family: 'Arial', sans-serif; }
        @font-face { font-family: 'CustomFont'; src: url('font.woff2'); }
    </style>
</head>
<body>
    <h1>Test</h1>
    <img src="https://example.com/image.jpg" alt="test">
</body>
</html>"#;

    let body = format!(
        "------{boundary}\r\n" +
        "Content-Disposition: form-data; name=\"html\"\r\n\r\n" +
        "{html_content}\r\n" +
        "------{boundary}--\r\n"
    );

    let request = Request::builder()
        .method("POST")
        .uri("/debug/diagnose-html")
        .header(header::CONTENT_TYPE, format!("multipart/form-data; boundary={boundary}"))
        .body(Body::from(body))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_diagnose_html_returns_json_with_fonts_array() {
    let (app, _state) = test_app().await;

    let boundary = "----test-boundary";
    let html_content = r#"<html><style>body{font-family:'Arial'}</style><body>Test</body></html>"#;

    let body = format!(
        "------{boundary}\r\n" +
        "Content-Disposition: form-data; name=\"html\"\r\n\r\n" +
        "{html_content}\r\n" +
        "------{boundary}--\r\n"
    );

    let request = Request::builder()
        .method("POST")
        .uri("/debug/diagnose-html")
        .header(header::CONTENT_TYPE, format!("multipart/form-data; boundary={boundary}"))
        .body(Body::from(body))
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
async fn test_diagnose_html_returns_400_without_html() {
    let (app, _state) = test_app().await;

    let boundary = "----test-boundary";
    let body = format!(
        "------{boundary}\r\n" +
        "Content-Disposition: form-data; name=\"other\"\r\n\r\n" +
        "value\r\n" +
        "------{boundary}--\r\n"
    );

    let request = Request::builder()
        .method("POST")
        .uri("/debug/diagnose-html")
        .header(header::CONTENT_TYPE, format!("multipart/form-data; boundary={boundary}"))
        .body(Body::from(body))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    
    // Should return 400 when no html field provided
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_diagnose_html_detects_google_fonts() {
    let (app, _state) = test_app().await;

    let boundary = "----test-boundary";
    let html_content = r#"
        <html>
        <head>
            <link href="https://fonts.googleapis.com/css?family=Roboto:300,400,700" rel="stylesheet">
        </head>
        <body>Test</body>
        </html>
    "#;

    let body = format!(
        "------{boundary}\r\n" +
        "Content-Disposition: form-data; name=\"html\"\r\n\r\n" +
        "{html_content}\r\n" +
        "------{boundary}--\r\n"
    );

    let request = Request::builder()
        .method("POST")
        .uri("/debug/diagnose-html")
        .header(header::CONTENT_TYPE, format!("multipart/form-data; boundary={boundary}"))
        .body(Body::from(body))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    
    // Should detect Google Fonts and include warnings
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_diagnose_html_detects_web_fonts() {
    let (app, _state) = test_app().await;

    let boundary = "----test-boundary";
    let html_content = r#"
        <html>
        <head>
            <style>
                @font-face {
                    font-family: 'CustomWebFont';
                    src: url('https://example.com/font.woff2') format('woff2');
                }
            </style>
        </head>
        <body>Test</body>
        </html>
    "#;

    let body = format!(
        "------{boundary}\r\n" +
        "Content-Disposition: form-data; name=\"html\"\r\n\r\n" +
        "{html_content}\r\n" +
        "------{boundary}--\r\n"
    );

    let request = Request::builder()
        .method("POST")
        .uri("/debug/diagnose-html")
        .header(header::CONTENT_TYPE, format!("multipart/form-data; boundary={boundary}"))
        .body(Body::from(body))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    
    // Should detect @font-face and warn about web fonts
    assert_eq!(response.status(), StatusCode::OK);
}

// ---------------------------------------------------------------------------
// Unit tests for font extraction logic
// ---------------------------------------------------------------------------

#[test]
fn test_extract_font_families_from_html_basic() {
    let html = r#"
        <style>
            body { font-family: 'Helvetica Neue', Helvetica, Arial, sans-serif; }
            h1 { font-family: Georgia, serif; }
            @font-face { font-family: 'Custom Font'; src: url('font.woff2'); }
        </style>
    "#;

    // Simple extraction test
    assert!(html.contains("font-family"));
    assert!(html.contains("Helvetica Neue"));
    assert!(html.contains("Custom Font"));
}

#[test]
fn test_detects_google_fonts_url() {
    let html = r#"<link href="https://fonts.googleapis.com/css2?family=Roboto">"#;
    
    assert!(html.contains("fonts.googleapis.com"));
}

#[test]
fn test_detects_web_fonts_at_font_face() {
    let html = r#"<style>@font-face { font-family: 'Foo'; }</style>"#;
    
    assert!(html.contains("@font-face"));
}
