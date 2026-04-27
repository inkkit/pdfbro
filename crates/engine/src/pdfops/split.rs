//! `split` — produce one or more PDFs from a single source.

use crate::types::{EngineError, EngineResult, PageRanges};

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
/// document, in generation order.
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
    super::parse_input(pdf)?;
    Err(EngineError::Internal("split: not yet implemented".into()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pdfops::test_support::make_blank_pdf;

    #[test]
    fn split_every_n_zero_rejected() {
        let pdf = make_blank_pdf();
        let err = split(&pdf, &SplitMode::EveryN(0)).unwrap_err();
        assert!(matches!(err, EngineError::InvalidOption(_)));
    }
}
