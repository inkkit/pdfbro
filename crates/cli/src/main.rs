#![deny(rust_2018_idioms)]
#![forbid(unsafe_code)]
//! `folio` — Folio's command-line entry point.
//!
//! `main` is intentionally tiny: install a tracing subscriber configured
//! against the user's chosen log format, build a multi-thread tokio
//! runtime, and dispatch the parsed clap args onto a subcommand handler.
//! All commands return `anyhow::Result<()>`; the dispatcher walks the
//! error chain to pick a [`exit::ExitCode`].

mod args;
mod commands;
mod exit;
mod io_helpers;
mod model;
mod parse;

use std::io::IsTerminal;
use std::process::ExitCode as ProcExitCode;

use clap::Parser;

use crate::args::{Cli, Commands, GlobalOpts, LogFormat};
use crate::exit::{ExitCode, exit_for_anyhow};

fn main() -> ProcExitCode {
    // Parse first so usage / parse errors flow through clap → exit 2.
    let cli = match Cli::try_parse() {
        Ok(c) => c,
        Err(e) => {
            // clap renders its own errors and uses exit code 2 for
            // usage / parse problems; success-style outputs (--help,
            // --version) go to stdout.
            let _ = e.print();
            return match e.kind() {
                clap::error::ErrorKind::DisplayHelp | clap::error::ErrorKind::DisplayVersion => {
                    ProcExitCode::SUCCESS
                }
                _ => ProcExitCode::from(ExitCode::Usage.as_i32() as u8),
            };
        }
    };

    init_tracing(&cli.global);

    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(e) => {
            eprintln!("error: failed to build tokio runtime: {e}");
            return ProcExitCode::from(ExitCode::Generic.as_i32() as u8);
        }
    };

    let result = runtime.block_on(dispatch(cli));

    match result {
        Ok(()) => ProcExitCode::SUCCESS,
        Err(err) => {
            let code = exit_for_anyhow(&err);
            eprintln!("error: {err:#}");
            ProcExitCode::from(code.as_i32() as u8)
        }
    }
}

async fn dispatch(cli: Cli) -> anyhow::Result<()> {
    let Cli { global, command } = cli;
    match command {
        Commands::Convert(args) => commands::convert::run(&global, &args).await,
        Commands::Batch(args) => commands::batch::run(&global, &args).await,
        Commands::Merge(args) => commands::pdfops::run_merge(&args),
        Commands::Split(args) => commands::pdfops::run_split(&args),
        Commands::Flatten(args) => commands::pdfops::run_flatten(&args),
        Commands::Metadata { action } => commands::metadata::run(&action),
        Commands::Completions { shell } => commands::completions::run(shell),
    }
}

/// Install a `tracing-subscriber` writing to stderr.
///
/// Precedence:
///
/// 1. `RUST_LOG` (if set) is parsed by `EnvFilter` and overrides verbosity flags.
/// 2. `--quiet` → `off`.
/// 3. `-v..-vvv` → `info`, `debug`, `trace` respectively.
/// 4. Default → `warn`.
///
/// The format is JSON when `--log-format json` is requested or stderr is
/// not a TTY; otherwise plain text.
fn init_tracing(global: &GlobalOpts) {
    use tracing_subscriber::{EnvFilter, fmt};

    let env_filter = std::env::var("RUST_LOG").ok();
    let filter = if let Some(spec) = env_filter {
        EnvFilter::try_new(spec).unwrap_or_else(|_| EnvFilter::new("warn"))
    } else if global.quiet {
        EnvFilter::new("off")
    } else {
        EnvFilter::new(match global.verbose {
            0 => "warn",
            1 => "info",
            2 => "debug",
            _ => "trace",
        })
    };

    let want_json = match global.log_format {
        Some(LogFormat::Json) => true,
        Some(LogFormat::Text) => false,
        None => !std::io::stderr().is_terminal(),
    };

    let stderr_is_tty = std::io::stderr().is_terminal();
    let builder = fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .with_target(false)
        .with_ansi(stderr_is_tty);

    if want_json {
        let _ = builder.json().try_init();
    } else {
        let _ = builder.try_init();
    }
}
