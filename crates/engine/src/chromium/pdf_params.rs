//! Translation from [`PdfOptions`] to chromiumoxide's
//! `Page.printToPDF` parameters.
//!
//! Implements the *Page.printToPDF parameter mapping* table in
//! `docs/specs/11-engine-chromium.md`.

use chromiumoxide::cdp::browser_protocol::page::{PrintToPdfParams, PrintToPdfTransferMode};

use crate::types::{MediaType, PdfOptions};

/// Build CDP `Page.printToPDF` parameters from a [`PdfOptions`].
///
/// The caller is responsible for invoking `opts.validate()` first; this
/// function does no validation of its own.
pub(crate) fn build_printtopdf_params(opts: &PdfOptions) -> PrintToPdfParams {
    let display_header_footer = opts.header_template.is_some() || opts.footer_template.is_some();

    PrintToPdfParams {
        landscape: Some(opts.landscape),
        display_header_footer: Some(display_header_footer),
        print_background: Some(opts.print_background),
        scale: Some(f64::from(opts.scale)),
        paper_width: Some(f64::from(opts.paper.width_in)),
        paper_height: Some(f64::from(opts.paper.height_in)),
        margin_top: Some(f64::from(opts.margin.top)),
        margin_bottom: Some(f64::from(opts.margin.bottom)),
        margin_left: Some(f64::from(opts.margin.left)),
        margin_right: Some(f64::from(opts.margin.right)),
        page_ranges: opts.page_ranges.as_ref().map(ToString::to_string),
        header_template: opts.header_template.clone(),
        footer_template: opts.footer_template.clone(),
        prefer_css_page_size: Some(opts.prefer_css_page_size),
        transfer_mode: Some(PrintToPdfTransferMode::ReturnAsBase64),
        generate_tagged_pdf: None,
        generate_document_outline: None,
    }
}

/// `Emulation.setEmulatedMedia` value.
pub(crate) fn media_kind(media: MediaType) -> &'static str {
    match media {
        MediaType::Print => "print",
        MediaType::Screen => "screen",
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Margins, PageRanges, PaperSize, WaitCondition};

    #[test]
    fn printtopdf_params_built_from_pdfoptions_defaults() {
        let opts = PdfOptions::default();
        let p = build_printtopdf_params(&opts);

        assert_eq!(p.landscape, Some(false));
        assert_eq!(p.display_header_footer, Some(false));
        assert_eq!(p.print_background, Some(false));
        assert_eq!(p.scale, Some(1.0));
        assert!((p.paper_width.unwrap() - 8.27).abs() < 1e-3);
        assert!((p.paper_height.unwrap() - 11.69).abs() < 1e-3);
        assert!((p.margin_top.unwrap() - 0.39).abs() < 1e-3);
        assert!((p.margin_left.unwrap() - 0.39).abs() < 1e-3);
        assert_eq!(p.page_ranges, None);
        assert_eq!(p.header_template, None);
        assert_eq!(p.footer_template, None);
        assert_eq!(p.prefer_css_page_size, Some(false));
        assert_eq!(
            p.transfer_mode,
            Some(PrintToPdfTransferMode::ReturnAsBase64)
        );
    }

    #[test]
    fn printtopdf_params_propagates_overrides() {
        let opts = PdfOptions {
            paper: PaperSize::LETTER,
            margin: Margins::ZERO,
            landscape: true,
            scale: 0.75,
            print_background: false,
            omit_background: false,
            prefer_css_page_size: true,
            emulate_media: Some(MediaType::Screen),
            page_ranges: Some(PageRanges::parse("1-3,5").unwrap()),
            header_template: Some("<h1>Hi</h1>".into()),
            footer_template: Some("<span></span>".into()),
            wait: WaitCondition::Load,
            single_page: false,
            emulated_media_features: Vec::new(),
        };
        let p = build_printtopdf_params(&opts);

        assert_eq!(p.landscape, Some(true));
        assert_eq!(p.display_header_footer, Some(true));
        assert_eq!(p.print_background, Some(false));
        assert_eq!(p.scale, Some(0.75));
        assert!((p.paper_width.unwrap() - 8.5).abs() < 1e-3);
        assert!((p.paper_height.unwrap() - 11.0).abs() < 1e-3);
        assert_eq!(p.margin_top, Some(0.0));
        assert_eq!(p.margin_left, Some(0.0));
        assert_eq!(p.page_ranges.as_deref(), Some("1-3,5"));
        assert_eq!(p.header_template.as_deref(), Some("<h1>Hi</h1>"));
        assert_eq!(p.footer_template.as_deref(), Some("<span></span>"));
        assert_eq!(p.prefer_css_page_size, Some(true));
    }

    #[test]
    fn display_header_footer_off_when_only_one_template_present() {
        let mut opts = PdfOptions {
            footer_template: Some("<span class='pageNumber'></span>".into()),
            ..PdfOptions::default()
        };
        let p = build_printtopdf_params(&opts);
        assert_eq!(p.display_header_footer, Some(true));

        opts.footer_template = None;
        opts.header_template = Some("<h1>Hi</h1>".into());
        let p = build_printtopdf_params(&opts);
        assert_eq!(p.display_header_footer, Some(true));

        opts.header_template = None;
        let p = build_printtopdf_params(&opts);
        assert_eq!(p.display_header_footer, Some(false));
    }

    #[test]
    fn media_kind_matches_spec() {
        assert_eq!(media_kind(MediaType::Print), "print");
        assert_eq!(media_kind(MediaType::Screen), "screen");
    }
}
