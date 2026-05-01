# LibreOffice Performance: unoserver + LO 26.x Design

## Goal

Close the 2.4× LibreOffice latency gap against Gotenberg by replacing per-request
`soffice` subprocess spawning with a persistent `unoserver` UNO listener, and
upgrading LibreOffice from Debian bookworm's 7.4 to TDF's 26.2.

**Target:** p50 LibreOffice latency ≤ 550ms (from 1256ms), matching or beating
Gotenberg's 528ms on equivalent hardware.

---

## Background and Facts

Benchmark results from Docker run (2 CPU / 2 GB, 2026-05-01):

| Workload         | Folio p50 | Gotenberg p50 | Gap    |
|------------------|-----------|---------------|--------|
| libreoffice-docx | 1256ms    | 528ms         | 2.4×   |

Two root causes identified:

1. **Version gap**: Debian bookworm ships LibreOffice 7.4. LibreOffice 25.8
   introduced a ~30% file-loading improvement (internal document model
   restructuring). Gotenberg ships 26.2.

2. **Architecture gap**: Folio spawns a fresh `soffice` process per request
   (~200–400ms startup cost). Gotenberg runs `unoserver`, a persistent LibreOffice
   UNO listener that keeps LO loaded between requests.

---

## Architecture

```
Request
  │
  ▼
SupervisedLibreOfficeEngine
  │  (existing, unchanged)
  ▼
LibreOfficeEngine::convert()
  │  semaphore gate (unchanged)
  ▼
convert::run_convert()          ← HTTP POST instead of subprocess spawn
  │
  │  multipart/form-data
  │  POST http://127.0.0.1:2003/
  ▼
unoserver (Python, persistent)  ← new: managed child process
  │  UNO bridge (C++ via libpyuno)
  ▼
LibreOffice 26.2 (persistent)   ← stays loaded between requests
  │
  ▼
PDF bytes
```

`unoserver` is a Python process Folio spawns and supervises at engine startup,
identical in lifecycle to how Chromium is supervised. It communicates with
LibreOffice via its internal UNO C++ bridge (not interpreted per-call). Python's
per-request overhead is 2–5ms — under 0.5% of conversion time.

---

## Dockerfile Changes

### LibreOffice upgrade (TDF 26.2 on bookworm)

Replace the existing `libreoffice-*` apt install block in the `folio` and
`folio-libreoffice` stages with:

```dockerfile
ARG LIBREOFFICE_VERSION=26.2

RUN curl -fsSL \
      "https://deb.libreoffice.org/libreoffice/libreoffice-${LIBREOFFICE_VERSION//./-}/Release.key" \
      | gpg --dearmor -o /usr/share/keyrings/libreoffice.gpg && \
    printf "deb [signed-by=/usr/share/keyrings/libreoffice.gpg] \
      https://deb.libreoffice.org/libreoffice/libreoffice-%s/ bookworm main\n" \
      "${LIBREOFFICE_VERSION//./-}" \
      > /etc/apt/sources.list.d/libreoffice.list && \
    apt-get update -qq && \
    DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends \
      libreoffice-writer \
      libreoffice-calc \
      libreoffice-impress \
      libreoffice-draw \
      python3-minimal \
      python3-pip && \
    pip3 install --no-cache-dir unoserver==2.2.1 && \
    rm -rf /var/lib/apt/lists/* /tmp/* /var/tmp/*
```

### New environment variable

```dockerfile
ENV SAL_USE_VCLPLUGIN=svp
```

`SAL_USE_VCLPLUGIN=svp` forces LibreOffice to use the headless SVP rendering
backend, skipping virtual display probing. Saves ~50–100ms on LibreOffice's
internal startup path. Set at image level so both `unoserver` and any probe
commands inherit it.

The `LIBREOFFICE_VERSION` ARG allows pinned bumps via
`--build-arg LIBREOFFICE_VERSION=26.2` without editing the Dockerfile body.

---

## Engine Changes

### New file: `crates/engine/src/libreoffice/unoserver.rs`

Manages the unoserver child process. Public API:

```rust
pub struct UnoserverProcess {
    child: tokio::process::Child,
    port: u16,
}

impl UnoserverProcess {
    /// Spawn unoserver on `port`, wait until it accepts HTTP requests.
    /// `ready_timeout` is the maximum time to wait for LO to initialise.
    pub async fn spawn(port: u16, ready_timeout: Duration) -> EngineResult<Self>;

    /// The port unoserver is listening on.
    pub fn port(&self) -> u16;
}

impl Drop for UnoserverProcess {
    // Send SIGTERM to the child process.
}
```

**Spawn command:**
```
python3 -m unoserver --interface 127.0.0.1 --port <port>
```
Inherits `SAL_USE_VCLPLUGIN=svp` from the process environment (set at image level).

**Ready detection:** Poll `GET http://127.0.0.1:<port>/` with 500ms interval.
First HTTP response (any status code) means the server is accepting connections.
If `ready_timeout` elapses without a response, return `EngineError::Timeout`.

**Crash monitoring:** A background `tokio::spawn` task awaits `child.wait()`.
On unexpected exit it logs a `tracing::error!` event. The `LibreOfficeEngine`
restarts unoserver via the restart logic described in Error Handling below.

### Modified: `crates/engine/src/libreoffice/convert.rs`

Replace the `tokio::process::Command` subprocess block with an HTTP POST:

```rust
pub(super) async fn run_convert(
    client: &reqwest::Client,
    port: u16,
    timeout: Duration,
    input: &Path,
    opts: &OfficeOptions,
) -> EngineResult<Vec<u8>> {
    let file_bytes = tokio::fs::read(input).await?;
    let ext = input.extension().and_then(|s| s.to_str()).unwrap_or_default();
    let filtername = filter::for_extension(ext);   // same lookup table as before

    let mut form = reqwest::multipart::Form::new()
        .part("file", reqwest::multipart::Part::bytes(file_bytes)
            .file_name(input.file_name()...))
        .text("output-file", "output.pdf");

    if filtername != "pdf" {
        form = form.text("filtername", filtername.to_string());
    }
    if let Some(blob) = opts.filter_blob() {
        form = form.text("filteroptions", blob);
    }

    let url = format!("http://127.0.0.1:{port}/");
    let response = tokio::time::timeout(
        timeout,
        client.post(&url).multipart(form).send(),
    )
    .await
    .map_err(|_| EngineError::Timeout(timeout))?
    .map_err(|e| EngineError::Internal(format!("unoserver request failed: {e}")))?;

    if !response.status().is_success() {
        let body = response.text().await.unwrap_or_default();
        return Err(EngineError::Internal(format!(
            "unoserver error {}: {body}", status
        )));
    }

    Ok(response.bytes().await?.to_vec())
}
```

The temp directory for input is no longer needed for output staging (unoserver
returns bytes directly), but is still used to stage the input file safely.

### Modified: `crates/engine/src/libreoffice/mod.rs`

**`LibreOfficeConfig` additions:**

```rust
pub struct LibreOfficeConfig {
    // ... existing fields ...

    /// Port unoserver listens on. Default: 2003.
    pub unoserver_port: u16,
    /// Maximum time to wait for unoserver to be ready at startup. Default: 60s.
    pub unoserver_ready_timeout: Duration,
}
```

**`Inner` struct additions:**

```rust
struct Inner {
    unoserver: Mutex<UnoserverProcess>,
    http_client: reqwest::Client,  // shared, connection-pooled
    timeout: Duration,
    semaphore: Semaphore,
}
```

**`launch()` updated flow:**
1. Spawn `UnoserverProcess::spawn(config.unoserver_port, config.unoserver_ready_timeout)`.
2. Build a `reqwest::Client` with `tcp_keepalive` and `pool_max_idle_per_host(1)`.
3. Store both in `Inner`.

**`healthy()` updated:** HTTP `GET http://127.0.0.1:<port>/` with a 5s timeout.
Returns `true` on any HTTP response (unoserver is alive), `false` on connection error.

**No changes to:**
- `convert_many()` — still fans out via `convert()`
- `OfficeOptions` and `filter_blob()`
- `filter::for_extension()`

### Dependency addition: `reqwest`

Add to `crates/engine/Cargo.toml`:

```toml
[dependencies]
reqwest = { version = "0.12", default-features = false, features = ["rustls-tls", "multipart"] }
```

`rustls-tls` keeps OpenSSL out of the image. Only localhost HTTP is used, so
TLS is not exercised, but the feature flag is the idiomatic choice for this
codebase.

---

## Error Handling

### Crash recovery

A background task in `LibreOfficeEngine::launch()` monitors the unoserver
process. On unexpected exit:

1. Acquire the `Mutex<UnoserverProcess>`.
2. Attempt restart up to **3 times** with exponential backoff: 2s, 4s, 8s.
3. Each attempt: `UnoserverProcess::spawn(port, ready_timeout)`.
4. On success: replace the stored process, log `info!("unoserver restarted")`.
5. On 3rd failure: log `error!("unoserver failed to restart, conversions will fail")`.
   Subsequent `convert()` calls return `EngineError::Internal("unoserver unavailable")`.

Requests in-flight during a crash receive a connection-refused error from
`reqwest`, mapped to `EngineError::Internal`. The semaphore is not poisoned —
the next request will attempt normally and either succeed (if restarted) or get
the unavailable error.

### Port conflict

`UnoserverProcess::spawn()` attempts to bind on `port`. If the port is already
in use, unoserver exits immediately with a non-zero status. `spawn()` detects
this during ready-polling (no response within timeout) and returns
`EngineError::Internal("unoserver failed to start: port 2003 may be in use")`.

### Bad / corrupt input files

unoserver returns HTTP 500 with an error body. `run_convert()` maps this to
`EngineError::Internal` containing the response body text.

### Timeout

The existing 120s `config.timeout` applies to the full HTTP round-trip.
`tokio::time::timeout` wraps the `client.send()` + `response.bytes()` chain.

---

## Testing

### Unit tests (no LO required)

- `filter_blob()` roundtrip tests: unchanged, no LO needed.
- `UnoserverProcess::spawn()` with a mock HTTP server to verify ready-polling logic.
- `run_convert()` with a mock unoserver (wiremock or a tiny `axum` test server)
  that returns a known PDF blob — verifies multipart field names and filter
  option encoding.

### Integration tests (require LO in CI)

`crates/engine/tests/libreoffice.rs` — existing tests exercise the full stack.
They will exercise the new HTTP path automatically once `convert.rs` is updated.
No test changes required beyond updating any process-spawn assertions.

### Benchmark validation

After implementation, re-run `cargo run -p bench -- --docker` with the same
docker-compose.bench.yml setup used for the baseline. Target: Folio
`libreoffice-docx` p50 ≤ 550ms.

---

## Files Changed

| File | Change |
|------|--------|
| `Dockerfile` | TDF LO 26.2 apt repo, unoserver pip install, `SAL_USE_VCLPLUGIN=svp` |
| `crates/engine/Cargo.toml` | Add `reqwest` dependency |
| `crates/engine/src/libreoffice/unoserver.rs` | New: process management |
| `crates/engine/src/libreoffice/convert.rs` | Replace subprocess with HTTP POST |
| `crates/engine/src/libreoffice/mod.rs` | Add `unoserver` + `http_client` to `Inner`, update config |
| `crates/engine/src/libreoffice/discover.rs` | Remove: `find_soffice()` is no longer needed (unoserver locates soffice itself); `probe()` is replaced by the HTTP ready-check in `unoserver.rs` |
| `crates/server/src/supervised_engine.rs` | No changes |
| `crates/server/src/config.rs` | Expose `unoserver_port` and `unoserver_ready_timeout` config knobs |

---

## What Does Not Change

- `SupervisedLibreOfficeEngine` — identical
- `OfficeOptions` public API — identical
- `filter::for_extension()` — identical
- `convert_many()` — identical
- HTTP routes and request/response shapes — identical
- Semaphore-based concurrency control — identical
- All existing `OfficeOptions` unit tests — pass without modification
