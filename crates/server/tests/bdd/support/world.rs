//! Cucumber World implementation for BDD tests.
//!
//! Supports two modes:
//! - Docker mode: Uses testcontainers (FOLIO_BDD_MODE=docker)
//! - Direct mode: Spawns server as child process (FOLIO_BDD_MODE=direct, default)

use std::collections::HashMap;
use std::process::Stdio;

use cucumber::World;
use reqwest::Client;
use tokio::process::Child;

/// Test state shared across BDD steps.
#[derive(Debug, World)]
#[world(init = Self::new)]
pub struct FolioWorld {
    /// HTTP client for requests
    pub client: Client,

    /// Last HTTP response
    pub response: Option<reqwest::Response>,

    /// Response body bytes
    pub body: Option<Vec<u8>>,

    /// Temporary directory for test files
    pub temp_dir: tempfile::TempDir,

    /// Server base URL (e.g., "http://localhost:3000")
    pub base_url: Option<String>,

    /// Child process handle (for direct mode)
    pub server_process: Option<Child>,
}

impl FolioWorld {
    /// Create new World instance.
    fn new() -> Self {
        Self {
            client: Client::new(),
            response: None,
            body: None,
            temp_dir: tempfile::tempdir().unwrap(),
            base_url: None,
            server_process: None,
        }
    }

    /// Start Folio server with environment variables.
    pub async fn start_container(&mut self, env: HashMap<String, String>) {
        let mode = std::env::var("FOLIO_BDD_MODE").unwrap_or_else(|_| "direct".to_string());

        match mode.as_str() {
            "docker" => self.start_docker_container(env).await,
            _ => self.start_direct_server(env).await,
        }
    }

    /// Start server as direct child process (fast, no Docker required).
    async fn start_direct_server(&mut self, env: HashMap<String, String>) {
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
        let mut child = cmd.spawn().expect("Failed to spawn folio-server. Make sure you're running from the workspace root.");

        // Wait for "Listening on" message
        let stdout = child.stdout.take().expect("Failed to get stdout");
        let mut reader = tokio::io::BufReader::new(stdout);
        let mut line = String::new();

        // Read lines until we see "Listening on"
        let start_time = std::time::Instant::now();
        let timeout = std::time::Duration::from_secs(120); // Allow time for compilation

        loop {
            if start_time.elapsed() > timeout {
                panic!("Timeout waiting for server to start");
            }

            use tokio::io::AsyncBufReadExt;
            line.clear();
            match tokio::time::timeout(
                std::time::Duration::from_secs(1),
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
                Err(_) => {
                    // Timeout, continue checking
                }
            }

            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
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
                    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                }
            }
        }

        self.base_url = Some(base_url);
        self.server_process = Some(child);

        eprintln!("✓ Server started at {}", self.base_url.as_ref().unwrap());
    }

    /// Start Docker container (for CI/CD).
    #[cfg(feature = "docker")]
    async fn start_docker_container(&mut self, env: HashMap<String, String>) {
        use testcontainers::Container;
        use testcontainers::GenericImage;
        use testcontainers::core::WaitFor;

        let mut image = GenericImage::new("deesh2025/no-name", "latest")
            .with_wait_for(WaitFor::message_on_stdout("Listening on"));

        // Add environment variables
        for (key, value) in &env {
            image = image.with_env_var(key, value);
        }

        let container = image.start().await.expect("Failed to start container");
        let port = container
            .get_host_port_ipv4(3000)
            .await
            .expect("Failed to get container port");

        self.base_url = Some(format!("http://localhost:{}", port));

        // Give container time to fully start
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

        eprintln!("✓ Container started at {}", self.base_url.as_ref().unwrap());

        // Keep container alive (stored as static to prevent drop)
        // Note: This is a simplified version; real implementation needs proper storage
        Box::leak(Box::new(container));
    }

    #[cfg(not(feature = "docker"))]
    async fn start_docker_container(&mut self, _env: HashMap<String, String>) {
        panic!("Docker mode not available. Compile with --features docker");
    }

    /// Stop server.
    pub async fn stop_container(&mut self) {
        if let Some(mut child) = self.server_process.take() {
            // Try graceful shutdown
            let _ = child.kill().await;
            let _ = child.wait().await;
        }
        self.base_url = None;
    }
}

impl Drop for FolioWorld {
    fn drop(&mut self) {
        // Server will be stopped when dropped
        if let Some(mut child) = self.server_process.take() {
            let _ = std::process::Command::new("pkill")
                .arg("-f")
                .arg("folio-server")
                .output();
        }
    }
}
