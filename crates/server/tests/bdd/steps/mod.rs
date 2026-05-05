//! Step definitions registration.
//!
//! All BDD steps are registered here using cucumber 0.21 macros.

use cucumber::{given, then, when};
use cucumber::gherkin::Step;

use crate::support::world::PdfBroWorld;

pub mod container;
pub mod http;
pub mod pdf;
pub mod webhook;

// =================================================================
// Container steps (Given)
// =================================================================

#[given("I have a default pdfbro container")]
async fn default_container(world: &mut PdfBroWorld) {
    container::default_container(world).await;
}

#[given(regex = r#"I have a pdfbro container with the following environment variable\(s\):"#)]
async fn container_with_env(world: &mut PdfBroWorld, step: &cucumber::gherkin::Step) {
    let table = step.table.as_ref().expect("Expected environment variables table");
    container::container_with_env(world, table).await;
}

// =================================================================
// HTTP steps (When)
// =================================================================

#[when(regex = r#"^I make a "(GET|POST|PUT|DELETE|HEAD)" request to "(.+)"$"#)]
async fn make_request(world: &mut PdfBroWorld, method: String, endpoint: String) {
    http::make_request(world, method, endpoint).await;
}

#[when(regex = r#"^I make a "(GET|POST|PUT|DELETE|HEAD)" request to "(.+)" with the following header\(s\):$"#)]
async fn make_request_with_headers(
    world: &mut PdfBroWorld,
    method: String,
    endpoint: String,
    step: &Step,
) {
    let table = step.table.as_ref().expect("Expected headers table");
    http::make_request_with_headers(world, method, endpoint, table).await;
}

#[when(regex = r#"^I make a "(POST)" request to "(.+)" with the following form data and header\(s\):$"#)]
async fn make_request_with_form(
    world: &mut PdfBroWorld,
    method: String,
    endpoint: String,
    step: &Step,
) {
    let table = step.table.as_ref().expect("Expected form data table");
    http::make_request_with_form(world, method, endpoint, table).await;
}

#[when(regex = r#"^I make concurrent "(POST)" requests to "(.+)" with the following form data:$"#)]
async fn make_concurrent_requests(
    world: &mut PdfBroWorld,
    method: String,
    endpoint: String,
    step: &Step,
) {
    let table = step.table.as_ref().expect("Expected form data table");
    http::make_concurrent_requests(world, method, endpoint, table).await;
}

// =================================================================
// Response assertion steps (Then)
// =================================================================

#[then(regex = r#"the response status code should be (\d+)"#)]
async fn check_status_code(world: &mut PdfBroWorld, expected: u16) {
    http::check_status_code(world, expected).await;
}

#[then(regex = r#"^all responses should have status code (\d+)$"#)]
async fn check_all_status_codes(world: &mut PdfBroWorld, expected: u16) {
    http::check_all_status_codes(world, expected).await;
}

#[then(regex = r#"the response header "(.+)" should be "(.+)""#)]
async fn check_header(world: &mut PdfBroWorld, name: String, value: String) {
    http::check_header(world, name, value).await;
}

#[then(regex = r#"the response body should match JSON:"#)]
async fn check_json_body(world: &mut PdfBroWorld, step: &Step) {
    let expected = step.docstring.clone().unwrap_or_default();
    http::check_json_body(world, expected).await;
}

#[then(regex = r#"the response body should contain "(.+)""#)]
async fn response_body_should_contain(world: &mut PdfBroWorld, expected: String) {
    let body = String::from_utf8_lossy(world.body.as_deref().unwrap_or(&[]));
    assert!(
        body.contains(&*expected),
        "Expected response body to contain {expected:?}, got: {body}"
    );
}

#[then(regex = r#"the response body should match string:"#)]
async fn response_body_should_match_string(world: &mut PdfBroWorld, step: &Step) {
    let expected = step.docstring.as_deref().unwrap_or("").trim().to_string();
    let body = String::from_utf8_lossy(world.body.as_deref().unwrap_or(&[]));
    assert!(
        body.contains(&*expected),
        "Expected response body to contain {expected:?}, got: {body}"
    );
}

#[then(regex = r#"the response body should contain string:"#)]
async fn response_body_should_contain_string(world: &mut PdfBroWorld, step: &Step) {
    let expected = step.docstring.as_deref().unwrap_or("").trim().to_string();
    let body = String::from_utf8_lossy(world.body.as_deref().unwrap_or(&[]));
    assert!(
        body.contains(&*expected),
        "Expected response body to contain {expected:?}, got: {body}"
    );
}

// =================================================================
// PDF assertion steps (Then)
// =================================================================

#[then(regex = r#"the response PDF\(s\) should pass PDF/A validation"#)]
async fn check_response_pdfa_valid(world: &mut PdfBroWorld) {
    pdf::check_response_pdfa_valid(world).await;
}

#[then(regex = r#"the response PDF\(s\) should be encrypted"#)]
async fn check_response_encrypted(world: &mut PdfBroWorld) {
    pdf::check_response_encrypted(world).await;
}

#[then(regex = r#"there should be (\d+) PDF\(s\) in the response"#)]
async fn check_pdf_count(world: &mut PdfBroWorld, count: usize) {
    pdf::check_pdf_count(world, count).await;
}

#[then(regex = r#"there should be the following file\(s\) in the response:"#)]
async fn check_files_in_response(world: &mut PdfBroWorld, step: &Step) {
    let table = step.table.as_ref().expect("Expected files table");
    let files: Vec<String> = table.rows.iter().map(|row| row[0].clone()).collect();
    pdf::check_files_in_response(world, files).await;
}

#[then(regex = r#"the "(.+)" PDF should have (\d+) page\(s\)"#)]
async fn check_page_count(world: &mut PdfBroWorld, filename: String, pages: usize) {
    pdf::check_page_count(world, filename, pages).await;
}

#[then(regex = r#"the "(.+)" PDF should have the following content at page (\d+):"#)]
async fn check_page_content(world: &mut PdfBroWorld, filename: String, page: usize, step: &Step) {
    let content = step.docstring.clone().unwrap_or_default();
    pdf::check_page_content(world, filename, page, content).await;
}

#[then(regex = r#"the "(.+)" PDF should NOT have the following content at page (\d+):"#)]
async fn check_page_not_contain(world: &mut PdfBroWorld, filename: String, page: usize, step: &Step) {
    let content = step.docstring.clone().unwrap_or_default();
    pdf::check_page_not_contain(world, filename, page, content).await;
}

#[then(regex = r#"the "(.+)" PDF should be set to landscape orientation"#)]
async fn check_landscape(world: &mut PdfBroWorld, filename: String) {
    pdf::check_landscape(world, filename).await;
}

#[then(regex = r#"the "(.+)" PDF should NOT be set to landscape orientation"#)]
async fn check_not_landscape(world: &mut PdfBroWorld, filename: String) {
    pdf::check_not_landscape(world, filename).await;
}

#[then(regex = r#"all concurrent responses should have (\d+) PDF\(s\)"#)]
async fn check_concurrent_pdf_count(world: &mut PdfBroWorld, count: usize) {
    pdf::check_concurrent_pdf_count(world, count).await;
}

#[then(regex = r#"all concurrent response status codes should be (\d+)"#)]
async fn check_all_concurrent_status_codes(world: &mut PdfBroWorld, expected: u16) {
    http::check_all_status_codes(world, expected).await;
}

// =================================================================
// Container log steps (Then)
// =================================================================

#[then(regex = r#"the logs should contain "(.+)""#)]
async fn check_logs_contain(world: &mut PdfBroWorld, substring: String) {
    container::check_logs_contain(world, substring).await;
}

// =================================================================
// PDF/A and image steps (Then)
// =================================================================

#[then(regex = r#"the "(.+)" PDF should pass PDF/A validation"#)]
async fn check_pdfa_valid(world: &mut PdfBroWorld, filename: String) {
    pdf::check_pdfa_valid(world, filename).await;
}

#[then(regex = r#"the "(.+)" PDF should have (\d+) image\(s\)"#)]
async fn check_image_count(world: &mut PdfBroWorld, filename: String, count: usize) {
    pdf::check_image_count(world, filename, count).await;
}

// =================================================================
// HTTP with basic auth (When)
// =================================================================

#[when(regex = r#"^I make a "(GET|POST)" request to "(.+)" with basic auth "(.+)":"(.+)"$"#)]
async fn make_request_basic_auth(
    world: &mut PdfBroWorld,
    method: String,
    endpoint: String,
    username: String,
    password: String,
) {
    http::make_request_with_basic_auth(world, method, endpoint, username, password).await;
}

// =================================================================
// Static server stub (Given I have a static server)
// Used by downloadFrom scenarios; those are real pdfbro features so this
// step starts the nginx fixture server via the FIXTURE_SERVER_URL env var
// if set, otherwise it's a no-op (scenarios relying on it should pass the
// fixture URL via downloadFrom field).
// =================================================================

#[given(regex = r#"I have a static server"#)]
async fn setup_static_server(_world: &mut PdfBroWorld) {
    // No-op: downloadFrom scenarios supply URLs explicitly in their form data.
    // The fixture server (docker-compose.bench.yml fixture-server) must be
    // running separately when these tests are executed in integration mode.
}

// =================================================================
// Webhook stub steps (for @pdfbro-skip scenarios)
// =================================================================

#[given(regex = r#"I have a webhook server"#)]
async fn setup_webhook(world: &mut PdfBroWorld) {
    webhook::setup_webhook_server(world).await;
}

#[when(regex = r#"I wait for the asynchronous request to the webhook"#)]
async fn wait_for_webhook(world: &mut PdfBroWorld) {
    webhook::wait_for_webhook(world).await;
}

#[then(regex = r#"the webhook request header "(.+)" should be "(.+)""#)]
async fn check_webhook_header(world: &mut PdfBroWorld, name: String, value: String) {
    webhook::check_webhook_header(world, name, value).await;
}

#[then(regex = r#"there should be (\d+) PDF\(s\) in the webhook request"#)]
async fn check_webhook_pdfs(world: &mut PdfBroWorld, count: usize) {
    webhook::check_webhook_pdf_count(world, count).await;
}
