//! Integration tests for Scalar API Documentation.
//!
//! Tests the `/docs` and `/openapi.json` endpoints.

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
// GET /openapi.json tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_openapi_spec_returns_200() {
    let (app, _state) = test_app().await;

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
    let (app, _state) = test_app().await;

    let request = Request::builder()
        .method("GET")
        .uri("/openapi.json")
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

#[tokio::test]
async fn test_openapi_spec_contains_openapi_version() {
    let (app, _state) = test_app().await;

    let request = Request::builder()
        .method("GET")
        .uri("/openapi.json")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    
    assert_eq!(response.status(), StatusCode::OK);
    
    let body_bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body_str = String::from_utf8_lossy(&body_bytes);
    let json: serde_json::Value = serde_json::from_str(&body_str).unwrap();
    
    assert!(json.get("openapi").is_some());
    assert_eq!(json.get("openapi").unwrap().as_str().unwrap(), "3.0.3");
}

#[tokio::test]
async fn test_openapi_spec_contains_api_info() {
    let (app, _state) = test_app().await;

    let request = Request::builder()
        .method("GET")
        .uri("/openapi.json")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    
    assert_eq!(response.status(), StatusCode::OK);
    
    let body_bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body_str = String::from_utf8_lossy(&body_bytes);
    let json: serde_json::Value = serde_json::from_str(&body_str).unwrap();
    
    let info = json.get("info").unwrap();
    assert!(info.get("title").is_some());
    assert!(info.get("version").is_some());
}

#[tokio::test]
async fn test_openapi_spec_contains_paths() {
    let (app, _state) = test_app().await;

    let request = Request::builder()
        .method("GET")
        .uri("/openapi.json")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    
    assert_eq!(response.status(), StatusCode::OK);
    
    let body_bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body_str = String::from_utf8_lossy(&body_bytes);
    let json: serde_json::Value = serde_json::from_str(&body_str).unwrap();
    
    let paths = json.get("paths").unwrap();
    
    // Check for key endpoints
    assert!(paths.get("/health").is_some());
    assert!(paths.get("/forms/chromium/convert/html").is_some());
    assert!(paths.get("/forms/pdfengines/optimise").is_some());
    assert!(paths.get("/debug/fonts").is_some());
    assert!(paths.get("/preview/url").is_some());
    assert!(paths.get("/estimate").is_some());
}

#[tokio::test]
async fn test_openapi_spec_contains_tags() {
    let (app, _state) = test_app().await;

    let request = Request::builder()
        .method("GET")
        .uri("/openapi.json")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    
    assert_eq!(response.status(), StatusCode::OK);
    
    let body_bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body_str = String::from_utf8_lossy(&body_bytes);
    let json: serde_json::Value = serde_json::from_str(&body_str).unwrap();
    
    let tags = json.get("tags").unwrap().as_array().unwrap();
    
    // Check for feature tags
    let tag_names: Vec<&str> = tags.iter()
        .map(|t| t.get("name").unwrap().as_str().unwrap())
        .collect();
    
    assert!(tag_names.contains(&"Health"));
    assert!(tag_names.contains(&"Chromium"));
    assert!(tag_names.contains(&"PDF Engines"));
    assert!(tag_names.contains(&"Font Doctor"));
    assert!(tag_names.contains(&"Live Preview"));
    assert!(tag_names.contains(&"Size Estimator"));
}

// ---------------------------------------------------------------------------
// GET /docs tests (Scalar UI)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_docs_endpoint_returns_200() {
    let (app, _state) = test_app().await;

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
    let (app, _state) = test_app().await;

    let request = Request::builder()
        .method("GET")
        .uri("/docs")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    
    assert_eq!(response.status(), StatusCode::OK);
    
    let content_type = response.headers()
        .get(header::CONTENT_TYPE)
        .unwrap()
        .to_str()
        .unwrap();
    assert!(content_type.contains("text/html"));
}

#[tokio::test]
async fn test_docs_contains_scalar_reference() {
    let (app, _state) = test_app().await;

    let request = Request::builder()
        .method("GET")
        .uri("/docs")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    
    assert_eq!(response.status(), StatusCode::OK);
    
    let body_bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body_str = String::from_utf8_lossy(&body_bytes);
    
    // Should contain Scalar API reference
    assert!(body_str.contains("@scalar/api-reference") || body_str.contains("scalar"));
}

#[tokio::test]
async fn test_docs_contains_openapi_url() {
    let (app, _state) = test_app().await;

    let request = Request::builder()
        .method("GET")
        .uri("/docs")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    
    assert_eq!(response.status(), StatusCode::OK);
    
    let body_bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body_str = String::from_utf8_lossy(&body_bytes);
    
    // Should reference the OpenAPI spec endpoint
    assert!(body_str.contains("/openapi.json"));
}

#[tokio::test]
async fn test_docs_contains_api_title() {
    let (app, _state) = test_app().await;

    let request = Request::builder()
        .method("GET")
        .uri("/docs")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    
    assert_eq!(response.status(), StatusCode::OK);
    
    let body_bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body_str = String::from_utf8_lossy(&body_bytes);
    
    // Should contain API title
    assert!(body_str.contains("Folio") || body_str.contains("folio"));
}

// ---------------------------------------------------------------------------
// API endpoint documentation completeness tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_openapi_spec_contains_optimise_endpoint() {
    let (app, _state) = test_app().await;

    let request = Request::builder()
        .method("GET")
        .uri("/openapi.json")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    
    assert_eq!(response.status(), StatusCode::OK);
    
    let body_bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body_str = String::from_utf8_lossy(&body_bytes);
    let json: serde_json::Value = serde_json::from_str(&body_str).unwrap();
    
    let paths = json.get("paths").unwrap();
    let optimise = paths.get("/forms/pdfengines/optimise").unwrap();
    
    // Should have POST method with proper documentation
    let post = optimise.get("post").unwrap();
    assert!(post.get("summary").is_some());
    assert!(post.get("description").is_some());
    assert!(post.get("requestBody").is_some());
    assert!(post.get("responses").is_some());
}

#[tokio::test]
async fn test_openapi_spec_contains_font_doctor_endpoints() {
    let (app, _state) = test_app().await;

    let request = Request::builder()
        .method("GET")
        .uri("/openapi.json")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    
    assert_eq!(response.status(), StatusCode::OK);
    
    let body_bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body_str = String::from_utf8_lossy(&body_bytes);
    let json: serde_json::Value = serde_json::from_str(&body_str).unwrap();
    
    let paths = json.get("paths").unwrap();
    
    // All Font Doctor endpoints should be documented
    assert!(paths.get("/debug/fonts").is_some());
    assert!(paths.get("/debug/validate-fonts").is_some());
    assert!(paths.get("/debug/diagnose-html").is_some());
}

#[tokio::test]
async fn test_openapi_spec_contains_preview_endpoints() {
    let (app, _state) = test_app().await;

    let request = Request::builder()
        .method("GET")
        .uri("/openapi.json")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    
    assert_eq!(response.status(), StatusCode::OK);
    
    let body_bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body_str = String::from_utf8_lossy(&body_bytes);
    let json: serde_json::Value = serde_json::from_str(&body_str).unwrap();
    
    let paths = json.get("paths").unwrap();
    
    // All Live Preview endpoints should be documented
    assert!(paths.get("/preview/url").is_some());
    assert!(paths.get("/preview/html").is_some());
    assert!(paths.get("/preview/markdown").is_some());
    assert!(paths.get("/preview/compare").is_some());
}

#[tokio::test]
async fn test_openapi_spec_contains_estimate_endpoints() {
    let (app, _state) = test_app().await;

    let request = Request::builder()
        .method("GET")
        .uri("/openapi.json")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    
    assert_eq!(response.status(), StatusCode::OK);
    
    let body_bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body_str = String::from_utf8_lossy(&body_bytes);
    let json: serde_json::Value = serde_json::from_str(&body_str).unwrap();
    
    let paths = json.get("paths").unwrap();
    
    // All Size Estimator endpoints should be documented
    assert!(paths.get("/estimate").is_some());
    assert!(paths.get("/estimate/form").is_some());
    assert!(paths.get("/estimate/batch").is_some());
}

#[tokio::test]
async fn test_openapi_spec_contains_response_schemas() {
    let (app, _state) = test_app().await;

    let request = Request::builder()
        .method("GET")
        .uri("/openapi.json")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    
    assert_eq!(response.status(), StatusCode::OK);
    
    let body_bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body_str = String::from_utf8_lossy(&body_bytes);
    let json: serde_json::Value = serde_json::from_str(&body_str).unwrap();
    
    let paths = json.get("paths").unwrap();
    let fonts = paths.get("/debug/fonts").unwrap();
    let get = fonts.get("get").unwrap();
    let responses = get.get("responses").unwrap();
    let ok_response = responses.get("200").unwrap();
    
    // Should have content schema
    assert!(ok_response.get("content").is_some());
}
