//! Cucumber World implementation for BDD tests.
//!
//! Direct mode: Spawns server as child process (no Docker required)

use std::collections::HashMap;
use std::process::Stdio;

use cucumber::World;
use reqwest::Client;
use tokio::process::Child;
use tokio::io::{AsyncBufReadExt, BufReader};

/// Test state shared across BDD steps.
#[derive(Debug, World)]
#[world(init = Self::new)]
pub struct FolioWorld {
    /// HTTP client for requests
    pub client: Client,

    /// Last HTTP status code
    pub status_code: Option<u16>,

    /// Response body bytes
    pub body: Option<Vec<u8>>,

    /// Temporary directory for test files
    pub temp_dir: tempfile::TempDir,

    /// Server base URL (e.g., "http://localhost:3000")
    pub base_url: Option<String>,

    /// Child process handle
    pub server_process: Option<Child>,
}

impl FolioWorld {
    /// Create new World instance.
    fn new() -> Self {
        Self {
            client: Client::new(),
            status_code: None,
            body: None,
            temp_dir: tempfile::tempdir().unwrap(),
            base_url: None,
            server_process: None,
        }
    }

    /// Start Folio server with environment variables.
    pub async fn start_container(&mut self, env: HashMap<String, String>) {
        let port = 3000;
        let base_url = format!("http://localhost:{}", port);

        // Build command to run folio-server
        let mut cmd = tokio::process::Command::new("cargo");
        cmd.arg("run")
            .arg("--bin")
            .arg("folio-server")
            .arg("--")
            .arg("serve")
            .arg("--port")
            .arg(port.to_string())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        // Add environment variables
        for (key, value) in &env {
            cmd.env(key, value);
        }

        // Spawn process
        let mut child = cmd.spawn().expect(
            "Failed to spawn folio-server. Make sure you're running from the workspace root."
        );

        // Wait for "Listening on" message
        let stdout = child.stdout.take().expect("Failed to get stdout");
        let mut reader = BufReader::new(stdout);
        let mut line = String::new();

        let start_time = std::time::Instant::now();
        let timeout = std::time::Duration::from_secs(180); // Allow time for compilation

        loop {
            if start_time.elapsed() > timeout {
                panic!("Timeout waiting for server to start");
            }

            line.clear();
            match tokio::time::timeout(
                std::time::Duration::from_millis(500),
                reader.read_line(&mut line)
            ).await {
                Ok(Ok(0)) => break, // EOF
                Ok(Ok(_)) => {
                    eprintln!("[SERVER] {}", line.trim());
                    if line.contains("Listening on") {
                        break;
                    }
                }
                Ok(Err(e)) => panic!("Failed to read stdout: {}", e),
                Err(_) => continue, // Timeout, keep waiting
            }
        }

        // Give server a moment to fully initialize
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;

        // Test connectivity
        let client = Client::new();
        let start_time = std::time::Instant::now();
        loop {
            if start_time.elapsed() > std::time::Duration::from_secs(10) {
                panic!("Server did not become ready");
            }

            match client.get(&format!("{}/health", base_url)).send().await {
                Ok(resp) if resp.status().is_success() => break,
                _ => {
                    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
                }
            }
        }

        self.base_url = Some(base_url);
        self.server_process = Some(child);

        eprintln!("✓ Server started at {}", self.base_url.as_ref().unwrap());
    }

    /// Stop server.
    pub async fn stop_container(&mut self) {
        if let Some(mut child) = self.server_process.take() {
            let _ = child.kill().await;
            let _ = child.wait().await;
        }
        self.base_url = None;
    }
}

impl Drop for FolioWorld {
    fn drop(&mut self) {
        // Try to kill server process
        if self.server_process.is_some() {
            let _ = std::process::Command::new("pkill")
                .arg("-f")
                .arg("folio-server")
                .output();
        }
    }
}
