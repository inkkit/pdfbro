//! Subcommand handlers. Each handler returns `anyhow::Result<()>`; the
//! main dispatcher converts that into a process exit code via
//! `crate::exit::exit_for_anyhow`.

pub(crate) mod batch;
pub(crate) mod completions;
pub(crate) mod convert;
pub(crate) mod encrypt;
pub(crate) mod metadata;
pub(crate) mod pdfops;
