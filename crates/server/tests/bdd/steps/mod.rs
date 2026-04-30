//! Step definitions registration.
//!
//! All BDD steps are registered here using cucumber 0.21 macros.

use cucumber::{given, then, when};

use crate::support::world::FolioWorld;

pub mod container;
pub mod http;
pub mod pdf;
pub mod webhook;

// =================================================================
// Container steps (Given)
// =================================================================

#[given("I have a default Folio container")]
async fn default_container(world: &mut FolioWorld) {
    container::default_container(world).await;
}

#[given(regex = r#"I have a Folio container with the following environment variable\(s\):"#)]
async fn container_with_env(world: &mut FolioWorld, step: &cucumber::gherkin::Step) {
    let table = step.table.as_ref().expect("Expected environment variables table");
    container::container_with_env(world, table).await;
}

// =================================================================
// HTTP steps (When)
// =================================================================

#[when(regex = r#"^I make a "(GET|POST|PUT|DELETE)" request to "(.+)"$"#)]
async fn make_request(world: &mut FolioWorld, method: String, endpoint: String) {
    http::make_request(world, method, endpoint).await;
}

#[when(regex = r#"^I make a "(POST)" request to "(.+)" with the following form data and header\(s\):$"#)]
async fn make_request_with_form(
    world: &mut FolioWorld,
    method: String,
    endpoint: String,
    step: &cucumber::gherkin::Step,
) {
    let table = step.table.as_ref().expect("Expected form data table");
    http::make_request_with_form(world, method, endpoint, table).await;
}

#[when(regex = r#"^I make concurrent "(POST)" requests to "(.+)" with the following form data:$"#)]
async fn make_concurrent_requests(
    world: &mut FolioWorld,
    method: String,
    endpoint: String,
    step: &cucumber::gherkin::Step,
) {
    let table = step.table.as_ref().expect("Expected form data table");
    http::make_concurrent_requests(world, method, endpoint, table).await;
}

// =================================================================
// Response assertion steps (Then)
// =================================================================

#[then(regex = r#"the response status code should be (\d+)"#)]
async fn check_status_code(world: &mut FolioWorld, expected: u16) {
    http::check_status_code(world, expected).await;
}

#[then(regex = r#"^all responses should have status code (\d+)$"#)]
async fn check_all_status_codes(world: &mut FolioWorld, expected: u16) {
    http::check_all_status_codes(world, expected).await;
}

#[then(regex = r#"the response header "(.+)" should be "(.+)""#)]
async fn check_header(world: &mut FolioWorld, name: String, value: String) {
    http::check_header(world, name, value).await;
}

#[then(regex = r#"the response body should match JSON:"#)]
async fn check_json_body(world: &mut FolioWorld, step: &cucumber::gherkin::Step) {
    let expected = step.docstring.clone().unwrap_or_default();
    http::check_json_body(world, expected).await;
}

// =================================================================
// PDF assertion steps (Then)
// =================================================================

#[then(regex = r#"there should be (\d+) PDF\(s\) in the response"#)]
async fn check_pdf_count(world: &mut FolioWorld, count: usize) {
    pdf::check_pdf_count(world, count).await;
}

#[then(regex = r#"there should be the following file\(s\) in the response:"#)]
async fn check_files_in_response(world: &mut FolioWorld, step: &cucumber::gherkin::Step) {
    let table = step.table.as_ref().expect("Expected files table");
    let files: Vec<String> = table.rows.iter().map(|row| row[0].clone()).collect();
    pdf::check_files_in_response(world, files).await;
}

#[then(regex = r#"the "(.+)" PDF should have (\d+) page\(s\)"#)]
async fn check_page_count(world: &mut FolioWorld, filename: String, pages: usize) {
    pdf::check_page_count(world, filename, pages).await;
}

#[then(regex = r#"the "(.+)" PDF should have the following content at page (\d+):"#)]
async fn check_page_content(world: &mut FolioWorld, filename: String, page: usize, step: &cucumber::gherkin::Step) {
    let content = step.docstring.clone().unwrap_or_default();
    pdf::check_page_content(world, filename, page, content).await;
}

// =================================================================
// Container log steps (Then)
// =================================================================

#[then(regex = r#"the logs should contain "(.+)""#)]
async fn check_logs_contain(world: &mut FolioWorld, substring: String) {
    container::check_logs_contain(world, substring).await;
}

// =================================================================
// PDF/A and image steps (Then)
// =================================================================

#[then(regex = r#"the "(.+)" PDF should pass PDF/A validation"#)]
async fn check_pdfa_valid(world: &mut FolioWorld, filename: String) {
    pdf::check_pdfa_valid(world, filename).await;
}

#[then(regex = r#"the "(.+)" PDF should have (\d+) image\(s\)"#)]
async fn check_image_count(world: &mut FolioWorld, filename: String, count: usize) {
    pdf::check_image_count(world, filename, count).await;
}

// =================================================================
// HTTP with basic auth (When)
// =================================================================

#[when(regex = r#"^I make a "(GET|POST)" request to "(.+)" with basic auth "(.+)":"(.+)"$"#)]
async fn make_request_basic_auth(
    world: &mut FolioWorld,
    method: String,
    endpoint: String,
    username: String,
    password: String,
) {
    http::make_request_with_basic_auth(world, method, endpoint, username, password).await;
}

// =================================================================
// Webhook stub steps (for @folio-skip scenarios)
// =================================================================

#[given(regex = r#"I have a webhook server"#)]
async fn setup_webhook(world: &mut FolioWorld) {
    webhook::setup_webhook_server(world).await;
}

#[when(regex = r#"I wait for the asynchronous request to the webhook"#)]
async fn wait_for_webhook(world: &mut FolioWorld) {
    webhook::wait_for_webhook(world).await;
}

#[then(regex = r#"the webhook request header "(.+)" should be "(.+)""#)]
async fn check_webhook_header(world: &mut FolioWorld, name: String, value: String) {
    webhook::check_webhook_header(world, name, value).await;
}

#[then(regex = r#"there should be (\d+) PDF\(s\) in the webhook request"#)]
async fn check_webhook_pdfs(world: &mut FolioWorld, count: usize) {
    webhook::check_webhook_pdf_count(world, count).await;
}
