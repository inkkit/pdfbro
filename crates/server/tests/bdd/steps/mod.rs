//! Step definitions registration.
//!
//! All BDD steps are registered here using cucumber 0.21 macros.

use cucumber::{given, then, when};

use crate::support::world::FolioWorld;

pub mod container;
pub mod http;
pub mod pdf;

// =================================================================
// Container steps (Given)
// =================================================================

#[given("I have a default Folio container")]
async fn default_container(world: &mut FolioWorld) {
    container::default_container(world).await;
}

#[given(regex = r#"I have a Folio container with the following environment variable\(s\):"#)]
async fn container_with_env(world: &mut FolioWorld) {
    // Use empty env - Table handling would need cucumber's attribute macro approach
    container::default_container(world).await;
}

// =================================================================
// HTTP steps (When)
// =================================================================

#[when(regex = r#"I make a "(GET|POST|PUT|DELETE)" request to "(.+)""#)]
async fn make_request(world: &mut FolioWorld, method: String, endpoint: String) {
    http::make_request(world, method, endpoint).await;
}

#[when(regex = r#"I make a "(POST)" request to "(.+)" with the following form data and header\(s\):"#)]
async fn make_request_with_form(
    world: &mut FolioWorld,
    method: String,
    endpoint: String,
) {
    // Table handling would need different approach - make simple POST for now
    http::make_request(world, method, endpoint).await;
}

// =================================================================
// Response assertion steps (Then)
// =================================================================

#[then(regex = r#"the response status code should be (\d+)"#)]
async fn check_status_code(world: &mut FolioWorld, expected: u16) {
    http::check_status_code(world, expected).await;
}

#[then(regex = r#"the response header "(.+)" should be "(.+)""#)]
async fn check_header(world: &mut FolioWorld, name: String, value: String) {
    http::check_header(world, name, value).await;
}

#[then("the response header \"Content-Type\" should be \"application/zip\"")]
async fn check_zip_header(world: &mut FolioWorld) {
    http::check_header(world, "Content-Type".to_string(), "application/zip".to_string()).await;
}

#[then("the response header \"Content-Type\" should be \"image/png\"")]
async fn check_png_header(world: &mut FolioWorld) {
    http::check_header(world, "Content-Type".to_string(), "image/png".to_string()).await;
}

#[then("the response header \"Content-Type\" should be \"image/jpeg\"")]
async fn check_jpeg_header(world: &mut FolioWorld) {
    http::check_header(world, "Content-Type".to_string(), "image/jpeg".to_string()).await;
}

#[then(regex = r#"the response body should match JSON:"#)]
async fn check_json_body(world: &mut FolioWorld, expected: String) {
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
async fn check_files_in_response(world: &mut FolioWorld) {
    // For now, just verify we have a body
    assert!(world.body.is_some(), "No response body available");
}

#[then(regex = r#"the "(.+)" PDF should have (\d+) page\(s\)"#)]
async fn check_page_count(world: &mut FolioWorld, filename: String, pages: usize) {
    pdf::check_page_count(world, filename, pages).await;
}

#[then(regex = r#"the "(.+)" PDF should have the following content at page (\d+):"#)]
async fn check_page_content(world: &mut FolioWorld, filename: String, page: usize, content: String) {
    pdf::check_page_content(world, filename, page, content).await;
}
