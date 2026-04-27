You are implementing **spec 13** of the Folio project: `engine::pdfops`.

## Your task

Read these three files in order, then implement spec 13 end to end:

1. `docs/specs/00-overview.md` — process, conventions, definition of done.
2. `docs/specs/10-engine-types.md` — types you will use (already implemented in `crates/engine/src/types.rs` on `dev`). You will heavily use `EngineError`, `EngineResult`, and `PageRanges`.
3. `docs/specs/13-engine-pdfops.md` — your work order.

The implementation lives in a new submodule `crates/engine/src/pdfops/`. The spec is the source of truth for the public API, behavior, errors, edge cases, test plan, and acceptance criteria.

## Branch and workflow

1. `git checkout -b feat/spec-13-pdfops dev`
2. Add `lopdf` (already in the workspace `[workspace.dependencies]`) and `proptest` (dev only) to `crates/engine/Cargo.toml`.
3. Create `crates/engine/src/pdfops/mod.rs`. You may split into submodules (`merge.rs`, `split.rs`, `flatten.rs`, `metadata.rs`, `watermark.rs`, `rotate.rs`).
4. Add `pub mod pdfops;` and re-export the public functions and types from `crates/engine/src/lib.rs`.
5. Implement per the spec. Match the public signatures **verbatim**. All operations are stateless **free functions** — there is no struct.
6. Land tests:
   - **Unit tests** (in-source `#[cfg(test)] mod tests` blocks): no fixtures required. Cover the validation rules, the encrypted-input rejection, and edge cases.
   - **Integration tests** under `crates/engine/tests/pdfops.rs`. Use the small fixture PDFs listed in the spec (`single_page_a4.pdf`, `three_page_letter.pdf`, `with_form.pdf`, `with_annotations.pdf`, `unicode_title.pdf`, `encrypted.pdf`). If they don't already exist, generate them — see the *Fixture generation* note below — and commit under `crates/engine/tests/fixtures/pdf/`. Each fixture <50 KB.
   - **Property tests** (`proptest`) per the spec's *Property tests* section.

## Fixture generation note

You can generate the required fixtures programmatically at the top of
the integration-test file using `lopdf::Document` builders, except for
`with_form.pdf` and `encrypted.pdf`. For those, generate once locally
with a tool like `qpdf --encrypt '' '' 256 -- input.pdf encrypted.pdf`
or by hand-crafting the AcroForm dict, then commit the binaries.
Document the regeneration command in
`crates/engine/tests/fixtures/pdf/README.md`.

## Definition of done

- [ ] `cargo test -p engine` — green (all non-ignored tests pass; this spec has no `#[ignore]`d tests since no external tools are required).
- [ ] `cargo clippy -p engine --all-targets -- -D warnings` — clean.
- [ ] `cargo fmt --check` — clean.
- [ ] `cargo doc -p engine --no-deps` — no warnings.
- [ ] All ops are stateless. No `static`, no `lazy_static`, no global mutable state.
- [ ] Every op sets `/Producer` to `format!("folio/{}", env!("CARGO_PKG_VERSION"))`.
- [ ] Encrypted-input rejection is uniform across every public function (covered by an explicit test).
- [ ] No `unsafe`. No `unwrap`/`expect` outside `#[cfg(test)]` and `#[test]`.
- [ ] Every `[ ]` box in the spec's *Acceptance* section is satisfied.

## Boundaries

- **Touch only:** `crates/engine/Cargo.toml`, `crates/engine/src/pdfops/**`, `crates/engine/src/lib.rs` (just `pub mod` + re-exports), `crates/engine/tests/pdfops.rs`, `crates/engine/tests/fixtures/pdf/**`.
- **Do not touch:** `crates/engine/src/types.rs`, `crates/engine/src/chromium/**` (another agent), `crates/engine/src/libreoffice/**` (another agent).
- **Do not touch:** `crates/cli/`, `crates/server/`, `crates/py/`, `crates/js/`.

## Commit style

Conventional commits scoped by spec ID. Examples:

- `chore(engine/13): add lopdf and proptest deps`
- `feat(engine/13): merge with renumbered object IDs`
- `feat(engine/13): split with ByRanges/EveryN/OnePagePerFile`
- `feat(engine/13): flatten interactive form widgets to page content`
- `feat(engine/13): metadata read/write with UTF-16BE for non-ASCII`
- `feat(engine/13): watermark text and PNG with positioning and tiling`
- `feat(engine/13): rotate by 0/90/180/270 with PageRanges targeting`
- `test(engine/13): property tests for merge associativity and split-merge round-trip`

One commit per logical chunk. Tests pass on every commit.

## When you're done

Report:

1. Branch name (`feat/spec-13-pdfops`).
2. Commit count and final SHA.
3. Any deviations from the spec, with rationale.
4. List of fixture PDFs committed and how to regenerate each.
5. Confirm the encrypted-input rejection rule holds for every public function.

Begin by reading the three files listed above. Do not start coding before you can articulate how `lopdf` exposes the page tree, the cross-reference table, and `Document::renumber_objects`.
