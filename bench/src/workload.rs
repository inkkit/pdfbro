use std::path::PathBuf;

pub struct WorkloadDef {
    pub name: &'static str,
    pub description: &'static str,
    pub pdfbro_route: &'static str,
    pub gotenberg_route: &'static str,
    pub fixtures: Vec<PathBuf>,
    /// Override the multipart field name for all fixtures (pdfbro/Gotenberg use "files").
    pub fixture_field: &'static str,
    /// Override the multipart filename for all fixtures (e.g. HTML endpoints require "index.html").
    pub fixture_filename: Option<&'static str>,
    pub extra_fields: Vec<(&'static str, &'static str)>,
    pub expected_pages: Option<usize>,
}

pub fn all_workloads() -> Vec<WorkloadDef> {
    let fixtures_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures");

    vec![
        WorkloadDef {
            name: "html-small",
            description: "Minimal HTML, no external assets",
            pdfbro_route: "/forms/chromium/convert/html",
            gotenberg_route: "/forms/chromium/convert/html",
            fixtures: vec![fixtures_dir.join("html_small.html")],
            fixture_field: "files",
            fixture_filename: Some("index.html"),
            extra_fields: vec![],
            expected_pages: None,
        },
        WorkloadDef {
            name: "html-large",
            description: "HTML with web fonts and a data table",
            pdfbro_route: "/forms/chromium/convert/html",
            gotenberg_route: "/forms/chromium/convert/html",
            fixtures: vec![fixtures_dir.join("html_large.html")],
            fixture_field: "files",
            fixture_filename: Some("index.html"),
            extra_fields: vec![],
            expected_pages: None,
        },
        WorkloadDef {
            name: "url-local",
            description: "Local axum fixture server URL (no real network)",
            pdfbro_route: "/forms/chromium/convert/url",
            gotenberg_route: "/forms/chromium/convert/url",
            fixtures: vec![],
            fixture_field: "files",
            fixture_filename: None,
            extra_fields: vec![("url", "http://host.docker.internal:18080/bench.html")],
            expected_pages: None,
        },
        WorkloadDef {
            name: "libreoffice-docx",
            description: "50 KB DOCX file converted to PDF",
            pdfbro_route: "/forms/libreoffice/convert",
            gotenberg_route: "/forms/libreoffice/convert",
            fixtures: vec![fixtures_dir.join("sample.docx")],
            fixture_field: "files",
            fixture_filename: None,
            extra_fields: vec![],
            expected_pages: None,
        },
        WorkloadDef {
            name: "pdfengines-merge",
            description: "5 × 20-page PDFs merged",
            pdfbro_route: "/forms/pdfengines/merge",
            gotenberg_route: "/forms/pdfengines/merge",
            fixtures: vec![
                fixtures_dir.join("page_1.pdf"),
                fixtures_dir.join("page_2.pdf"),
                fixtures_dir.join("page_3.pdf"),
                fixtures_dir.join("page_4.pdf"),
                fixtures_dir.join("page_5.pdf"),
            ],
            fixture_field: "files",
            fixture_filename: None,
            extra_fields: vec![],
            expected_pages: None,
        },
    ]
}
