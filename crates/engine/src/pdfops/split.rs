//! `split` — produce one or more PDFs from a single source.
//!
//! For each requested chunk we clone the parsed `Document`, call
//! `delete_pages` with every page _not_ in the chunk, and finalize the
//! result through the common pipeline (producer stamp, compress, save).
//! Empty resolved chunks (after page-range clamping) are skipped so the
//! function never produces an empty PDF.

use std::collections::BTreeSet;

use crate::types::{EngineError, EngineResult, PageRange, PageRanges};

/// How `split` should partition the source document.
#[derive(Debug, Clone)]
pub enum SplitMode {
    /// One output PDF per `PageRanges` chunk, in order. Pages absent from
    /// every chunk are dropped. Empty resolved chunks are skipped.
    ByRanges(Vec<PageRanges>),
    /// Split every `N` pages, in order. Last chunk may be shorter.
    EveryN(u32),
    /// One output PDF per single page.
    OnePagePerFile,
}

/// Split `pdf` according to `mode`, returning one byte vector per output
/// document in generation order.
///
/// # Errors
///
/// - [`EngineError::InvalidOption`] if `mode` is `EveryN(0)`.
/// - [`EngineError::Internal`] if the input fails to parse, is encrypted,
///   or any chunk fails to save.
pub fn split(pdf: &[u8], mode: &SplitMode) -> EngineResult<Vec<Vec<u8>>> {
    if let SplitMode::EveryN(0) = mode {
        return Err(EngineError::InvalidOption("EveryN requires N >= 1".into()));
    }
    let original = super::parse_input(pdf)?;
    let total = original.get_pages().len() as u32;

    let chunks: Vec<Vec<u32>> = match mode {
        SplitMode::ByRanges(ranges_per_chunk) => ranges_per_chunk
            .iter()
            .map(|r| pages_for_ranges(r, total))
            .filter(|pages| !pages.is_empty())
            .collect(),
        SplitMode::EveryN(n) => every_n_chunks(total, *n),
        SplitMode::OnePagePerFile => (1..=total).map(|p| vec![p]).collect(),
    };

    let mut out = Vec::with_capacity(chunks.len());
    for keep in chunks {
        let keep_set: BTreeSet<u32> = keep.into_iter().collect();
        let to_remove: Vec<u32> = (1..=total).filter(|p| !keep_set.contains(p)).collect();
        let mut doc = original.clone();
        if !to_remove.is_empty() {
            doc.delete_pages(&to_remove);
        }
        out.push(super::finalize(doc)?);
    }
    Ok(out)
}

/// Resolve a [`PageRanges`] expression into a sorted, deduplicated list of
/// 1-indexed page numbers, clamped to `1..=total`.
fn pages_for_ranges(ranges: &PageRanges, total: u32) -> Vec<u32> {
    if total == 0 {
        return Vec::new();
    }
    let mut pages: BTreeSet<u32> = BTreeSet::new();
    for r in ranges.as_slice() {
        match *r {
            PageRange::Single(n) => {
                if (1..=total).contains(&n) {
                    pages.insert(n);
                }
            }
            PageRange::Closed(a, b) => {
                let lo = a.max(1);
                let hi = b.min(total);
                if lo <= hi {
                    for p in lo..=hi {
                        pages.insert(p);
                    }
                }
            }
            PageRange::OpenEnd(a) => {
                let lo = a.max(1);
                for p in lo..=total {
                    pages.insert(p);
                }
            }
        }
    }
    pages.into_iter().collect()
}

/// Build chunks of size at most `n` covering the closed range `1..=total`.
fn every_n_chunks(total: u32, n: u32) -> Vec<Vec<u32>> {
    if total == 0 || n == 0 {
        return Vec::new();
    }
    let mut chunks = Vec::new();
    let mut start = 1u32;
    while start <= total {
        let end = (start + n - 1).min(total);
        chunks.push((start..=end).collect());
        start = end + 1;
    }
    chunks
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pdfops::test_support::{make_blank_pdf, make_multipage_pdf};
    use lopdf::Document;

    fn page_count(pdf: &[u8]) -> usize {
        Document::load_mem(pdf).unwrap().get_pages().len()
    }

    #[test]
    fn split_every_n_zero_rejected() {
        let pdf = make_blank_pdf();
        let err = split(&pdf, &SplitMode::EveryN(0)).unwrap_err();
        assert!(matches!(err, EngineError::InvalidOption(_)));
    }

    #[test]
    fn split_every_n_clamps_when_total_smaller_than_n() {
        let pdf = make_multipage_pdf(3, 612, 792);
        let chunks = split(&pdf, &SplitMode::EveryN(7)).unwrap();
        assert_eq!(chunks.len(), 1);
        assert_eq!(page_count(&chunks[0]), 3);
    }

    #[test]
    fn split_every_n_yields_expected_counts() {
        let pdf = make_multipage_pdf(3, 612, 792);
        let chunks = split(&pdf, &SplitMode::EveryN(2)).unwrap();
        assert_eq!(chunks.len(), 2);
        assert_eq!(page_count(&chunks[0]), 2);
        assert_eq!(page_count(&chunks[1]), 1);
    }

    #[test]
    fn split_one_page_per_file() {
        let pdf = make_multipage_pdf(4, 612, 792);
        let chunks = split(&pdf, &SplitMode::OnePagePerFile).unwrap();
        assert_eq!(chunks.len(), 4);
        for c in &chunks {
            assert_eq!(page_count(c), 1);
        }
    }

    #[test]
    fn split_by_ranges_clamps_to_total() {
        let pdf = make_multipage_pdf(3, 612, 792);
        let r = PageRanges::parse("1-1000").unwrap();
        let chunks = split(&pdf, &SplitMode::ByRanges(vec![r])).unwrap();
        assert_eq!(chunks.len(), 1);
        assert_eq!(page_count(&chunks[0]), 3);
    }

    #[test]
    fn split_by_ranges_skips_empty_chunks() {
        let pdf = make_multipage_pdf(3, 612, 792);
        let chunks = split(
            &pdf,
            &SplitMode::ByRanges(vec![PageRanges::parse("5-10").unwrap()]),
        )
        .unwrap();
        assert!(chunks.is_empty());
    }

    #[test]
    fn split_by_ranges_extracts_specific_pages() {
        let pdf = make_multipage_pdf(5, 612, 792);
        let chunks = split(
            &pdf,
            &SplitMode::ByRanges(vec![
                PageRanges::parse("1,3").unwrap(),
                PageRanges::parse("4-").unwrap(),
            ]),
        )
        .unwrap();
        assert_eq!(chunks.len(), 2);
        assert_eq!(page_count(&chunks[0]), 2); // pages 1 and 3
        assert_eq!(page_count(&chunks[1]), 2); // pages 4 and 5
    }

    #[test]
    fn pages_for_ranges_dedupes_and_clamps() {
        let r = PageRanges::parse("1-3,2,3-,7").unwrap();
        let pages = pages_for_ranges(&r, 5);
        assert_eq!(pages, vec![1, 2, 3, 4, 5]);
    }

    #[test]
    fn pages_for_ranges_empty_when_total_zero() {
        let r = PageRanges::parse("1-3").unwrap();
        assert!(pages_for_ranges(&r, 0).is_empty());
    }

    #[test]
    fn every_n_chunks_basic() {
        assert_eq!(every_n_chunks(0, 3), Vec::<Vec<u32>>::new());
        assert_eq!(every_n_chunks(5, 2), vec![vec![1, 2], vec![3, 4], vec![5]]);
        assert_eq!(every_n_chunks(3, 1), vec![vec![1], vec![2], vec![3]]);
    }
}
