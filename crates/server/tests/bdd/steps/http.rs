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
    let url = format!("{}{}", world.base_url.as_ref().unwrap(), endpoint);

    let response = match method.as_str() {
        "GET" => world.client.get(&url).send().await,
        "POST" => world.client.post(&url).send().await,
        "PUT" => world.client.put(&url).send().await,
        "DELETE" => world.client.delete(&url).send().await,
        _ => panic!("Unsupported HTTP method: {}", method),
    };

    let response = response.expect("Failed to make HTTP request");
    let status = response.status().as_u16();

    // Collect headers
    let mut headers = std::collections::HashMap::new();
    for (name, value) in response.headers() {
        headers.insert(name.as_str().to_string(), value.to_str().unwrap_or("").to_string());
    }

    // Read body - need to take ownership
    let body = response
        .bytes()
        .await
        .expect("Failed to read response body")
        .to_vec();

    // Store body, status, and headers
    world.body = Some(body);
    world.status_code = Some(status);
    world.response_headers = Some(headers);
}

/// Step: Then the response status code should be 200
pub async fn check_status_code(world: &mut FolioWorld, expected: u16) {
    let actual = world.status_code.expect("No status code available");
    assert_eq!(
        actual, expected,
        "Expected status code {}, got {}",
        expected, actual
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
    world.body = Some(body);
    world.status_code = Some(status);
    world.response_headers = Some(resp_headers);
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
                    // Read file from testdata directory
                    let file_path = format!(
                        "tests/bdd/testdata/{}",
                        field_value
                    );
                    let content = tokio::fs::read(&file_path)
                        .await
                        .expect(&format!("Failed to read file: {}", file_path));

                    // Guess mime type from extension
                    let mime = guess_mime_type(field_value);

                    let part = reqwest::multipart::Part::bytes(content)
                        .file_name(field_value.clone())
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
