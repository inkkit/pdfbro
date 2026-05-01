//! Map `engine::EngineError` into a Python exception hierarchy.

use engine::EngineError;
use pyo3::create_exception;
use pyo3::exceptions::PyException;
use pyo3::prelude::*;

create_exception!(_native, FolioError, PyException);
create_exception!(_native, ChromeNotFoundError, FolioError);
create_exception!(_native, ChromeFetchError, FolioError);
create_exception!(_native, ChromiumError, FolioError);
create_exception!(_native, OfficeError, FolioError);
create_exception!(_native, EngineDisabledError, FolioError);
create_exception!(_native, TimeoutError, FolioError);
create_exception!(_native, ValidationError, FolioError);

pub fn engine_to_py(err: EngineError) -> PyErr {
    match err {
        EngineError::ChromeNotFound { .. } => ChromeNotFoundError::new_err(err.to_string()),
        EngineError::Timeout(_) => TimeoutError::new_err(err.to_string()),
        EngineError::InvalidOption(_) | EngineError::InvalidPageRange(_) => {
            ValidationError::new_err(err.to_string())
        }
        EngineError::ChromeLaunch(_) | EngineError::Cdp(_) | EngineError::Navigation { .. } => {
            ChromiumError::new_err(err.to_string())
        }
        // No specific Office variant in EngineError; office failures surface as
        // Internal / Io / Pdf depending on what failed. Route everything else
        // to the generic FolioError. If finer routing matters later, the engine
        // can grow an Office variant.
        _ => FolioError::new_err(err.to_string()),
    }
}

#[cfg(feature = "chrome-fetch")]
pub fn fetch_to_py(err: engine::chrome_fetch::ChromeFetchError) -> PyErr {
    use engine::chrome_fetch::ChromeFetchError as E;
    match err {
        E::NotFoundAndDownloadDisabled => ChromeNotFoundError::new_err(err.to_string()),
        _ => ChromeFetchError::new_err(err.to_string()),
    }
}

pub fn register(py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add("FolioError", py.get_type_bound::<FolioError>())?;
    m.add("ChromeNotFoundError", py.get_type_bound::<ChromeNotFoundError>())?;
    m.add("ChromeFetchError", py.get_type_bound::<ChromeFetchError>())?;
    m.add("ChromiumError", py.get_type_bound::<ChromiumError>())?;
    m.add("OfficeError", py.get_type_bound::<OfficeError>())?;
    m.add("EngineDisabledError", py.get_type_bound::<EngineDisabledError>())?;
    m.add("TimeoutError", py.get_type_bound::<TimeoutError>())?;
    m.add("ValidationError", py.get_type_bound::<ValidationError>())?;
    Ok(())
}
