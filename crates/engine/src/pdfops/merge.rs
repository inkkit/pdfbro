//! `merge` — concatenate a sequence of PDFs preserving order.
//!
//! Implementation deferred to a follow-up commit; the public surface is
//! present so the encrypted-input gate is uniform across all ops.

use crate::types::{EngineError, EngineResult};

/// Concatenate a sequence of PDFs into a single document, preserving order.
///
/// # Errors
///
/// - [`EngineError::InvalidOption`] if `pdfs` is empty.
/// - [`EngineError::Internal`] if any input fails to parse, is encrypted,
///   or the merged document fails to save.
pub fn merge(pdfs: &[&[u8]]) -> EngineResult<Vec<u8>> {
    if pdfs.is_empty() {
        return Err(EngineError::InvalidOption(
            "merge requires at least one input".into(),
        ));
    }
    // Validate each input through the encrypted-rejection gate before
    // erroring out as not-yet-implemented. This makes the encrypted-input
    // rejection rule uniform across every public function from day one.
    for (idx, &bytes) in pdfs.iter().enumerate() {
        super::parse_input(bytes).map_err(|e| match e {
            EngineError::Internal(msg) => {
                EngineError::Internal(format!("merge: input #{}: {msg}", idx + 1))
            }
            other => other,
        })?;
    }
    Err(EngineError::Internal("merge: not yet implemented".into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merge_empty_input_rejected() {
        let err = merge(&[]).unwrap_err();
        assert!(matches!(err, EngineError::InvalidOption(_)));
    }
}
