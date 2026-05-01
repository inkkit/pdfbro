#![allow(dead_code)]

//! HTTP request/response step definitions.
//!
//! Maps Gotenberg's HTTP steps:
//! - `iMakeARequestToGotenberg` -> `make_request`
//! - `theResponseStatusCodeShouldBe` -> `check_status_code`
//! - `theResponseHeaderShouldBe` -> `check_header`
//! - `theResponseBodyShouldMatchJSON` -> `check_json_body`

use cucumber::gherkin::Table;
use reqwest;

use crate::support::world::FolioWorld;

/// Step: When I make a "GET" request to "/health"
pub async fn make_request(world: &mut FolioWorld, method: String, endpoint: String) {
    make_request_with_request_headers(world, method, endpoint, &[]).await;
}

/// Step: When I make a "GET" request to "/health" with the following header(s):
pub async fn make_request_with_headers(
    world: &mut FolioWorld,
    method: String,
    endpoint: String,
    table: &cucumber::gherkin::Table,
) {
    let extra: Vec<(String, String)> = table
        .rows
        .iter()
        .filter(|row| row.len() >= 2)
        .map(|row| (row[0].clone(), row[1].clone()))
        .collect();
    make_request_with_request_headers(world, method, endpoint, &extra).await;
}

async fn make_request_with_request_headers(
    world: &mut FolioWorld,
    method: String,
    endpoint: String,
    extra_headers: &[(String, String)],
) {
    let url = format!("{}{}", world.base_url.as_ref().unwrap(), endpoint);

    let mut req = match method.as_str() {
        "GET" => world.client.get(&url),
        "POST" => world.client.post(&url),
        "PUT" => world.client.put(&url),
        "DELETE" => world.client.delete(&url),
        "HEAD" => world.client.head(&url),
        _ => panic!("Unsupported HTTP method: {}", method),
    };

    for (name, value) in extra_headers {
        req = req.header(name.as_str(), value.as_str());
    }

    let response = req.send().await.expect("Failed to make HTTP request");
    let status = response.status().as_u16();

    let mut headers = std::collections::HashMap::new();
    for (name, value) in response.headers() {
        headers.insert(name.as_str().to_string(), value.to_str().unwrap_or("").to_string());
    }

    let body = response
        .bytes()
        .await
        .expect("Failed to read response body")
        .to_vec();

    world.body = Some(body);
    world.status_code = Some(status);
    world.response_headers = Some(headers);
}

/// Step: Then the response status code should be 200
pub async fn check_status_code(world: &mut FolioWorld, expected: u16) {
    let actual = world.status_code.expect("No status code available");
    let body = String::from_utf8_lossy(world.body.as_deref().unwrap_or(&[]));
    assert_eq!(
        actual, expected,
        "Expected status code {}, got {}. Body: {}",
        expected, actual, body
    );
}

/// Step: Then the response header "Content-Type" should be "application/json"
pub async fn check_header(world: &mut FolioWorld, header_name: String, expected: String) {
    let headers = world.response_headers.as_ref().expect("No response headers available");
    let lower_name = header_name.to_lowercase();
    let actual = headers.get(&lower_name).unwrap_or_else(|| {
        panic!(
            "Header '{}' (lowercase: '{}') not found in response. Available headers: {:?}",
            header_name, lower_name,
            headers.keys().collect::<Vec<_>>()
        )
    });
    assert_eq!(
        actual, &expected,
        "Expected header '{}' to be '{}', got '{}'",
        header_name, expected, actual
    );
}

/// Step: Then the response body should match JSON:
/// """
/// {"status": "up"}
/// """
pub async fn check_json_body(world: &mut FolioWorld, expected: String) {
    let body_str = String::from_utf8_lossy(world.body.as_ref().unwrap());

    // Parse both as JSON for comparison
    let expected_json: serde_json::Value =
        serde_json::from_str(&expected).expect("Failed to parse expected JSON");
    let actual_json: serde_json::Value =
        serde_json::from_str(&body_str).expect("Failed to parse actual JSON");

    // Compare (with special handling for "ignore" values)
    assert_json_matches(&expected_json, &actual_json, "");
}

/// Step: When I make a POST request with form data
pub async fn make_request_with_form(
    world: &mut FolioWorld,
    _method: String,
    endpoint: String,
    table: &Table,
) {
    let url = format!("{}{}", world.base_url.as_ref().unwrap(), endpoint);

    // Build multipart form and collect headers from table
    let (form, headers) = build_form_and_headers_from_table(world, table).await;

    let mut req = world.client.post(&url).multipart(form);
    for (name, value) in headers {
        req = req.header(name, value);
    }
    let response = req.send().await;

    let response = response.expect("Failed to make HTTP request");
    let status = response.status().as_u16();

    // Collect response headers
    let mut resp_headers = std::collections::HashMap::new();
    for (name, value) in response.headers() {
        resp_headers.insert(name.as_str().to_string(), value.to_str().unwrap_or("").to_string());
    }

    // Read body - need to take ownership
    let body = response
        .bytes()
        .await
        .expect("Failed to read response body")
        .to_vec();

    // Store body, status, and headers
    world.body = Some(body.clone());
    world.status_code = Some(status);
    world.response_headers = Some(resp_headers.clone());

    // Teststore: save successful PDF/ZIP responses so subsequent steps can reference them
    if status == 200 {
        let content_type = resp_headers.get("content-type").cloned().unwrap_or_default();
        let content_disp = resp_headers.get("content-disposition").cloned().unwrap_or_default();
        let filename = extract_filename_from_disposition(&content_disp);
        let teststore = std::path::Path::new("tests/bdd/testdata/teststore");
        let _ = std::fs::create_dir_all(teststore);

        if content_type.starts_with("application/pdf") {
            if !filename.is_empty() {
                let _ = std::fs::write(teststore.join(&filename), &body);
            }
        } else if content_type.starts_with("application/zip") {
            if !filename.is_empty() {
                let _ = std::fs::write(teststore.join(&filename), &body);
            }
            // Also extract zip contents into teststore
            if let Ok(mut archive) = zip::ZipArchive::new(std::io::Cursor::new(&body)) {
                for i in 0..archive.len() {
                    if let Ok(mut entry) = archive.by_index(i) {
                        let entry_name = entry.name().to_string();
                        if entry_name.ends_with(".pdf") {
                            let mut contents = Vec::new();
                            use std::io::Read;
                            let _ = entry.read_to_end(&mut contents);
                            let _ = std::fs::write(teststore.join(&entry_name), &contents);
                        }
                    }
                }
            }
        }
    }
}

fn extract_filename_from_disposition(disposition: &str) -> String {
    // Parse: attachment; filename="foo.pdf"
    for part in disposition.split(';') {
        let part = part.trim();
        if let Some(rest) = part.strip_prefix("filename=") {
            return rest.trim_matches('"').to_string();
        }
    }
    String::new()
}

/// Build multipart form from Gherkin table, also extracting headers.
async fn build_form_and_headers_from_table(
    _world: &mut FolioWorld,
    table: &Table,
) -> (reqwest::multipart::Form, Vec<(String, String)>) {
    let mut form = reqwest::multipart::Form::new();
    let mut headers = Vec::new();

    // Table in cucumber 0.21 is Vec<Vec<String>>
    for row in table.rows.iter() {
        if row.len() >= 3 {
            let field_name = &row[0];
            let field_value = &row[1];
            let field_type = &row[2];

            match field_type.as_str() {
                "file" => {
                    // Strip optional "testdata/" prefix (Gotenberg-style paths)
                    let relative = field_value
                        .strip_prefix("testdata/")
                        .unwrap_or(field_value);
                    let file_path = format!("tests/bdd/testdata/{}", relative);
                    let content = tokio::fs::read(&file_path)
                        .await
                        .unwrap_or_else(|e| panic!("Failed to read file {file_path}: {e}"));

                    // Use basename as the multipart filename
                    let file_name = std::path::Path::new(relative)
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or(relative)
                        .to_string();

                    let mime = guess_mime_type(&file_name);

                    let part = reqwest::multipart::Part::bytes(content)
                        .file_name(file_name)
                        .mime_str(&mime)
                        .unwrap();

                    form = form.part(field_name.clone(), part);
                }
                "field" => {
                    // Regular form field (same as text)
                    form = form.text(field_name.clone(), field_value.clone());
                }
                "header" => {
                    headers.push((field_name.clone(), field_value.clone()));
                }
                _ => {
                    // Default: treat as text field
                    form = form.text(field_name.clone(), field_value.clone());
                }
            }
        }
    }

    (form, headers)
}

/// Guess MIME type from file extension
fn guess_mime_type(filename: &str) -> String {
    if filename.ends_with(".pdf") {
        "application/pdf".to_string()
    } else if filename.ends_with(".html") || filename.ends_with(".htm") {
        "text/html".to_string()
    } else if filename.ends_with(".md") {
        "text/markdown".to_string()
    } else if filename.ends_with(".docx") {
        "application/vnd.openxmlformats-officedocument.wordprocessingml.document".to_string()
    } else if filename.ends_with(".xml") {
        "application/xml".to_string()
    } else if filename.ends_with(".png") {
        "image/png".to_string()
    } else if filename.ends_with(".jpg") || filename.ends_with(".jpeg") {
        "image/jpeg".to_string()
    } else {
        "application/octet-stream".to_string()
    }
}

/// Compare JSON values, handling "ignore" placeholder
fn assert_json_matches(expected: &serde_json::Value, actual: &serde_json::Value, path: &str) {
    match (expected, actual) {
        // Handle "ignore" placeholder
        (serde_json::Value::String(s), _) if s == "ignore" => {
            // Skip comparison
        }
        // Compare objects
        (serde_json::Value::Object(exp_map), serde_json::Value::Object(act_map)) => {
            for (key, exp_val) in exp_map {
                let new_path = format!("{}.{}", path, key);
                let act_val = act_map
                    .get(key)
                    .expect(&format!("Missing key: {}", new_path));
                assert_json_matches(exp_val, act_val, &new_path);
            }
        }
        // Compare arrays
        (serde_json::Value::Array(exp_arr), serde_json::Value::Array(act_arr)) => {
            assert_eq!(
                exp_arr.len(),
                act_arr.len(),
                "Array length mismatch at {}: expected {}, got {}",
                path,
                exp_arr.len(),
                act_arr.len()
            );
            for (i, (exp, act)) in exp_arr.iter().zip(act_arr.iter()).enumerate() {
                let new_path = format!("{}[{}]", path, i);
                assert_json_matches(exp, act, &new_path);
            }
        }
        // Compare primitives
        _ => {
            assert_eq!(
                expected, actual,
                "Value mismatch at {}: expected {:?}, got {:?}",
                path, expected, actual
            );
        }
    }
}

/// Step: When I make concurrent "POST" requests to "/forms/chromium/convert/html" with the following form data:
/// Sends 10 concurrent requests and collects all responses.
pub async fn make_concurrent_requests(world: &mut FolioWorld, _method: String, endpoint: String, table: &Table) {
    let url = format!("{}{}", world.base_url.as_ref().unwrap(), endpoint);

    // Store table data for rebuilding forms (Form doesn't implement Clone)
    let table_rows: Vec<(String, String, String)> = table.rows.iter()
        .filter(|row| row.len() >= 3)
        .map(|row| (row[0].clone(), row[1].clone(), row[2].clone()))
        .collect();

    // Spawn 10 concurrent requests
    let client = world.client.clone();
    let mut handles = Vec::new();

    for _ in 0..10 {
        let client = client.clone();
        let url = url.clone();
        let rows = table_rows.clone();

        let handle = tokio::spawn(async move {
            // Rebuild form for this request
            let mut form = reqwest::multipart::Form::new();
            for (field_name, field_value, field_type) in rows {
                match field_type.as_str() {
                    "file" => {
                        let file_path = format!("tests/bdd/testdata/{}", field_value);
                        let content = match tokio::fs::read(&file_path).await {
                            Ok(c) => c,
                            Err(_) => return (0, Vec::new()),
                        };
                        let mime = guess_mime_type(&field_value);
                        let part = reqwest::multipart::Part::bytes(content)
                            .file_name(field_value)
                            .mime_str(&mime)
                            .unwrap();
                        form = form.part(field_name, part);
                    }
                    "field" => {
                        form = form.text(field_name, field_value);
                    }
                    _ => {}
                }
            }

            let response = client
                .post(&url)
                .multipart(form)
                .send()
                .await;

            match response {
                Ok(resp) => {
                    let status = resp.status().as_u16();
                    let body = resp.bytes().await.unwrap_or_default().to_vec();
                    (status, body)
                }
                Err(_) => (0, Vec::new()),
            }
        });
        handles.push(handle);
    }

    // Collect all responses
    let mut responses = Vec::new();
    for handle in handles {
        if let Ok(result) = handle.await {
            responses.push(result);
        }
    }

    world.concurrent_responses = Some(responses);
}

/// Step: Then all responses should have status code 200
/// Verifies all concurrent responses have the expected status code.
pub async fn check_all_status_codes(world: &mut FolioWorld, expected: u16) {
    let responses = world.concurrent_responses.as_ref().expect("No concurrent responses available");

    for (i, (status, _)) in responses.iter().enumerate() {
        assert_eq!(*status, expected, "Response {} expected status {}, got {}", i, expected, status);
    }
}

/// Step: When I make a "GET" request to "/health" with basic auth "user":"pass"
pub async fn make_request_with_basic_auth(
    world: &mut FolioWorld,
    method: String,
    endpoint: String,
    username: String,
    password: String,
) {
    let url = format!("{}{}", world.base_url.as_ref().unwrap(), endpoint);
    let response = world
        .client
        .request(reqwest::Method::from_bytes(method.as_bytes()).unwrap(), &url)
        .basic_auth(username, Some(password))
        .send()
        .await
        .expect("Failed to make HTTP request");

    let status = response.status().as_u16();
    let mut headers = std::collections::HashMap::new();
    for (name, value) in response.headers() {
        headers.insert(name.as_str().to_string(), value.to_str().unwrap_or("").to_string());
    }
    let body = response.bytes().await.expect("Failed to read response body").to_vec();
    world.body = Some(body);
    world.status_code = Some(status);
    world.response_headers = Some(headers);
}
