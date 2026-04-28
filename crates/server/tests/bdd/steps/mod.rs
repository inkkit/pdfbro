//! Step definitions registration.
//!
//! All BDD steps are registered here with their regex patterns.

use cucumber::{gherkin::Step, Steps};

use crate::support::world::FolioWorld;

pub mod container;
pub mod http;
pub mod pdf;

/// Create and return all step definitions.
pub fn steps() -> Steps<FolioWorld> {
    let mut steps = Steps::new();

    // =================================================================
    // Container steps (Given)
    // =================================================================

    steps.given(
        "I have a default Folio container",
        container::default_container,
    );

    steps.given_regex(
        r#"I have a Folio container with the following environment variable\(s\):"#,
        container::container_with_env,
    );

    // =================================================================
    // HTTP steps (When)
    // =================================================================

    steps.when_regex(
        r#"I make a "(GET|POST|PUT|DELETE)" request to "(.+)""#,
        |world: &mut FolioWorld, method: String, endpoint: String| {
            http::make_request(world, method, endpoint)
        },
    );

    steps.when_regex(
        r#"I make a "(POST)" request to "(.+)" with the following form data and header\(s\):"#,
        http::make_request_with_form,
    );

    // =================================================================
    // Response assertion steps (Then)
    // =================================================================

    steps.then_regex(
        r#"the response status code should be (\d+)"#,
        |world: &mut FolioWorld, expected: u16| {
            http::check_status_code(world, expected)
        },
    );

    steps.then_regex(
        r#"the response header "(.+)" should be "(.+)""#,
        |world: &mut FolioWorld, name: String, value: String| {
            http::check_header(world, name, value)
        },
    );

    // Special content type checks
    steps.then(
        "the response header \"Content-Type\" should be \"application/zip\"",
        |world: &mut FolioWorld| {
            http::check_header(world, "Content-Type".to_string(), "application/zip".to_string())
        },
    );

    steps.then(
        "the response header \"Content-Type\" should be \"image/png\"",
        |world: &mut FolioWorld| {
            http::check_header(world, "Content-Type".to_string(), "image/png".to_string())
        },
    );

    steps.then(
        "the response header \"Content-Type\" should be \"image/jpeg\"",
        |world: &mut FolioWorld| {
            http::check_header(world, "Content-Type".to_string(), "image/jpeg".to_string())
        },
    );

    steps.then_regex(
        r#"the response body should match JSON:"#,
        |world: &mut FolioWorld, expected: String| {
            http::check_json_body(world, expected)
        },
    );

    // =================================================================
    // PDF assertion steps (Then)
    // =================================================================

    steps.then_regex(
        r#"there should be (\d+) PDF\(s\) in the response"#,
        |world: &mut FolioWorld, count: usize| {
            pdf::check_pdf_count(world, count)
        },
    );

    steps.then_regex(
        r#"there should be the following file\(s\) in the response:"#,
        pdf::check_files_in_response,
    );

    steps.then_regex(
        r#"the "(.+)" PDF should have (\d+) page\(s\)"#,
        |world: &mut FolioWorld, filename: String, pages: usize| {
            pdf::check_page_count(world, filename, pages)
        },
    );

    steps.then_regex(
        r#"the "(.+)" PDF should have the following content at page (\d+):"#,
        |world: &mut FolioWorld, filename: String, page: usize, content: String| {
            pdf::check_page_content(world, filename, page, content)
        },
    );

    steps
}
