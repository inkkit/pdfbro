//! Startup banner for `folio-server`.
//!
//! Prints a decorative ASCII logo, version, and a capability matrix on
//! startup when the log format is text and stdout is a TTY.  The output
//! is sent directly to stdout so it appears once before structured logs
//! begin.

use std::io::IsTerminal;

use crate::config::LogFormat;
use crate::ServerConfig;

/// Print the startup banner if conditions are right.
///
/// The banner is suppressed when `log_format` is [`LogFormat::Json`] or
/// when stdout is not a terminal.
pub fn print(config: &ServerConfig, chromium_ready: bool, libreoffice_ready: bool) {
    if matches!(config.log_format, LogFormat::Json) || !std::io::stdout().is_terminal() {
        return;
    }

    let version = env!("CARGO_PKG_VERSION");

    println!(
        r#"
  ███████╗  ██████╗  ██╗      ██╗  ██████╗
  ██╔════╝ ██╔═══██╗ ██║      ██║ ██╔═══██╗
  █████╗   ██║   ██║ ██║      ██║ ██║   ██║
  ██╔══╝   ██║   ██║ ██║      ██║ ██║   ██║
  ██║      ╚██████╔╝ ███████╗ ██║ ╚██████╔╝
  ╚═╝       ╚═════╝  ╚══════╝ ╚═╝  ╚═════╝

  Folio  —  A Rust-powered document-to-PDF API
  Version: {version}
  ─────────────────────────────────────────────────────

  Modules     api  chromium  libreoffice  pdfengines  webhook

  Chromium      {}
  LibreOffice   {}

  PDF Engines
    merge       ✓
    split       ✓
    flatten     ✓
    metadata    ✓  (read / write)
    convert     ✓
    bookmarks   ✓  (read / write)
    watermark   ✓
    stamp       ✓
    encrypt     ✓
    decrypt     ✓
    rotate      ✓

  API           http://{}:{}
"#,
        status(chromium_ready),
        status(libreoffice_ready),
        config.host,
        config.port,
    );
}

fn status(ready: bool) -> &'static str {
    if ready {
        "ready"
    } else {
        "unavailable"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{IpAddr, Ipv4Addr};

    fn dummy_config() -> ServerConfig {
        ServerConfig {
            host: IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
            port: 3000,
            concurrency: 4,
            max_body_bytes: 1024,
            request_timeout: std::time::Duration::from_secs(30),
            chrome_path: None,
            no_sandbox: None,
            soffice_path: None,
            log_level: "info".to_string(),
            log_format: LogFormat::Text,
        }
    }

    #[test]
    fn print_does_not_panic() {
        // We can't assert stdout in a unit test easily, but we can at
        // least exercise the formatting code path.
        let config = dummy_config();
        print(&config, true, true);
    }
}

