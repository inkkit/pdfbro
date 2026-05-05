//! `/forms/chromium/convert/{html,url,markdown}` route handlers.
//!
//! All three handlers share the same `PdfOptions` + `RequestContext`
//! parsing path; the only differences are which input the engine call
//! receives (HTML string / URL / Markdown).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Instant;

use axum::extract::{Multipart, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::Response;
use engine::{Cookie, MediaType, PageRanges, PdfAProfile, PdfOptions, RequestContext, WaitCondition, CaptureMode, ScreenshotFormat, ScreenshotOptions};

use crate::error::{ApiError, ApiResult};
use crate::multipart::FormFields;
use crate::routes::util::{apply_encryption, output_filename, pdf_response};
use crate::state::AppState;

const INDEX_HTML: &str = "index.html";

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

/// `POST /forms/chromium/convert/html`.
pub async fn chromium_html(
    State(state): State<AppState>,
    headers: HeaderMap,
    mp: Multipart,
) -> ApiResult<Response> {
    let _permit = acquire_permit(&state).await?;
    let mut form = FormFields::from_multipart(mp).await?;
    crate::download::inject_downloads(&mut form, &state.config).await?;
    let index = form
        .find_named("files", INDEX_HTML)
        .ok_or_else(|| ApiError::MissingFile(INDEX_HTML.to_string()))?;
    let html = tokio::fs::read_to_string(&index.path)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    let mut opts = parse_pdf_options(&form.map)?;
    apply_header_footer_files(&mut opts, &form).await?;
    opts.validate()?;
    let ctx = parse_request_context(&form.map)?;
    let native_page_ranges = parse_native_page_ranges(&form.map)?;
    validate_chromium_extra_fields(&form.map)?;

    if let Some(resp) = crate::webhook::maybe_spawn_webhook(
        &headers,
        &state,
        crate::webhook::WebhookOperation::ChromiumConvertHtml,
        crate::webhook::JobData::ChromiumHtml {
            html: html.clone().into_bytes(),
            options: opts.clone(),
            ctx: ctx.clone(),
        },
    ).await? {
        return Ok(resp);
    }

    let base_url = file_url_for(&index.path);
    let start = Instant::now();
    let result = state
        .chromium
        .as_ref()
        .unwrap()
        .html_to_pdf(&html, Some(&base_url), &opts, &ctx)
        .await;
    let duration = start.elapsed().as_secs_f64();

    match &result {
        Ok(pdf) => {
            state.metrics.record_conversion(
                "chromium",
                "/forms/chromium/convert/html",
                true,
                duration,
                pdf.len() as u64,
            );
            state.metrics.record_engine_conversion(
                "chromium",
                "/forms/chromium/convert/html",
            );
        }
        Err(_) => {
            state.metrics.record_conversion(
                "chromium",
                "/forms/chromium/convert/html",
                false,
                duration,
                0,
            );
        }
    }

    let pdf = result?;
    validate_native_page_ranges_against_pdf(&pdf, &native_page_ranges)?;
    let pdf = apply_metadata_field(pdf, &form.map).await?;
    let pdf = apply_encryption(pdf, &form.map).await?;
    Ok(pdf_response(pdf, &output_filename(&headers, "result")))
}

/// `POST /forms/chromium/convert/url`.
pub async fn chromium_url(
    State(state): State<AppState>,
    headers: HeaderMap,
    mp: Multipart,
) -> ApiResult<Response> {
    let _permit = acquire_permit(&state).await?;
    let mut form = FormFields::from_multipart(mp).await?;
    crate::download::inject_downloads(&mut form, &state.config).await?;
    let url = form
        .map
        .get("url")
        .ok_or(ApiError::MissingField("url"))?
        .clone();
    if url.trim().is_empty() {
        return Err(ApiError::MissingField("url"));
    }
    // Reject syntactically invalid URLs before handing to the browser,
    // so that malformed input yields 400 rather than 502 (navigation failure).
    if url::Url::parse(&url).is_err() {
        return Err(ApiError::InvalidField {
            field: "url",
            message: format!("`{url}` is not a valid URL"),
        });
    }
    let opts = parse_pdf_options(&form.map)?;
    opts.validate()?;
    let ctx = parse_request_context(&form.map)?;

    if let Some(resp) = crate::webhook::maybe_spawn_webhook(
        &headers,
        &state,
        crate::webhook::WebhookOperation::ChromiumConvertUrl,
        crate::webhook::JobData::ChromiumUrl {
            url: url.clone(),
            options: opts.clone(),
            ctx: ctx.clone(),
        },
    ).await? {
        return Ok(resp);
    }

    let start = Instant::now();
    let result = state.chromium.as_ref().unwrap().url_to_pdf(&url, &opts, &ctx).await;
    let duration = start.elapsed().as_secs_f64();

    match &result {
        Ok(pdf) => {
            state.metrics.record_conversion(
                "chromium",
                "/forms/chromium/convert/url",
                true,
                duration,
                pdf.len() as u64,
            );
            state.metrics.record_engine_conversion(
                "chromium",
                "/forms/chromium/convert/url",
            );
        }
        Err(_) => {
            state.metrics.record_conversion(
                "chromium",
                "/forms/chromium/convert/url",
                false,
                duration,
                0,
            );
        }
    }

    let pdf = result?;
    Ok(pdf_response(pdf, &output_filename(&headers, "result")))
}

/// `POST /forms/chromium/convert/markdown`.
///
/// Two input shapes are supported:
/// 1. Wrapper-template form: an `index.html` containing one or more
///    `<link rel="markdown" href="X.md">` tags. Referenced markdown files
///    are rendered to HTML and inlined where the link tag appeared.
/// 2. Simple form: at least one `.md` file is uploaded; the first is
///    rendered directly via `markdown_to_pdf`.
///
/// The wrapper takes precedence when both shapes are present.
pub async fn chromium_markdown(
    State(state): State<AppState>,
    headers: HeaderMap,
    mp: Multipart,
) -> ApiResult<Response> {
    let _permit = acquire_permit(&state).await?;
    let mut form = FormFields::from_multipart(mp).await?;
    crate::download::inject_downloads(&mut form, &state.config).await?;
    let mut opts = parse_pdf_options(&form.map)?;
    apply_header_footer_files(&mut opts, &form).await?;
    opts.validate()?;
    let ctx = parse_request_context(&form.map)?;

    if let Some(index) = form.find_named("files", INDEX_HTML) {
        let wrapper = tokio::fs::read_to_string(&index.path)
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))?;
        let inlined = inline_markdown_links(&wrapper, &form).await?;

        if let Some(resp) = crate::webhook::maybe_spawn_webhook(
            &headers,
            &state,
            crate::webhook::WebhookOperation::ChromiumConvertMarkdown,
            crate::webhook::JobData::ChromiumMarkdown {
                html: inlined.clone().into_bytes(),
                options: opts.clone(),
                ctx: ctx.clone(),
            },
        ).await? {
            return Ok(resp);
        }

        let base_url = file_url_for(&index.path);
        let start = Instant::now();
        let result = state
            .chromium
            .as_ref()
            .unwrap()
            .html_to_pdf(&inlined, Some(&base_url), &opts, &ctx)
            .await;
        let duration = start.elapsed().as_secs_f64();

        match &result {
            Ok(pdf) => {
                state.metrics.record_conversion(
                    "chromium",
                    "/forms/chromium/convert/markdown",
                    true,
                    duration,
                    pdf.len() as u64,
                );
                state.metrics.record_engine_conversion(
                    "chromium",
                    "/forms/chromium/convert/markdown",
                );
            }
            Err(_) => {
                state.metrics.record_conversion(
                    "chromium",
                    "/forms/chromium/convert/markdown",
                    false,
                    duration,
                    0,
                );
            }
        }

        let pdf = result?;
        let pdf = apply_metadata_field(pdf, &form.map).await?;
        let pdf = apply_encryption(pdf, &form.map).await?;
        return Ok(pdf_response(pdf, &output_filename(&headers, "result")));
    }

    // Simple form: render the first .md file directly.
    let md_file = form
        .files
        .iter()
        .find(|f| {
            f.field_name == "files"
                && Path::new(&f.filename)
                    .extension()
                    .map(|e| e.eq_ignore_ascii_case("md"))
                    .unwrap_or(false)
        })
        .ok_or_else(|| ApiError::MissingFile("*.md".to_string()))?;

    let md = tokio::fs::read_to_string(&md_file.path)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    let html = render_markdown_to_html(&md);
    if let Some(resp) = crate::webhook::maybe_spawn_webhook(
        &headers,
        &state,
        crate::webhook::WebhookOperation::ChromiumConvertMarkdown,
        crate::webhook::JobData::ChromiumMarkdown {
            html: html.into_bytes(),
            options: opts.clone(),
            ctx: ctx.clone(),
        },
    ).await? {
        return Ok(resp);
    }

    let start = Instant::now();
    let result = state.chromium.as_ref().unwrap().markdown_to_pdf(&md, &opts, &ctx).await;
    let duration = start.elapsed().as_secs_f64();

    match &result {
        Ok(pdf) => {
            state.metrics.record_conversion(
                "chromium",
                "/forms/chromium/convert/markdown",
                true,
                duration,
                pdf.len() as u64,
            );
            state.metrics.record_engine_conversion(
                "chromium",
                "/forms/chromium/convert/markdown",
            );
        }
        Err(_) => {
            state.metrics.record_conversion(
                "chromium",
                "/forms/chromium/convert/markdown",
                false,
                duration,
                0,
            );
        }
    }

    let pdf = result?;
    let pdf = apply_metadata_field(pdf, &form.map).await?;
    let pdf = apply_encryption(pdf, &form.map).await?;
    Ok(pdf_response(pdf, &output_filename(&headers, "result")))
}

// ---------------------------------------------------------------------------
// Post-processing helpers (header/footer files, metadata)
// ---------------------------------------------------------------------------

/// If `header.html` or `footer.html` are present as uploaded files and no
/// template was already set via a form field, read their contents and apply
/// them as the header/footer template strings.
async fn apply_header_footer_files(
    opts: &mut PdfOptions,
    form: &FormFields,
) -> ApiResult<()> {
    if opts.header_template.is_none() {
        if let Some(f) = form.find_named("files", "header.html") {
            let content = tokio::fs::read_to_string(&f.path)
                .await
                .map_err(|e| ApiError::Internal(e.to_string()))?;
            opts.header_template = Some(content);
        }
    }
    if opts.footer_template.is_none() {
        if let Some(f) = form.find_named("files", "footer.html") {
            let content = tokio::fs::read_to_string(&f.path)
                .await
                .map_err(|e| ApiError::Internal(e.to_string()))?;
            opts.footer_template = Some(content);
        }
    }
    Ok(())
}

/// Apply the `metadata` form field (if present) to an already-generated PDF.
async fn apply_metadata_field(
    pdf: Vec<u8>,
    map: &HashMap<String, String>,
) -> ApiResult<Vec<u8>> {
    let Some(meta_str) = map.get("metadata").filter(|s| !s.is_empty()) else {
        return Ok(pdf);
    };
    let meta: engine::Metadata =
        serde_json::from_str(meta_str).map_err(|e| ApiError::InvalidField {
            field: "metadata",
            message: e.to_string(),
        })?;
    tokio::task::spawn_blocking(move || engine::write_metadata(&pdf, &meta))
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .map_err(ApiError::Engine)
}

// ---------------------------------------------------------------------------
// Wrapper-template inlining
// ---------------------------------------------------------------------------

async fn inline_markdown_links(wrapper: &str, form: &FormFields) -> ApiResult<String> {
    // Handles two markdown-embedding patterns:
    // 1. <link rel="markdown" href="file.md">  (HTML link tag)
    // 2. {{ toHTML "file.md" }}                 (Gotenberg Go-template syntax)
    // Both are replaced in-place with the rendered HTML.
    let mut out = String::with_capacity(wrapper.len());
    let mut cursor = 0usize;
    while cursor < wrapper.len() {
        let rest = &wrapper[cursor..];
        // Find which pattern appears first.
        let link_pos = find_markdown_link(rest).map(|l| (l.start, l.end, l.href.clone()));
        let tohtml_pos = find_tohtml_call(rest).map(|t| (t.start, t.end, t.filename.clone()));
        let chosen = match (link_pos, tohtml_pos) {
            (Some(a), Some(b)) => Some(if a.0 <= b.0 { a } else { b }),
            (Some(a), None) => Some(a),
            (None, Some(b)) => Some(b),
            (None, None) => None,
        };
        match chosen {
            Some((start, end, filename)) => {
                out.push_str(&rest[..start]);
                let f = form
                    .find_named("files", &filename)
                    .ok_or_else(|| ApiError::MissingFile(filename.clone()))?;
                let md = tokio::fs::read_to_string(&f.path)
                    .await
                    .map_err(|e| ApiError::Internal(e.to_string()))?;
                let html = render_markdown(&md);
                out.push_str(&html);
                cursor += end;
            }
            None => {
                out.push_str(rest);
                break;
            }
        }
    }
    Ok(out)
}

fn render_markdown(md: &str) -> String {
    use pulldown_cmark::{Options, Parser, html};
    let mut html_out = String::new();
    let parser = Parser::new_ext(md, Options::all());
    html::push_html(&mut html_out, parser);
    html_out
}

#[derive(Debug)]
struct MarkdownLink {
    start: usize,
    end: usize,
    href: String,
}

/// Best-effort scanner for `<link rel="markdown" href="...">` tags.
fn find_markdown_link(haystack: &str) -> Option<MarkdownLink> {
    let bytes = haystack.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
        if bytes[i] != b'<' {
            i += 1;
            continue;
        }
        // Found a tag-open; scan to matching '>'.
        let close = match haystack[i..].find('>') {
            Some(p) => i + p,
            None => return None,
        };
        let tag = &haystack[i..close + 1];
        if is_markdown_link_tag(tag)
            && let Some(href) = extract_href(tag)
        {
            return Some(MarkdownLink {
                start: i,
                end: close + 1,
                href,
            });
        }
        i = close + 1;
    }
    None
}

fn is_markdown_link_tag(tag: &str) -> bool {
    // tag begins with `<link` (case-insensitive) and contains rel="markdown".
    let lower = tag.to_ascii_lowercase();
    if !lower.starts_with("<link") {
        return false;
    }
    // crude attr extraction: look for rel="markdown" or rel='markdown'.
    lower.contains("rel=\"markdown\"") || lower.contains("rel='markdown'")
}

fn extract_href(tag: &str) -> Option<String> {
    let lower = tag.to_ascii_lowercase();
    let key = "href=";
    let start = lower.find(key)? + key.len();
    let rest = &tag[start..];
    let quote = rest.chars().next()?;
    if quote != '"' && quote != '\'' {
        return None;
    }
    let end_rel = rest[1..].find(quote)?;
    Some(rest[1..1 + end_rel].to_string())
}

struct ToHtmlCall {
    start: usize,
    end: usize,
    filename: String,
}

/// Find the first `{{ toHTML "filename" }}` or `{{ toHTML 'filename' }}` call.
fn find_tohtml_call(haystack: &str) -> Option<ToHtmlCall> {
    let mut pos = 0;
    while pos < haystack.len() {
        let open = haystack[pos..].find("{{")? + pos;
        let close = haystack[open..].find("}}")? + open + 2;
        let inner = haystack[open + 2..close - 2].trim();
        // Match: toHTML "<filename>" or toHTML '<filename>'
        if let Some(rest) = inner.strip_prefix("toHTML") {
            let rest = rest.trim_start();
            if let Some(filename) = extract_quoted(rest) {
                return Some(ToHtmlCall { start: open, end: close, filename });
            }
        }
        pos = close;
    }
    None
}

fn extract_quoted(s: &str) -> Option<String> {
    let s = s.trim();
    let quote = s.chars().next()?;
    if quote != '"' && quote != '\'' {
        return None;
    }
    let inner = &s[1..];
    let end = inner.find(quote)?;
    Some(inner[..end].to_string())
}

// ---------------------------------------------------------------------------
// Form-map → engine option parsing
// ---------------------------------------------------------------------------

/// Build a [`PdfOptions`] from the captured form map. Defaults are
/// inherited from [`PdfOptions::default`]; any field not present in the
/// map is left untouched.
pub fn parse_pdf_options(map: &HashMap<String, String>) -> ApiResult<PdfOptions> {
    let mut opts = PdfOptions::default();

    if let Some(v) = opt_f32(map, "paperWidth")? {
        opts.paper.width_in = v;
    }
    if let Some(v) = opt_f32(map, "paperHeight")? {
        opts.paper.height_in = v;
    }
    if let Some(v) = opt_f32(map, "marginTop")? {
        opts.margin.top = v;
    }
    if let Some(v) = opt_f32(map, "marginRight")? {
        opts.margin.right = v;
    }
    if let Some(v) = opt_f32(map, "marginBottom")? {
        opts.margin.bottom = v;
    }
    if let Some(v) = opt_f32(map, "marginLeft")? {
        opts.margin.left = v;
    }
    if let Some(v) = opt_bool(map, "landscape")? {
        opts.landscape = v;
    }
    if let Some(v) = opt_f32(map, "scale")? {
        opts.scale = v;
    }
    if let Some(v) = opt_bool(map, "printBackground")? {
        opts.print_background = v;
    }

    if let Some(v) = opt_bool(map, "omitBackground")? {
        opts.omit_background = v;
    }

    // Gotenberg parity: omitBackground requires printBackground=true.
    if opts.omit_background && !opts.print_background {
        return Err(ApiError::InvalidField {
            field: "omitBackground",
            message: "omitBackground=true requires printBackground=true".to_string(),
        });
    }

    if let Some(v) = opt_bool(map, "preferCssPageSize")? {
        opts.prefer_css_page_size = v;
    }
    if let Some(s) = map.get("pageRanges").map(String::as_str) {
        let trimmed = s.trim();
        if !trimmed.is_empty() {
            opts.page_ranges = Some(PageRanges::parse(trimmed)?);
        }
    }
    if let Some(s) = map.get("headerTemplate")
        && !s.is_empty()
    {
        opts.header_template = Some(s.clone());
    }
    if let Some(s) = map.get("footerTemplate")
        && !s.is_empty()
    {
        opts.footer_template = Some(s.clone());
    }
    if let Some(s) = map.get("emulatedMediaType").map(String::as_str) {
        opts.emulate_media = match s.trim().to_ascii_lowercase().as_str() {
            "print" => Some(MediaType::Print),
            "screen" => Some(MediaType::Screen),
            other => {
                return Err(ApiError::InvalidField {
                    field: "emulatedMediaType",
                    message: format!("expected `print` or `screen`, got `{other}`"),
                });
            }
        };
    }

    // Wait conditions. At most one of waitForExpression / waitForSelector /
    // waitDelay / waitWindowStatus may be set; if multiple are present, that's an error.
    let wait_count = [
        "waitForExpression",
        "waitForSelector",
        "waitDelay",
        "waitWindowStatus",
    ]
    .iter()
    .filter(|k| map.get(**k).map(|s| !s.is_empty()).unwrap_or(false))
    .count();
    if wait_count > 1 {
        return Err(ApiError::InvalidField {
            field: "wait",
            message: "set only one of waitForExpression, waitForSelector, waitDelay, waitWindowStatus"
                .to_string(),
        });
    }
    if let Some(s) = nonempty(map, "waitForExpression") {
        opts.wait = WaitCondition::Expression { expression: s };
    } else if let Some(s) = nonempty(map, "waitForSelector") {
        opts.wait = WaitCondition::Selector { selector: s };
    } else if let Some(s) = nonempty(map, "waitDelay") {
        let d = humantime::parse_duration(&s).map_err(|e| ApiError::InvalidField {
            field: "waitDelay",
            message: e.to_string(),
        })?;
        opts.wait = WaitCondition::Delay { duration: d };
    } else if let Some(s) = nonempty(map, "waitWindowStatus") {
        opts.wait = WaitCondition::WindowStatus { status: s };
    }

    if let Some(v) = opt_bool(map, "singlePage")? {
        opts.single_page = v;
    }

    if let Some(s) = nonempty(map, "emulatedMediaFeatures") {
        let map: std::collections::HashMap<String, String> =
            serde_json::from_str(&s).map_err(|e| ApiError::InvalidField {
                field: "emulatedMediaFeatures",
                message: e.to_string(),
            })?;
        opts.emulated_media_features = map.into_iter().collect();
    }

    Ok(opts)
}

/// Parse and validate the `nativePageRanges` form field.
/// Returns `None` if the field is absent or empty.
/// Returns a parse error if the value is not a valid page-range expression.
fn parse_native_page_ranges(map: &HashMap<String, String>) -> ApiResult<Option<(PageRanges, String)>> {
    let Some(raw) = map.get("nativePageRanges").filter(|s| !s.is_empty()) else {
        return Ok(None);
    };
    let trimmed = raw.trim();
    match PageRanges::parse(trimmed) {
        Ok(ranges) => Ok(Some((ranges, trimmed.to_string()))),
        Err(_) => Err(ApiError::InvalidField {
            field: "nativePageRanges",
            message: format!(
                "Chromium does not handle the page ranges '{trimmed}' (nativePageRanges) syntax"
            ),
        }),
    }
}

/// After rendering, verify that the nativePageRanges (if provided) do not
/// exceed the actual page count of the rendered PDF.
fn validate_native_page_ranges_against_pdf(
    pdf: &[u8],
    native: &Option<(PageRanges, String)>,
) -> ApiResult<()> {
    let Some((ranges, raw)) = native else {
        return Ok(());
    };
    let page_count = lopdf::Document::load_mem(pdf)
        .map(|doc| doc.get_pages().len() as u32)
        .unwrap_or(0);
    // If none of the requested pages fall within [1, page_count], the range
    // exceeds the document's actual page count.
    if ranges.expand(page_count).is_empty() {
        return Err(ApiError::InvalidField {
            field: "nativePageRanges",
            message: format!(
                "The page ranges '{raw}' (nativePageRanges) exceeds the page count"
            ),
        });
    }
    Ok(())
}

/// Validate extra fields that are accepted by the chromium handlers but not
/// part of core `PdfOptions` parsing: `splitMode`, `pdfa`, `pdfua`.
fn validate_chromium_extra_fields(map: &HashMap<String, String>) -> ApiResult<()> {
    if let Some(mode) = map.get("splitMode").filter(|s| !s.is_empty()) {
        match mode.as_str() {
            "intervals" | "pages" => {}
            other => {
                return Err(ApiError::InvalidField {
                    field: "splitMode",
                    message: format!("invalid splitMode '{other}': expected 'intervals' or 'pages'"),
                });
            }
        }
    }
    if let Some(pdfa) = map.get("pdfa").filter(|s| !s.is_empty()) {
        pdfa.parse::<PdfAProfile>().map_err(|_| ApiError::InvalidField {
            field: "pdfa",
            message: format!("invalid PDF/A profile '{pdfa}'"),
        })?;
    }
    if let Some(pdfua) = map.get("pdfua").filter(|s| !s.is_empty()) {
        match pdfua.trim().to_ascii_lowercase().as_str() {
            "true" | "false" | "1" | "0" | "yes" | "no" | "on" | "off" => {}
            other => {
                return Err(ApiError::InvalidField {
                    field: "pdfua",
                    message: format!("expected boolean, got `{other}`"),
                });
            }
        }
    }
    Ok(())
}

/// Build a [`RequestContext`] from the captured form map.
pub fn parse_request_context(map: &HashMap<String, String>) -> ApiResult<RequestContext> {
    let mut ctx = RequestContext::default();

    if let Some(s) = nonempty(map, "userAgent") {
        ctx.user_agent = Some(s);
    }

    if let Some(s) = nonempty(map, "extraHttpHeaders") {
        let parsed: HashMap<String, String> =
            serde_json::from_str(&s).map_err(|e| ApiError::InvalidField {
                field: "extraHttpHeaders",
                message: e.to_string(),
            })?;
        ctx.extra_headers = parsed;
    }

    if let Some(s) = nonempty(map, "cookies") {
        ctx.cookies = parse_cookies_json(&s)?;
    }

    if let Some(s) = nonempty(map, "failOnHttpStatusCodes") {
        let parsed: Vec<u16> = serde_json::from_str(&s).map_err(|e| ApiError::InvalidField {
            field: "failOnHttpStatusCodes",
            message: e.to_string(),
        })?;
        ctx.fail_on_status = parsed;
    }

    if let Some(s) = nonempty(map, "failOnResourceHttpStatusCodes") {
        let parsed: Vec<u16> = serde_json::from_str(&s).map_err(|e| ApiError::InvalidField {
            field: "failOnResourceHttpStatusCodes",
            message: e.to_string(),
        })?;
        ctx.fail_on_resource_status = parsed;
    }

    if let Some(v) = opt_bool(map, "failOnResourceLoadingFailed")? {
        ctx.fail_on_resource_loading_failed = v;
    }

    if let Some(v) = opt_bool(map, "failOnConsoleExceptions")? {
        ctx.fail_on_console_exceptions = v;
    }

    // skipNetworkIdleEvent / skipNetworkAlmostIdleEvent are merged into a
    // single engine flag — Chrome does not distinguish the two via CDP.
    let skip_idle = opt_bool(map, "skipNetworkIdleEvent")?.unwrap_or(false);
    let skip_almost_idle = opt_bool(map, "skipNetworkAlmostIdleEvent")?.unwrap_or(false);
    if skip_idle || skip_almost_idle {
        ctx.skip_network_idle = true;
    }

    if let Some(s) = nonempty(map, "ignoreResourceHttpStatusDomains") {
        ctx.ignore_resource_status_domains = parse_string_list(&s);
    }

    Ok(ctx)
}

/// Parse a list of strings from a form field. Accepts either a JSON array
/// (`["a","b"]`) or a comma/newline-separated list (`a, b`). Empty
/// entries are dropped; whitespace is trimmed.
fn parse_string_list(s: &str) -> Vec<String> {
    let trimmed = s.trim();
    if trimmed.starts_with('[') {
        if let Ok(v) = serde_json::from_str::<Vec<String>>(trimmed) {
            return v
                .into_iter()
                .map(|x| x.trim().to_string())
                .filter(|x| !x.is_empty())
                .collect();
        }
    }
    trimmed
        .split(|c: char| c == ',' || c == '\n')
        .map(|p| p.trim().to_string())
        .filter(|p| !p.is_empty())
        .collect()
}

fn parse_cookies_json(s: &str) -> ApiResult<Vec<Cookie>> {
    #[derive(serde::Deserialize)]
    struct CookieDto {
        name: String,
        value: String,
        #[serde(default)]
        domain: Option<String>,
        #[serde(default)]
        path: Option<String>,
        #[serde(default)]
        secure: bool,
        #[serde(default, rename = "httpOnly", alias = "http_only")]
        http_only: bool,
    }
    let dtos: Vec<CookieDto> = serde_json::from_str(s).map_err(|e| ApiError::InvalidField {
        field: "cookies",
        message: e.to_string(),
    })?;
    Ok(dtos
        .into_iter()
        .map(|d| Cookie {
            name: d.name,
            value: d.value,
            domain: d.domain,
            path: d.path,
            secure: d.secure,
            http_only: d.http_only,
        })
        .collect())
}

// ---------------------------------------------------------------------------
// Small typed accessors over the form map
// ---------------------------------------------------------------------------

fn nonempty(map: &HashMap<String, String>, key: &str) -> Option<String> {
    map.get(key).filter(|s| !s.is_empty()).cloned()
}

fn opt_f32(map: &HashMap<String, String>, key: &'static str) -> ApiResult<Option<f32>> {
    match map.get(key) {
        None => Ok(None),
        Some(s) if s.is_empty() => Ok(None),
        Some(s) => s
            .trim()
            .parse::<f32>()
            .map(Some)
            .map_err(|e| ApiError::InvalidField {
                field: key,
                message: e.to_string(),
            }),
    }
}

fn opt_bool(map: &HashMap<String, String>, key: &'static str) -> ApiResult<Option<bool>> {
    match map.get(key) {
        None => Ok(None),
        Some(s) if s.is_empty() => Ok(None),
        Some(s) => match s.trim().to_ascii_lowercase().as_str() {
            "1" | "true" | "yes" | "on" => Ok(Some(true)),
            "0" | "false" | "no" | "off" => Ok(Some(false)),
            other => Err(ApiError::InvalidField {
                field: key,
                message: format!("expected boolean, got `{other}`"),
            }),
        },
    }
}

// ---------------------------------------------------------------------------
// Plumbing
// ---------------------------------------------------------------------------

async fn acquire_permit(state: &AppState) -> ApiResult<tokio::sync::OwnedSemaphorePermit> {
    state
        .sem
        .clone()
        .acquire_owned()
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))
}

fn file_url_for(path: &Path) -> String {
    // Best-effort; we expect tempdir paths to be UTF-8 on the platforms we
    // care about. If conversion fails, fall through to a relative path.
    let absolute: PathBuf = match path.canonicalize() {
        Ok(p) => p,
        Err(_) => path.to_path_buf(),
    };
    let s = absolute.to_string_lossy();
    if s.starts_with('/') {
        format!("file://{s}")
    } else {
        format!("file:///{s}")
    }
}

// ---------------------------------------------------------------------------
// Screenshot handlers
// ---------------------------------------------------------------------------

/// `POST /forms/chromium/screenshot/html`.
pub async fn chromium_screenshot_html(
    State(state): State<AppState>,
    headers: HeaderMap,
    mp: Multipart,
) -> ApiResult<Response> {
    let _permit = acquire_permit(&state).await?;
    let mut form = FormFields::from_multipart(mp).await?;
    crate::download::inject_downloads(&mut form, &state.config).await?;
    let index = form
        .find_named("files", INDEX_HTML)
        .ok_or_else(|| ApiError::MissingFile(INDEX_HTML.to_string()))?;
    let html = tokio::fs::read_to_string(&index.path)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    let opts = parse_screenshot_options(&form.map)?;

    if let Some(resp) = crate::webhook::maybe_spawn_webhook(
        &headers,
        &state,
        crate::webhook::WebhookOperation::ChromiumScreenshotHtml,
        crate::webhook::JobData::ChromiumScreenshotHtml {
            html: html.clone().into_bytes(),
            options: opts.clone(),
        },
    ).await? {
        return Ok(resp);
    }

    let start = Instant::now();
    let result = state.chromium.as_ref().unwrap().html_to_screenshot(&html, &opts).await;
    let duration = start.elapsed().as_secs_f64();

    match &result {
        Ok(image) => {
            state.metrics.record_conversion(
                "chromium",
                "/forms/chromium/screenshot/html",
                true,
                duration,
                image.len() as u64,
            );
            state.metrics.record_engine_conversion(
                "chromium",
                "/forms/chromium/screenshot/html",
            );
        }
        Err(_) => {
            state.metrics.record_conversion(
                "chromium",
                "/forms/chromium/screenshot/html",
                false,
                duration,
                0,
            );
        }
    }

    let image = result?;
    let ext = opts.format.extension();
    let filename = screenshot_filename(&headers, "screenshot", ext);

    Ok(image_response(image, &filename, opts.format.content_type()))
}

/// `POST /forms/chromium/screenshot/url`.
pub async fn chromium_screenshot_url(
    State(state): State<AppState>,
    headers: HeaderMap,
    mp: Multipart,
) -> ApiResult<Response> {
    let _permit = acquire_permit(&state).await?;
    let mut form = FormFields::from_multipart(mp).await?;
    crate::download::inject_downloads(&mut form, &state.config).await?;
    let url = form.map.get("url").ok_or(ApiError::MissingField("url"))?;

    let opts = parse_screenshot_options(&form.map)?;

    if let Some(resp) = crate::webhook::maybe_spawn_webhook(
        &headers,
        &state,
        crate::webhook::WebhookOperation::ChromiumScreenshotUrl,
        crate::webhook::JobData::ChromiumScreenshotUrl {
            url: url.clone(),
            options: opts.clone(),
        },
    ).await? {
        return Ok(resp);
    }

    let start = Instant::now();
    let result = state.chromium.as_ref().unwrap().url_to_screenshot(url, &opts).await;
    let duration = start.elapsed().as_secs_f64();

    match &result {
        Ok(image) => {
            state.metrics.record_conversion(
                "chromium",
                "/forms/chromium/screenshot/url",
                true,
                duration,
                image.len() as u64,
            );
            state.metrics.record_engine_conversion(
                "chromium",
                "/forms/chromium/screenshot/url",
            );
        }
        Err(_) => {
            state.metrics.record_conversion(
                "chromium",
                "/forms/chromium/screenshot/url",
                false,
                duration,
                0,
            );
        }
    }

    let image = result?;
    let ext = opts.format.extension();
    let filename = screenshot_filename(&headers, "screenshot", ext);

    Ok(image_response(image, &filename, opts.format.content_type()))
}

/// `POST /forms/chromium/screenshot/markdown`.
pub async fn chromium_screenshot_markdown(
    State(state): State<AppState>,
    headers: HeaderMap,
    mp: Multipart,
) -> ApiResult<Response> {
    let _permit = acquire_permit(&state).await?;
    let mut form = FormFields::from_multipart(mp).await?;
    crate::download::inject_downloads(&mut form, &state.config).await?;
    let index = form
        .find_named("files", "index.html")
        .ok_or_else(|| ApiError::MissingFile("index.html".to_string()))?;
    let wrapper = tokio::fs::read_to_string(&index.path)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    let html = inline_markdown_links(&wrapper, &form).await?;

    let opts = parse_screenshot_options(&form.map)?;

    if let Some(resp) = crate::webhook::maybe_spawn_webhook(
        &headers,
        &state,
        crate::webhook::WebhookOperation::ChromiumScreenshotMarkdown,
        crate::webhook::JobData::ChromiumScreenshotMarkdown {
            html: html.clone().into_bytes(),
            options: opts.clone(),
        },
    ).await? {
        return Ok(resp);
    }

    let image = state.chromium.as_ref().unwrap().html_to_screenshot(&html, &opts).await?;

    let ext = opts.format.extension();
    let filename = screenshot_filename(&headers, "screenshot", ext);

    Ok(image_response(image, &filename, opts.format.content_type()))
}

fn screenshot_filename(headers: &HeaderMap, default_stem: &str, ext: &str) -> String {
    let stem = headers
        .get("Gotenberg-Output-Filename")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.trim_end_matches(&format!(".{ext}")))
        .unwrap_or(default_stem);
    format!("{stem}.{ext}")
}

fn parse_screenshot_options(map: &HashMap<String, String>) -> ApiResult<ScreenshotOptions> {
    let format = match map.get("format").map(|s| s.as_str()) {
        Some("jpeg") | Some("jpg") => {
            let quality = map.get("quality").and_then(|s| s.parse::<u8>().ok()).unwrap_or(80);
            ScreenshotFormat::Jpeg { quality }
        }
        Some("webp") => {
            let quality = map.get("quality").and_then(|s| s.parse::<u8>().ok()).unwrap_or(80);
            ScreenshotFormat::Webp { quality }
        }
        _ => ScreenshotFormat::Png,
    };

    let mode = match map.get("fullPage").map(|s| s.as_str()) {
        Some("true") | Some("1") => CaptureMode::FullPage,
        _ => CaptureMode::Viewport,
    };

    let width = map.get("width").and_then(|s| s.parse::<u32>().ok()).unwrap_or(1920);
    let height = map.get("height").and_then(|s| s.parse::<u32>().ok()).unwrap_or(1080);
    
    let device_scale_factor = map.get("scale")
        .and_then(|s| s.parse::<f32>().ok())
        .unwrap_or(1.0);

    Ok(ScreenshotOptions {
        format,
        mode,
        width,
        height,
        device_scale_factor,
        wait_condition: WaitCondition::Load,
        extra_headers: HashMap::new(),
        background_color: map.get("backgroundColor").cloned(),
    })
}

fn image_response(data: Vec<u8>, filename: &str, content_type: &str) -> Response {
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, content_type)
        .header(header::CONTENT_DISPOSITION, format!("attachment; filename=\"{}\"", filename))
        .body(data.into())
        .unwrap()
}

fn render_markdown_to_html(md: &str) -> String {
    use pulldown_cmark::{Parser, Options, html};
    
    let mut opts = Options::empty();
    opts.insert(Options::ENABLE_TABLES);
    opts.insert(Options::ENABLE_STRIKETHROUGH);
    
    let parser = Parser::new_ext(md, opts);
    let mut html_output = String::new();
    html::push_html(&mut html_output, parser);
    
    format!(
        r#"<!DOCTYPE html>
<html>
<head>
<meta charset="UTF-8">
<style>
body {{ font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif; line-height: 1.6; padding: 2em; max-width: 900px; margin: 0 auto; }}
table {{ border-collapse: collapse; width: 100%; }}
th, td {{ border: 1px solid #ddd; padding: 8px; text-align: left; }}
th {{ background-color: #f2f2f2; }}
code {{ background-color: #f4f4f4; padding: 2px 4px; border-radius: 3px; }}
pre {{ background-color: #f4f4f4; padding: 16px; overflow-x: auto; border-radius: 5px; }}
blockquote {{ border-left: 4px solid #ddd; padding-left: 16px; margin-left: 0; color: #666; }}
</style>
</head>
<body>
{}
</body>
</html>"#,
        html_output
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fm(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect()
    }

    #[test]
    fn pdf_options_round_trip_through_form_map() {
        let map = fm(&[
            ("paperWidth", "8.5"),
            ("paperHeight", "11.0"),
            ("marginTop", "0.5"),
            ("marginRight", "0.5"),
            ("marginBottom", "0.5"),
            ("marginLeft", "0.5"),
            ("landscape", "true"),
            ("scale", "1.25"),
            ("printBackground", "false"),
            ("preferCssPageSize", "true"),
            ("pageRanges", "1-3,7-"),
            ("emulatedMediaType", "screen"),
            ("waitDelay", "1500ms"),
        ]);
        let opts = parse_pdf_options(&map).unwrap();
        assert!((opts.paper.width_in - 8.5).abs() < 1e-3);
        assert!((opts.paper.height_in - 11.0).abs() < 1e-3);
        assert!((opts.margin.top - 0.5).abs() < 1e-3);
        assert!(opts.landscape);
        assert!((opts.scale - 1.25).abs() < 1e-3);
        assert!(!opts.print_background);
        assert!(opts.prefer_css_page_size);
        assert!(opts.page_ranges.is_some());
        assert_eq!(opts.emulate_media, Some(MediaType::Screen));
        match opts.wait {
            WaitCondition::Delay { duration } => {
                assert_eq!(duration.as_millis(), 1500);
            }
            other => panic!("expected Delay, got {other:?}"),
        }
    }

    #[test]
    fn pdf_options_default_when_map_empty() {
        let opts = parse_pdf_options(&HashMap::new()).unwrap();
        let d = PdfOptions::default();
        assert_eq!(opts.paper, d.paper);
        assert_eq!(opts.margin, d.margin);
        assert_eq!(opts.scale, d.scale);
        assert_eq!(opts.landscape, d.landscape);
    }

    #[test]
    fn pdf_options_invalid_bool_rejected() {
        let map = fm(&[("landscape", "maybe")]);
        let err = parse_pdf_options(&map).unwrap_err();
        match err {
            ApiError::InvalidField { field, .. } => assert_eq!(field, "landscape"),
            other => panic!("expected InvalidField, got {other:?}"),
        }
    }

    #[test]
    fn pdf_options_invalid_emulate_media_rejected() {
        let map = fm(&[("emulatedMediaType", "carrier-pigeon")]);
        let err = parse_pdf_options(&map).unwrap_err();
        match err {
            ApiError::InvalidField { field, .. } => assert_eq!(field, "emulatedMediaType"),
            other => panic!("expected InvalidField, got {other:?}"),
        }
    }

    #[test]
    fn pdf_options_multiple_wait_conditions_rejected() {
        let map = fm(&[("waitDelay", "1s"), ("waitForSelector", "#ok")]);
        let err = parse_pdf_options(&map).unwrap_err();
        match err {
            ApiError::InvalidField { field, .. } => assert_eq!(field, "wait"),
            other => panic!("expected InvalidField, got {other:?}"),
        }
    }

    #[test]
    fn request_context_round_trip_basic() {
        let map = fm(&[
            ("userAgent", "Mozilla/5.0 pdfbro-test"),
            ("extraHttpHeaders", r#"{"X-Trace":"abc"}"#),
            (
                "cookies",
                r#"[{"name":"a","value":"1","domain":"example.com","path":"/","secure":true,"httpOnly":true}]"#,
            ),
            ("failOnHttpStatusCodes", "[500,503]"),
        ]);
        let ctx = parse_request_context(&map).unwrap();
        assert_eq!(ctx.user_agent.as_deref(), Some("Mozilla/5.0 pdfbro-test"));
        assert_eq!(
            ctx.extra_headers.get("X-Trace").map(String::as_str),
            Some("abc")
        );
        assert_eq!(ctx.cookies.len(), 1);
        let c = &ctx.cookies[0];
        assert_eq!(c.name, "a");
        assert_eq!(c.value, "1");
        assert_eq!(c.domain.as_deref(), Some("example.com"));
        assert!(c.secure);
        assert!(c.http_only);
        assert_eq!(ctx.fail_on_status, vec![500, 503]);
    }

    #[test]
    fn request_context_default_when_empty() {
        let ctx = parse_request_context(&HashMap::new()).unwrap();
        assert!(ctx.user_agent.is_none());
        assert!(ctx.extra_headers.is_empty());
        assert!(ctx.cookies.is_empty());
        assert!(ctx.fail_on_status.is_empty());
    }

    #[test]
    fn extra_http_headers_invalid_json_returns_invalid_option() {
        let map = fm(&[("extraHttpHeaders", "not-json")]);
        let err = parse_request_context(&map).unwrap_err();
        match err {
            ApiError::InvalidField { field, .. } => assert_eq!(field, "extraHttpHeaders"),
            other => panic!("expected InvalidField, got {other:?}"),
        }
    }

    #[test]
    fn cookies_with_attrs_parse() {
        let map = fm(&[(
            "cookies",
            r#"[{"name":"x","value":"y","secure":false,"http_only":false}]"#,
        )]);
        let ctx = parse_request_context(&map).unwrap();
        assert_eq!(ctx.cookies.len(), 1);
        let c = &ctx.cookies[0];
        assert!(!c.secure);
        assert!(!c.http_only);
        assert!(c.domain.is_none());
        assert!(c.path.is_none());
    }

    #[test]
    fn fail_on_status_codes_parse() {
        let map = fm(&[("failOnHttpStatusCodes", "[401, 403, 500]")]);
        let ctx = parse_request_context(&map).unwrap();
        assert_eq!(ctx.fail_on_status, vec![401, 403, 500]);
    }

    #[test]
    fn wait_window_status_parses() {
        let map = fm(&[("waitWindowStatus", "ready")]);
        let opts = parse_pdf_options(&map).unwrap();
        assert_eq!(
            opts.wait,
            WaitCondition::WindowStatus {
                status: "ready".into()
            }
        );
    }

    #[test]
    fn wait_window_status_conflicts_with_other_wait() {
        let map = fm(&[("waitWindowStatus", "ready"), ("waitDelay", "1s")]);
        let err = parse_pdf_options(&map).unwrap_err();
        match err {
            ApiError::InvalidField { field, .. } => assert_eq!(field, "wait"),
            other => panic!("expected InvalidField, got {other:?}"),
        }
    }

    #[test]
    fn fail_on_resource_status_codes_parse() {
        let map = fm(&[("failOnResourceHttpStatusCodes", "[404, 502]")]);
        let ctx = parse_request_context(&map).unwrap();
        assert_eq!(ctx.fail_on_resource_status, vec![404, 502]);
    }

    #[test]
    fn fail_on_resource_status_codes_invalid_json() {
        let map = fm(&[("failOnResourceHttpStatusCodes", "nope")]);
        let err = parse_request_context(&map).unwrap_err();
        match err {
            ApiError::InvalidField { field, .. } => {
                assert_eq!(field, "failOnResourceHttpStatusCodes")
            }
            other => panic!("expected InvalidField, got {other:?}"),
        }
    }

    #[test]
    fn fail_on_resource_loading_failed_parses() {
        let map = fm(&[("failOnResourceLoadingFailed", "true")]);
        let ctx = parse_request_context(&map).unwrap();
        assert!(ctx.fail_on_resource_loading_failed);
    }

    #[test]
    fn fail_on_console_exceptions_parses() {
        let map = fm(&[("failOnConsoleExceptions", "true")]);
        let ctx = parse_request_context(&map).unwrap();
        assert!(ctx.fail_on_console_exceptions);
    }

    #[test]
    fn skip_network_idle_event_parses() {
        let map = fm(&[("skipNetworkIdleEvent", "true")]);
        let ctx = parse_request_context(&map).unwrap();
        assert!(ctx.skip_network_idle);
    }

    #[test]
    fn skip_network_almost_idle_event_also_parses() {
        let map = fm(&[("skipNetworkAlmostIdleEvent", "true")]);
        let ctx = parse_request_context(&map).unwrap();
        assert!(ctx.skip_network_idle);
    }

    #[test]
    fn skip_network_idle_off_by_default() {
        let ctx = parse_request_context(&HashMap::new()).unwrap();
        assert!(!ctx.skip_network_idle);
    }

    #[test]
    fn ignore_resource_domains_json_array() {
        let map = fm(&[(
            "ignoreResourceHttpStatusDomains",
            r#"["googleapis.com", "fonts.gstatic.com"]"#,
        )]);
        let ctx = parse_request_context(&map).unwrap();
        assert_eq!(
            ctx.ignore_resource_status_domains,
            vec![
                "googleapis.com".to_string(),
                "fonts.gstatic.com".to_string()
            ]
        );
    }

    #[test]
    fn ignore_resource_domains_comma_list() {
        let map = fm(&[(
            "ignoreResourceHttpStatusDomains",
            "googleapis.com, fonts.gstatic.com",
        )]);
        let ctx = parse_request_context(&map).unwrap();
        assert_eq!(
            ctx.ignore_resource_status_domains,
            vec![
                "googleapis.com".to_string(),
                "fonts.gstatic.com".to_string()
            ]
        );
    }

    #[test]
    fn ignore_resource_domains_default_empty() {
        let ctx = parse_request_context(&HashMap::new()).unwrap();
        assert!(ctx.ignore_resource_status_domains.is_empty());
    }

    #[test]
    fn finds_markdown_link_with_double_quotes() {
        let s = r#"<html><head><link rel="markdown" href="intro.md"></head></html>"#;
        let m = find_markdown_link(s).unwrap();
        assert_eq!(m.href, "intro.md");
        assert_eq!(
            &s[m.start..m.end],
            r#"<link rel="markdown" href="intro.md">"#
        );
    }

    #[test]
    fn finds_markdown_link_case_insensitive() {
        let s = r#"<LINK REL="markdown" HREF="X.md" />"#;
        let m = find_markdown_link(s).unwrap();
        assert_eq!(m.href, "X.md");
    }

    #[test]
    fn ignores_non_markdown_links() {
        let s = r#"<link rel="stylesheet" href="x.css">"#;
        assert!(find_markdown_link(s).is_none());
    }

    #[test]
    fn render_markdown_emits_html_table() {
        let html = render_markdown("| a | b |\n|---|---|\n| 1 | 2 |\n");
        assert!(html.contains("<table>"));
        assert!(html.contains("<td>1</td>"));
    }

    #[test]
    fn omit_background_requires_print_background() {
        let map: HashMap<String, String> = [
            ("omitBackground".to_string(), "true".to_string()),
            ("printBackground".to_string(), "false".to_string()),
        ]
        .into_iter()
        .collect();
        let err = parse_pdf_options(&map).unwrap_err();
        assert!(matches!(err, ApiError::InvalidField { field: "omitBackground", .. }));
    }

    #[test]
    fn omit_background_with_print_background_ok() {
        let map: HashMap<String, String> = [
            ("omitBackground".to_string(), "true".to_string()),
            ("printBackground".to_string(), "true".to_string()),
        ]
        .into_iter()
        .collect();
        let opts = parse_pdf_options(&map).unwrap();
        assert!(opts.omit_background);
        assert!(opts.print_background);
    }

    #[test]
    fn omit_background_defaults_false() {
        let opts = parse_pdf_options(&HashMap::new()).unwrap();
        assert!(!opts.omit_background);
    }
}
