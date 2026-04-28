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
    // Run cucumber with default config - features auto-discovered from tests/bdd/features
    FolioWorld::run("tests/bdd/features").await;
}
