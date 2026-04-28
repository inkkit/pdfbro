//! Cucumber World implementation for BDD tests.
//!
//! Holds test state across steps including HTTP client,
//! Docker container, and response data.

use std::collections::HashMap;

use cucumber::World;
use reqwest::Client;
use testcontainers::Container;
use testcontainers::core::WaitFor;
use testcontainers::GenericImage;

/// Test state shared across BDD steps.
#[derive(Debug, World)]
#[world(init = Self::new)]
pub struct FolioWorld {
    /// HTTP client for requests
    pub client: Client,

    /// Active Docker container
    pub container: Option<Container<GenericImage>>,

    /// Last HTTP response
    pub response: Option<reqwest::Response>,

    /// Response body bytes
    pub body: Option<Vec<u8>>,

    /// Temporary directory for test files
    pub temp_dir: tempfile::TempDir,

    /// Container base URL (e.g., "http://localhost:12345")
    pub base_url: Option<String>,
}

impl FolioWorld {
    /// Create new World instance.
    fn new() -> Self {
        Self {
            client: Client::new(),
            container: None,
            response: None,
            body: None,
            temp_dir: tempfile::tempdir().unwrap(),
            base_url: None,
        }
    }

    /// Start Folio Docker container with environment variables.
    pub async fn start_container(&mut self, env: HashMap<String, String>) {
        let mut image = GenericImage::new("deesh2025/no-name", "latest")
            .with_wait_for(WaitFor::message_on_stdout("Listening on"));

        // Add environment variables
        for (key, value) in env {
            image = image.with_env_var(&key, &value);
        }

        let container = image.start().await.expect("Failed to start container");
        let port = container
            .get_host_port_ipv4(3000)
            .await
            .expect("Failed to get container port");

        self.base_url = Some(format!("http://localhost:{}", port));
        self.container = Some(container);

        // Give container time to fully start
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
    }

    /// Stop and remove container.
    pub async fn stop_container(&mut self) {
        if let Some(container) = self.container.take() {
            drop(container);
        }
        self.base_url = None;
    }
}

impl Drop for FolioWorld {
    fn drop(&mut self) {
        // Container will be stopped when dropped
        if let Some(container) = self.container.take() {
            drop(container);
        }
    }
}
