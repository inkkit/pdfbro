//! CLI startup banner for `pdfbro`.
//!
//! Prints a decorative ASCII logo and version on startup when
//! stdout is a TTY and the log format is text.  Suppressed when
//! `--log-format json` or `NO_COLOR` is set.

use std::io::IsTerminal;

use crate::args::LogFormat;
use crate::args::GlobalOpts;

/// Print the CLI banner if conditions are right.
pub fn print(global: &GlobalOpts) {
    let want_json = match global.log_format {
        Some(LogFormat::Json) => true,
        Some(LogFormat::Text) => false,
        None => !std::io::stdout().is_terminal(),
    };

    if want_json || !std::io::stdout().is_terminal() {
        return;
    }

    let c = use_color();
    let version = env!("CARGO_PKG_VERSION");

    println!(
        "\n{}\n\n    {}  {}\n    {}: {}\n    {}\n",
        ascii_logo(),
        format!("{}{}", color("PDF", "36;1", c), color("bro", "36", c)),
        color("— A Rust-powered document-to-PDF API", "0", c),
        color("Version", "2", c),
        color(version, "0", c),
        color("─────────────────────────────────────────────────────", "2", c),
    );
}

fn use_color() -> bool {
    std::io::stdout().is_terminal() && std::env::var("NO_COLOR").is_err()
}

fn color(s: &str, code: &str, enabled: bool) -> String {
    if enabled {
        format!("\x1b[{}m{}\x1b[0m", code, s)
    } else {
        s.to_string()
    }
}

fn ascii_logo() -> String {
    const LOGO: &str = r#"██████╗ ██████╗ ███████╗
██╔══██╗██╔══██╗██╔════╝
██████╔╝██║  ██║█████╗
██╔═══╝ ██║  ██║██╔══╝
██║     ██████╔╝██║
╚═╝     ╚═════╝ ╚═╝ [bro]"#;
    LOGO.lines().map(|l| format!("    {l}")).collect::<Vec<_>>().join("\n")
}
