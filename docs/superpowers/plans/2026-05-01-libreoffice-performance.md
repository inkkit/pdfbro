# LibreOffice Performance: unoserver + LO 26.x — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace per-request `soffice` subprocess spawning with a persistent `unoserver` UNO listener and upgrade LibreOffice from 7.4 to 26.2, reducing p50 LibreOffice latency from 1256ms to ≤ 550ms.

**Architecture:** `LibreOfficeEngine::launch()` spawns and supervises a `unoserver` child process (Python, persistent LibreOffice UNO listener). Each `convert()` call sends a multipart HTTP POST to unoserver over localhost instead of forking a new `soffice` process. A background task polls the unoserver process every 5 seconds and restarts it on crash (up to 3 times with exponential backoff).

**Tech Stack:** Rust (tokio, reqwest 0.12 multipart), Python unoserver 2.2.1 (pip), LibreOffice 26.2 from TDF apt repo, Debian bookworm base image.

---

## File Map

| Action | File |
|--------|------|
| Modify | `crates/engine/Cargo.toml` |
| Create | `crates/engine/src/libreoffice/unoserver.rs` |
| Rewrite | `crates/engine/src/libreoffice/convert.rs` |
| Modify | `crates/engine/src/libreoffice/mod.rs` |
| Delete | `crates/engine/src/libreoffice/discover.rs` |
| Modify | `crates/server/src/config.rs` |
| Modify | `crates/server/src/main.rs` |
| Modify | `Dockerfile` |

---

### Task 1: Add `reqwest` dependency to the engine crate

**Files:**
- Modify: `crates/engine/Cargo.toml`

`reqwest` is already in the workspace (`Cargo.toml` line 65) with `features = ["json", "multipart", "rustls-tls"]`. We just need to add it to the `engine` crate as an optional dependency gated on the `libreoffice` feature.

- [ ] **Step 1: Add reqwest as optional dependency**

Open `crates/engine/Cargo.toml`. Replace:

```toml
[features]
default = ["chromium", "libreoffice"]
chromium = ["dep:chromiumoxide", "dep:futures-util", "dep:pulldown-cmark", "dep:urlencoding"]
libreoffice = []
```

with:

```toml
[features]
default = ["chromium", "libreoffice"]
chromium = ["dep:chromiumoxide", "dep:futures-util", "dep:pulldown-cmark", "dep:urlencoding"]
libreoffice = ["dep:reqwest"]
```

Then add to `[dependencies]`:

```toml
reqwest = { workspace = true, optional = true }
```

Place it after `pulldown-cmark` and before `serde`.

- [ ] **Step 2: Verify it compiles**

```bash
cargo check --features libreoffice -p engine
```

Expected: no errors.

- [ ] **Step 3: Commit**

```bash
git add crates/engine/Cargo.toml
git commit -m "chore(engine): add reqwest as optional libreoffice-feature dep"
```

---

### Task 2: Create `unoserver.rs` — process lifecycle management

**Files:**
- Create: `crates/engine/src/libreoffice/unoserver.rs`

This module owns the unoserver child process. It spawns `python3 -m unoserver`, polls TCP until the port accepts connections, and sends SIGTERM on drop.

- [ ] **Step 1: Write the failing test**

Create `crates/engine/src/libreoffice/unoserver.rs` with the test block first:

```rust
use std::path::Path;
use std::time::Duration;

use tokio::process::Child;
use tracing::info;

use crate::types::{EngineError, EngineResult};

pub(super) struct UnoserverProcess {
    child: Child,
    port: u16,
}

impl UnoserverProcess {
    pub(super) async fn spawn(
        port: u16,
        ready_timeout: Duration,
        executable: Option<&Path>,
    ) -> EngineResult<Self> {
        todo!()
    }

    pub(super) fn port(&self) -> u16 {
        self.port
    }

    pub(super) fn try_wait(&mut self) -> std::io::Result<Option<std::process::ExitStatus>> {
        self.child.try_wait()
    }
}

impl Drop for UnoserverProcess {
    fn drop(&mut self) {
        let _ = self.child.start_kill();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn spawn_times_out_when_port_not_bound() {
        // Port 19876 has nothing listening — spawn should fail with Timeout.
        let result = UnoserverProcess::spawn(
            19876,
            Duration::from_millis(300),
            None,
        )
        .await;
        assert!(
            matches!(result, Err(EngineError::Timeout(_))),
            "expected Timeout, got: {result:?}"
        );
    }
}
```

- [ ] **Step 2: Run the test to verify it fails**

```bash
cargo test --features libreoffice -p engine libreoffice::unoserver::tests::spawn_times_out_when_port_not_bound
```

Expected: FAIL — `todo!()` panics.

- [ ] **Step 3: Implement `UnoserverProcess::spawn`**

Replace the `todo!()` with the full implementation:

```rust
pub(super) async fn spawn(
    port: u16,
    ready_timeout: Duration,
    executable: Option<&Path>,
) -> EngineResult<Self> {
    info!(port, "Starting unoserver");

    let mut cmd = tokio::process::Command::new("python3");
    cmd.args([
        "-m",
        "unoserver",
        "--interface",
        "127.0.0.1",
        "--port",
        &port.to_string(),
    ]);
    if let Some(exe) = executable {
        cmd.arg("--executable");
        cmd.arg(exe);
    }
    cmd.kill_on_drop(true)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());

    let child = cmd
        .spawn()
        .map_err(|e| EngineError::Internal(format!("failed to spawn unoserver: {e}")))?;

    // Poll TCP until the port accepts connections or timeout elapses.
    let addr = format!("127.0.0.1:{port}");
    let deadline = tokio::time::Instant::now() + ready_timeout;
    loop {
        if tokio::time::Instant::now() >= deadline {
            return Err(EngineError::Timeout(ready_timeout));
        }
        match tokio::net::TcpStream::connect(&addr).await {
            Ok(_) => {
                info!(port, "unoserver ready");
                break;
            }
            Err(_) => {
                tokio::time::sleep(Duration::from_millis(500)).await;
            }
        }
    }

    Ok(Self { child, port })
}
```

- [ ] **Step 4: Run the test to verify it passes**

```bash
cargo test --features libreoffice -p engine libreoffice::unoserver::tests::spawn_times_out_when_port_not_bound
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/engine/src/libreoffice/unoserver.rs
git commit -m "feat(engine/lo): add UnoserverProcess — spawn, ready-poll, drop/SIGTERM"
```

---

### Task 3: Rewrite `convert.rs` — HTTP POST to unoserver

**Files:**
- Rewrite: `crates/engine/src/libreoffice/convert.rs`

Replace the entire `soffice --convert-to` subprocess implementation with a `reqwest` multipart POST to the running unoserver. All existing unit tests in this file test the old subprocess path and are replaced with tests for the new HTTP path.

- [ ] **Step 1: Write the failing tests**

Replace the entire content of `crates/engine/src/libreoffice/convert.rs` with:

```rust
//! HTTP-based conversion via a running unoserver process.

use std::path::Path;
use std::time::Duration;

use crate::types::{EngineError, EngineResult};

use super::OfficeOptions;
use super::filter::for_extension;

/// Send `input` to unoserver for PDF conversion and return the PDF bytes.
///
/// `client` must be the shared `reqwest::Client` from `LibreOfficeEngine::Inner`.
/// `port` is the localhost port unoserver is listening on.
pub(super) async fn run_convert(
    client: &reqwest::Client,
    port: u16,
    timeout: Duration,
    input: &Path,
    opts: &OfficeOptions,
) -> EngineResult<Vec<u8>> {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::StatusCode;
    use axum::response::Response;
    use axum::routing::post;
    use axum::Router;
    use tempfile::Builder;

    async fn start_mock_unoserver(
        handler: impl Fn() -> Response<Body> + Send + Sync + Clone + 'static,
    ) -> u16 {
        let app = Router::new().route(
            "/",
            post(move || {
                let h = handler.clone();
                async move { h() }
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        port
    }

    fn fake_docx() -> tempfile::NamedTempFile {
        let f = Builder::new().suffix(".docx").tempfile().unwrap();
        std::fs::write(f.path(), b"PK fake docx content").unwrap();
        f
    }

    #[tokio::test]
    async fn run_convert_returns_pdf_bytes_on_success() {
        let port = start_mock_unoserver(|| {
            Response::builder()
                .status(StatusCode::OK)
                .header("Content-Type", "application/pdf")
                .body(Body::from(b"%PDF-1.4 fake".to_vec()))
                .unwrap()
        })
        .await;

        let tmp = fake_docx();
        let client = reqwest::Client::new();
        let result = run_convert(&client, port, Duration::from_secs(5), tmp.path(), &OfficeOptions::default()).await;
        assert!(result.is_ok(), "{result:?}");
        assert_eq!(result.unwrap(), b"%PDF-1.4 fake");
    }

    #[tokio::test]
    async fn run_convert_maps_http_500_to_engine_error() {
        let port = start_mock_unoserver(|| {
            Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(Body::from("unsupported format"))
                .unwrap()
        })
        .await;

        let tmp = fake_docx();
        let client = reqwest::Client::new();
        let result = run_convert(&client, port, Duration::from_secs(5), tmp.path(), &OfficeOptions::default()).await;
        assert!(matches!(result, Err(EngineError::Internal(_))), "{result:?}");
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("500") || msg.contains("unsupported"), "{msg}");
    }

    #[tokio::test]
    async fn run_convert_returns_error_when_nothing_listening() {
        let client = reqwest::Client::new();
        let tmp = fake_docx();
        // Port 19877 — nothing is listening here.
        let result = run_convert(&client, 19877, Duration::from_millis(200), tmp.path(), &OfficeOptions::default()).await;
        assert!(result.is_err(), "expected error when nothing listening");
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

```bash
cargo test --features libreoffice -p engine libreoffice::convert::tests
```

Expected: FAIL — `todo!()` panics on all three tests.

- [ ] **Step 3: Implement `run_convert`**

Replace `todo!()` with:

```rust
pub(super) async fn run_convert(
    client: &reqwest::Client,
    port: u16,
    timeout: Duration,
    input: &Path,
    opts: &OfficeOptions,
) -> EngineResult<Vec<u8>> {
    let file_bytes = tokio::fs::read(input).await.map_err(EngineError::Io)?;

    let filename = input
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("document")
        .to_string();

    let ext = input
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or_default();
    let filtername = for_extension(ext);

    let file_part = reqwest::multipart::Part::bytes(file_bytes).file_name(filename);

    let mut form = reqwest::multipart::Form::new()
        .text("output-file", "output.pdf")
        .part("file", file_part);

    if filtername != "pdf" {
        form = form.text("filtername", filtername.to_string());
    }
    if let Some(blob) = opts.filter_blob() {
        form = form.text("filteroptions", blob);
    }

    let url = format!("http://127.0.0.1:{port}/");

    let result = tokio::time::timeout(timeout, async {
        let resp = client
            .post(&url)
            .multipart(form)
            .send()
            .await
            .map_err(|e| EngineError::Internal(format!("unoserver request: {e}")))?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(EngineError::Internal(format!("unoserver {status}: {body}")));
        }

        resp.bytes()
            .await
            .map(|b| b.to_vec())
            .map_err(|e| EngineError::Internal(format!("unoserver read body: {e}")))
    })
    .await
    .map_err(|_| EngineError::Timeout(timeout))??;

    Ok(result)
}
```

- [ ] **Step 4: Run tests to verify they pass**

```bash
cargo test --features libreoffice -p engine libreoffice::convert::tests
```

Expected: all 3 PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/engine/src/libreoffice/convert.rs
git commit -m "feat(engine/lo): replace soffice subprocess with HTTP POST to unoserver"
```

---

### Task 4: Update `mod.rs` — wire `UnoserverProcess` into `LibreOfficeEngine`

**Files:**
- Modify: `crates/engine/src/libreoffice/mod.rs`

This is the core wiring task. We add `mod unoserver;`, update `LibreOfficeConfig` with the two new fields, rebuild `Inner` to hold the HTTP client and unoserver process (removing the old `exe` path), rewrite `launch()` to spawn unoserver, rewrite `healthy()` to use TCP connect, and update `convert()` to pass the new parameters to `run_convert()`.

- [ ] **Step 1: Add `mod unoserver;` and remove `mod discover;`**

In `crates/engine/src/libreoffice/mod.rs`, find the module declarations at the top:

```rust
pub mod filter;

mod convert;
mod discover;
```

Replace with:

```rust
pub mod filter;

mod convert;
mod unoserver;
```

- [ ] **Step 2: Update imports**

Find the existing import block:

```rust
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tokio::sync::Semaphore;

use tracing::{debug, info, instrument};

use crate::types::{EngineError, EngineResult, PageRanges};
```

Replace with:

```rust
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tokio::sync::{Mutex, Semaphore};

use tracing::{debug, info, instrument};

use crate::types::{EngineError, EngineResult, PageRanges};
use self::unoserver::UnoserverProcess;
```

- [ ] **Step 3: Update `LibreOfficeConfig`**

Find the current `LibreOfficeConfig` struct definition and replace it entirely:

```rust
/// Engine-wide configuration. Pass to [`LibreOfficeEngine::launch`].
#[derive(Debug, Clone)]
pub struct LibreOfficeConfig {
    /// Path to `soffice` (or `libreoffice`). `None` = let unoserver find it on `$PATH`.
    pub executable: Option<PathBuf>,
    /// Per-conversion timeout. Default 120s.
    pub timeout: Duration,
    /// Maximum concurrent HTTP requests to unoserver. Default
    /// [`std::thread::available_parallelism`].
    pub max_concurrency: usize,
    /// Use lazy initialization (start on first request).
    pub lazy_start: bool,
    /// Idle shutdown timeout. `None` = no idle shutdown.
    pub idle_shutdown_timeout: Option<Duration>,
    /// Port unoserver listens on. Default 2003.
    pub unoserver_port: u16,
    /// Maximum time to wait for unoserver to become ready at startup. Default 60s.
    pub unoserver_ready_timeout: Duration,
}

impl Default for LibreOfficeConfig {
    fn default() -> Self {
        Self {
            executable: None,
            timeout: Duration::from_secs(120),
            max_concurrency: std::thread::available_parallelism()
                .map(|n| n.get())
                .unwrap_or(4),
            lazy_start: false,
            idle_shutdown_timeout: None,
            unoserver_port: 2003,
            unoserver_ready_timeout: Duration::from_secs(60),
        }
    }
}
```

- [ ] **Step 4: Update `Inner` struct**

Find the current `Inner` struct:

```rust
struct Inner {
    exe: PathBuf,
    timeout: Duration,
    semaphore: Semaphore,
}
```

Replace with:

```rust
struct Inner {
    unoserver: Mutex<UnoserverProcess>,
    unoserver_port: u16,
    unoserver_ready_timeout: Duration,
    executable: Option<PathBuf>,
    http_client: reqwest::Client,
    timeout: Duration,
    semaphore: Semaphore,
}
```

- [ ] **Step 5: Rewrite `launch()`**

Find the current `launch()` method body (starting at `pub async fn launch`) and replace it entirely:

```rust
#[instrument(skip(config), fields(unoserver_port = config.unoserver_port))]
pub async fn launch(config: LibreOfficeConfig) -> EngineResult<Self> {
    info!("Launching LibreOffice engine via unoserver");

    let unoserver = UnoserverProcess::spawn(
        config.unoserver_port,
        config.unoserver_ready_timeout,
        config.executable.as_deref(),
    )
    .await?;

    let http_client = reqwest::Client::builder()
        .tcp_keepalive(Duration::from_secs(30))
        .pool_max_idle_per_host(1)
        .build()
        .map_err(|e| EngineError::Internal(format!("reqwest client build: {e}")))?;

    let max = config.max_concurrency.max(1);
    info!(
        unoserver_port = config.unoserver_port,
        timeout = ?config.timeout,
        max_concurrency = max,
        "LibreOffice engine launched"
    );

    let inner = Arc::new(Inner {
        unoserver: Mutex::new(unoserver),
        unoserver_port: config.unoserver_port,
        unoserver_ready_timeout: config.unoserver_ready_timeout,
        executable: config.executable.clone(),
        http_client,
        timeout: config.timeout,
        semaphore: Semaphore::new(max),
    });

    // Background task: monitor unoserver process and restart on crash.
    {
        let inner2 = Arc::clone(&inner);
        tokio::spawn(async move {
            let mut consecutive_failures = 0u32;
            loop {
                tokio::time::sleep(Duration::from_secs(5)).await;
                let mut guard = inner2.unoserver.lock().await;
                match guard.try_wait() {
                    Ok(Some(status)) => {
                        tracing::error!(exit_status = ?status, "unoserver exited unexpectedly");
                        consecutive_failures += 1;
                        if consecutive_failures > 3 {
                            tracing::error!(
                                "unoserver failed to restart after 3 attempts, giving up"
                            );
                            break;
                        }
                        let backoff = Duration::from_secs(2u64.pow(consecutive_failures - 1));
                        drop(guard);
                        tokio::time::sleep(backoff).await;
                        let mut guard = inner2.unoserver.lock().await;
                        match UnoserverProcess::spawn(
                            inner2.unoserver_port,
                            inner2.unoserver_ready_timeout,
                            inner2.executable.as_deref(),
                        )
                        .await
                        {
                            Ok(new_proc) => {
                                *guard = new_proc;
                                consecutive_failures = 0;
                                tracing::info!("unoserver restarted successfully");
                            }
                            Err(e) => {
                                tracing::error!(error = %e, "unoserver restart failed");
                            }
                        }
                    }
                    Ok(None) => {
                        consecutive_failures = 0;
                    }
                    Err(e) => {
                        tracing::warn!(error = %e, "unoserver try_wait error");
                    }
                }
            }
        });
    }

    Ok(Self { inner })
}
```

- [ ] **Step 6: Rewrite `healthy()`**

Find the current `healthy()` method:

```rust
pub async fn healthy(&self) -> bool {
    discover::probe(&self.inner.exe, Duration::from_secs(30))
        .await
        .is_ok()
}
```

Replace with:

```rust
pub async fn healthy(&self) -> bool {
    let addr = format!("127.0.0.1:{}", self.inner.unoserver_port);
    matches!(
        tokio::time::timeout(
            Duration::from_secs(5),
            tokio::net::TcpStream::connect(&addr),
        )
        .await,
        Ok(Ok(_))
    )
}
```

- [ ] **Step 7: Update `convert()` to call the new `run_convert` signature**

Find the line inside `convert()` that calls `convert::run_convert`:

```rust
let result = convert::run_convert(&self.inner.exe, self.inner.timeout, input, opts).await;
```

Replace with:

```rust
let result = convert::run_convert(
    &self.inner.http_client,
    self.inner.unoserver_port,
    self.inner.timeout,
    input,
    opts,
)
.await;
```

Also, remove the `input.exists()` check and the `_span` line from `convert()` since those depended on the old exe-based flow. Keep everything else (semaphore, timing, logging).

Actually keep the `input.exists()` check — it's still valid and useful to fail fast before an HTTP call. Just remove `let _span = self.logger();` since `logger()` referenced `exe`.

Remove the `logger()` method entirely from the impl block (it was only used for the old tracing span tagged with `engine="libreoffice"`).

- [ ] **Step 8: Verify the module compiles**

```bash
cargo check --features libreoffice -p engine 2>&1
```

Expected: no errors. Fix any remaining references to `discover` or `exe` if the compiler flags them.

- [ ] **Step 9: Run all engine unit tests**

```bash
cargo test --features libreoffice -p engine 2>&1
```

Expected: all existing `office_options_*` tests pass. The `launch_with_missing_executable_path_errors` test will now fail (it tested the old discover path). Delete it — the equivalent is now the `spawn_times_out_when_port_not_bound` test in `unoserver.rs`.

Find and delete this test in `mod.rs`:

```rust
#[tokio::test]
async fn launch_with_missing_executable_path_errors() {
    let cfg = LibreOfficeConfig {
        executable: Some(PathBuf::from("/nonexistent/__folio_no_soffice")),
        ..LibreOfficeConfig::default()
    };
    let err = LibreOfficeEngine::launch(cfg)
        .await
        .expect_err("should fail");
    assert!(matches!(err, EngineError::Internal(_)));
}
```

- [ ] **Step 10: Also update the `libreoffice_config_default_matches_spec` test**

Find this test in `mod.rs`:

```rust
#[test]
fn libreoffice_config_default_matches_spec() {
    let c = LibreOfficeConfig::default();
    assert!(c.executable.is_none());
    assert_eq!(c.timeout, Duration::from_secs(120));
    assert!(c.max_concurrency >= 1);
}
```

Replace with:

```rust
#[test]
fn libreoffice_config_default_matches_spec() {
    let c = LibreOfficeConfig::default();
    assert!(c.executable.is_none());
    assert_eq!(c.timeout, Duration::from_secs(120));
    assert!(c.max_concurrency >= 1);
    assert_eq!(c.unoserver_port, 2003);
    assert_eq!(c.unoserver_ready_timeout, Duration::from_secs(60));
}
```

- [ ] **Step 11: Run all engine tests again**

```bash
cargo test --features libreoffice -p engine 2>&1
```

Expected: all tests pass.

- [ ] **Step 12: Commit**

```bash
git add crates/engine/src/libreoffice/mod.rs
git commit -m "feat(engine/lo): wire UnoserverProcess into LibreOfficeEngine, update config and launch"
```

---

### Task 5: Delete `discover.rs`

**Files:**
- Delete: `crates/engine/src/libreoffice/discover.rs`

`discover.rs` provided `find_soffice()` (now unused — unoserver locates soffice itself) and `probe()` (now replaced by TCP ready-polling in `unoserver.rs`). The `mod discover;` declaration was already removed in Task 4.

- [ ] **Step 1: Delete the file**

```bash
rm crates/engine/src/libreoffice/discover.rs
```

- [ ] **Step 2: Verify no remaining references**

```bash
cargo check --features libreoffice -p engine 2>&1
```

Expected: no errors about `discover`.

- [ ] **Step 3: Commit**

```bash
git add -A crates/engine/src/libreoffice/
git commit -m "chore(engine/lo): remove discover.rs (replaced by unoserver ready-polling)"
```

---

### Task 6: Add `unoserver_port` and `unoserver_ready_timeout` to server config

**Files:**
- Modify: `crates/server/src/config.rs`
- Modify: `crates/server/src/main.rs`

The server config needs to expose the two new `LibreOfficeConfig` fields as CLI flags and env vars, then wire them through `libreoffice_config_from()`.

- [ ] **Step 1: Write the failing test**

In `crates/server/src/config.rs`, in the `#[cfg(test)]` block at the bottom, add this test before the closing `}`:

```rust
#[test]
fn libreoffice_unoserver_defaults() {
    let args = ServerArgs::default();
    let cfg = ServerConfig::resolve(&args, &env(&[])).unwrap();
    assert_eq!(cfg.libreoffice_unoserver_port, 2003);
    assert_eq!(cfg.libreoffice_unoserver_ready_timeout, Duration::from_secs(60));
}

#[test]
fn libreoffice_unoserver_from_env() {
    let args = ServerArgs::default();
    let cfg = ServerConfig::resolve(
        &args,
        &env(&[
            ("LIBREOFFICE_UNOSERVER_PORT", "3003"),
            ("LIBREOFFICE_UNOSERVER_READY_TIMEOUT", "120s"),
        ]),
    )
    .unwrap();
    assert_eq!(cfg.libreoffice_unoserver_port, 3003);
    assert_eq!(cfg.libreoffice_unoserver_ready_timeout, Duration::from_secs(120));
}
```

- [ ] **Step 2: Run the test to verify it fails**

```bash
cargo test -p server libreoffice_unoserver 2>&1
```

Expected: FAIL — fields `libreoffice_unoserver_port` and `libreoffice_unoserver_ready_timeout` don't exist yet.

- [ ] **Step 3: Add the fields to `ServerArgs`**

In `crates/server/src/config.rs`, in the `ServerArgs` struct, find the libreoffice section (around the `libreoffice_idle_shutdown_timeout` field) and add after it:

```rust
/// Port for unoserver to listen on (default 2003).
#[arg(long, value_name = "PORT", env = "LIBREOFFICE_UNOSERVER_PORT")]
pub libreoffice_unoserver_port: Option<u16>,

/// How long to wait for unoserver to become ready at startup (e.g., "60s", "2m").
#[arg(long, value_name = "DUR", env = "LIBREOFFICE_UNOSERVER_READY_TIMEOUT")]
pub libreoffice_unoserver_ready_timeout: Option<String>,
```

- [ ] **Step 4: Add the fields to `ServerConfig`**

In the `ServerConfig` struct, after `libreoffice_idle_shutdown_timeout`, add:

```rust
/// Port unoserver listens on.
pub libreoffice_unoserver_port: u16,
/// Timeout waiting for unoserver to be ready.
pub libreoffice_unoserver_ready_timeout: Duration,
```

- [ ] **Step 5: Add resolution logic in `ServerConfig::resolve()`**

In the `resolve()` method, after the `libreoffice_idle_shutdown_timeout` resolution block, add:

```rust
let libreoffice_unoserver_port = match args.libreoffice_unoserver_port {
    Some(p) => p,
    None => env
        .get("LIBREOFFICE_UNOSERVER_PORT")
        .and_then(|v| v.parse::<u16>().ok())
        .unwrap_or(2003),
};

let libreoffice_unoserver_ready_timeout = args
    .libreoffice_unoserver_ready_timeout
    .as_deref()
    .or_else(|| env.get("LIBREOFFICE_UNOSERVER_READY_TIMEOUT").map(|v| v.as_str()))
    .and_then(|v| humantime::parse_duration(v).ok())
    .unwrap_or(Duration::from_secs(60));
```

- [ ] **Step 6: Add fields to the `Ok(Self { ... })` constructor at the end of `resolve()`**

Find the `Ok(Self {` block and add the two fields to it:

```rust
libreoffice_unoserver_port,
libreoffice_unoserver_ready_timeout,
```

- [ ] **Step 7: Update `libreoffice_config_from()` in `main.rs`**

In `crates/server/src/main.rs`, find the function at line 237:

```rust
fn libreoffice_config_from(config: &ServerConfig) -> LibreOfficeConfig {
    let defaults = LibreOfficeConfig::default();
    LibreOfficeConfig {
        executable: config.soffice_path.clone(),
        timeout: config.request_timeout,
        max_concurrency: defaults.max_concurrency,
        lazy_start: config.libreoffice_lazy_start,
        idle_shutdown_timeout: config.libreoffice_idle_shutdown_timeout,
    }
}
```

Replace with:

```rust
fn libreoffice_config_from(config: &ServerConfig) -> LibreOfficeConfig {
    let defaults = LibreOfficeConfig::default();
    LibreOfficeConfig {
        executable: config.soffice_path.clone(),
        timeout: config.request_timeout,
        max_concurrency: defaults.max_concurrency,
        lazy_start: config.libreoffice_lazy_start,
        idle_shutdown_timeout: config.libreoffice_idle_shutdown_timeout,
        unoserver_port: config.libreoffice_unoserver_port,
        unoserver_ready_timeout: config.libreoffice_unoserver_ready_timeout,
    }
}
```

- [ ] **Step 8: Run the tests**

```bash
cargo test -p server libreoffice_unoserver 2>&1
```

Expected: both PASS.

- [ ] **Step 9: Run full server tests to check nothing broke**

```bash
cargo test --features "chromium libreoffice" -p server 2>&1
```

Expected: all pass.

- [ ] **Step 10: Commit**

```bash
git add crates/server/src/config.rs crates/server/src/main.rs
git commit -m "feat(server): add libreoffice_unoserver_port and unoserver_ready_timeout config"
```

---

### Task 7: Dockerfile — LibreOffice 26.2 + unoserver + SAL_USE_VCLPLUGIN

**Files:**
- Modify: `Dockerfile`

Two stages install LibreOffice: `folio` (full image, Chromium + LO) and `folio-libreoffice` (LO-only). Both need the same updated install block. We also add a top-level `ARG` for the LO version so bumping it in one place affects all stages.

- [ ] **Step 1: Add the `LIBREOFFICE_VERSION` ARG near the top**

Find the existing ARG block at the top of the Dockerfile (around line 1–8):

```dockerfile
ARG RUST_VERSION=1.88
ARG FOLIO_VERSION=0.1.0
ARG FOLIO_USER_UID=1001
ARG FOLIO_USER_GID=1001
# Pinned for reproducible builds — bump deliberately when upgrading.
# See: https://snapshot.debian.org/package/chromium/
ARG CHROMIUM_VERSION=142.0.7444.175-1
```

Add after `CHROMIUM_VERSION`:

```dockerfile
# TDF (The Document Foundation) LibreOffice version for apt repo.
# Slug format: major-minor with dots → hyphens (e.g. 26.2 → libreoffice-26-2).
ARG LIBREOFFICE_VERSION=26.2
ARG LIBREOFFICE_APT_SLUG=libreoffice-26-2
```

- [ ] **Step 2: Replace the LO install block in the `folio` stage**

The `folio` stage (FROM common-chromium) currently has this RUN block (around line 181–187):

```dockerfile
RUN apt-get update -qq && apt-get upgrade -yqq && \
    DEBIAN_FRONTEND=noninteractive apt-get install -y -qq --no-install-recommends \
        libreoffice-writer \
        libreoffice-calc \
        libreoffice-impress \
        libreoffice-draw \
    && rm -rf /var/lib/apt/lists/* /tmp/* /var/tmp/*
```

Replace with:

```dockerfile
ARG LIBREOFFICE_APT_SLUG

RUN apt-get update -qq && \
    DEBIAN_FRONTEND=noninteractive apt-get install -y -qq --no-install-recommends gnupg && \
    curl -fsSL "https://deb.libreoffice.org/${LIBREOFFICE_APT_SLUG}/Release.key" \
      | gpg --dearmor -o /usr/share/keyrings/libreoffice.gpg && \
    printf "deb [signed-by=/usr/share/keyrings/libreoffice.gpg] https://deb.libreoffice.org/%s/ bookworm main\n" \
      "${LIBREOFFICE_APT_SLUG}" > /etc/apt/sources.list.d/libreoffice.list && \
    apt-get update -qq && \
    DEBIAN_FRONTEND=noninteractive apt-get install -y -qq --no-install-recommends \
        libreoffice-writer \
        libreoffice-calc \
        libreoffice-impress \
        libreoffice-draw \
        python3-minimal \
        python3-pip && \
    pip3 install --no-cache-dir --break-system-packages unoserver==2.2.1 && \
    rm -rf /var/lib/apt/lists/* /tmp/* /var/tmp/*

ENV SAL_USE_VCLPLUGIN=svp
```

- [ ] **Step 3: Replace the LO install block in the `folio-libreoffice` stage**

The `folio-libreoffice` stage (FROM common) has the same pattern (around line 251–257):

```dockerfile
RUN apt-get update -qq && apt-get upgrade -yqq && \
    DEBIAN_FRONTEND=noninteractive apt-get install -y -qq --no-install-recommends \
        libreoffice-writer \
        libreoffice-calc \
        libreoffice-impress \
        libreoffice-draw \
    && rm -rf /var/lib/apt/lists/* /tmp/* /var/tmp/*
```

Replace with the identical block from Step 2:

```dockerfile
ARG LIBREOFFICE_APT_SLUG

RUN apt-get update -qq && \
    DEBIAN_FRONTEND=noninteractive apt-get install -y -qq --no-install-recommends gnupg && \
    curl -fsSL "https://deb.libreoffice.org/${LIBREOFFICE_APT_SLUG}/Release.key" \
      | gpg --dearmor -o /usr/share/keyrings/libreoffice.gpg && \
    printf "deb [signed-by=/usr/share/keyrings/libreoffice.gpg] https://deb.libreoffice.org/%s/ bookworm main\n" \
      "${LIBREOFFICE_APT_SLUG}" > /etc/apt/sources.list.d/libreoffice.list && \
    apt-get update -qq && \
    DEBIAN_FRONTEND=noninteractive apt-get install -y -qq --no-install-recommends \
        libreoffice-writer \
        libreoffice-calc \
        libreoffice-impress \
        libreoffice-draw \
        python3-minimal \
        python3-pip && \
    pip3 install --no-cache-dir --break-system-packages unoserver==2.2.1 && \
    rm -rf /var/lib/apt/lists/* /tmp/* /var/tmp/*

ENV SAL_USE_VCLPLUGIN=svp
```

- [ ] **Step 4: Verify the Dockerfile builds (folio target)**

```bash
docker build --target folio -t folio-test:lo-perf . 2>&1 | tail -20
```

Expected: build succeeds. This will take several minutes (LO download). Watch for:
- `Successfully tagged folio-test:lo-perf`
- No errors in the apt install or pip install step.

If the TDF apt key URL fails (network issue or URL changed), check `https://deb.libreoffice.org/` in a browser to confirm the slug format for 26.2.

- [ ] **Step 5: Smoke-test unoserver is present in the image**

```bash
docker run --rm folio-test:lo-perf python3 -m unoserver --help 2>&1 | head -5
```

Expected: unoserver help text (not "No module named unoserver").

```bash
docker run --rm folio-test:lo-perf soffice --version 2>&1
```

Expected: `LibreOffice 26.2.x.x ...` (not `7.4`).

- [ ] **Step 6: Commit**

```bash
git add Dockerfile
git commit -m "feat(docker): upgrade LO to 26.2 via TDF apt, add unoserver, set SAL_USE_VCLPLUGIN=svp"
```

---

### Task 8: Benchmark validation

**Files:** none (run existing benchmark)

After Task 7, re-run the Docker benchmark to confirm the performance target is met.

- [ ] **Step 1: Start benchmark containers**

```bash
docker compose -f docker-compose.bench.yml build folio
docker compose -f docker-compose.bench.yml up -d
```

Wait 30 seconds for both containers to be healthy.

- [ ] **Step 2: Run the benchmark**

```bash
cargo run -p bench --release -- --skip pdfengines-merge 2>&1 | tee bench/results/lo-perf-validation.txt
```

- [ ] **Step 3: Verify target met**

Open `bench/results/lo-perf-validation.txt`. Check the `libreoffice-docx` row:

- Folio p50 ≤ 550ms ✓ (was 1256ms)
- Folio p50 ≤ Gotenberg p50 + 50ms ✓ (Gotenberg baseline: 528ms)

If p50 is above 550ms, check:
1. `docker logs bench-folio` — look for unoserver startup errors
2. `docker exec bench-folio python3 -c "import unoserver; print('ok')"` — verify pip install worked
3. `docker exec bench-folio soffice --version` — verify LO 26.2 is installed

- [ ] **Step 4: Commit results**

```bash
git add bench/results/lo-perf-validation.txt
git commit -m "bench: validate LO performance after unoserver + LO 26.2 upgrade"
```

---

## Self-Review

**Spec coverage:**
- LO version upgrade (7.4 → 26.2): Task 7 ✓
- unoserver process management: Tasks 2, 4 ✓
- HTTP POST conversion path: Task 3 ✓
- `SAL_USE_VCLPLUGIN=svp`: Task 7 ✓
- `unoserver_port` / `unoserver_ready_timeout` config: Task 6 ✓
- Crash recovery (3 retries, exponential backoff): Task 4 Step 5 ✓
- `reqwest` dep: Task 1 ✓
- `discover.rs` removal: Task 5 ✓
- `healthy()` updated to TCP connect: Task 4 Step 6 ✓
- `filter_blob()` / `filter::for_extension()` unchanged: verify — both still called in Task 3 ✓

**Placeholder scan:** None found.

**Type consistency:**
- `UnoserverProcess::spawn(port: u16, ready_timeout: Duration, executable: Option<&Path>)` — defined in Task 2, called in Task 4 Step 5 with `inner2.executable.as_deref()` → `Option<&Path>` ✓
- `run_convert(client: &reqwest::Client, port: u16, timeout: Duration, input: &Path, opts: &OfficeOptions)` — defined in Task 3, called in Task 4 Step 7 ✓
- `LibreOfficeConfig::unoserver_port: u16`, `unoserver_ready_timeout: Duration` — defined in Task 4 Step 3, used in Task 6 Step 7 ✓
