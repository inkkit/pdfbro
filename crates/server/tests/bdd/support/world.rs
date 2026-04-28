//! Cucumber World implementation for BDD tests.
//!
//! Direct mode: Spawns server as child process (no Docker required).
//! A single server is shared across all scenarios via a [`tokio::sync::Mutex`]
//! so that only one scenario performs the (slow) startup and every other
//! scenario waits and then reuses it.

use std::collections::HashMap;
use std::process::Stdio;
use std::sync::atomic::{AtomicU32, Ordering};

use cucumber::World;
use reqwest::Client;
use tokio::process::Child;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::Mutex;

/// Shared server state protected by an async mutex.
/// Exactly one scenario spawns the process; all others wait on the lock
/// and then reuse the running server.
struct SharedServer {
    base_url: String,
    #[allow(dead_code)]
    child: Child,
}

static SHARED_SERVER: Mutex<Option<SharedServer>> = Mutex::const_new(None);

/// PID of the shared server child, stored for emergency cleanup.
static SHARED_PID: AtomicU32 = AtomicU32::new(0);

/// Test state shared across BDD steps.
#[derive(Debug, World)]
#[world(init = Self::new)]
pub struct FolioWorld {
    /// HTTP client for requests
    pub client: Client,

    /// Last HTTP status code
    pub status_code: Option<u16>,

    /// Response headers (name -> value)
    pub response_headers: Option<std::collections::HashMap<String, String>>,

    /// Response body bytes
    pub body: Option<Vec<u8>>,

    /// Temporary directory for test files
    #[allow(dead_code)]
    pub temp_dir: tempfile::TempDir,

    /// Server base URL (e.g., "http://localhost:3000")
    pub base_url: Option<String>,

    /// Child process handle (kept for API compatibility; shared server is
    /// stored in the global [`SHARED_SERVER`] mutex instead).
    #[allow(dead_code)]
    pub server_process: Option<Child>,
}

impl FolioWorld {
    /// Create new World instance.
    fn new() -> Self {
        Self {
            client: Client::new(),
            status_code: None,
            response_headers: None,
            body: None,
            temp_dir: tempfile::tempdir().unwrap(),
            base_url: None,
            server_process: None,
        }
    }

    /// Locate the pre-built `folio-server` binary.
    ///
    /// Searches `target/debug` and `target/release` relative to the workspace root,
    /// deriving the workspace root from the current executable's path when running
    /// under `cargo test`.
    fn find_folio_server_binary() -> std::path::PathBuf {
        let current_exe = std::env::current_exe().expect("current_exe unavailable");
        // current_exe is roughly target/<profile>/deps/<crate>-<hash>
        // Workspace root is four levels up from there.
        let workspace_root = current_exe
            .parent()
            .and_then(|p| p.parent()) // <profile>
            .and_then(|p| p.parent()) // target
            .and_then(|p| p.parent()) // workspace root
            .expect("Cannot derive workspace root from current_exe path")
            .to_path_buf();

        // Derive active profile from current_exe path so debug tests
        // pick the debug binary and release tests pick release.
        let exe = current_exe.to_string_lossy();
        let preferred = if exe.contains("/release/") || exe.contains("\\release\\") {
            ["release", "debug"]
        } else {
            ["debug", "release"]
        };
        for profile in preferred {
            let candidate = workspace_root.join("target").join(profile).join("folio-server");
            if candidate.exists() {
                return candidate;
            }
        }

        panic!(
            "folio-server binary not found in target/debug or target/release. \
             Build it first with `cargo build --bin folio-server`"
        );
    }

    /// Find an unused TCP port on localhost.
    fn find_free_port() -> u16 {
        let listener = std::net::TcpListener::bind("127.0.0.1:0")
            .expect("Failed to bind to find free port");
        listener.local_addr().unwrap().port()
    }

    /// Start Folio server with environment variables.
    ///
    /// The first call to this method across all scenarios acquires the
    /// [`SHARED_SERVER`] mutex, spawns the binary, waits for it to become
    /// ready, and stores the handle. Every subsequent call sees the running
    /// server and reuses it.
    pub async fn start_container(&mut self, env: HashMap<String, String>) {
        let mut guard = SHARED_SERVER.lock().await;

        // If a server is already running, make sure it is still healthy.
        if let Some(ref mut server) = *guard {
            if Self::health_check(&server.base_url).await {
                self.base_url = Some(server.base_url.clone());
                eprintln!("[BDD] Reusing shared server at {}", server.base_url);
                return;
            }
            eprintln!(
                "[BDD] Shared server at {} is dead, killing and respawning",
                server.base_url
            );
            let _ = server.child.kill().await;
            let _ = server.child.wait().await;
        }

        // Clean up any stale Chromium lock left by a previous crashed run
        // so the fresh server can launch Chrome successfully.
        Self::cleanup_chromium_lock();

        let port = Self::find_free_port();
        let base_url = format!("http://localhost:{}", port);

        let bin_path = Self::find_folio_server_binary();
        eprintln!(
            "[BDD] Spawning folio-server on port {} ({})",
            port,
            bin_path.display()
        );

        let mut cmd = tokio::process::Command::new(&bin_path);
        cmd.arg("serve")
            .arg("--port")
            .arg(port.to_string())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        for (key, value) in &env {
            cmd.env(key, value);
        }

        let mut child = cmd.spawn().unwrap_or_else(|e| {
            panic!(
                "Failed to spawn folio-server at {}. \
                 Build it first with `cargo build --bin folio-server`: {}",
                bin_path.display(),
                e
            )
        });

        // Remember PID so we can kill it in an emergency even if the Child
        // handle is later lost.
        if let Some(pid) = child.id() {
            SHARED_PID.store(pid, Ordering::SeqCst);
        }

        // Drain stderr on a background task so we can see crash logs.
        let stderr = child.stderr.take().expect("Failed to get stderr");
        tokio::spawn(async move {
            let mut err_reader = BufReader::new(stderr);
            let mut err_line = String::new();
            while let Ok(n) = err_reader.read_line(&mut err_line).await {
                if n == 0 {
                    break;
                }
                eprintln!("[SERVER ERR] {}", err_line.trim());
                err_line.clear();
            }
        });

        // Wait for "listening" message (tracing output is lowercase)
        let stdout = child.stdout.take().expect("Failed to get stdout");
        let mut reader = BufReader::new(stdout);
        let mut line = String::new();

        let start_time = std::time::Instant::now();
        let timeout = std::time::Duration::from_secs(30);

        loop {
            if start_time.elapsed() > timeout {
                let _ = child.kill().await;
                panic!(
                    "Timeout waiting for server to start ({}s). \
                     Check binary exists and port {} is free.",
                    timeout.as_secs(),
                    port
                );
            }

            line.clear();
            match tokio::time::timeout(
                std::time::Duration::from_millis(500),
                reader.read_line(&mut line),
            )
            .await
            {
                Ok(Ok(0)) => {
                    let _ = child.kill().await;
                    panic!(
                        "Server exited before becoming ready (EOF on stdout). \
                         Check [SERVER ERR] lines above for the crash reason."
                    );
                }
                Ok(Ok(_)) => {
                    eprintln!("[SERVER] {}", line.trim());
                    if line.contains("listening") {
                        break;
                    }
                }
                Ok(Err(e)) => {
                    let _ = child.kill().await;
                    panic!("Failed to read stdout: {}", e);
                }
                Err(_) => continue, // Timeout, keep waiting
            }
        }

        // Give the router a moment to finish registration.
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;

        // Wait for the health endpoint to respond.
        let health_start = std::time::Instant::now();
        loop {
            if health_start.elapsed() > std::time::Duration::from_secs(10) {
                let _ = child.kill().await;
                panic!("Server did not become ready on {}/health", base_url);
            }

            match Client::new()
                .get(&format!("{}/health", base_url))
                .send()
                .await
            {
                Ok(resp) if resp.status().is_success() => break,
                _ => {
                    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
                }
            }
        }

        *guard = Some(SharedServer {
            base_url: base_url.clone(),
            child,
        });
        self.base_url = Some(base_url.clone());

        eprintln!("[BDD] ✓ Server started at {}", base_url);
    }

    /// No-op in shared-server mode: the server is intentionally kept alive
    /// across scenarios so that later steps reuse it.
    #[allow(dead_code)]
    pub async fn stop_container(&mut self) {
        // Shared server lifecycle is managed by the global mutex and cleaned
        // up automatically when the test process exits.
    }

    /// Remove the `chromiumoxide-runner` directory that holds the
    /// Chromium `SingletonLock`. Without this, a crashed prior run can
    /// prevent the new server from launching Chrome.
    fn cleanup_chromium_lock() {
        let runner = std::env::temp_dir().join("chromiumoxide-runner");
        if runner.exists() {
            let _ = std::fs::remove_dir_all(&runner);
            eprintln!("[BDD] Cleaned up stale Chromium lock at {}", runner.display());
        }
    }

    /// Quick health probe used when deciding whether to reuse the shared
    /// server.
    async fn health_check(url: &str) -> bool {
        Client::new()
            .get(format!("{}/health", url))
            .send()
            .await
            .map_or(false, |r| r.status().is_success())
    }
}

impl Drop for FolioWorld {
    fn drop(&mut self) {
        // Clean up the Chromium profile lock so the next test invocation
        // (or scenario, if we ever switch back to per-scenario servers)
        // can launch a fresh browser.
        FolioWorld::cleanup_chromium_lock();
    }
}
