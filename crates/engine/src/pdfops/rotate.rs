//! `rotate` — rotate selected pages by 90° increments.
//!
//! For each targeted page, the new `/Rotate` value is computed as
//! `(existing + angle).rem_euclid(360)`. The lookup of `existing` is on
//! the page leaf only — inherited rotations from the page tree are not
//! resolved here (MVP scope; documented in spec 13's edge cases).

use lopdf::Object;

use crate::types::{EngineError, EngineResult, PageRanges};

/// Rotate selected pages by `0/90/180/270` degrees clockwise.
///
/// Negative angles and `360` are normalised via `rem_euclid(360)` so e.g.
/// `-90 == 270` and `360 == 0` (no-op rewrite that still re-saves).
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

    let mut doc = super::parse_input(pdf)?;
    let total = doc.get_pages().len() as u32;
    let targets: Vec<(u32, lopdf::ObjectId)> = doc
        .get_pages()
        .into_iter()
        .filter(|(p, _)| pages.contains(*p, total))
        .collect();

    for (_, page_id) in targets {
        let current = current_rotate(&doc, page_id);
        let new_rot = (current + normalised).rem_euclid(360);
        if let Ok(Object::Dictionary(d)) = doc.get_object_mut(page_id) {
            d.set("Rotate", i64::from(new_rot));
        }
    }

    super::finalize(doc)
}

/// Read the leaf page's `/Rotate` value, defaulting to `0`. Does not
/// resolve rotations inherited from `/Pages` ancestors.
fn current_rotate(doc: &lopdf::Document, page_id: lopdf::ObjectId) -> i32 {
    let Ok(Object::Dictionary(d)) = doc.get_object(page_id) else {
        return 0;
    };
    d.get(b"Rotate")
        .ok()
        .and_then(|o| o.as_i64().ok())
        .map(|n| (n as i32).rem_euclid(360))
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pdfops::test_support::{make_blank_pdf, make_multipage_pdf};
    use lopdf::Document;

    fn rotate_of(pdf: &[u8], page_num: u32) -> Option<i64> {
        let doc = Document::load_mem(pdf).unwrap();
        let pages = doc.get_pages();
        let id = *pages.get(&page_num)?;
        let dict = doc.get_object(id).unwrap().as_dict().unwrap();
        dict.get(b"Rotate").ok().and_then(|o| o.as_i64().ok())
    }

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

    #[test]
    fn rotate_accepts_normalised_angles() {
        let pdf = make_blank_pdf();
        let pages = PageRanges::parse("1").unwrap();
        for ok in [0, 90, 180, 270, 360, -90, -180, -270, 720] {
            assert!(
                rotate(&pdf, &pages, ok).is_ok(),
                "expected ok for angle {ok}"
            );
        }
    }

    #[test]
    fn rotate_normalizes_360_to_0_noop() {
        let pdf = make_blank_pdf();
        let pages = PageRanges::parse("1").unwrap();
        let out = rotate(&pdf, &pages, 360).unwrap();
        // /Rotate of 0 is permitted to be present-with-zero or absent;
        // the no-op result must at least round-trip.
        let r = rotate_of(&out, 1).unwrap_or(0);
        assert_eq!(r % 360, 0);
    }

    #[test]
    fn rotate_only_targeted_pages() {
        let pdf = make_multipage_pdf(3, 612, 792);
        let pages = PageRanges::parse("1,3").unwrap();
        let out = rotate(&pdf, &pages, 90).unwrap();
        assert_eq!(rotate_of(&out, 1), Some(90));
        assert_eq!(rotate_of(&out, 2), None);
        assert_eq!(rotate_of(&out, 3), Some(90));
    }

    #[test]
    fn rotate_accumulates_with_existing() {
        let pdf = make_blank_pdf();
        let pages = PageRanges::parse("1").unwrap();
        let once = rotate(&pdf, &pages, 90).unwrap();
        assert_eq!(rotate_of(&once, 1), Some(90));
        let twice = rotate(&once, &pages, 90).unwrap();
        assert_eq!(rotate_of(&twice, 1), Some(180));
        let thrice = rotate(&twice, &pages, 90).unwrap();
        assert_eq!(rotate_of(&thrice, 1), Some(270));
        let four = rotate(&thrice, &pages, 90).unwrap();
        assert_eq!(rotate_of(&four, 1), Some(0));
    }

    #[test]
    fn rotate_negative_angle_normalises() {
        let pdf = make_blank_pdf();
        let pages = PageRanges::parse("1").unwrap();
        let out = rotate(&pdf, &pages, -90).unwrap();
        assert_eq!(rotate_of(&out, 1), Some(270));
    }
}
