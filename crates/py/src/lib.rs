//! pdfbro Python bindings — see `bindings/python/README.md`.

mod errors;
mod pdfbro_async;
mod pdfbro_sync;
mod launch;
mod runtime;
mod types;

use pyo3::prelude::*;

#[pymodule]
fn _native(py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    // Initialize the tokio runtime builder that pyo3-async-runtimes will use
    // to drive futures returned by AsyncPdfBro methods.
    // pyo3_async_runtimes::tokio::init takes a Builder (not a built Runtime).
    let mut builder = tokio::runtime::Builder::new_multi_thread();
    builder.enable_all().thread_name("pdfbro-py-async");
    pyo3_async_runtimes::tokio::init(builder);
    errors::register(py, m)?;
    m.add_class::<pdfbro_sync::PdfBro>()?;
    m.add_class::<pdfbro_async::AsyncPdfBro>()?;
    Ok(())
}
