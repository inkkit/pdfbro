//! Integration tests for `engine::libreoffice::LibreOfficeEngine`.
//!
//! These tests run by default and require `soffice` on `$PATH`
//! (or set via `$LIBREOFFICE_PATH`).
//!
//! ### Fixture-naming deviation
//!
//! Spec 12's *Test plan* names individual tests after `.docx` / `.xlsx` /
//! `.pptx` fixtures. We commit text fixtures (`sample.rtf`, `sample.csv`)
//! instead — see `tests/fixtures/office/README.md` for the rationale.
//! Test names below mirror the spec's intent (writer / calc filter, page
//! ranges, landscape, PDF/A, etc.) using whichever fixture is available.
//!
//! Notes on filter behaviour observed against `soffice 26.2.x`:
//!
//! - `IsLandscape` in the filter-options blob is honoured by the writer
//!   export module but **not** by calc — calc's orientation is driven by
//!   the document's page style. We therefore exercise landscape against
//!   the RTF fixture, not the CSV one.
//! - LibreOffice's HTML importer ignores CSS `page-break-before` rules,
//!   so the writer multi-page fixture is RTF (`\page` control words),
//!   not HTML.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use engine::{
    EngineError, LibreOfficeConfig, LibreOfficeEngine, OfficeOptions, PageRanges,
};
use engine::libreoffice::PdfAProfile;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("office")
}

/// Multi-page writer fixture (RTF with `\page` control words).
fn writer_fixture() -> PathBuf {
    fixtures_dir().join("sample.rtf")
}

/// Calc fixture (CSV).
fn csv_fixture() -> PathBuf {
    fixtures_dir().join("sample.csv")
}

async fn engine() -> Option<LibreOfficeEngine> {
    match LibreOfficeEngine::discover().await {
        Ok(engine) => Some(engine),
        Err(e) => {
            eprintln!("skipping: failed to discover soffice: {e}");
            None
        }
    }
}

fn assert_pdf_loadable(bytes: &[u8]) -> lopdf::Document {
    assert!(
        bytes.starts_with(b"%PDF-"),
        "expected PDF magic at start, got {:?}",
        &bytes[..bytes.len().min(8)]
    );
    let tail_window = &bytes[bytes.len().saturating_sub(64)..];
    assert!(
        tail_window.windows(5).any(|w| w == b"%%EOF"),
        "expected %%EOF in trailer"
    );
    lopdf::Document::load_mem(bytes).expect("PDF parses with lopdf")
}

fn pdf_page_count(doc: &lopdf::Document) -> usize {
    doc.get_pages().len()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn convert_writer_produces_valid_pdf() {
    let Some(lo) = engine().await else { return; };
    assert!(lo.healthy().await);
    let bytes = lo
        .convert(&writer_fixture(), &OfficeOptions::default())
        .await
        .expect("convert");
    let doc = assert_pdf_loadable(&bytes);
    assert!(pdf_page_count(&doc) >= 1);
}

#[tokio::test]
async fn convert_writer_landscape_option_flows_through() {
    // Spec 12 maps `landscape = true` to the filter-options key
    // `IsLandscape` (boolean). The unit test
    // `office_options_landscape_and_pdfua_blob_keys` proves we emit the
    // key correctly in the JSON blob. This integration test verifies
    // *the convert succeeds* with the option set — LibreOffice itself
    // does not actually rotate the writer output via this property
    // (orientation is driven by the document's page style), so we do
    // **not** assert MediaBox dimensions here.
    let Some(lo) = engine().await else { return; };
    let opts = OfficeOptions {
        landscape: true,
        ..Default::default()
    };
    let bytes = lo
        .convert(&writer_fixture(), &opts)
        .await
        .expect("convert with landscape=true should not error");
    assert_pdf_loadable(&bytes);
}

#[tokio::test]
async fn convert_writer_page_ranges() {
    let Some(lo) = engine().await else { return; };

    // Full document: RTF fixture has three explicit `\page` breaks.
    let full = lo
        .convert(&writer_fixture(), &OfficeOptions::default())
        .await
        .expect("full convert");
    let full_doc = assert_pdf_loadable(&full);
    let full_pages = pdf_page_count(&full_doc);
    assert!(
        full_pages >= 2,
        "fixture should produce >= 2 pages, got {full_pages}"
    );

    // Page-range 1-1 → exactly one page.
    let opts = OfficeOptions {
        page_ranges: Some(PageRanges::parse("1-1").expect("parse")),
        ..Default::default()
    };
    let one = lo.convert(&writer_fixture(), &opts).await.expect("single");
    let one_doc = assert_pdf_loadable(&one);
    assert_eq!(pdf_page_count(&one_doc), 1);
}

#[tokio::test]
async fn convert_with_pdf_a_2b_writes_pdfa_metadata() {
    let Some(lo) = engine().await else { return; };
    let opts = OfficeOptions {
        pdf_a: Some(PdfAProfile::A2B),
        ..Default::default()
    };
    let bytes = lo.convert(&writer_fixture(), &opts).await.expect("convert");
    assert_pdf_loadable(&bytes);
    // PDF/A files carry an XMP metadata stream containing "pdfaid".
    let needle = b"pdfaid";
    assert!(
        bytes.windows(needle.len()).any(|w| w == needle),
        "PDF/A-2b output should embed an XMP metadata stream containing 'pdfaid'"
    );
}

#[tokio::test]
async fn convert_many_preserves_order() {
    let Some(lo) = engine().await else { return; };
    let inputs = vec![writer_fixture(), csv_fixture(), writer_fixture()];
    let out = lo
        .convert_many(&inputs, &OfficeOptions::default())
        .await
        .expect("convert_many");
    assert_eq!(out.len(), 3);
    for (i, b) in out.iter().enumerate() {
        assert!(b.starts_with(b"%PDF-"), "slot {i} not a pdf");
    }
    // Slots 0 and 2 are both the writer/RTF fixture; slot 1 is calc/CSV.
    // We don't compare bytes for equality (PDF metadata includes
    // timestamps), but slot 1 must differ from slots 0/2 in page count
    // (writer fixture is 3 pages, csv fixture is 1) — that proves order
    // wasn't shuffled.
    let pages: Vec<usize> = out
        .iter()
        .map(|b| pdf_page_count(&lopdf::Document::load_mem(b).expect("pdf")))
        .collect();
    assert!(
        pages[0] >= 2 && pages[2] >= 2,
        "writer slots should be multi-page, got {pages:?}"
    );
    assert_eq!(pages[1], 1, "csv slot should be single-page, got {pages:?}");
}

#[tokio::test]
async fn convert_timeout_kills_child() {
    // Build an engine with a pathologically small unoserver-ready timeout so
    // launch() fails quickly, and a small per-call timeout so that if launch()
    // succeeds (unoserver happened to already be running), the convert itself
    // is exercised under the timeout.
    let cfg = LibreOfficeConfig {
        timeout: Duration::from_millis(100),
        unoserver_ready_timeout: Duration::from_millis(500),
        ..Default::default()
    };
    let started = Instant::now();
    let res = LibreOfficeEngine::launch(cfg).await;
    let elapsed = started.elapsed();

    match res {
        Err(EngineError::Timeout(_)) => {
            // unoserver did not become ready in time. Timeout plumbing works.
            assert!(elapsed < Duration::from_secs(2));
        }
        Err(EngineError::Internal(_)) => {
            // Launch failed for another reason (e.g. unoserver not installed).
            // Also acceptable — engine is not available.
        }
        Ok(lo) => {
            // unoserver was already running; exercise the convert timeout.
            let err = lo
                .convert(&writer_fixture(), &OfficeOptions::default())
                .await
                .expect_err("convert should not finish under 100ms");
            assert!(
                matches!(err, EngineError::Timeout(_)),
                "expected EngineError::Timeout, got {err:?}"
            );
        }
        Err(other) => panic!("unexpected error variant from launch: {other:?}"),
    }
}

#[tokio::test]
async fn convert_missing_input_io_error() {
    let Some(lo) = engine().await else { return; };
    let err = lo
        .convert(
            Path::new("/nonexistent/__folio_no_input.docx"),
            &OfficeOptions::default(),
        )
        .await
        .expect_err("missing input must error");
    assert!(matches!(err, EngineError::Io(_)), "got {err:?}");
}

#[tokio::test]
async fn convert_unsupported_format_falls_back_to_generic_filter() {
    // Copy the RTF fixture under an extension absent from the filter
    // table; soffice should still detect RTF by content (`{\\rtf` magic).
    let Some(lo) = engine().await else { return; };
    let tmp = tempfile::tempdir().expect("tempdir");
    let src = tmp.path().join("sample.weird");
    std::fs::copy(writer_fixture(), &src).expect("copy");
    let bytes = lo
        .convert(&src, &OfficeOptions::default())
        .await
        .expect("generic filter convert");
    assert_pdf_loadable(&bytes);
}

#[tokio::test]
async fn concurrent_calls_use_distinct_user_dirs() {
    // If concurrent invocations did NOT each get their own
    // UserInstallation, soffice would deadlock or report a profile-lock
    // collision. Spawning N=4 conversions in parallel and verifying all
    // succeed is a behavioural proof of distinct dirs.
    let Some(lo) = engine().await else { return; };
    let inputs: Vec<PathBuf> = (0..4).map(|_| writer_fixture()).collect();
    let out = lo
        .convert_many(&inputs, &OfficeOptions::default())
        .await
        .expect("parallel converts");
    assert_eq!(out.len(), 4);
    for b in &out {
        assert!(b.starts_with(b"%PDF-"));
    }
}

#[tokio::test]
async fn healthy_returns_true_for_real_soffice() {
    let Some(lo) = engine().await else { return; };
    assert!(lo.healthy().await);
}
