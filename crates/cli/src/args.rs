//! `clap` derive structs for the entire `folio` binary surface.
//!
//! Each subcommand has its own `*Args` struct; PDF / RequestContext /
//! Office options are factored into reusable `*Flags` structs that are
//! `#[command(flatten)]`-ed into both `convert` and `batch`.

use std::path::PathBuf;
use std::time::Duration;

use clap::{ArgAction, ArgGroup, Args, Parser, Subcommand, ValueEnum};
use clap_complete::Shell;
use engine::{Cookie, Margins, PageRanges, PaperSize, WaitCondition};

use crate::parse;

// ---------------------------------------------------------------------------
// Top-level
// ---------------------------------------------------------------------------

/// Folio command-line interface.
#[derive(Debug, Parser)]
#[command(
    name = "folio",
    version,
    about = "Convert HTML / URL / Markdown / Office to PDF, and post-process PDFs",
    propagate_version = true
)]
pub(crate) struct Cli {
    #[command(flatten)]
    pub(crate) global: GlobalOpts,

    #[command(subcommand)]
    pub(crate) command: Commands,
}

/// Global options — apply to every subcommand. Marked `global = true`
/// so they may appear before or after the subcommand on the command line.
#[derive(Debug, Args)]
pub(crate) struct GlobalOpts {
    /// Increase log verbosity (-v info, -vv debug, -vvv trace).
    #[arg(short, long, action = ArgAction::Count, global = true)]
    pub(crate) verbose: u8,

    /// Suppress log output (overrides -v).
    #[arg(short, long, global = true)]
    pub(crate) quiet: bool,

    /// Log format. Default: text on a TTY, json otherwise.
    #[arg(long, value_enum, global = true)]
    pub(crate) log_format: Option<LogFormat>,

    /// Override the Chrome executable path.
    #[arg(long, global = true)]
    pub(crate) chrome: Option<PathBuf>,

    /// Pass --no-sandbox to Chrome (default true on Linux).
    #[arg(long, global = true, overrides_with = "sandbox")]
    pub(crate) no_sandbox: bool,

    /// Force the Chrome sandbox on.
    #[arg(long, global = true, overrides_with = "no_sandbox")]
    pub(crate) sandbox: bool,

    /// Per-render timeout, e.g. "60s", "2m". Default 60s.
    #[arg(long, global = true, value_parser = parse::parse_duration)]
    pub(crate) timeout: Option<Duration>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub(crate) enum LogFormat {
    Text,
    Json,
}

#[derive(Debug, Subcommand)]
pub(crate) enum Commands {
    /// Convert a single input source to PDF.
    Convert(ConvertArgs),

    /// Convert many files matching a glob from one directory tree to another.
    Batch(BatchArgs),

    /// Merge two or more PDFs into a single output document.
    Merge(MergeArgs),

    /// Split a PDF into multiple smaller documents.
    Split(SplitArgs),

    /// Flatten interactive widgets and annotations.
    Flatten(FlattenArgs),

    /// Read or write the document metadata (`/Info` dictionary).
    Metadata {
        #[command(subcommand)]
        action: MetadataAction,
    },

    /// Encrypt a PDF with password protection.
    Encrypt(EncryptArgs),

    /// Remove encryption from a PDF.
    Decrypt(DecryptArgs),

    /// Emit a shell completion script to stdout.
    Completions {
        /// Target shell.
        shell: Shell,
    },
}

// ---------------------------------------------------------------------------
// convert
// ---------------------------------------------------------------------------

#[derive(Debug, Args)]
#[command(group = ArgGroup::new("convert_input").required(true).multiple(false).args([
    "html", "url", "markdown", "office", "stdin",
]))]
pub(crate) struct ConvertArgs {
    /// HTML file to convert.
    #[arg(long, value_name = "FILE")]
    pub(crate) html: Option<PathBuf>,
    /// URL to fetch and render.
    #[arg(long, value_name = "URL")]
    pub(crate) url: Option<String>,
    /// Markdown file to convert.
    #[arg(long, value_name = "FILE")]
    pub(crate) markdown: Option<PathBuf>,
    /// Office document to convert via LibreOffice.
    #[arg(long, value_name = "FILE")]
    pub(crate) office: Option<PathBuf>,
    /// Read input from stdin (use `--as` to set the kind).
    #[arg(long)]
    pub(crate) stdin: bool,

    /// What kind of document is being read on stdin (only valid with `--stdin`).
    #[arg(long = "as", value_enum, default_value_t = StdinKind::Html)]
    pub(crate) stdin_kind: StdinKind,

    /// Output file path or `-` for stdout.
    #[arg(long, value_name = "FILE", required = true)]
    pub(crate) output: String,

    #[command(flatten)]
    pub(crate) pdf: PdfFlags,
    #[command(flatten)]
    pub(crate) req: RequestFlags,
    #[command(flatten)]
    pub(crate) office_opts: OfficeFlags,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub(crate) enum StdinKind {
    Html,
    Markdown,
}

// ---------------------------------------------------------------------------
// batch
// ---------------------------------------------------------------------------

#[derive(Debug, Args)]
pub(crate) struct BatchArgs {
    /// Walked recursively. Required.
    #[arg(long, value_name = "DIR", required = true)]
    pub(crate) input_dir: PathBuf,
    /// Output tree. Mirrors `--input-dir`. Required.
    #[arg(long, value_name = "DIR", required = true)]
    pub(crate) output_dir: PathBuf,
    /// File-name glob. Default: `**/*.{html,htm,md,markdown}`.
    #[arg(long, default_value = "**/*.{html,htm,md,markdown}")]
    pub(crate) pattern: String,
    /// Maximum concurrent renders. Default: number of CPUs.
    #[arg(long, value_name = "N")]
    pub(crate) concurrency: Option<usize>,
    /// What to do on a per-file failure.
    #[arg(long, value_enum, default_value_t = OnError::Skip)]
    pub(crate) on_error: OnError,
    /// Print planned conversions, do nothing.
    #[arg(long)]
    pub(crate) dry_run: bool,

    #[command(flatten)]
    pub(crate) pdf: PdfFlags,
    #[command(flatten)]
    pub(crate) req: RequestFlags,
    #[command(flatten)]
    pub(crate) office_opts: OfficeFlags,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub(crate) enum OnError {
    Stop,
    Skip,
}

// ---------------------------------------------------------------------------
// merge / split / flatten / metadata
// ---------------------------------------------------------------------------

#[derive(Debug, Args)]
pub(crate) struct MergeArgs {
    /// Output file path.
    #[arg(long, value_name = "FILE", required = true)]
    pub(crate) output: String,
    /// Input PDFs in concatenation order. `-` means stdin (only allowed once).
    #[arg(value_name = "INPUT", required = true, num_args = 1..)]
    pub(crate) inputs: Vec<String>,
}

#[derive(Debug, Args)]
pub(crate) struct SplitArgs {
    /// PDF to split.
    #[arg(value_name = "INPUT")]
    pub(crate) input: PathBuf,
    /// Directory to write output files into.
    #[arg(long, value_name = "DIR", required = true)]
    pub(crate) output_dir: PathBuf,
    /// File-name prefix. Default: `<input-basename>`.
    #[arg(long, value_name = "STR")]
    pub(crate) prefix: Option<String>,
    /// Split mode: `ranges:1-3,5,7-`, `every-n:5`, or `one-per-page`.
    #[arg(long, value_name = "SPEC", default_value = "one-per-page")]
    pub(crate) mode: String,
}

#[derive(Debug, Args)]
pub(crate) struct FlattenArgs {
    /// Input PDF (`-` for stdin).
    #[arg(value_name = "INPUT")]
    pub(crate) input: String,
    /// Output PDF (`-` for stdout).
    #[arg(long, value_name = "FILE", required = true)]
    pub(crate) output: String,
}

#[derive(Debug, Args)]
pub(crate) struct EncryptArgs {
    /// Input PDF (`-` for stdin).
    #[arg(value_name = "INPUT")]
    pub(crate) input: String,
    /// Output PDF (`-` for stdout).
    #[arg(long, value_name = "FILE", required = true)]
    pub(crate) output: String,
    /// User password (required to open).
    #[arg(long, value_name = "PASS")]
    pub(crate) user_password: Option<String>,
    /// Owner password (required to change permissions).
    #[arg(long, value_name = "PASS")]
    pub(crate) owner_password: Option<String>,
    /// Encryption algorithm.
    #[arg(long, value_enum, default_value_t = EncryptAlgorithm::Aes256)]
    pub(crate) algorithm: EncryptAlgorithm,
    /// Permission flags (comma-separated: print,print-hq,modify,annotate,fill-forms,extract,assemble,all,none,view-only).
    #[arg(long, default_value = "all")]
    pub(crate) permissions: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub(crate) enum EncryptAlgorithm {
    Aes128,
    Aes256,
}

#[derive(Debug, Args)]
pub(crate) struct DecryptArgs {
    /// Input PDF (`-` for stdin).
    #[arg(value_name = "INPUT")]
    pub(crate) input: String,
    /// Output PDF (`-` for stdout).
    #[arg(long, value_name = "FILE", required = true)]
    pub(crate) output: String,
    /// Password (user or owner).
    #[arg(long, value_name = "PASS", required = true)]
    pub(crate) password: String,
}

#[derive(Debug, Subcommand)]
pub(crate) enum MetadataAction {
    /// Read metadata to JSON on stdout.
    Read(MetadataReadArgs),
    /// Write metadata fields into a copy of `INPUT`.
    Write(MetadataWriteArgs),
}

#[derive(Debug, Args)]
pub(crate) struct MetadataReadArgs {
    /// Input PDF (`-` for stdin).
    #[arg(value_name = "INPUT")]
    pub(crate) input: String,
}

#[derive(Debug, Args)]
pub(crate) struct MetadataWriteArgs {
    /// Input PDF (`-` for stdin).
    #[arg(value_name = "INPUT")]
    pub(crate) input: String,
    /// Output PDF (`-` for stdout).
    #[arg(long, value_name = "FILE", required = true)]
    pub(crate) output: String,
    /// JSON file with a `Metadata` payload.
    #[arg(long, value_name = "FILE")]
    pub(crate) from_json: Option<PathBuf>,
    /// Repeatable `KEY=VALUE`. Empty value deletes the key.
    #[arg(long = "set", value_name = "KEY=VALUE", value_parser = parse::parse_set_kv)]
    pub(crate) set: Vec<(String, String)>,
}

// ---------------------------------------------------------------------------
// Shared option flags
// ---------------------------------------------------------------------------

/// All `PdfOptions` knobs from spec 10. Optional → unset → engine default.
#[derive(Debug, Args, Default)]
pub(crate) struct PdfFlags {
    #[arg(long, value_name = "SIZE", value_parser = parse::parse_paper)]
    pub(crate) paper: Option<PaperSize>,
    #[arg(long)]
    pub(crate) landscape: bool,
    #[arg(long, value_name = "SPEC", value_parser = parse::parse_margin)]
    pub(crate) margin: Option<Margins>,
    #[arg(long, value_name = "FLOAT")]
    pub(crate) scale: Option<f32>,
    #[arg(long = "print-background")]
    pub(crate) print_background: bool,
    #[arg(long, value_enum, value_name = "MEDIA")]
    pub(crate) emulate: Option<EmulateMedia>,
    #[arg(long, value_name = "RANGES", value_parser = parse::parse_page_ranges)]
    pub(crate) pages: Option<PageRanges>,
    #[arg(long, value_name = "FILE")]
    pub(crate) header_template: Option<PathBuf>,
    #[arg(long, value_name = "FILE")]
    pub(crate) footer_template: Option<PathBuf>,
    #[arg(long)]
    pub(crate) prefer_css_page_size: bool,
    #[arg(long, value_name = "SPEC", value_parser = parse::parse_wait)]
    pub(crate) wait: Option<WaitCondition>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub(crate) enum EmulateMedia {
    Print,
    Screen,
}

/// `RequestContext` knobs from spec 11.
#[derive(Debug, Args, Default)]
pub(crate) struct RequestFlags {
    #[arg(long, value_name = "STR")]
    pub(crate) user_agent: Option<String>,
    /// Repeatable: `--header "Name: Value"`.
    #[arg(long = "header", value_name = "NAME: VALUE", value_parser = parse::parse_header)]
    pub(crate) headers: Vec<(String, String)>,
    /// Repeatable: `--cookie "name=value;Domain=...;Path=...;Secure;HttpOnly"`.
    #[arg(long = "cookie", value_name = "COOKIE", value_parser = parse::parse_cookie)]
    pub(crate) cookies: Vec<Cookie>,
    /// Repeatable: a status code, family (`5xx`), or range (`500-503`).
    #[arg(long = "fail-on-status", value_name = "SPEC", value_parser = parse::parse_fail_on_status)]
    pub(crate) fail_on_status: Vec<Vec<u16>>,
    /// Base URL for `--html` / `--markdown` / `--stdin`. Ignored otherwise.
    #[arg(long, value_name = "URL")]
    pub(crate) base_url: Option<String>,
}

/// `OfficeOptions` knobs from spec 12.
#[derive(Debug, Args, Default)]
pub(crate) struct OfficeFlags {
    #[arg(long, value_enum, value_name = "PROFILE")]
    pub(crate) pdf_a: Option<PdfAFlag>,
    #[arg(long)]
    pub(crate) pdf_ua: bool,
    #[arg(long, value_name = "1..=100")]
    pub(crate) quality: Option<u8>,
    #[arg(long, value_name = "DPI")]
    pub(crate) max_image_resolution: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub(crate) enum PdfAFlag {
    #[value(name = "a1b")]
    A1B,
    #[value(name = "a2b")]
    A2B,
    #[value(name = "a3b")]
    A3B,
}
