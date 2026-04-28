//! BDD test runner using Cucumber.
//!
//! Entry point for running Gherkin feature tests.
//!
//! Run with:
//!   cargo test --test bdd
//!   cargo test --test bdd -- health
//!   cargo test --test bdd -- --nocapture

use cucumber::World;

mod steps;
mod support;

use support::world::FolioWorld;

#[tokio::main]
async fn main() {
    // cargo test passes libtest flags (e.g. --test-threads) to all test
    // binaries. This custom harness uses default CLI options so those
    // flags don't cause a clap error in cucumber's runner.
    FolioWorld::cucumber()
        .with_default_cli()
        .run("tests/bdd/features")
        .await;
}
