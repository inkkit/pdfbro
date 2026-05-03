//! Integration tests for `engine::pdfops`.
//!
//! These exercise the public surface end-to-end with PDFs built in memory
//! (no external fixture files required). Property tests verify
//! roundtrip invariants between `split` and `merge`.
//!
//! Implementation of spec 13's "Integration tests" + "Property tests"
//! sections — see `docs/specs/13-engine-pdfops.md`.

use engine::pdfops::{Metadata, SplitMode};
use engine::{PageRanges, flatten, merge, read_metadata, rotate, split, watermark, write_metadata};
use engine::{Position, WatermarkKind, WatermarkOptions};
use lopdf::{Document, Object, dictionary};
use proptest::prelude::*;

// ---------------------------------------------------------------------------
// Local PDF builders. These mirror the engine's in-source `test_support`
// helpers; we duplicate them here because `#[cfg(test)]` modules from a
// crate aren't visible to that crate's integration tests.
// ---------------------------------------------------------------------------

fn make_multipage_pdf(num_pages: u32) -> Vec<u8> {
    let mut doc = Document::with_version("1.5");
    let pages_id = doc.new_object_id();
    let resources_id = doc.add_object(dictionary! {});

    let mut kids = Vec::with_capacity(num_pages as usize);
    for _ in 0..num_pages {
        let content_id = doc.add_object(lopdf::Stream::new(dictionary! {}, Vec::new()));
        let page_id = doc.add_object(dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
            "Resources" => resources_id,
            "Contents" => content_id,
        });
        kids.push(Object::Reference(page_id));
    }

    doc.objects.insert(
        pages_id,
        Object::Dictionary(dictionary! {
            "Type" => "Pages",
            "Kids" => kids,
            "Count" => num_pages,
        }),
    );

    let catalog_id = doc.add_object(dictionary! {
        "Type" => "Catalog",
        "Pages" => pages_id,
    });
    doc.trailer.set("Root", catalog_id);

    let mut bytes = Vec::new();
    doc.save_to(&mut bytes).expect("save multipage pdf");
    bytes
}

fn page_count(pdf: &[u8]) -> usize {
    Document::load_mem(pdf).unwrap().get_pages().len()
}

fn producer_of(pdf: &[u8]) -> String {
    let doc = Document::load_mem(pdf).unwrap();
    let info_id = doc.trailer.get(b"Info").unwrap().as_reference().unwrap();
    let info = doc.get_object(info_id).unwrap().as_dict().unwrap();
    let bytes = match info.get(b"Producer").unwrap() {
        Object::String(b, _) => b.clone(),
        other => panic!("unexpected producer: {other:?}"),
    };
    String::from_utf8(bytes).unwrap()
}

// ---------------------------------------------------------------------------
// End-to-end smoke tests: every public op stamps /Producer and accepts the
// outputs of every other op as valid input.
// ---------------------------------------------------------------------------

#[test]
fn every_op_stamps_producer_and_accepts_others_output() {
    let base = make_multipage_pdf(3);
    let producer_prefix = "pdfbro/";

    let after_merge = merge(&[&base, &base]).unwrap();
    assert!(producer_of(&after_merge).starts_with(producer_prefix));
    assert_eq!(page_count(&after_merge), 6);

    let after_split = &split(&after_merge, &SplitMode::EveryN(2)).unwrap()[0];
    assert!(producer_of(after_split).starts_with(producer_prefix));

    let after_rotate = rotate(after_split, &PageRanges::parse("1").unwrap(), 90).unwrap();
    assert!(producer_of(&after_rotate).starts_with(producer_prefix));

    let after_flatten = flatten(&after_rotate).unwrap();
    assert!(producer_of(&after_flatten).starts_with(producer_prefix));

    let after_meta = write_metadata(
        &after_flatten,
        &Metadata {
            title: Some("Spec-13 Integration".into()),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(
        read_metadata(&after_meta).unwrap().title.as_deref(),
        Some("Spec-13 Integration"),
    );

    let after_watermark = watermark(
        &after_meta,
        &WatermarkOptions {
            kind: WatermarkKind::Text {
                text: "DRAFT".into(),
                font: None,
                font_size: 64.0,
                color: [0.6, 0.0, 0.0, 1.0],
            },
            opacity: 0.4,
            rotation_deg: 30.0,
            position: Position::Center,
            all_pages: true,
            tiled: false,
        },
    )
    .unwrap();
    assert!(producer_of(&after_watermark).starts_with(producer_prefix));
}

#[test]
fn merge_then_split_recovers_originals_by_page() {
    let a = make_multipage_pdf(2);
    let b = make_multipage_pdf(3);
    let merged = merge(&[&a, &b]).unwrap();
    assert_eq!(page_count(&merged), 5);

    // Split back into 2- and 3-page chunks.
    let chunks = split(
        &merged,
        &SplitMode::ByRanges(vec![
            PageRanges::parse("1-2").unwrap(),
            PageRanges::parse("3-").unwrap(),
        ]),
    )
    .unwrap();
    assert_eq!(chunks.len(), 2);
    assert_eq!(page_count(&chunks[0]), 2);
    assert_eq!(page_count(&chunks[1]), 3);
}

// ---------------------------------------------------------------------------
// Property tests
// ---------------------------------------------------------------------------

proptest! {
    /// For any small page count, splitting `EveryN` and merging the
    /// resulting chunks must produce a document with the same page count.
    #[test]
    fn split_every_n_then_merge_preserves_page_count(
        pages in 1u32..=8,
        n in 1u32..=4,
    ) {
        let pdf = make_multipage_pdf(pages);
        let chunks = split(&pdf, &SplitMode::EveryN(n)).unwrap();

        let chunk_refs: Vec<&[u8]> = chunks.iter().map(|c| c.as_slice()).collect();
        let merged = merge(&chunk_refs).unwrap();
        prop_assert_eq!(page_count(&merged), pages as usize);
    }

    /// Merging a list of valid PDFs must produce a document with page
    /// count equal to the sum of input page counts.
    #[test]
    fn merge_page_count_is_sum_of_inputs(
        sizes in proptest::collection::vec(1u32..=5, 1..=4),
    ) {
        let pdfs: Vec<Vec<u8>> = sizes.iter().map(|&n| make_multipage_pdf(n)).collect();
        let refs: Vec<&[u8]> = pdfs.iter().map(|p| p.as_slice()).collect();
        let merged = merge(&refs).unwrap();
        let expected: u32 = sizes.iter().sum();
        prop_assert_eq!(page_count(&merged), expected as usize);
    }

    /// Two groupings of three inputs ((a∘b)∘c vs a∘(b∘c)) must yield the
    /// same page count and order — i.e. `merge` is associative on counts.
    #[test]
    fn merge_associative_for_two_groupings(
        sizes in (1u32..=3, 1u32..=3, 1u32..=3),
    ) {
        let (sa, sb, sc) = sizes;
        let a = make_multipage_pdf(sa);
        let b = make_multipage_pdf(sb);
        let c = make_multipage_pdf(sc);

        let left = merge(&[&merge(&[&a, &b]).unwrap(), &c]).unwrap();
        let right = merge(&[&a, &merge(&[&b, &c]).unwrap()]).unwrap();
        prop_assert_eq!(page_count(&left), page_count(&right));
        prop_assert_eq!(page_count(&left), (sa + sb + sc) as usize);
    }
}

// ---------------------------------------------------------------------------
// Merge + Metadata round-trip using real testdata fixtures
// ---------------------------------------------------------------------------

#[test]
#[ignore = "requires testdata fixture files"]
fn read_metadata_from_current_teststore_foo_pdf() {
    // Read the current teststore/foo.pdf and dump its metadata to see what's in it.
    let pdf = std::fs::read("../server/tests/bdd/testdata/teststore/foo.pdf")
        .expect("teststore/foo.pdf must exist");
    let meta = engine::read_metadata(&pdf).unwrap();
    println!("Metadata from teststore/foo.pdf:");
    println!("  author: {:?}", meta.author);
    println!("  title: {:?}", meta.title);
    println!("  creator: {:?}", meta.creator);
    println!("  producer: {:?}", meta.producer);
    println!("  custom: {:?}", meta.custom);
    // This test just dumps info; it always passes.
}

#[test]
#[ignore = "requires testdata fixture files"]
fn merge_then_write_metadata_round_trips_author_with_real_pdfs() {
    // Uses the actual page_1.pdf / page_2.pdf from the BDD testdata folder
    // to reproduce the exact scenario that fails in the BDD tests.
    let pdf1 = std::fs::read("../server/tests/bdd/testdata/page_1.pdf")
        .expect("page_1.pdf must exist at crates/server/tests/bdd/testdata/page_1.pdf");
    let pdf2 = std::fs::read("../server/tests/bdd/testdata/page_2.pdf")
        .expect("page_2.pdf must exist");
    let merged = merge(&[&pdf1, &pdf2]).unwrap();

    let mut meta = Metadata::default();
    meta.author = Some("Julien Neuhart".into());

    let with_meta = write_metadata(&merged, &meta).unwrap();
    let read_back = read_metadata(&with_meta).unwrap();

    assert_eq!(read_back.author.as_deref(), Some("Julien Neuhart"),
        "Author should round-trip when merging real page_1/page_2 PDFs");
}

// ---------------------------------------------------------------------------
// Merge + Metadata round-trip
// ---------------------------------------------------------------------------

#[test]
fn merge_then_write_metadata_round_trips_author() {
    let pdf1 = make_multipage_pdf(1);
    let pdf2 = make_multipage_pdf(1);
    let merged = merge(&[&pdf1, &pdf2]).unwrap();

    let mut meta = Metadata::default();
    meta.author = Some("Julien Neuhart".into());
    meta.title = Some("Sample".into());

    let with_meta = write_metadata(&merged, &meta).unwrap();
    let read_back = read_metadata(&with_meta).unwrap();

    assert_eq!(read_back.author.as_deref(), Some("Julien Neuhart"));
    assert_eq!(read_back.title.as_deref(), Some("Sample"));
}

// ---------------------------------------------------------------------------
// Merge + Bookmarks round-trip
// ---------------------------------------------------------------------------

#[test]
fn merge_then_write_bookmarks_round_trips() {
    use engine::bookmarks::{read_bookmarks, write_bookmarks, Bookmark};

    let pdf1 = make_multipage_pdf(1);
    let pdf2 = make_multipage_pdf(1);
    let merged = merge(&[&pdf1, &pdf2]).unwrap();

    let bookmarks = vec![Bookmark {
        title: "Merged Index".into(),
        page: 1,
        children: vec![],
    }];

    let with_bm = write_bookmarks(&merged, &bookmarks).unwrap();
    let read_back = read_bookmarks(&with_bm).unwrap();

    assert_eq!(read_back.len(), 1);
    assert_eq!(read_back[0].title, "Merged Index");
    assert_eq!(read_back[0].page, 1);
}

// ---------------------------------------------------------------------------
// Test: exact BDD metadata JSON round-trip
// ---------------------------------------------------------------------------

#[test]
fn bdd_metadata_json_deserializes_and_round_trips_author() {
    // Uses the EXACT metadata JSON from the BDD test
    let meta_json = r#"{"Author":"Julien Neuhart","Copyright":"Julien Neuhart","CreateDate":"2006-09-18T16:27:50-04:00","Creator":"Gotenberg","Keywords":["first","second"],"Marked":true,"ModDate":"2006-09-18T16:27:50-04:00","PDFVersion":1.7,"Producer":"Gotenberg","Subject":"Sample","Title":"Sample","Trapped":"Unknown"}"#;
    let meta: Metadata = serde_json::from_str(meta_json).expect("JSON should parse");
    
    // Verify deserialization
    assert_eq!(meta.author.as_deref(), Some("Julien Neuhart"), "Author should deserialize");
    assert_eq!(meta.title.as_deref(), Some("Sample"), "Title should deserialize");
    assert_eq!(meta.creator.as_deref(), Some("Gotenberg"), "Creator should deserialize");
    
    // Now do the full round-trip with real PDFs
    let pdf1 = make_multipage_pdf(1);
    let pdf2 = make_multipage_pdf(1);
    let merged = merge(&[&pdf1, &pdf2]).unwrap();
    let with_meta = write_metadata(&merged, &meta).unwrap();
    let read_back = read_metadata(&with_meta).unwrap();
    
    assert_eq!(read_back.author.as_deref(), Some("Julien Neuhart"),
        "Author should survive merge+write_metadata+read_metadata with BDD metadata JSON");
    assert_eq!(read_back.title.as_deref(), Some("Sample"),
        "Title should survive");
}
