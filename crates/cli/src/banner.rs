//! CLI startup banner for `pdfbro`.
//!
//! Prints a decorative ASCII logo and version to **stderr** on startup
//! (so it never pollutes the data stream of pipeable subcommands like
//! `completions`, `convert -o -`, or `metadata read`). Suppressed when
//! `--log-format json` is explicitly requested.

use std::io::IsTerminal;

use crate::args::LogFormat;
use crate::args::GlobalOpts;

/// Print the CLI banner if conditions are right.
///
/// Goes to **stderr**, not stdout. The CLI's stdout is reserved for
/// machine-readable output (PDF bytes when `-o -`, shell completion
/// scripts, JSON metadata, etc.); decorating it with the banner
/// breaks every consumer downstream of a pipe.
///
/// Suppressed when `--log-format json` is explicitly set. Color is
/// automatically disabled when stderr is not a TTY (which is the
/// usual case under CI / pipes).
pub fn print(global: &GlobalOpts) {
    let want_json = matches!(global.log_format, Some(LogFormat::Json));
    if want_json {
        return;
    }

    let c = use_color();
    let version = env!("CARGO_PKG_VERSION");

    eprintln!(
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
    std::io::stderr().is_terminal() && std::env::var("NO_COLOR").is_err()
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
