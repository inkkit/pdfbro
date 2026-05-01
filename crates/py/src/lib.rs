//! Folio Python bindings — see `bindings/python/README.md`.

mod errors;
mod folio_async;
mod folio_sync;
mod launch;
mod runtime;
mod types;

use pyo3::prelude::*;

#[pymodule]
fn _native(py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    errors::register(py, m)?;
    m.add_class::<folio_sync::Folio>()?;
    m.add_class::<folio_async::AsyncFolio>()?;
    Ok(())
}
