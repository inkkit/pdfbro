//! Pure mappings from clap flag structs onto engine value types
//! (`PdfOptions`, `RequestContext`, `BrowserConfig`, `OfficeOptions`).
//!
//! Each builder is fallible only when reading user-supplied template
//! files from disk; otherwise it just shuffles fields.

use anyhow::Context;
use engine::{
    BrowserConfig, Cookie, MediaType, OfficeOptions, PdfOptions, RequestContext,
};
use engine::libreoffice::PdfAProfile;

use crate::args::{EmulateMedia, GlobalOpts, OfficeFlags, PdfAFlag, PdfFlags, RequestFlags};

/// Build a [`PdfOptions`] from the CLI flag struct.
///
/// Header / footer template files are read into strings here; missing
/// files surface as `anyhow::Error` carrying an `std::io::Error` source
/// so `exit_for_anyhow` maps them to exit code 5.
pub(crate) fn build_pdf_options(flags: &PdfFlags) -> anyhow::Result<PdfOptions> {
    let mut o = PdfOptions::default();
    if let Some(p) = flags.paper {
        o.paper = p;
    }
    if let Some(m) = flags.margin {
        o.margin = m;
    }
    if flags.landscape {
        o.landscape = true;
    }
    if let Some(s) = flags.scale {
        o.scale = s;
    }
    if flags.no_print_background {
        o.print_background = false;
    }
    if flags.prefer_css_page_size {
        o.prefer_css_page_size = true;
    }
    if let Some(em) = flags.emulate {
        o.emulate_media = match em {
            EmulateMedia::Print => MediaType::Print,
            EmulateMedia::Screen => MediaType::Screen,
        };
    }
    if let Some(pr) = &flags.pages {
        o.page_ranges = Some(pr.clone());
    }
    if let Some(p) = &flags.header_template {
        let body = std::fs::read_to_string(p)
            .with_context(|| format!("reading header template {}", p.display()))?;
        o.header_template = Some(body);
    }
    if let Some(p) = &flags.footer_template {
        let body = std::fs::read_to_string(p)
            .with_context(|| format!("reading footer template {}", p.display()))?;
        o.footer_template = Some(body);
    }
    if let Some(w) = &flags.wait {
        o.wait = w.clone();
    }
    Ok(o)
}

/// Build a [`RequestContext`] from CLI flags.
pub(crate) fn build_request(flags: &RequestFlags) -> RequestContext {
    let mut r = RequestContext {
        user_agent: flags.user_agent.clone(),
        ..RequestContext::default()
    };
    for (k, v) in &flags.headers {
        r.extra_headers.insert(k.clone(), v.clone());
    }
    for c in &flags.cookies {
        r.cookies.push(clone_cookie(c));
    }
    let mut codes: Vec<u16> = flags.fail_on_status.iter().flatten().copied().collect();
    codes.sort_unstable();
    codes.dedup();
    r.fail_on_status = codes;
    r
}

fn clone_cookie(c: &Cookie) -> Cookie {
    Cookie {
        name: c.name.clone(),
        value: c.value.clone(),
        domain: c.domain.clone(),
        path: c.path.clone(),
        secure: c.secure,
        http_only: c.http_only,
    }
}

/// Build a [`BrowserConfig`] from global CLI options. Fields the user
/// didn't override fall through to `BrowserConfig::default()`.
pub(crate) fn build_browser_config(global: &GlobalOpts) -> BrowserConfig {
    let mut c = BrowserConfig::default();
    if let Some(p) = &global.chrome {
        c.executable = Some(p.clone());
    }
    // `--sandbox` and `--no-sandbox` use `overrides_with`, so at most one
    // of these is true; both false means "leave the platform default".
    if global.sandbox {
        c.no_sandbox = false;
    } else if global.no_sandbox {
        c.no_sandbox = true;
    }
    if let Some(t) = global.timeout {
        c.timeout = t;
    }
    c
}

/// Build [`OfficeOptions`] for `--office` conversion. The shared
/// `--landscape` / `--pages` flags from `PdfFlags` are honoured.
pub(crate) fn build_office_options(pdf: &PdfFlags, office: &OfficeFlags) -> OfficeOptions {
    OfficeOptions {
        landscape: pdf.landscape,
        page_ranges: pdf.pages.clone(),
        pdf_a: office.pdf_a.map(|p| match p {
            PdfAFlag::A1B => PdfAProfile::A1B,
            PdfAFlag::A2B => PdfAProfile::A2B,
            PdfAFlag::A3B => PdfAProfile::A3B,
        }),
        pdf_ua: office.pdf_ua,
        quality: office.quality,
        max_image_resolution: office.max_image_resolution,
    }
}
