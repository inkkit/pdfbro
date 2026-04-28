//! Container lifecycle step definitions.
//!
//! Maps Gotenberg's container steps:
//! - `iHaveADefaultGotenbergContainer` -> `default_container`
//! - `iHaveAGotenbergContainerWithEnv` -> `container_with_env`

use std::collections::HashMap;

use cucumber::gherkin::Table;

use crate::support::world::FolioWorld;

/// Step: Given I have a default Folio container
pub async fn default_container(world: &mut FolioWorld) {
    world.start_container(HashMap::new()).await;
}

/// Step: Given I have a Folio container with environment variables
///
/// Table format:
/// | VAR_NAME | value |
/// | VAR_NAME2 | value2 |
pub async fn container_with_env(world: &mut FolioWorld, table: &Table) {
    let mut env = HashMap::new();

    // Skip header row if present
    let start_idx = if table.header.is_empty() { 0 } else { 0 };

    for row in table.rows.iter().skip(start_idx) {
        if row.cells.len() >= 2 {
            let key = row.cells[0].value.clone();
            let value = row.cells[1].value.clone();
            env.insert(key, value);
        }
    }

    world.start_container(env).await;
}
