//! `folio batch` — recursive directory walk with bounded concurrency.
//!
//! Files are matched against `--pattern` (a glob with brace expansion).
//! Each matching path is scheduled on a tokio task gated by a
//! `Semaphore`; a single `ChromiumEngine` (and, if needed,
//! `LibreOfficeEngine`) is reused across the run.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use anyhow::{Context, anyhow};
use engine::{ChromiumEngine, LibreOfficeEngine, OfficeOptions, PdfOptions, RequestContext};
use glob::Pattern;
use tokio::sync::Semaphore;
use walkdir::WalkDir;

use crate::args::{BatchArgs, GlobalOpts, OnError};
use crate::exit::{BatchPartialFailure, UsageError};
use crate::model;

/// Entry point for `folio batch`.
pub(crate) async fn run(global: &GlobalOpts, args: &BatchArgs) -> anyhow::Result<()> {
    if !args.input_dir.exists() {
        return Err(
            anyhow!("--input-dir does not exist: {}", args.input_dir.display()).context(UsageError),
        );
    }
    let input_dir = args
        .input_dir
        .canonicalize()
        .with_context(|| format!("resolving --input-dir {}", args.input_dir.display()))?;
    std::fs::create_dir_all(&args.output_dir)
        .with_context(|| format!("creating --output-dir {}", args.output_dir.display()))?;
    let output_dir = args
        .output_dir
        .canonicalize()
        .with_context(|| format!("resolving --output-dir {}", args.output_dir.display()))?;
    if output_dir == input_dir {
        return Err(anyhow!("--input-dir and --output-dir must differ").context(UsageError));
    }

    let patterns = expand_glob(&args.pattern)?;
    let mut entries: Vec<PathBuf> = Vec::new();
    for entry in WalkDir::new(&input_dir).follow_links(false) {
        let entry = entry.context("walking input dir")?;
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.path();
        let rel = path
            .strip_prefix(&input_dir)
            .unwrap_or(path)
            .to_string_lossy()
            .into_owned();
        if patterns.iter().any(|p| p.matches(&rel)) {
            entries.push(path.to_path_buf());
        }
    }
    entries.sort();

    if args.dry_run {
        for src in &entries {
            let dst = mirror_target(&input_dir, &args.output_dir, src);
            println!("{} -> {}", src.display(), dst.display());
        }
        tracing::info!(planned = entries.len(), "batch dry-run");
        return Ok(());
    }

    let concurrency = args.concurrency.unwrap_or_else(num_cpus_safe).max(1);
    let semaphore = Arc::new(Semaphore::new(concurrency));

    let needs_chrome = entries.iter().any(|p| !is_office_path(p));
    let needs_office = entries.iter().any(|p| is_office_path(p));

    let chrome = if needs_chrome {
        let cfg = model::build_browser_config(global);
        Some(
            ChromiumEngine::launch_with(cfg)
                .await
                .context("launching Chromium")?,
        )
    } else {
        None
    };
    let office = if needs_office {
        Some(
            LibreOfficeEngine::discover()
                .await
                .context("launching LibreOffice")?,
        )
    } else {
        None
    };

    let pdf_opts = Arc::new(model::build_pdf_options(&args.pdf)?);
    let request = Arc::new(model::build_request(&args.req));
    let office_opts = Arc::new(model::build_office_options(&args.pdf, &args.office_opts));
    let base_url = Arc::new(args.req.base_url.clone());

    let mut set = tokio::task::JoinSet::new();
    for src in &entries {
        let src = src.clone();
        let dst = mirror_target(&input_dir, &args.output_dir, &src);
        let permit = semaphore.clone();
        let chrome = chrome.clone();
        let office = office.clone();
        let pdf_opts = pdf_opts.clone();
        let request = request.clone();
        let office_opts = office_opts.clone();
        let base_url = base_url.clone();

        set.spawn(async move {
            let _permit = permit
                .acquire_owned()
                .await
                .map_err(|e| anyhow!("semaphore closed: {e}"))?;
            convert_one(
                &src,
                &dst,
                chrome.as_ref(),
                office.as_ref(),
                &pdf_opts,
                &request,
                &office_opts,
                base_url.as_deref(),
            )
            .await
        });
    }

    let mut succeeded = 0usize;
    let mut failed: Vec<(PathBuf, anyhow::Error)> = Vec::new();
    while let Some(joined) = set.join_next().await {
        match joined {
            Ok(Ok(path)) => {
                succeeded += 1;
                tracing::debug!(path = %path.display(), "batch ok");
            }
            Ok(Err(e)) => {
                let path = batch_path_of(&e).unwrap_or_default();
                tracing::error!(path = %path.display(), error = %format_chain(&e), "batch error");
                failed.push((path, e));
                if matches!(args.on_error, OnError::Stop) {
                    set.abort_all();
                    break;
                }
            }
            Err(e) => {
                tracing::error!(error = %e, "batch join error");
                failed.push((PathBuf::new(), anyhow!(e)));
                if matches!(args.on_error, OnError::Stop) {
                    set.abort_all();
                    break;
                }
            }
        }
    }

    if let Some(c) = chrome {
        let _ = c.shutdown().await;
    }

    tracing::info!(
        total = entries.len(),
        succeeded,
        failed = failed.len(),
        "batch summary"
    );

    if failed.is_empty() {
        Ok(())
    } else if matches!(args.on_error, OnError::Skip) {
        eprintln!("batch: {} of {} files failed", failed.len(), entries.len());
        Err(anyhow!("batch had failures").context(BatchPartialFailure {
            count: failed.len(),
        }))
    } else {
        // on-error = stop: surface the first failure so the engine
        // exit-code mapping kicks in.
        let mut iter = failed.into_iter();
        let Some((path, err)) = iter.next() else {
            return Ok(());
        };
        let display = if path.as_os_str().is_empty() {
            "<unknown>".to_string()
        } else {
            path.display().to_string()
        };
        Err(err.context(format!("batch: {display} failed")))
    }
}

/// Output path obtained by mirroring `src`'s relative location under
/// `out_root`, with the file extension switched to `.pdf`.
fn mirror_target(in_root: &Path, out_root: &Path, src: &Path) -> PathBuf {
    let rel = src.strip_prefix(in_root).unwrap_or(src);
    let mut dst = out_root.join(rel);
    dst.set_extension("pdf");
    dst
}

fn is_office_path(p: &Path) -> bool {
    matches!(
        p.extension()
            .and_then(|s| s.to_str())
            .map(str::to_ascii_lowercase)
            .as_deref(),
        Some("doc")
            | Some("docx")
            | Some("odt")
            | Some("rtf")
            | Some("xls")
            | Some("xlsx")
            | Some("ods")
            | Some("ppt")
            | Some("pptx")
            | Some("odp")
    )
}

/// Convert one file. The source is dispatched to Chromium or LibreOffice
/// based on its extension.
#[allow(clippy::too_many_arguments)]
async fn convert_one(
    src: &Path,
    dst: &Path,
    chrome: Option<&ChromiumEngine>,
    office: Option<&LibreOfficeEngine>,
    pdf_opts: &PdfOptions,
    request: &RequestContext,
    office_opts: &OfficeOptions,
    base_url: Option<&str>,
) -> anyhow::Result<PathBuf> {
    let started = Instant::now();
    let ext = src
        .extension()
        .and_then(|s| s.to_str())
        .map(str::to_ascii_lowercase)
        .unwrap_or_default();

    let (source, bytes_in, pdf) = match ext.as_str() {
        "html" | "htm" => {
            let body = std::fs::read_to_string(src)
                .with_context(|| format!("reading {}", src.display()))?;
            let bytes_in = body.len();
            let chrome = chrome.ok_or_else(|| anyhow!("Chromium engine not available"))?;
            let pdf = chrome
                .html_to_pdf(&body, base_url, pdf_opts, request)
                .await
                .map_err(|e| with_path(e, src))?;
            ("html", bytes_in, pdf)
        }
        "md" | "markdown" => {
            let body = std::fs::read_to_string(src)
                .with_context(|| format!("reading {}", src.display()))?;
            let bytes_in = body.len();
            let chrome = chrome.ok_or_else(|| anyhow!("Chromium engine not available"))?;
            let pdf = chrome
                .markdown_to_pdf(&body, pdf_opts, request)
                .await
                .map_err(|e| with_path(e, src))?;
            ("markdown", bytes_in, pdf)
        }
        _ if is_office_path(src) => {
            let bytes_in = std::fs::metadata(src)
                .with_context(|| format!("statting {}", src.display()))?
                .len() as usize;
            let office = office.ok_or_else(|| anyhow!("LibreOffice engine not available"))?;
            let pdf = office
                .convert(src, office_opts)
                .await
                .map_err(|e| with_path(e, src))?;
            ("office", bytes_in, pdf)
        }
        other => {
            return Err(anyhow!(
                "unsupported extension '{}' for {}",
                other,
                src.display()
            ));
        }
    };

    if let Some(parent) = dst.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("creating {}", parent.display()))?;
    }
    std::fs::write(dst, &pdf).with_context(|| format!("writing {}", dst.display()))?;

    let duration_ms = started.elapsed().as_millis() as u64;
    let pages = lopdf::Document::load_mem(&pdf)
        .ok()
        .map(|d| d.get_pages().len() as u32);
    tracing::info!(
        source,
        bytes_in,
        bytes_out = pdf.len(),
        duration_ms,
        ?pages,
        path = %src.display(),
        "render"
    );
    Ok(src.to_path_buf())
}

/// Annotate an `EngineError` (or any error) with the file path that
/// triggered it so failures in `batch` can be correlated.
fn with_path(err: impl std::error::Error + Send + Sync + 'static, src: &Path) -> anyhow::Error {
    anyhow::Error::new(err).context(BatchPath(src.to_path_buf()))
}

/// Sentinel context type carrying the offending input path.
#[derive(Debug)]
struct BatchPath(PathBuf);

impl std::fmt::Display for BatchPath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "while processing {}", self.0.display())
    }
}

impl std::error::Error for BatchPath {}

fn batch_path_of(err: &anyhow::Error) -> Option<PathBuf> {
    err.chain()
        .find_map(|e| e.downcast_ref::<BatchPath>().map(|b| b.0.clone()))
}

fn format_chain(err: &anyhow::Error) -> String {
    format!("{err:#}")
}

// ---------------------------------------------------------------------------
// Glob handling (with brace expansion).
// ---------------------------------------------------------------------------

/// Expand `{a,b,c}` brace alternations in `pattern`, then compile each
/// expansion into a [`glob::Pattern`].
pub(crate) fn expand_glob(pattern: &str) -> anyhow::Result<Vec<Pattern>> {
    let expansions = brace_expand(pattern);
    expansions
        .into_iter()
        .map(|s| Pattern::new(&s).map_err(|e| anyhow!("invalid --pattern '{s}': {e}")))
        .collect()
}

/// Expand top-level `{a,b,c}` groups (nested groups not supported) into
/// the cartesian product of alternates.
fn brace_expand(pattern: &str) -> Vec<String> {
    let mut out = vec![String::new()];
    let bytes = pattern.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'{' {
            let Some(close) = find_matching_brace(bytes, i) else {
                // unmatched '{': emit literally.
                for s in &mut out {
                    s.push('{');
                }
                i += 1;
                continue;
            };
            let inner = &pattern[i + 1..close];
            let alternates: Vec<&str> = inner.split(',').collect();
            let mut next = Vec::with_capacity(out.len() * alternates.len());
            for prefix in &out {
                for alt in &alternates {
                    let mut s = prefix.clone();
                    s.push_str(alt);
                    next.push(s);
                }
            }
            out = next;
            i = close + 1;
        } else {
            let c = bytes[i] as char;
            for s in &mut out {
                s.push(c);
            }
            i += 1;
        }
    }
    out
}

fn find_matching_brace(bytes: &[u8], open: usize) -> Option<usize> {
    let mut depth = 0;
    for (j, &b) in bytes.iter().enumerate().skip(open) {
        match b {
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(j);
                }
            }
            _ => {}
        }
    }
    None
}

fn num_cpus_safe() -> usize {
    std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn brace_expand_handles_default_pattern() {
        let exp = brace_expand("**/*.{html,htm,md,markdown}");
        assert_eq!(
            exp,
            vec![
                "**/*.html".to_string(),
                "**/*.htm".to_string(),
                "**/*.md".to_string(),
                "**/*.markdown".to_string(),
            ]
        );
    }

    #[test]
    fn brace_expand_no_braces_pass_through() {
        assert_eq!(brace_expand("**/*.docx"), vec!["**/*.docx".to_string()]);
    }

    #[test]
    fn brace_expand_handles_single_alternate() {
        assert_eq!(brace_expand("{x}"), vec!["x".to_string()]);
    }

    #[test]
    fn brace_expand_unmatched_brace_emits_literal() {
        assert_eq!(brace_expand("foo{bar"), vec!["foo{bar".to_string()]);
    }

    #[test]
    fn mirror_target_switches_extension() {
        let dst = mirror_target(
            Path::new("/in"),
            Path::new("/out"),
            Path::new("/in/docs/page.html"),
        );
        assert_eq!(dst, PathBuf::from("/out/docs/page.pdf"));
    }
}
