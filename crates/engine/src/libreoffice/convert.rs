//! XML-RPC conversion via a running unoserver process.
//!
//! unoserver 2.x exposes a `convert()` method over XML-RPC at its HTTP port.
//! This module builds the minimal request, sends it, and decodes the response.

use std::path::Path;
use std::time::Duration;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as B64;

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
    let indata_b64 = B64.encode(&file_bytes);

    let ext = input
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or_default();
    let lo_filter = for_extension(ext);
    // for_extension returns the soffice CLI format ("pdf:writer_pdf_Export").
    // unoserver's convert() takes only the bare LO filter name ("writer_pdf_Export").
    let filtername = lo_filter.split_once(':').map(|(_, name)| name);

    let body = build_xmlrpc_request(&indata_b64, filtername, opts.filter_blob().as_deref());

    let url = format!("http://127.0.0.1:{port}/");

    let result = tokio::time::timeout(timeout, async {
        let resp = client
            .post(&url)
            .header("Content-Type", "text/xml")
            .body(body)
            .send()
            .await
            .map_err(|e| EngineError::Internal(format!("unoserver request: {e}")))?;

        let status = resp.status();
        let text = resp
            .text()
            .await
            .map_err(|e| EngineError::Internal(format!("unoserver read body: {e}")))?;

        // unoserver returns 200 even for XML-RPC faults, so parse the body.
        if !status.is_success() {
            return Err(EngineError::Internal(format!(
                "unoserver HTTP {status}: {text}"
            )));
        }

        parse_xmlrpc_response(&text)
            .map_err(|e| EngineError::Internal(format!("unoserver fault: {e}")))
    })
    .await
    .map_err(|_| EngineError::Timeout(timeout))??;

    Ok(result)
}

// ── XML-RPC helpers ──────────────────────────────────────────────────────────

fn build_xmlrpc_request(
    indata_b64: &str,
    filtername: Option<&str>,
    filteroptions: Option<&str>,
) -> String {
    let filter_param = match filtername {
        Some(f) => format!(
            "<param><value><string>{}</string></value></param>",
            xml_escape(f)
        ),
        None => "<param><value><nil/></value></param>".to_string(),
    };
    // filter_options must be an array (even empty); unoserver iterates over it.
    let filteroptions_param = match filteroptions {
        Some(fo) => {
            // Each option is a "Name=Value" string element in the array.
            let items: String = fo
                .split('\n')
                .filter(|s| !s.is_empty())
                .map(|s| format!("<value><string>{}</string></value>", xml_escape(s)))
                .collect();
            format!(
                "<param><value><array><data>{items}</data></array></value></param>"
            )
        }
        None => {
            "<param><value><array><data></data></array></value></param>".to_string()
        }
    };
    // convert_to must be "pdf" when outpath is nil, otherwise unoserver
    // cannot determine the output format and raises a TypeError.
    format!(
        "<?xml version=\"1.0\"?>\n\
         <methodCall>\n\
           <methodName>convert</methodName>\n\
           <params>\n\
             <param><value><nil/></value></param>\n\
             <param><value><base64>{indata_b64}</base64></value></param>\n\
             <param><value><nil/></value></param>\n\
             <param><value><string>pdf</string></value></param>\n\
             {filter_param}\n\
             {filteroptions_param}\n\
             <param><value><boolean>0</boolean></value></param>\n\
           </params>\n\
         </methodCall>"
    )
}

fn parse_xmlrpc_response(body: &str) -> Result<Vec<u8>, String> {
    if body.contains("<fault>") {
        let msg = extract_between(body, "<string>", "</string>").unwrap_or("unknown fault");
        return Err(msg.to_string());
    }
    let b64 = extract_between(body, "<base64>", "</base64>")
        .ok_or_else(|| format!("no <base64> element in response: {body}"))?;
    // Python's base64 module wraps encoded output at 76 chars with newlines.
    // Strip all whitespace before decoding so embedded newlines don't break the decoder.
    let b64_clean: String = b64.chars().filter(|c| !c.is_ascii_whitespace()).collect();
    B64.decode(b64_clean.as_bytes())
        .map_err(|e| format!("base64 decode failed: {e}"))
}

fn extract_between<'a>(haystack: &'a str, open: &str, close: &str) -> Option<&'a str> {
    let start = haystack.find(open)? + open.len();
    let end = haystack[start..].find(close)? + start;
    Some(&haystack[start..end])
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::StatusCode;
    use axum::response::Response;
    use axum::routing::post;
    use axum::Router;
    use tempfile::Builder;

    fn xmlrpc_ok_response(pdf: &[u8]) -> String {
        format!(
            "<?xml version=\"1.0\"?>\n\
             <methodResponse>\n\
               <params>\n\
                 <param><value><base64>{}</base64></value></param>\n\
               </params>\n\
             </methodResponse>",
            B64.encode(pdf)
        )
    }

    fn xmlrpc_fault_response(msg: &str) -> String {
        format!(
            "<?xml version=\"1.0\"?>\n\
             <methodResponse>\n\
               <fault><value><struct>\n\
                 <member><name>faultCode</name><value><int>1</int></value></member>\n\
                 <member><name>faultString</name><value><string>{msg}</string></value></member>\n\
               </struct></value></fault>\n\
             </methodResponse>"
        )
    }

    async fn start_mock_unoserver(
        handler: impl Fn(String) -> Response<Body> + Send + Sync + Clone + 'static,
    ) -> u16 {
        let app = Router::new().route(
            "/",
            post(move |body: String| {
                let h = handler.clone();
                async move { h(body) }
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
        let port = start_mock_unoserver(|_body| {
            Response::builder()
                .status(StatusCode::OK)
                .header("Content-Type", "text/xml")
                .body(Body::from(xmlrpc_ok_response(b"%PDF-1.4 fake")))
                .unwrap()
        })
        .await;

        let tmp = fake_docx();
        let client = reqwest::Client::new();
        let result = run_convert(
            &client,
            port,
            Duration::from_secs(5),
            tmp.path(),
            &OfficeOptions::default(),
        )
        .await;
        assert!(result.is_ok(), "{result:?}");
        assert_eq!(result.unwrap(), b"%PDF-1.4 fake");
    }

    #[tokio::test]
    async fn run_convert_maps_xmlrpc_fault_to_engine_error() {
        let port = start_mock_unoserver(|_body| {
            Response::builder()
                .status(StatusCode::OK)
                .header("Content-Type", "text/xml")
                .body(Body::from(xmlrpc_fault_response("unsupported format")))
                .unwrap()
        })
        .await;

        let tmp = fake_docx();
        let client = reqwest::Client::new();
        let result = run_convert(
            &client,
            port,
            Duration::from_secs(5),
            tmp.path(),
            &OfficeOptions::default(),
        )
        .await;
        assert!(matches!(result, Err(EngineError::Internal(_))), "{result:?}");
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("unsupported format"), "{msg}");
    }

    #[tokio::test]
    async fn run_convert_returns_error_when_nothing_listening() {
        let client = reqwest::Client::new();
        let tmp = fake_docx();
        // Port 19877 — nothing is listening here.
        let result =
            run_convert(&client, 19877, Duration::from_millis(200), tmp.path(), &OfficeOptions::default()).await;
        assert!(result.is_err(), "expected error when nothing listening");
    }

    #[tokio::test]
    async fn run_convert_sends_correct_filtername_for_docx() {
        use std::sync::{Arc, Mutex};
        let captured: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
        let cap2 = Arc::clone(&captured);

        let port = start_mock_unoserver(move |body| {
            // The XML has two <string> params: first is convert_to="pdf",
            // second (after "pdf") is the filtername. Extract the second one.
            let after_pdf = body
                .find("<string>pdf</string>")
                .and_then(|pos| body.get(pos + "<string>pdf</string>".len()..));
            if let Some(rest) = after_pdf {
                if let Some(v) = extract_between(rest, "<string>", "</string>") {
                    *cap2.lock().unwrap() = Some(v.to_string());
                }
            }
            Response::builder()
                .status(StatusCode::OK)
                .header("Content-Type", "text/xml")
                .body(Body::from(xmlrpc_ok_response(b"%PDF-1.4")))
                .unwrap()
        })
        .await;

        let tmp = Builder::new().suffix(".docx").tempfile().unwrap();
        std::fs::write(tmp.path(), b"PK fake docx").unwrap();
        let client = reqwest::Client::new();
        let result = run_convert(
            &client,
            port,
            Duration::from_secs(5),
            tmp.path(),
            &OfficeOptions::default(),
        )
        .await;
        assert!(result.is_ok(), "{result:?}");

        let name = captured.lock().unwrap().clone();
        assert_eq!(
            name.as_deref(),
            Some("writer_pdf_Export"),
            "expected 'writer_pdf_Export', got: {name:?}"
        );
    }

    #[tokio::test]
    async fn run_convert_omits_filtername_for_unknown_extension() {
        use std::sync::{Arc, Mutex};
        let saw_filter: Arc<Mutex<bool>> = Arc::new(Mutex::new(false));
        let saw2 = Arc::clone(&saw_filter);

        let port = start_mock_unoserver(move |body| {
            // The request always has <string>pdf</string> for convert_to.
            // If there's an additional <string> after "pdf", a filtername was sent.
            let after_pdf = body
                .find("<string>pdf</string>")
                .and_then(|pos| body.get(pos + "<string>pdf</string>".len()..));
            if after_pdf.map_or(false, |rest| rest.contains("<string>")) {
                *saw2.lock().unwrap() = true;
            }
            Response::builder()
                .status(StatusCode::OK)
                .header("Content-Type", "text/xml")
                .body(Body::from(xmlrpc_ok_response(b"%PDF-1.4")))
                .unwrap()
        })
        .await;

        // ".zzz" is an unknown extension — for_extension returns "pdf" (no colon)
        let tmp = Builder::new().suffix(".zzz").tempfile().unwrap();
        std::fs::write(tmp.path(), b"unknown content").unwrap();

        let client = reqwest::Client::new();
        let result = run_convert(
            &client,
            port,
            Duration::from_secs(5),
            tmp.path(),
            &OfficeOptions::default(),
        )
        .await;
        assert!(result.is_ok(), "{result:?}");
        assert!(
            !*saw_filter.lock().unwrap(),
            "filtername should not be sent for unknown extensions"
        );
    }

    #[test]
    fn build_xmlrpc_request_contains_filtername() {
        let req = build_xmlrpc_request("DATA==", Some("writer_pdf_Export"), None);
        assert!(req.contains("writer_pdf_Export"), "{req}");
        assert!(req.contains("<base64>DATA==</base64>"), "{req}");
    }

    #[test]
    fn build_xmlrpc_request_without_filtername_uses_nil() {
        let req = build_xmlrpc_request("DATA==", None, None);
        // Should contain exactly one <string> element: the convert_to="pdf".
        // No extra <string> for filtername.
        let count = req.matches("<string>").count();
        assert_eq!(count, 1, "expected only convert_to string, got {count}: {req}");
        assert!(req.contains("<string>pdf</string>"), "{req}");
    }

    #[test]
    fn parse_xmlrpc_response_decodes_pdf() {
        let body = xmlrpc_ok_response(b"%PDF-1.4 test");
        let result = parse_xmlrpc_response(&body).unwrap();
        assert_eq!(result, b"%PDF-1.4 test");
    }

    #[test]
    fn parse_xmlrpc_response_returns_err_on_fault() {
        let body = xmlrpc_fault_response("conversion failed");
        let result = parse_xmlrpc_response(&body);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("conversion failed"));
    }
}
