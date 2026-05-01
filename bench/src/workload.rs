use std::path::PathBuf;

pub struct WorkloadDef {
    pub name: &'static str,
    pub description: &'static str,
    pub folio_route: &'static str,
    pub gotenberg_route: &'static str,
    pub fixtures: Vec<PathBuf>,
    pub extra_fields: Vec<(&'static str, &'static str)>,
    pub expected_pages: Option<usize>,
}

pub fn all_workloads() -> Vec<WorkloadDef> {
    let fixtures_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures");

    vec![
        WorkloadDef {
            name: "html-small",
            description: "Minimal HTML, no external assets",
            folio_route: "/forms/chromium/convert/html",
            gotenberg_route: "/forms/chromium/convert/html",
            fixtures: vec![fixtures_dir.join("html_small.html")],
            extra_fields: vec![],
            expected_pages: None,
        },
        WorkloadDef {
            name: "html-large",
            description: "HTML with web fonts and a data table",
            folio_route: "/forms/chromium/convert/html",
            gotenberg_route: "/forms/chromium/convert/html",
            fixtures: vec![fixtures_dir.join("html_large.html")],
            extra_fields: vec![],
            expected_pages: None,
        },
        WorkloadDef {
            name: "url-local",
            description: "Local axum fixture server URL (no real network)",
            folio_route: "/forms/chromium/convert/url",
            gotenberg_route: "/forms/chromium/convert/url",
            fixtures: vec![],
            extra_fields: vec![("url", "http://host.docker.internal:18080/bench.html")],
            expected_pages: None,
        },
        WorkloadDef {
            name: "libreoffice-docx",
            description: "50 KB DOCX file converted to PDF",
            folio_route: "/forms/libreoffice/convert",
            gotenberg_route: "/forms/libreoffice/convert",
            fixtures: vec![fixtures_dir.join("sample.docx")],
            extra_fields: vec![],
            expected_pages: None,
        },
        WorkloadDef {
            name: "pdfengines-merge",
            description: "5 × 20-page PDFs merged",
            folio_route: "/forms/pdfengines/merge",
            gotenberg_route: "/forms/pdfengines/merge",
            fixtures: vec![
                fixtures_dir.join("page_1.pdf"),
                fixtures_dir.join("page_2.pdf"),
                fixtures_dir.join("page_3.pdf"),
                fixtures_dir.join("page_4.pdf"),
                fixtures_dir.join("page_5.pdf"),
            ],
            extra_fields: vec![],
            expected_pages: None,
        },
    ]
}
