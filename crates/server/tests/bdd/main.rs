//! BDD test runner using Cucumber.
//!
//! Entry point for running Gherkin feature tests.
//!
//! Run with:
//!   cargo test --test bdd
//!   cargo test --test bdd -- health
//!   cargo test --test bdd -- --nocapture

use std::path::PathBuf;

use cucumber::Cucumber;

mod steps;
mod support;

use steps::steps;
use support::world::FolioWorld;

fn main() {
    // Use tokio runtime
    let rt = tokio::runtime::Runtime::new().unwrap();

    rt.block_on(async {
        Cucumber::<FolioWorld>::new()
            .features(&[PathBuf::from("tests/bdd/features")])
            .steps(steps())
            .run_and_exit()
            .await;
    });
}
