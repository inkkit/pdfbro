//! Shared single tokio runtime used by sync PdfBro.
//! AsyncPdfBro uses pyo3-async-runtimes' bridged runtime instead.

use std::sync::OnceLock;
use tokio::runtime::Runtime;

static RUNTIME: OnceLock<Runtime> = OnceLock::new();

pub fn runtime() -> &'static Runtime {
    RUNTIME.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .thread_name("pdfbro-py")
            .build()
            .expect("init pdfbro-py tokio runtime")
    })
}
