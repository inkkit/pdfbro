use std::path::Path;
use std::time::Duration;

use tokio::process::Child;
use tracing::info;

use crate::types::{EngineError, EngineResult};

pub(super) struct UnoserverProcess {
    child: Child,
    port: u16,
}

impl std::fmt::Debug for UnoserverProcess {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UnoserverProcess")
            .field("port", &self.port)
            .finish_non_exhaustive()
    }
}

impl UnoserverProcess {
    pub(super) async fn spawn(
        port: u16,
        ready_timeout: Duration,
        executable: Option<&Path>,
    ) -> EngineResult<Self> {
        // Port 0 means "ask the OS for a free ephemeral port". We bind a
        // listener, capture the port, drop the listener, then hand the port
        // to unoserver. Avoids clashes when several engines start in
        // sequence (e.g. integration tests) where the previous process
        // hasn't fully released the socket yet.
        let port = if port == 0 {
            let listener = std::net::TcpListener::bind("127.0.0.1:0").map_err(|e| {
                EngineError::Internal(format!("failed to pick free port: {e}"))
            })?;
            listener
                .local_addr()
                .map_err(|e| EngineError::Internal(format!("local_addr: {e}")))?
                .port()
        } else {
            port
        };
        info!(port, "Starting unoserver");

        let mut cmd = tokio::process::Command::new("unoserver");
        cmd.args([
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
        // Port 19876 has nothing listening — expect Timeout when unoserver starts
        // but never binds, or Internal when the binary isn't installed at all.
        let result = UnoserverProcess::spawn(19876, Duration::from_millis(300), None).await;
        assert!(
            matches!(
                result,
                Err(EngineError::Timeout(_)) | Err(EngineError::Internal(_))
            ),
            "expected Timeout or Internal, got: {result:?}"
        );
    }
}
