//! `/openapi.json` route handler for serving the OpenAPI specification.
//!
//! This provides the API spec for Scalar interactive documentation.

use axum::extract::State;
use axum::Json;
use serde_json::{Value, json};

use crate::state::AppState;

/// Resolve the public base URL for the OpenAPI `servers` block.
///
/// Priority:
/// 1. `PUBLIC_URL` env var — explicit override, works on any platform.
/// 2. `FLY_APP_NAME` env var — auto-detected on Fly.io deployments.
/// 3. `http://{host}:{port}` derived from server config — local / unknown.
fn resolve_server_url(state: &AppState) -> String {
    if let Ok(url) = std::env::var("PUBLIC_URL") {
        return url.trim_end_matches('/').to_string();
    }
    if let Ok(app) = std::env::var("FLY_APP_NAME") {
        return format!("https://{app}.fly.dev");
    }
    let cfg = &state.config;
    let host = cfg.host;
    let port = cfg.port;
    if port == 80 {
        format!("http://{host}")
    } else if port == 443 {
        format!("https://{host}")
    } else {
        format!("http://{host}:{port}")
    }
}

/// `GET /openapi.json` - Returns the OpenAPI 3.0 specification for Folio.
pub async fn openapi_spec(State(state): State<AppState>) -> Json<Value> {
    let server_url = resolve_server_url(&state);
    Json(json!({
        "openapi": "3.0.3",
        "info": {
            "title": "Folio API",
            "description": "PDF generation and manipulation API (Gotenberg-compatible). Built with Rust + Chromium + LibreOffice.",
            "version": "0.1.0",
            "contact": {
                "name": "Folio Team",
                "url": "https://github.com/vel/folio"
            }
        },
        "servers": [
            {
                "url": server_url,
                "description": "API server"
            }
        ],
        "tags": [
            {
                "name": "Health",
                "description": "Health check and version endpoints"
            },
            {
                "name": "Chromium",
                "description": "HTML/URL to PDF conversion using Chrome"
            },
            {
                "name": "LibreOffice",
                "description": "Document conversion using LibreOffice"
            },
            {
                "name": "PDF Engines",
                "description": "PDF manipulation operations (merge, split, optimize, etc.)"
            },
            {
                "name": "Font Doctor",
                "description": "Font diagnostics and validation (Spec 43)"
            },
            {
                "name": "Live Preview",
                "description": "HTML/URL to image preview (Spec 45)"
            },
            {
                "name": "Size Estimator",
                "description": "PDF size estimation before conversion (Spec 46)"
            }
        ],
        "paths": {
            "/health": {
                "get": {
                    "tags": ["Health"],
                    "summary": "Health check",
                    "description": "Returns the health status of the server",
                    "responses": {
                        "200": {
                            "description": "Server is healthy",
                            "content": {
                                "application/json": {
                                    "schema": {
                                        "type": "object",
                                        "properties": {
                                            "status": { "type": "string", "example": "up" }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            },
            "/version": {
                "get": {
                    "tags": ["Health"],
                    "summary": "Version information",
                    "description": "Returns the server version",
                    "responses": {
                        "200": {
                            "description": "Version info",
                            "content": {
                                "application/json": {
                                    "schema": {
                                        "type": "object",
                                        "properties": {
                                            "version": { "type": "string", "example": "0.1.0" }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            },
            "/forms/chromium/convert/html": {
                "post": {
                    "tags": ["Chromium"],
                    "summary": "Convert HTML to PDF",
                    "description": "Converts HTML content to PDF using Chromium",
                    "requestBody": {
                        "required": true,
                        "content": {
                            "multipart/form-data": {
                                "schema": {
                                    "type": "object",
                                    "properties": {
                                        "files": { "type": "string", "format": "binary", "description": "HTML file to convert" },
                                        "paperWidth": { "type": "number", "example": 8.5 },
                                        "paperHeight": { "type": "number", "example": 11 },
                                        "marginTop": { "type": "number", "example": 0.5 },
                                        "marginBottom": { "type": "number", "example": 0.5 },
                                        "marginLeft": { "type": "number", "example": 0.5 },
                                        "marginRight": { "type": "number", "example": 0.5 }
                                    }
                                }
                            }
                        }
                    },
                    "responses": {
                        "200": {
                            "description": "PDF file",
                            "content": {
                                "application/pdf": {
                                    "schema": { "type": "string", "format": "binary" }
                                }
                            }
                        }
                    }
                }
            },
            "/forms/chromium/convert/url": {
                "post": {
                    "tags": ["Chromium"],
                    "summary": "Convert URL to PDF",
                    "description": "Converts a webpage to PDF using Chromium",
                    "requestBody": {
                        "required": true,
                        "content": {
                            "multipart/form-data": {
                                "schema": {
                                    "type": "object",
                                    "properties": {
                                        "url": { "type": "string", "example": "https://example.com" },
                                        "paperWidth": { "type": "number", "example": 8.5 },
                                        "paperHeight": { "type": "number", "example": 11 }
                                    }
                                }
                            }
                        }
                    },
                    "responses": {
                        "200": {
                            "description": "PDF file",
                            "content": {
                                "application/pdf": {
                                    "schema": { "type": "string", "format": "binary" }
                                }
                            }
                        }
                    }
                }
            },
            "/forms/pdfengines/optimise": {
                "post": {
                    "tags": ["PDF Engines"],
                    "summary": "Optimize PDF file size",
                    "description": "Compress PDF using Ghostscript or qpdf (Spec 42)",
                    "requestBody": {
                        "required": true,
                        "content": {
                            "multipart/form-data": {
                                "schema": {
                                    "type": "object",
                                    "properties": {
                                        "files": { "type": "string", "format": "binary", "description": "PDF file to optimize" },
                                        "preset": { "type": "string", "enum": ["screen", "ebook", "printer"], "example": "screen" },
                                        "backend": { "type": "string", "enum": ["ghostscript", "qpdf"], "example": "ghostscript" }
                                    }
                                }
                            }
                        }
                    },
                    "responses": {
                        "200": {
                            "description": "Optimized PDF file",
                            "headers": {
                                "X-Original-Size": { "schema": { "type": "integer" } },
                                "X-Optimised-Size": { "schema": { "type": "integer" } },
                                "X-Compression-Ratio": { "schema": { "type": "number" } },
                                "X-Reduction-Percent": { "schema": { "type": "string" } },
                                "X-Backend-Used": { "schema": { "type": "string" } }
                            },
                            "content": {
                                "application/pdf": {
                                    "schema": { "type": "string", "format": "binary" }
                                }
                            }
                        }
                    }
                }
            },
            "/forms/pdfengines/merge": {
                "post": {
                    "tags": ["PDF Engines"],
                    "summary": "Merge multiple PDFs",
                    "requestBody": {
                        "required": true,
                        "content": {
                            "multipart/form-data": {
                                "schema": {
                                    "type": "object",
                                    "properties": {
                                        "files": { "type": "array", "items": { "type": "string", "format": "binary" }, "description": "PDF files to merge" }
                                    }
                                }
                            }
                        }
                    },
                    "responses": {
                        "200": {
                            "description": "Merged PDF file",
                            "content": {
                                "application/pdf": {
                                    "schema": { "type": "string", "format": "binary" }
                                }
                            }
                        }
                    }
                }
            },
            "/forms/pdfengines/split": {
                "post": {
                    "tags": ["PDF Engines"],
                    "summary": "Split PDF into multiple files",
                    "requestBody": {
                        "required": true,
                        "content": {
                            "multipart/form-data": {
                                "schema": {
                                    "type": "object",
                                    "properties": {
                                        "files": { "type": "string", "format": "binary", "description": "PDF file to split" },
                                        "splitMode": { "type": "string", "enum": ["intervals", "pages"], "example": "intervals" },
                                        "splitSpan": { "type": "string", "example": "1-3,5-7" }
                                    }
                                }
                            }
                        }
                    },
                    "responses": {
                        "200": {
                            "description": "ZIP file with split PDFs",
                            "content": {
                                "application/zip": {
                                    "schema": { "type": "string", "format": "binary" }
                                }
                            }
                        }
                    }
                }
            },
            "/debug/fonts": {
                "get": {
                    "tags": ["Font Doctor"],
                    "summary": "List system fonts",
                    "description": "Returns all available system fonts (Spec 43)",
                    "responses": {
                        "200": {
                            "description": "List of fonts",
                            "content": {
                                "application/json": {
                                    "schema": {
                                        "type": "object",
                                        "properties": {
                                            "fonts": {
                                                "type": "array",
                                                "items": {
                                                    "type": "object",
                                                    "properties": {
                                                        "family": { "type": "string" },
                                                        "path": { "type": "string" }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            },
            "/debug/validate-fonts": {
                "post": {
                    "tags": ["Font Doctor"],
                    "summary": "Validate fonts in HTML/CSS",
                    "description": "Check if fonts will render correctly (Spec 43)",
                    "requestBody": {
                        "required": true,
                        "content": {
                            "multipart/form-data": {
                                "schema": {
                                    "type": "object",
                                    "properties": {
                                        "html": { "type": "string", "description": "HTML content to analyze" },
                                        "css": { "type": "string", "description": "CSS content to analyze" },
                                        "fonts": { "type": "string", "description": "Comma-separated font list" }
                                    }
                                }
                            }
                        }
                    },
                    "responses": {
                        "200": {
                            "description": "Validation results",
                            "content": {
                                "application/json": {
                                    "schema": {
                                        "type": "object",
                                        "properties": {
                                            "fonts": {
                                                "type": "array",
                                                "items": {
                                                    "type": "object",
                                                    "properties": {
                                                        "family": { "type": "string" },
                                                        "available": { "type": "boolean" },
                                                        "suggestion": { "type": "string" }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            },
            "/debug/diagnose-html": {
                "post": {
                    "tags": ["Font Doctor"],
                    "summary": "Full font diagnostics",
                    "description": "Comprehensive font analysis for HTML (Spec 43)",
                    "requestBody": {
                        "required": true,
                        "content": {
                            "multipart/form-data": {
                                "schema": {
                                    "type": "object",
                                    "properties": {
                                        "html": { "type": "string", "description": "HTML content to diagnose" }
                                    }
                                }
                            }
                        }
                    },
                    "responses": {
                        "200": {
                            "description": "Diagnostics report",
                            "content": {
                                "application/json": {
                                    "schema": {
                                        "type": "object",
                                        "properties": {
                                            "fonts": { "type": "array" },
                                            "warnings": { "type": "array", "items": { "type": "string" } },
                                            "suggestions": { "type": "array", "items": { "type": "string" } }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            },
            "/preview/url": {
                "get": {
                    "tags": ["Live Preview"],
                    "summary": "Preview URL as image",
                    "description": "Renders a URL to image for quick preview (Spec 45)",
                    "parameters": [
                        { "name": "url", "in": "query", "required": true, "schema": { "type": "string" } },
                        { "name": "format", "in": "query", "schema": { "type": "string", "enum": ["png", "jpeg", "webp"], "default": "png" } },
                        { "name": "width", "in": "query", "schema": { "type": "integer", "default": 1920 } },
                        { "name": "height", "in": "query", "schema": { "type": "integer", "default": 1080 } },
                        { "name": "full_page", "in": "query", "schema": { "type": "boolean", "default": false } }
                    ],
                    "responses": {
                        "200": {
                            "description": "Preview image",
                            "content": {
                                "image/png": { "schema": { "type": "string", "format": "binary" } },
                                "image/jpeg": { "schema": { "type": "string", "format": "binary" } },
                                "image/webp": { "schema": { "type": "string", "format": "binary" } }
                            }
                        }
                    }
                }
            },
            "/preview/html": {
                "post": {
                    "tags": ["Live Preview"],
                    "summary": "Preview HTML as image",
                    "description": "Renders HTML to image for quick preview (Spec 45)",
                    "requestBody": {
                        "required": true,
                        "content": {
                            "multipart/form-data": {
                                "schema": {
                                    "type": "object",
                                    "properties": {
                                        "files": { "type": "string", "format": "binary", "description": "HTML file" },
                                        "format": { "type": "string", "enum": ["png", "jpeg", "webp"], "default": "png" },
                                        "width": { "type": "integer", "default": 1920 },
                                        "height": { "type": "integer", "default": 1080 },
                                        "full_page": { "type": "boolean", "default": false }
                                    }
                                }
                            }
                        }
                    },
                    "responses": {
                        "200": {
                            "description": "Preview image",
                            "content": {
                                "image/png": { "schema": { "type": "string", "format": "binary" } }
                            }
                        }
                    }
                }
            },
            "/estimate": {
                "post": {
                    "tags": ["Size Estimator"],
                    "summary": "Estimate PDF size",
                    "description": "Analyze HTML and predict PDF size before conversion (Spec 46)",
                    "requestBody": {
                        "required": true,
                        "content": {
                            "application/json": {
                                "schema": {
                                    "type": "object",
                                    "properties": {
                                        "html": { "type": "string", "description": "HTML content to analyze" },
                                        "url": { "type": "string", "description": "URL to analyze" }
                                    }
                                }
                            }
                        }
                    },
                    "responses": {
                        "200": {
                            "description": "Size estimation",
                            "content": {
                                "application/json": {
                                    "schema": {
                                        "type": "object",
                                        "properties": {
                                            "estimated_size_mb": { "type": "number", "example": 2.5 },
                                            "confidence": { "type": "string", "example": "medium" },
                                            "breakdown": {
                                                "type": "object",
                                                "properties": {
                                                    "fonts_mb": { "type": "number" },
                                                    "images_mb": { "type": "number" },
                                                    "markup_mb": { "type": "number" },
                                                    "overhead_mb": { "type": "number" }
                                                }
                                            },
                                            "warnings": { "type": "array", "items": { "type": "string" } },
                                            "suggestions": { "type": "array", "items": { "type": "string" } }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            },
            "/estimate/batch": {
                "post": {
                    "tags": ["Size Estimator"],
                    "summary": "Batch size estimation",
                    "description": "Estimate sizes for multiple URLs (Spec 46)",
                    "requestBody": {
                        "required": true,
                        "content": {
                            "application/json": {
                                "schema": {
                                    "type": "object",
                                    "properties": {
                                        "urls": { "type": "array", "items": { "type": "string" } }
                                    }
                                }
                            }
                        }
                    },
                    "responses": {
                        "200": {
                            "description": "Batch estimation results"
                        }
                    }
                }
            }
        }
    }))
}
