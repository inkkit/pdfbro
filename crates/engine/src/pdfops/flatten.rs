//! `flatten` — rasterise interactive form widgets and annotations into
//! static page content.

use crate::types::{EngineError, EngineResult};

/// Flatten interactive form fields and annotations into static page
/// content. Idempotent on already-flat PDFs.
///
/// # Errors
///
/// [`EngineError::Internal`] if the input fails to parse, is encrypted,
/// or the result fails to save.
pub fn flatten(pdf: &[u8]) -> EngineResult<Vec<u8>> {
    super::parse_input(pdf)?;
    Err(EngineError::Internal("flatten: not yet implemented".into()))
}
