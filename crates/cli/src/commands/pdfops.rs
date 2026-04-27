//! `folio merge` / `split` / `flatten` — pure pdfops over byte buffers.

use anyhow::{Context, anyhow};
use engine::PageRanges;
use engine::pdfops::{self, SplitMode};

use crate::args::{FlattenArgs, MergeArgs, SplitArgs};
use crate::exit::UsageError;
use crate::io_helpers::{is_stdin, read_input_sync, write_output};

/// `folio merge --output FILE INPUT...`
pub(crate) fn run_merge(args: &MergeArgs) -> anyhow::Result<()> {
    let stdin_count = args.inputs.iter().filter(|s| is_stdin(s)).count();
    if stdin_count > 1 {
        return Err(anyhow!("stdin can only be used once in `merge` inputs").context(UsageError));
    }

    let mut buffers: Vec<Vec<u8>> = Vec::with_capacity(args.inputs.len());
    for spec in &args.inputs {
        buffers.push(read_input_sync(spec)?);
    }
    let refs: Vec<&[u8]> = buffers.iter().map(Vec::as_slice).collect();

    let pdf = pdfops::merge(&refs).context("merging PDFs")?;
    write_output(&args.output, &pdf)
}

/// `folio split INPUT --output-dir DIR --mode SPEC --prefix STR`
pub(crate) fn run_split(args: &SplitArgs) -> anyhow::Result<()> {
    let pdf_bytes =
        std::fs::read(&args.input).with_context(|| format!("reading {}", args.input.display()))?;

    let mode = parse_split_mode(&args.mode)?;
    let chunks = pdfops::split(&pdf_bytes, &mode).context("splitting PDF")?;

    std::fs::create_dir_all(&args.output_dir)
        .with_context(|| format!("creating {}", args.output_dir.display()))?;

    let prefix = args
        .prefix
        .clone()
        .or_else(|| {
            args.input
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
        })
        .unwrap_or_else(|| "split".to_string());

    let width = chunks.len().to_string().len().max(3);

    for (i, bytes) in chunks.iter().enumerate() {
        let name = format!("{prefix}-{:0width$}.pdf", i + 1, width = width);
        let dst = args.output_dir.join(&name);
        std::fs::write(&dst, bytes).with_context(|| format!("writing {}", dst.display()))?;
    }

    tracing::info!(
        chunks = chunks.len(),
        out_dir = %args.output_dir.display(),
        "split"
    );
    Ok(())
}

/// `folio flatten INPUT --output FILE`
pub(crate) fn run_flatten(args: &FlattenArgs) -> anyhow::Result<()> {
    let pdf_bytes = read_input_sync(&args.input)?;
    let out = pdfops::flatten(&pdf_bytes).context("flattening PDF")?;
    write_output(&args.output, &out)
}

// ---------------------------------------------------------------------------
// --mode parsing
// ---------------------------------------------------------------------------

fn parse_split_mode(s: &str) -> anyhow::Result<SplitMode> {
    let trimmed = s.trim();
    if trimmed == "one-per-page" {
        return Ok(SplitMode::OnePagePerFile);
    }
    if let Some(rest) = trimmed.strip_prefix("ranges:") {
        let r = PageRanges::parse(rest)
            .map_err(|e| anyhow!("invalid --mode ranges spec '{rest}': {e}").context(UsageError))?;
        return Ok(SplitMode::ByRanges(vec![r]));
    }
    if let Some(rest) = trimmed.strip_prefix("every-n:") {
        let n: u32 = rest
            .parse()
            .map_err(|_| anyhow!("invalid --mode every-n value '{rest}'").context(UsageError))?;
        if n == 0 {
            return Err(anyhow!("--mode every-n requires N >= 1").context(UsageError));
        }
        return Ok(SplitMode::EveryN(n));
    }
    Err(
        anyhow!("invalid --mode '{s}': expected 'one-per-page', 'ranges:RANGES', or 'every-n:N'")
            .context(UsageError),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_split_mode_one_per_page() {
        assert!(matches!(
            parse_split_mode("one-per-page").unwrap(),
            SplitMode::OnePagePerFile
        ));
    }

    #[test]
    fn parse_split_mode_every_n() {
        assert!(matches!(
            parse_split_mode("every-n:5").unwrap(),
            SplitMode::EveryN(5)
        ));
        assert!(parse_split_mode("every-n:0").is_err());
        assert!(parse_split_mode("every-n:nope").is_err());
    }

    #[test]
    fn parse_split_mode_ranges() {
        match parse_split_mode("ranges:1-3,5,7-").unwrap() {
            SplitMode::ByRanges(v) => assert_eq!(v.len(), 1),
            other => panic!("expected ByRanges, got {other:?}"),
        }
        assert!(parse_split_mode("ranges:bogus").is_err());
    }

    #[test]
    fn parse_split_mode_unknown() {
        assert!(parse_split_mode("nope").is_err());
    }
}
