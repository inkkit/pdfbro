//! `folio completions <shell>` — emit a clap completion script to stdout.

use clap::CommandFactory;
use clap_complete::{Shell, generate};

use crate::args::Cli;

/// Render a completion script for `shell` to stdout.
pub(crate) fn run(shell: Shell) -> anyhow::Result<()> {
    let mut cmd = Cli::command();
    let bin_name = cmd.get_name().to_string();
    let mut out = std::io::stdout().lock();
    generate(shell, &mut cmd, bin_name, &mut out);
    Ok(())
}
