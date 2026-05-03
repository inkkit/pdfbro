#![allow(dead_code)]

//! Container lifecycle step definitions.
//!
//! Maps Gotenberg's container steps:
//! - `iHaveADefaultGotenbergContainer` -> `default_container`
//! - `iHaveAGotenbergContainerWithEnv` -> `container_with_env`

use std::collections::HashMap;

use cucumber::gherkin::Table;

use crate::support::world::PdfBroWorld;

/// Step: Given I have a default pdfbro container
pub async fn default_container(world: &mut PdfBroWorld) {
    world.start_container(HashMap::new()).await;
}

/// Step: Given I have a pdfbro container with environment variables
///
/// Table format:
/// | VAR_NAME | value |
/// | VAR_NAME2 | value2 |
/// Step: Then the logs should contain "<substring>"
pub async fn check_logs_contain(world: &mut PdfBroWorld, substring: String) {
    let found = world.logs.iter().any(|line| line.contains(&substring));
    assert!(
        found,
        "Expected logs to contain {:?} but found:\n{}",
        substring,
        world.logs.join("\n")
    );
}

pub async fn container_with_env(world: &mut PdfBroWorld, table: &Table) {
    let mut env = HashMap::new();

    // Table in cucumber 0.21 is Vec<Vec<String>>
    for row in table.rows.iter() {
        if row.len() >= 2 {
            let key = row[0].clone();
            let value = row[1].clone();
            env.insert(key, value);
        }
    }

    world.start_container(env).await;
}
