//! HTTP request/response step definitions.
//!
//! Maps Gotenberg's HTTP steps:
//! - `iMakeARequestToGotenberg` -> `make_request`
//! - `theResponseStatusCodeShouldBe` -> `check_status_code`
//! - `theResponseHeaderShouldBe` -> `check_header`
//! - `theResponseBodyShouldMatchJSON` -> `check_json_body`

use cucumber::gherkin::Table;

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
    world.response = Some(response);

    // Read body
    let body = world
        .response
        .as_ref()
        .unwrap()
        .bytes()
        .await
        .expect("Failed to read response body")
        .to_vec();
    world.body = Some(body);
}

/// Step: Then the response status code should be 200
pub async fn check_status_code(world: &mut FolioWorld, expected: u16) {
    let actual = world.response.as_ref().unwrap().status().as_u16();
    assert_eq!(
        actual, expected,
        "Expected status code {}, got {}",
        expected, actual
    );
}

/// Step: Then the response header "Content-Type" should be "application/json"
pub async fn check_header(world: &mut FolioWorld, header_name: String, expected: String) {
    let headers = world.response.as_ref().unwrap().headers();
    let actual = headers
        .get(&header_name)
        .map(|v| v.to_str().unwrap_or(""))
        .unwrap_or("");
    assert_eq!(
        actual, expected,
        "Expected header {} to be '{}', got '{}'",
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
    method: String,
    endpoint: String,
    table: &Table,
) {
    let url = format!("{}{}", world.base_url.as_ref().unwrap(), endpoint);

    // Build multipart form
    let form = build_form_from_table(world, table).await;

    let response = match method.as_str() {
        "POST" => world.client.post(&url).multipart(form).send().await,
        _ => panic!("Only POST supported for form data"),
    };

    let response = response.expect("Failed to make HTTP request");
    world.response = Some(response);

    // Read body
    let body = world
        .response
        .as_ref()
        .unwrap()
        .bytes()
        .await
        .expect("Failed to read response body")
        .to_vec();
    world.body = Some(body);
}

/// Build multipart form from Gherkin table
async fn build_form_from_table(
    world: &mut FolioWorld,
    table: &Table,
) -> reqwest::multipart::Form {
    let mut form = reqwest::multipart::Form::new();

    for row in table.rows.iter() {
        if row.cells.len() >= 3 {
            let field_name = &row.cells[0].value;
            let field_value = &row.cells[1].value;
            let field_type = &row.cells[2].value;

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

                    let part = reqwest::multipart::Part::bytes(content)
                        .file_name(field_value.clone())
                        .mime_str("application/pdf")
                        .unwrap();

                    form = form.part(field_name.clone(), part);
                }
                "header" => {
                    // Headers are handled separately
                }
                _ => {
                    // Regular form field
                    form = form.text(field_name.clone(), field_value.clone());
                }
            }
        }
    }

    form
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
