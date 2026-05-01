//! HTTP-based conversion via a running unoserver process.

use std::path::Path;
use std::time::Duration;

use crate::types::{EngineError, EngineResult};

use super::OfficeOptions;
use super::filter::for_extension;

/// Send `input` to unoserver for PDF conversion and return the PDF bytes.
///
/// `client` must be the shared `reqwest::Client` from `LibreOfficeEngine::Inner`.
/// `port` is the localhost port unoserver is listening on.
pub(super) async fn run_convert(
    client: &reqwest::Client,
    port: u16,
    timeout: Duration,
    input: &Path,
    opts: &OfficeOptions,
) -> EngineResult<Vec<u8>> {
    let file_bytes = tokio::fs::read(input).await.map_err(EngineError::Io)?;

    let filename = input
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("document")
        .to_string();

    let ext = input
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or_default();
    let lo_filter = for_extension(ext);
    // for_extension returns "pdf:writer_pdf_Export" (CLI format).
    // unoserver expects just the filter name: "writer_pdf_Export".
    let filtername = lo_filter.split_once(':').map(|(_, name)| name);

    let file_part = reqwest::multipart::Part::bytes(file_bytes).file_name(filename);

    let mut form = reqwest::multipart::Form::new()
        .text("output-file", "output.pdf")
        .part("file", file_part);

    if let Some(name) = filtername {
        form = form.text("filtername", name);
    }
    if let Some(blob) = opts.filter_blob() {
        form = form.text("filteroptions", blob);
    }

    let url = format!("http://127.0.0.1:{port}/");

    let result = tokio::time::timeout(timeout, async {
        let resp = client
            .post(&url)
            .multipart(form)
            .send()
            .await
            .map_err(|e| EngineError::Internal(format!("unoserver request: {e}")))?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(EngineError::Internal(format!("unoserver {status}: {body}")));
        }

        resp.bytes()
            .await
            .map(|b| b.to_vec())
            .map_err(|e| EngineError::Internal(format!("unoserver read body: {e}")))
    })
    .await
    .map_err(|_| EngineError::Timeout(timeout))??;

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::StatusCode;
    use axum::response::Response;
    use axum::routing::post;
    use axum::Router;
    use tempfile::Builder;

    async fn start_mock_unoserver(
        handler: impl Fn() -> Response<Body> + Send + Sync + Clone + 'static,
    ) -> u16 {
        let app = Router::new().route(
            "/",
            post(move || {
                let h = handler.clone();
                async move { h() }
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        port
    }

    fn fake_docx() -> tempfile::NamedTempFile {
        let f = Builder::new().suffix(".docx").tempfile().unwrap();
        std::fs::write(f.path(), b"PK fake docx content").unwrap();
        f
    }

    #[tokio::test]
    async fn run_convert_returns_pdf_bytes_on_success() {
        let port = start_mock_unoserver(|| {
            Response::builder()
                .status(StatusCode::OK)
                .header("Content-Type", "application/pdf")
                .body(Body::from(b"%PDF-1.4 fake".to_vec()))
                .unwrap()
        })
        .await;

        let tmp = fake_docx();
        let client = reqwest::Client::new();
        let result = run_convert(&client, port, Duration::from_secs(5), tmp.path(), &OfficeOptions::default()).await;
        assert!(result.is_ok(), "{result:?}");
        assert_eq!(result.unwrap(), b"%PDF-1.4 fake");
    }

    #[tokio::test]
    async fn run_convert_maps_http_500_to_engine_error() {
        let port = start_mock_unoserver(|| {
            Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(Body::from("unsupported format"))
                .unwrap()
        })
        .await;

        let tmp = fake_docx();
        let client = reqwest::Client::new();
        let result = run_convert(&client, port, Duration::from_secs(5), tmp.path(), &OfficeOptions::default()).await;
        assert!(matches!(result, Err(EngineError::Internal(_))), "{result:?}");
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("500") || msg.contains("unsupported"), "{msg}");
    }

    #[tokio::test]
    async fn run_convert_returns_error_when_nothing_listening() {
        let client = reqwest::Client::new();
        let tmp = fake_docx();
        // Port 19877 — nothing is listening here.
        let result = run_convert(&client, 19877, Duration::from_millis(200), tmp.path(), &OfficeOptions::default()).await;
        assert!(result.is_err(), "expected error when nothing listening");
    }

    #[tokio::test]
    async fn run_convert_sends_correct_filtername_for_docx() {
        use axum::extract::Multipart;
        use std::sync::{Arc, Mutex};

        let captured_filtername: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
        let captured_clone = Arc::clone(&captured_filtername);

        let app = Router::new().route(
            "/",
            post(move |mut multipart: Multipart| {
                let captured = Arc::clone(&captured_clone);
                async move {
                    while let Ok(Some(field)) = multipart.next_field().await {
                        if field.name() == Some("filtername") {
                            let val = field.text().await.unwrap_or_default();
                            *captured.lock().unwrap() = Some(val);
                        } else {
                            // drain other fields
                            let _ = field.bytes().await;
                        }
                    }
                    Response::builder()
                        .status(StatusCode::OK)
                        .header("Content-Type", "application/pdf")
                        .body(Body::from(b"%PDF-1.4".to_vec()))
                        .unwrap()
                }
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        let tmp = Builder::new().suffix(".docx").tempfile().unwrap();
        std::fs::write(tmp.path(), b"PK fake docx").unwrap();

        let client = reqwest::Client::new();
        let result = run_convert(&client, port, Duration::from_secs(5), tmp.path(), &OfficeOptions::default()).await;
        assert!(result.is_ok(), "{result:?}");

        let name = captured_filtername.lock().unwrap().clone();
        assert_eq!(
            name.as_deref(),
            Some("writer_pdf_Export"),
            "expected 'writer_pdf_Export', got: {name:?}"
        );
    }

    #[tokio::test]
    async fn run_convert_omits_filtername_for_unknown_extension() {
        use axum::extract::Multipart;
        use std::sync::{Arc, Mutex};

        let saw_filtername: Arc<Mutex<bool>> = Arc::new(Mutex::new(false));
        let saw_clone = Arc::clone(&saw_filtername);

        let app = Router::new().route(
            "/",
            post(move |mut multipart: Multipart| {
                let saw = Arc::clone(&saw_clone);
                async move {
                    while let Ok(Some(field)) = multipart.next_field().await {
                        if field.name() == Some("filtername") {
                            *saw.lock().unwrap() = true;
                        }
                        let _ = field.bytes().await;
                    }
                    Response::builder()
                        .status(StatusCode::OK)
                        .header("Content-Type", "application/pdf")
                        .body(Body::from(b"%PDF-1.4".to_vec()))
                        .unwrap()
                }
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        // ".zzz" is an unknown extension — for_extension returns "pdf" (no colon)
        let tmp = Builder::new().suffix(".zzz").tempfile().unwrap();
        std::fs::write(tmp.path(), b"unknown content").unwrap();

        let client = reqwest::Client::new();
        let result = run_convert(&client, port, Duration::from_secs(5), tmp.path(), &OfficeOptions::default()).await;
        assert!(result.is_ok(), "{result:?}");
        assert!(!*saw_filtername.lock().unwrap(), "filtername should not be sent for unknown extensions");
    }
}
