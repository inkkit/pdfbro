//! Stream helpers: read input from a path or stdin (`-`), write output
//! to a path or stdout (`-`).
//!
//! `write_stdout_pdf` writes raw PDF bytes to stdout (locked) and flushes
//! before returning so callers piping into another process see complete
//! output even on early exit.

use std::io::{Read, Write};
use std::path::Path;

use anyhow::Context;
use tokio::io::AsyncReadExt;

/// Returns `true` iff `spec` denotes stdin (a single dash).
pub(crate) fn is_stdin(spec: &str) -> bool {
    spec == "-"
}

/// Returns `true` iff `spec` denotes stdout (a single dash).
pub(crate) fn is_stdout(spec: &str) -> bool {
    spec == "-"
}

/// Read a file (or stdin if `spec == "-"`) into a byte vector.
pub(crate) fn read_input_sync(spec: &str) -> anyhow::Result<Vec<u8>> {
    if is_stdin(spec) {
        let mut buf = Vec::new();
        std::io::stdin()
            .lock()
            .read_to_end(&mut buf)
            .context("reading stdin")?;
        Ok(buf)
    } else {
        std::fs::read(spec).with_context(|| format!("reading {spec}"))
    }
}

/// Async variant of [`read_input_sync`] for places that want to stay on
/// the tokio runtime (e.g. `convert --stdin`).
pub(crate) async fn read_stdin_async() -> anyhow::Result<Vec<u8>> {
    let mut buf = Vec::new();
    tokio::io::stdin()
        .read_to_end(&mut buf)
        .await
        .context("reading stdin")?;
    Ok(buf)
}

/// Write `bytes` to `spec` — `-` for stdout, otherwise a filesystem path.
/// Parent directories of file targets are created on demand.
pub(crate) fn write_output(spec: &str, bytes: &[u8]) -> anyhow::Result<()> {
    if is_stdout(spec) {
        write_stdout_pdf(bytes)
    } else {
        let path = Path::new(spec);
        if let Some(parent) = path.parent()
            && !parent.as_os_str().is_empty()
        {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("creating output directory {}", parent.display()))?;
        }
        std::fs::write(path, bytes).with_context(|| format!("writing {spec}"))?;
        Ok(())
    }
}

fn write_stdout_pdf(bytes: &[u8]) -> anyhow::Result<()> {
    let stdout = std::io::stdout();
    let mut h = stdout.lock();
    h.write_all(bytes).context("writing to stdout")?;
    h.flush().context("flushing stdout")?;
    Ok(())
}
