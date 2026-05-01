//! Map `engine::EngineError` and `chrome_fetch::ChromeFetchError` to
//! tagged napi `Error` instances. The JS-side loader (Task 9) inspects
//! the `[Tag]` prefix on the message and raises a typed JS error subclass.

use engine::EngineError;
use napi::{Error, Status};

/// Convert an [`EngineError`] to a napi [`Error`] with a tagged message.
pub fn engine_to_napi(err: EngineError) -> Error {
    let (status, msg) = match err {
        EngineError::ChromeNotFound { .. } =>
            (Status::GenericFailure, format!("[ChromeNotFound] {err}")),
        EngineError::Timeout(_) =>
            (Status::GenericFailure, format!("[Timeout] {err}")),
        EngineError::InvalidOption(_) | EngineError::InvalidPageRange(_) =>
            (Status::InvalidArg, format!("[Validation] {err}")),
        EngineError::ChromeLaunch(_) | EngineError::Cdp(_) | EngineError::Navigation { .. } =>
            (Status::GenericFailure, format!("[Chromium] {err}")),
        // No specific Office variant; route everything else to FolioError on the JS side.
        _ => (Status::GenericFailure, err.to_string()),
    };
    Error::new(status, msg)
}

/// Convert a [`ChromeFetchError`] to a napi [`Error`] with a tagged message.
#[cfg(feature = "chrome-fetch")]
pub fn fetch_to_napi(err: engine::chrome_fetch::ChromeFetchError) -> Error {
    use engine::chrome_fetch::ChromeFetchError as E;
    let prefix = match err {
        E::NotFoundAndDownloadDisabled => "[ChromeNotFound]",
        _ => "[ChromeFetch]",
    };
    Error::new(Status::GenericFailure, format!("{prefix} {err}"))
}
