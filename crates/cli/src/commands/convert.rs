//! `folio convert` — single conversion (HTML / URL / Markdown / Office /
//! stdin) into a single PDF.

use std::time::Instant;

use anyhow::{Context, anyhow};
use engine::{ChromiumEngine, LibreOfficeEngine, PdfOptions, RequestContext};

use crate::args::{ConvertArgs, GlobalOpts, StdinKind};
use crate::io_helpers::{is_stdout, read_stdin_async, write_output};
use crate::model;

/// Entry point for `folio convert`. Decides which engine to invoke based
/// on which mutually-exclusive `--{html,url,markdown,office,stdin}` flag
/// the user supplied (clap enforces exactly one).
pub(crate) async fn run(global: &GlobalOpts, args: &ConvertArgs) -> anyhow::Result<()> {
    let opts = model::build_pdf_options(&args.pdf)?;
    let request = model::build_request(&args.req);
    let stdout_target = is_stdout(&args.output);

    let started = Instant::now();
    let (source, bytes_in, pdf) = render(global, args, opts, request).await?;

    write_output(&args.output, &pdf).context("writing output")?;

    if !stdout_target {
        log_render_success(source, bytes_in, &pdf, started);
    } else {
        // When writing PDF bytes to stdout, the success log goes to
        // stderr (tracing is configured against stderr globally).
        log_render_success(source, bytes_in, &pdf, started);
    }
    Ok(())
}

async fn render(
    global: &GlobalOpts,
    args: &ConvertArgs,
    opts: PdfOptions,
    request: RequestContext,
) -> anyhow::Result<(&'static str, Option<usize>, Vec<u8>)> {
    if let Some(p) = &args.html {
        let body = std::fs::read_to_string(p)
            .with_context(|| format!("reading --html {}", p.display()))?;
        let bytes_in = body.len();
        let engine = launch_chromium(global).await?;
        let result = engine
            .html_to_pdf(&body, args.req.base_url.as_deref(), &opts, &request)
            .await;
        let _ = engine.shutdown().await;
        let pdf = result?;
        Ok(("html", Some(bytes_in), pdf))
    } else if let Some(url) = &args.url {
        let engine = launch_chromium(global).await?;
        let result = engine.url_to_pdf(url, &opts, &request).await;
        let _ = engine.shutdown().await;
        let pdf = result?;
        Ok(("url", None, pdf))
    } else if let Some(p) = &args.markdown {
        let body = std::fs::read_to_string(p)
            .with_context(|| format!("reading --markdown {}", p.display()))?;
        let bytes_in = body.len();
        let engine = launch_chromium(global).await?;
        let result = engine.markdown_to_pdf(&body, &opts, &request).await;
        let _ = engine.shutdown().await;
        let pdf = result?;
        Ok(("markdown", Some(bytes_in), pdf))
    } else if let Some(p) = &args.office {
        let bytes_in = std::fs::metadata(p)
            .with_context(|| format!("statting --office {}", p.display()))?
            .len() as usize;
        let office_opts = model::build_office_options(&args.pdf, &args.office_opts);
        let engine = LibreOfficeEngine::discover()
            .await
            .context("launching LibreOffice")?;
        let pdf = engine.convert(p, &office_opts).await?;
        Ok(("office", Some(bytes_in), pdf))
    } else if args.stdin {
        let body = read_stdin_async().await?;
        let bytes_in = body.len();
        let engine = launch_chromium(global).await?;
        let pdf_result = match args.stdin_kind {
            StdinKind::Html => {
                let html = String::from_utf8(body)
                    .map_err(|e| anyhow!("--stdin --as html requires UTF-8 input: {e}"))?;
                engine
                    .html_to_pdf(&html, args.req.base_url.as_deref(), &opts, &request)
                    .await
            }
            StdinKind::Markdown => {
                let md = String::from_utf8(body)
                    .map_err(|e| anyhow!("--stdin --as markdown requires UTF-8 input: {e}"))?;
                engine.markdown_to_pdf(&md, &opts, &request).await
            }
        };
        let _ = engine.shutdown().await;
        let pdf = pdf_result?;
        Ok(("stdin", Some(bytes_in), pdf))
    } else {
        // clap's required ArgGroup makes this unreachable; defensive.
        Err(anyhow!(
            "no input source provided (one of --html, --url, --markdown, --office, --stdin)"
        ))
    }
}

async fn launch_chromium(global: &GlobalOpts) -> anyhow::Result<ChromiumEngine> {
    let cfg = model::build_browser_config(global);
    ChromiumEngine::launch_with(cfg)
        .await
        .context("launching Chromium")
}

fn log_render_success(source: &str, bytes_in: Option<usize>, pdf: &[u8], started: Instant) {
    let bytes_out = pdf.len();
    let duration_ms = started.elapsed().as_millis() as u64;
    let pages = lopdf::Document::load_mem(pdf)
        .ok()
        .map(|d| d.get_pages().len() as u32);
    match (bytes_in, pages) {
        (Some(b), Some(p)) => tracing::info!(
            source,
            bytes_in = b,
            bytes_out,
            duration_ms,
            pages = p,
            "render"
        ),
        (Some(b), None) => tracing::info!(source, bytes_in = b, bytes_out, duration_ms, "render"),
        (None, Some(p)) => tracing::info!(source, bytes_out, duration_ms, pages = p, "render"),
        (None, None) => tracing::info!(source, bytes_out, duration_ms, "render"),
    }
}
