//! HTTP Basic Authentication middleware.
//!
//! Provides Basic Auth protection when `--api-basic-auth-username` is set.

use axum::body::Body;
use axum::http::{Request, Response, StatusCode, header};
use std::sync::Arc;
use tower::{Layer, Service};
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

/// Basic Auth credentials.
#[derive(Clone, Debug)]
pub struct BasicAuthConfig {
    /// Username for authentication.
    pub username: String,
    /// Password for authentication.
    pub password: String,
}

impl BasicAuthConfig {
    /// Create new Basic Auth config.
    pub fn new(username: String, password: String) -> Self {
        Self { username, password }
    }
}

/// Middleware layer for Basic Auth.
#[derive(Clone, Debug)]
pub struct BasicAuthLayer {
    config: Arc<BasicAuthConfig>,
}

impl BasicAuthLayer {
    /// Create a new Basic Auth layer.
    pub fn new(config: BasicAuthConfig) -> Self {
        Self {
            config: Arc::new(config),
        }
    }
}

impl<S> Layer<S> for BasicAuthLayer {
    type Service = BasicAuthMiddleware<S>;

    fn layer(&self, inner: S) -> Self::Service {
        BasicAuthMiddleware {
            inner,
            config: Arc::clone(&self.config),
        }
    }
}

/// Basic Auth middleware service.
#[derive(Clone, Debug)]
pub struct BasicAuthMiddleware<S> {
    inner: S,
    config: Arc<BasicAuthConfig>,
}

impl<S> Service<Request<Body>> for BasicAuthMiddleware<S>
where
    S: Service<Request<Body>, Response = Response<Body>> + Clone + Send + 'static,
    S::Future: Send + 'static,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: Request<Body>) -> Self::Future {
        let mut inner = self.inner.clone();
        let config = Arc::clone(&self.config);

        Box::pin(async move {
            // Check Authorization header
            if let Some(auth_header) = req.headers().get(header::AUTHORIZATION) {
                if let Ok(auth_str) = auth_header.to_str() {
                    if auth_str.starts_with("Basic ") {
                        let credentials = &auth_str[6..]; // Skip "Basic "
                        if let Ok(decoded) = base64_decode(credentials) {
                            if let Some((username, password)) = decoded.split_once(':') {
                                if username == config.username && password == config.password {
                                    return inner.call(req).await;
                                }
                            }
                        }
                    }
                }
            }

            // Authentication failed - return 401
            let response = Response::builder()
                .status(StatusCode::UNAUTHORIZED)
                .header(header::WWW_AUTHENTICATE, "Basic realm=\"pdfbro\"")
                .body(Body::from(r#"{"error": "Unauthorized"}"#))
                .unwrap();

            Ok(response)
        })
    }
}

/// Simple base64 decoder for Basic Auth credentials.
fn base64_decode(input: &str) -> Result<String, ()> {
    use std::collections::HashMap;

    // Base64 character to value mapping
    let mut char_map: HashMap<char, u8> = HashMap::new();
    for (i, c) in "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/".chars().enumerate() {
        char_map.insert(c, i as u8);
    }

    let mut result = Vec::new();
    let chars: Vec<char> = input.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        // Get 4 characters (or fewer if at end)
        let c1 = char_map.get(&chars.get(i).copied().unwrap_or('=')).copied().unwrap_or(0);
        let c2 = char_map.get(&chars.get(i + 1).copied().unwrap_or('=')).copied().unwrap_or(0);
        let c3 = char_map.get(&chars.get(i + 2).copied().unwrap_or('=')).copied().unwrap_or(0);
        let c4 = char_map.get(&chars.get(i + 3).copied().unwrap_or('=')).copied().unwrap_or(0);

        // Decode 3 bytes
        let b1 = (c1 << 2) | (c2 >> 4);
        result.push(b1);

        if chars.get(i + 2).copied().unwrap_or('=') != '=' {
            let b2 = ((c2 & 0x0F) << 4) | (c3 >> 2);
            result.push(b2);
        }

        if chars.get(i + 3).copied().unwrap_or('=') != '=' {
            let b3 = ((c3 & 0x03) << 6) | c4;
            result.push(b3);
        }

        i += 4;
    }

    String::from_utf8(result).map_err(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_decode_basic() {
        // "admin:password" in base64
        let decoded = base64_decode("YWRtaW46cGFzc3dvcmQ=").unwrap();
        assert_eq!(decoded, "admin:password");
    }

    #[test]
    fn base64_decode_with_special_chars() {
        // "user:pass123" in base64
        let decoded = base64_decode("dXNlcjpwYXNzMTIz").unwrap();
        assert_eq!(decoded, "user:pass123");
    }
}
