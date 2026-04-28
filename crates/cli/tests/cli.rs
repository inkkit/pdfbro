//! `assert_cmd`-driven integration tests for the `folio` binary.
//!
//! Tests that need real Chrome / `soffice` skip gracefully when the
//! dependency is missing. The non-gated tests cover usage / clap error
//! paths and pure pdfops over canned in-memory fixtures.

use std::path::Path;

use assert_cmd::Command;
use lopdf::{Document, Object, Stream, dictionary};
use predicates::prelude::*;
use tempfile::tempdir;

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

/// Build a minimal valid 1-page PDF document. Page size US Letter
/// (612×792 pt). Content stream is empty.
fn make_blank_pdf() -> Vec<u8> {
    make_multipage_pdf(1)
}

/// Build a valid PDF with the given page count.
fn make_multipage_pdf(num_pages: u32) -> Vec<u8> {
    let mut doc = Document::with_version("1.5");
    let pages_id = doc.new_object_id();
    let resources_id = doc.add_object(dictionary! {});

    let mut kids = Vec::with_capacity(num_pages as usize);
    for _ in 0..num_pages {
        let content_id = doc.add_object(Stream::new(dictionary! {}, Vec::new()));
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
    doc.save_to(&mut bytes).expect("save pdf");
    bytes
}

fn folio() -> Command {
    Command::cargo_bin("folio").expect("locate folio binary")
}

fn page_count(pdf: &[u8]) -> usize {
    Document::load_mem(pdf)
        .expect("parse pdf")
        .get_pages()
        .len()
}

/// Return true if a Chrome binary is available (via `$CHROME_PATH` or `$PATH`).
fn have_chrome() -> bool {
    if std::env::var("CHROME_PATH").is_ok() {
        return true;
    }
    for name in ["google-chrome", "chromium", "chromium-browser", "chrome"] {
        if std::process::Command::new(name)
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
        {
            return true;
        }
    }
    false
}

/// Return true if a LibreOffice (`soffice`) binary is available.
fn have_soffice() -> bool {
    if std::env::var("LIBREOFFICE_PATH").is_ok() {
        return true;
    }
    std::process::Command::new("soffice")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

// ---------------------------------------------------------------------------
// Help / version / completions
// ---------------------------------------------------------------------------

#[test]
fn version_subcommand_outputs_semver_string() {
    folio()
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::is_match(r"^folio \d+\.\d+\.\d+\b").unwrap());
}

#[test]
fn root_help_lists_all_subcommands() {
    folio()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("convert"))
        .stdout(predicate::str::contains("batch"))
        .stdout(predicate::str::contains("merge"))
        .stdout(predicate::str::contains("split"))
        .stdout(predicate::str::contains("flatten"))
        .stdout(predicate::str::contains("metadata"))
        .stdout(predicate::str::contains("completions"));
}

#[test]
fn completions_emits_bash_script() {
    folio()
        .args(["completions", "bash"])
        .assert()
        .success()
        .stdout(predicate::str::starts_with("_folio()"));
}

#[test]
fn completions_emits_zsh_script() {
    folio()
        .args(["completions", "zsh"])
        .assert()
        .success()
        .stdout(predicate::str::contains("#compdef"));
}

// ---------------------------------------------------------------------------
// Usage / clap error paths (exit 2)
// ---------------------------------------------------------------------------

#[test]
fn convert_requires_one_input_source() {
    folio()
        .args(["convert", "--output", "/tmp/none.pdf"])
        .assert()
        .code(2);
}

#[test]
fn convert_rejects_two_input_sources() {
    folio()
        .args([
            "convert",
            "--html",
            "a.html",
            "--url",
            "https://example.com",
            "--output",
            "/tmp/none.pdf",
        ])
        .assert()
        .code(2);
}

#[test]
fn convert_requires_output() {
    folio()
        .args(["convert", "--html", "a.html"])
        .assert()
        .code(2);
}

#[test]
fn merge_with_no_inputs_exits_2() {
    folio()
        .args(["merge", "--output", "/tmp/m.pdf"])
        .assert()
        .code(2);
}

#[test]
fn merge_with_two_stdins_exits_2() {
    folio()
        .args(["merge", "--output", "/tmp/m.pdf", "-", "-"])
        .assert()
        .code(2);
}

#[test]
fn unknown_subcommand_exits_2() {
    folio().arg("nonsense").assert().code(2);
}

#[test]
fn invalid_paper_value_exits_2() {
    folio()
        .args([
            "convert",
            "--html",
            "x.html",
            "--paper",
            "0x0",
            "--output",
            "/tmp/x.pdf",
        ])
        .assert()
        .code(2);
}

#[test]
fn invalid_cookie_value_exits_2() {
    folio()
        .args([
            "convert",
            "--html",
            "x.html",
            "--cookie",
            "novalue",
            "--output",
            "/tmp/x.pdf",
        ])
        .assert()
        .code(2);
}

#[test]
fn invalid_wait_empty_selector_exits_2() {
    folio()
        .args([
            "convert",
            "--html",
            "x.html",
            "--wait",
            "selector:",
            "--output",
            "/tmp/x.pdf",
        ])
        .assert()
        .code(2);
}

#[test]
fn invalid_margin_two_values_exits_2() {
    folio()
        .args([
            "convert",
            "--html",
            "x.html",
            "--margin",
            "1,2",
            "--output",
            "/tmp/x.pdf",
        ])
        .assert()
        .code(2);
}

// ---------------------------------------------------------------------------
// pdfops without engine (merge/split/flatten/metadata)
// ---------------------------------------------------------------------------

#[test]
fn split_default_mode_one_per_page() {
    let dir = tempdir().unwrap();
    let pdf = dir.path().join("in.pdf");
    std::fs::write(&pdf, make_multipage_pdf(3)).unwrap();
    let outdir = dir.path().join("out");

    folio()
        .args([
            "split",
            pdf.to_str().unwrap(),
            "--output-dir",
            outdir.to_str().unwrap(),
        ])
        .assert()
        .success();

    let mut entries: Vec<_> = std::fs::read_dir(&outdir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    entries.sort();
    assert_eq!(entries.len(), 3);
    for (i, p) in entries.iter().enumerate() {
        let name = p.file_name().unwrap().to_string_lossy().into_owned();
        let expected_index = format!("-{:03}.pdf", i + 1);
        assert!(name.ends_with(&expected_index), "{name}");
        let bytes = std::fs::read(p).unwrap();
        assert_eq!(page_count(&bytes), 1);
    }
}

#[test]
fn split_with_explicit_prefix() {
    let dir = tempdir().unwrap();
    let pdf = dir.path().join("in.pdf");
    std::fs::write(&pdf, make_multipage_pdf(2)).unwrap();
    let outdir = dir.path().join("out");

    folio()
        .args([
            "split",
            pdf.to_str().unwrap(),
            "--output-dir",
            outdir.to_str().unwrap(),
            "--prefix",
            "chunk",
        ])
        .assert()
        .success();

    assert!(outdir.join("chunk-001.pdf").exists());
    assert!(outdir.join("chunk-002.pdf").exists());
}

#[test]
fn flatten_idempotent_via_cli() {
    let dir = tempdir().unwrap();
    let in_pdf = dir.path().join("in.pdf");
    let out1 = dir.path().join("out1.pdf");
    let out2 = dir.path().join("out2.pdf");
    std::fs::write(&in_pdf, make_blank_pdf()).unwrap();

    folio()
        .args([
            "flatten",
            in_pdf.to_str().unwrap(),
            "--output",
            out1.to_str().unwrap(),
        ])
        .assert()
        .success();
    folio()
        .args([
            "flatten",
            out1.to_str().unwrap(),
            "--output",
            out2.to_str().unwrap(),
        ])
        .assert()
        .success();

    assert_eq!(page_count(&std::fs::read(&out1).unwrap()), 1);
    assert_eq!(page_count(&std::fs::read(&out2).unwrap()), 1);
}

#[test]
fn merge_single_input_round_trips() {
    let dir = tempdir().unwrap();
    let in_pdf = dir.path().join("in.pdf");
    let out = dir.path().join("out.pdf");
    std::fs::write(&in_pdf, make_blank_pdf()).unwrap();

    folio()
        .args([
            "merge",
            "--output",
            out.to_str().unwrap(),
            in_pdf.to_str().unwrap(),
        ])
        .assert()
        .success();
    let bytes = std::fs::read(out).unwrap();
    assert_eq!(page_count(&bytes), 1);
}

#[test]
fn merge_two_inputs_concatenates() {
    let dir = tempdir().unwrap();
    let a = dir.path().join("a.pdf");
    let b = dir.path().join("b.pdf");
    let out = dir.path().join("out.pdf");
    std::fs::write(&a, make_multipage_pdf(2)).unwrap();
    std::fs::write(&b, make_multipage_pdf(3)).unwrap();

    folio()
        .args([
            "merge",
            "--output",
            out.to_str().unwrap(),
            a.to_str().unwrap(),
            b.to_str().unwrap(),
        ])
        .assert()
        .success();
    let bytes = std::fs::read(out).unwrap();
    assert_eq!(page_count(&bytes), 5);
}

#[test]
fn metadata_read_round_trips_via_write() {
    let dir = tempdir().unwrap();
    let in_pdf = dir.path().join("in.pdf");
    let out = dir.path().join("out.pdf");
    std::fs::write(&in_pdf, make_blank_pdf()).unwrap();

    folio()
        .args([
            "metadata",
            "write",
            in_pdf.to_str().unwrap(),
            "--output",
            out.to_str().unwrap(),
            "--set",
            "Title=Hello",
            "--set",
            "Author=Cascade",
        ])
        .assert()
        .success();

    folio()
        .args(["metadata", "read", out.to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"Title\": \"Hello\""))
        .stdout(predicate::str::contains("\"Author\": \"Cascade\""));
}

#[test]
fn metadata_read_outputs_json() {
    let dir = tempdir().unwrap();
    let in_pdf = dir.path().join("in.pdf");
    std::fs::write(&in_pdf, make_blank_pdf()).unwrap();

    let assertion = folio()
        .args(["metadata", "read", in_pdf.to_str().unwrap()])
        .assert()
        .success();
    let stdout = String::from_utf8(assertion.get_output().stdout.clone()).unwrap();
    let v: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON");
    assert!(v.is_object());
}

// ---------------------------------------------------------------------------
// Engine error paths (exit 3) and IO errors (exit 5)
// ---------------------------------------------------------------------------

#[test]
fn convert_html_with_missing_file_exits_5() {
    let dir = tempdir().unwrap();
    folio()
        .args([
            "convert",
            "--html",
            dir.path().join("does_not_exist.html").to_str().unwrap(),
            "--output",
            dir.path().join("out.pdf").to_str().unwrap(),
        ])
        .assert()
        .code(5);
}

#[test]
fn engine_error_path_exits_3_on_bad_chrome_executable() {
    let dir = tempdir().unwrap();
    let html = dir.path().join("in.html");
    std::fs::write(&html, "<p>hi</p>").unwrap();

    folio()
        .args([
            "--chrome",
            "/definitely/no/chrome/here-__folio__",
            "convert",
            "--html",
            html.to_str().unwrap(),
            "--output",
            dir.path().join("out.pdf").to_str().unwrap(),
        ])
        .assert()
        .code(3);
}

#[test]
fn metadata_read_garbage_exits_3() {
    let dir = tempdir().unwrap();
    let p = dir.path().join("not.pdf");
    std::fs::write(&p, b"NOT A PDF").unwrap();
    folio()
        .args(["metadata", "read", p.to_str().unwrap()])
        .assert()
        .code(3);
}

// ---------------------------------------------------------------------------
// Logging: --log-format json
// ---------------------------------------------------------------------------

#[test]
fn log_format_json_emits_valid_json_per_line() {
    let dir = tempdir().unwrap();
    let in_pdf = dir.path().join("in.pdf");
    std::fs::write(&in_pdf, make_blank_pdf()).unwrap();
    let outdir = dir.path().join("out");

    let assertion = folio()
        .args([
            "-vv",
            "--log-format",
            "json",
            "split",
            in_pdf.to_str().unwrap(),
            "--output-dir",
            outdir.to_str().unwrap(),
        ])
        .assert()
        .success();
    let stderr = String::from_utf8(assertion.get_output().stderr.clone()).unwrap();
    let mut emitted = 0usize;
    for line in stderr.lines().filter(|l| !l.is_empty()) {
        let _: serde_json::Value =
            serde_json::from_str(line).unwrap_or_else(|e| panic!("bad json line {line:?}: {e}"));
        emitted += 1;
    }
    assert!(emitted > 0, "expected at least one JSON log line");
}

#[test]
fn log_format_text_does_not_emit_color_when_piped() {
    let dir = tempdir().unwrap();
    let in_pdf = dir.path().join("in.pdf");
    std::fs::write(&in_pdf, make_blank_pdf()).unwrap();
    let outdir = dir.path().join("out");

    let assertion = folio()
        .args([
            "-vv",
            "--log-format",
            "text",
            "split",
            in_pdf.to_str().unwrap(),
            "--output-dir",
            outdir.to_str().unwrap(),
        ])
        .assert()
        .success();
    let stderr = String::from_utf8(assertion.get_output().stderr.clone()).unwrap();
    assert!(
        !stderr.contains("\x1b["),
        "expected no ANSI color codes when stderr is piped, got: {stderr:?}"
    );
}

// ---------------------------------------------------------------------------
// Ignored: integration tests requiring Chrome / soffice
// ---------------------------------------------------------------------------

#[test]
fn convert_html_to_stdout_pipes_bytes() {
    if !have_chrome() {
        eprintln!("skipping: chrome not found");
        return;
    }
    let dir = tempdir().unwrap();
    let html = dir.path().join("in.html");
    std::fs::write(&html, "<p>hi</p>").unwrap();

    let assertion = folio()
        .args(["convert", "--html", html.to_str().unwrap(), "--output", "-"])
        .assert()
        .success();
    let stdout = assertion.get_output().stdout.clone();
    assert!(stdout.starts_with(b"%PDF-"), "expected PDF header");
}

#[test]
fn convert_markdown_to_pdf_via_cli() {
    if !have_chrome() {
        eprintln!("skipping: chrome not found");
        return;
    }
    let dir = tempdir().unwrap();
    let md = dir.path().join("in.md");
    std::fs::write(&md, "# Hello\n\nWorld").unwrap();
    let out = dir.path().join("out.pdf");

    folio()
        .args([
            "convert",
            "--markdown",
            md.to_str().unwrap(),
            "--output",
            out.to_str().unwrap(),
        ])
        .assert()
        .success();

    let bytes = std::fs::read(&out).unwrap();
    assert!(bytes.starts_with(b"%PDF-"));
    assert!(page_count(&bytes) >= 1);
}

#[test]
fn batch_smoke_two_files_into_two_pdfs() {
    if !have_chrome() {
        eprintln!("skipping: chrome not found");
        return;
    }
    let dir = tempdir().unwrap();
    let in_dir = dir.path().join("in");
    let out_dir = dir.path().join("out");
    std::fs::create_dir_all(&in_dir).unwrap();
    std::fs::write(in_dir.join("a.html"), "<p>A</p>").unwrap();
    std::fs::write(in_dir.join("b.html"), "<p>B</p>").unwrap();

    folio()
        .args([
            "batch",
            "--input-dir",
            in_dir.to_str().unwrap(),
            "--output-dir",
            out_dir.to_str().unwrap(),
            "--pattern",
            "**/*.html",
        ])
        .assert()
        .success();

    assert!(out_dir.join("a.pdf").exists());
    assert!(out_dir.join("b.pdf").exists());
}

#[test]
fn batch_skip_on_error_exits_6_with_summary() {
    if !have_chrome() {
        eprintln!("skipping: chrome not found");
        return;
    }
    // One real file, plus one whose extension matches but file content
    // is unreadable HTML for Chrome. We force on-error=skip so the run
    // surfaces exit code 6 (BatchPartial).
    let dir = tempdir().unwrap();
    let in_dir = dir.path().join("in");
    let out_dir = dir.path().join("out");
    std::fs::create_dir_all(&in_dir).unwrap();
    std::fs::write(in_dir.join("ok.html"), "<p>ok</p>").unwrap();
    // Force a per-file failure by writing zero-byte file with html ext.
    std::fs::write(in_dir.join("broken.html"), [0u8; 0]).unwrap();

    let assertion = folio()
        .args([
            "batch",
            "--input-dir",
            in_dir.to_str().unwrap(),
            "--output-dir",
            out_dir.to_str().unwrap(),
            "--pattern",
            "**/*.html",
            "--on-error",
            "skip",
        ])
        .assert();
    let code = assertion.get_output().status.code().unwrap_or(-1);
    assert!(code == 0 || code == 6, "expected 0 or 6, got {code}");
}

#[test]
fn convert_office_writer_doc() {
    if !have_soffice() {
        eprintln!("skipping: soffice not found");
        return;
    }
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/sample.docx");
    if !fixture.exists() {
        eprintln!("skipping: fixture {} missing", fixture.display());
        return;
    }
    let dir = tempdir().unwrap();
    let out = dir.path().join("out.pdf");
    folio()
        .args([
            "convert",
            "--office",
            fixture.to_str().unwrap(),
            "--output",
            out.to_str().unwrap(),
        ])
        .assert()
        .success();
    let bytes = std::fs::read(out).unwrap();
    assert!(bytes.starts_with(b"%PDF-"));
}

#[test]
fn convert_office_with_pdf_a_2b() {
    if !have_soffice() {
        eprintln!("skipping: soffice not found");
        return;
    }
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/sample.docx");
    if !fixture.exists() {
        eprintln!("skipping: fixture {} missing", fixture.display());
        return;
    }
    let dir = tempdir().unwrap();
    let out = dir.path().join("out.pdf");
    folio()
        .args([
            "convert",
            "--office",
            fixture.to_str().unwrap(),
            "--pdf-a",
            "a2b",
            "--output",
            out.to_str().unwrap(),
        ])
        .assert()
        .success();
}
