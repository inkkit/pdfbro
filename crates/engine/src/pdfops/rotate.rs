//! `rotate` — rotate selected pages by 90° increments.

use crate::types::{EngineError, EngineResult, PageRanges};

/// Rotate pages by `0/90/180/270` degrees clockwise.
///
/// Other angles return [`EngineError::InvalidOption`]. Negative angles and
/// `360` are normalised via `rem_euclid(360)` so e.g. `-90 == 270` and
/// `360 == 0` (no-op rewrite).
///
/// # Errors
///
/// - [`EngineError::InvalidOption`] if the angle does not normalise to one
///   of `{0, 90, 180, 270}`.
/// - [`EngineError::Internal`] if the input fails to parse, is encrypted,
///   or the result fails to save.
pub fn rotate(pdf: &[u8], pages: &PageRanges, angle_deg: i32) -> EngineResult<Vec<u8>> {
    let normalised = angle_deg.rem_euclid(360);
    if !matches!(normalised, 0 | 90 | 180 | 270) {
        return Err(EngineError::InvalidOption(format!(
            "angle must be 0/90/180/270 (got {angle_deg})"
        )));
    }
    let _ = pages;
    super::parse_input(pdf)?;
    Err(EngineError::Internal("rotate: not yet implemented".into()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pdfops::test_support::make_blank_pdf;

    #[test]
    fn rotate_invalid_angle_rejected() {
        let pdf = make_blank_pdf();
        let pages = PageRanges::parse("1").unwrap();
        for bad in [45, 91, -45, 1, 359] {
            let err = rotate(&pdf, &pages, bad).unwrap_err();
            assert!(
                matches!(err, EngineError::InvalidOption(_)),
                "expected InvalidOption for angle {bad}, got {err:?}"
            );
        }
    }
}
