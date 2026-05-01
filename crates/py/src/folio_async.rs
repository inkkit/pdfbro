//! Filled in by Task 7. Empty placeholder so `_native` module compiles.
use pyo3::prelude::*;

#[pyclass(name = "AsyncFolio", module = "folio")]
pub struct AsyncFolio;

#[pymethods]
impl AsyncFolio {
    #[new]
    fn new() -> Self {
        Self
    }
}
