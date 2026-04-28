//! Startup banner for `folio-server`.
//!
//! Prints a decorative ASCII logo, version, and a capability matrix on
//! startup when the log format is text and stdout is a TTY.  The output
//! is sent directly to stdout so it appears once before structured logs
//! begin.
//!
//! Layout is data-driven: each section computes its own column width so
//! adding or removing rows never breaks alignment.  Color is applied
//! *after* padding so escape codes do not interfere with spacing.

use std::io::IsTerminal;

use crate::config::LogFormat;
use crate::ServerConfig;

/// A single label / value pair rendered as one banner line.
struct Row<'a> {
    label: &'a str,
    value: String,
}

/// Print the startup banner if conditions are right.
///
/// The banner is suppressed when `log_format` is [`LogFormat::Json`] or
/// when stdout is not a terminal.  Color is also disabled when the
/// `NO_COLOR` environment variable is present.
pub fn print(config: &ServerConfig, chromium_ready: bool, libreoffice_ready: bool) {
    if matches!(config.log_format, LogFormat::Json) || !std::io::stdout().is_terminal() {
        return;
    }

    let c = use_color();
    let version = env!("CARGO_PKG_VERSION");

    // ── Services section ─────────────────────────────────────────────
    let services = vec![
        Row {
            label: "Chromium",
            value: status(chromium_ready, c),
        },
        Row {
            label: "LibreOffice",
            value: status(libreoffice_ready, c),
        },
    ];
    let service_width = compute_width(&services);

    // ── PDF Engines section ──────────────────────────────────────────
    let engines = vec![
        Row { label: "merge",     value: check(c) },
        Row { label: "split",     value: check(c) },
        Row { label: "flatten",   value: check(c) },
        Row { label: "metadata",  value: rw_check(c) },
        Row { label: "convert",   value: check(c) },
        Row { label: "bookmarks", value: rw_check(c) },
        Row { label: "watermark", value: check(c) },
        Row { label: "stamp",     value: check(c) },
        Row { label: "encrypt",   value: check(c) },
        Row { label: "decrypt",   value: check(c) },
        Row { label: "rotate",    value: check(c) },
    ];
    let engine_width = compute_width(&engines);

    // Single shared width so [OK]/[FAIL] tags align across sections.
    let label_width = service_width.max(engine_width);

    let services_block = services
        .iter()
        .map(|r| format_row(r.label, &r.value, label_width))
        .collect::<Vec<_>>()
        .join("\n");

    let engines_block = engines
        .iter()
        .map(|r| format_row(r.label, &r.value, label_width))
        .collect::<Vec<_>>()
        .join("\n");

    println!(
        "\n{}\n\n{}  {}\n  {}: {}\n  {}\n\n{}\n{}\n\n{}\n{}\n\n{}\n  http://{}:{}\n",
        ascii_logo(),
        color("Folio", "36;1", c),      // cyan bold
        color("— A Rust-powered document-to-PDF API", "0", c),
        color("Version", "2", c),         // dim
        color(version, "0", c),
        color("─────────────────────────────────────────────────────", "2", c),
        color("Services", "2", c),
        services_block,
        color("PDF Engines", "2", c),
        engines_block,
        color("API", "2", c),
        config.host,
        config.port,
    );
}

// ── Helpers ─────────────────────────────────────────────────────────

/// Width of the longest label in a section.
fn compute_width(rows: &[Row<'_>]) -> usize {
    rows.iter().map(|r| r.label.len()).max().unwrap_or(0)
}

/// Render one row: plain left-aligned label, already-colored value.
fn format_row(label: &str, value: &str, width: usize) -> String {
    format!("  {:<width$} {}", label, value, width = width)
}

/// `true` when stdout is a TTY **and** `NO_COLOR` is absent.
fn use_color() -> bool {
    std::io::stdout().is_terminal() && std::env::var("NO_COLOR").is_err()
}

/// Wrap a string in an ANSI color sequence (or pass through).
fn color(s: &str, code: &str, enabled: bool) -> String {
    if enabled {
        format!("\x1b[{}m{}\x1b[0m", code, s)
    } else {
        s.to_string()
    }
}

/// Colored status tag with a fixed visible width so columns stay aligned
/// when the state flips between ready / unavailable.
fn status(ready: bool, c: bool) -> String {
    let plain = if ready {
        format!("{:<20}", "[OK] ready")
    } else {
        format!("{:<20}", "[FAIL] unavailable")
    };
    color(&plain, if ready { "32" } else { "31" }, c)
}

/// Simple OK tag for capability rows.
fn check(c: bool) -> String {
    color("[OK]", "32", c)
}

/// Read-write OK tag with annotation.
fn rw_check(c: bool) -> String {
    format!("{}  {}", color("[OK]", "32", c), color("(read / write)", "2", c))
}

/// Pixel-art FOLIO logo.
fn ascii_logo() -> &'static str {
    r#"  ███████╗  ██████╗  ██╗      ██╗  ██████╗
  ██╔════╝ ██╔═══██╗ ██║      ██║ ██╔═══██╗
  █████╗   ██║   ██║ ██║      ██║ ██║   ██║
  ██╔══╝   ██║   ██║ ██║      ██║ ██║   ██║
  ██║      ╚██████╔╝ ███████╗ ██║ ╚██████╔╝
  ╚═╝       ╚═════╝  ╚══════╝ ╚═╝  ╚═════╝"#
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

