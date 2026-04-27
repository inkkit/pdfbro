You are implementing **spec 12** of the Folio project: `engine::libreoffice::LibreOfficeEngine`.

## Your task

Read these three files in order, then implement spec 12 end to end:

1. `docs/specs/00-overview.md` — process, conventions, definition of done.
2. `docs/specs/10-engine-types.md` — types you will use (already implemented in `crates/engine/src/types.rs` on `dev`).
3. `docs/specs/12-engine-libreoffice.md` — your work order.

The implementation lives in a new submodule `crates/engine/src/libreoffice/`. The spec is the source of truth for the public API, behavior, errors, edge cases, test plan, and acceptance criteria.

## Branch and workflow

1. `git checkout -b feat/spec-12-libreoffice dev`
2. Add `tempfile` and ensure `tokio` has the `process` feature in `crates/engine/Cargo.toml`. (Workspace `tokio` already has `features = ["full"]` so process is included; verify by reading the root `Cargo.toml`.)
3. Create `crates/engine/src/libreoffice/mod.rs`. You may split into submodules (`discover.rs`, `filter.rs`, `convert.rs`) at your discretion.
4. Add `pub mod libreoffice;` and `pub use libreoffice::{LibreOfficeEngine, LibreOfficeConfig, OfficeOptions, PdfAProfile};` to `crates/engine/src/lib.rs`.
5. Implement per the spec. Match the public signatures **verbatim**.
6. Land tests:
   - **Unit tests** (no `soffice` required): pure-logic tests for the filter table, validation rules, and option-blob generation.
   - **Integration tests** (`soffice` required): under `crates/engine/tests/libreoffice.rs`, marked `#[ignore]`. Read `LIBREOFFICE_PATH` env var or autodiscover.
   - Add small fixture documents under `crates/engine/tests/fixtures/office/` (`sample.docx`, `sample.xlsx`, `sample.pptx`). Generate them with LibreOffice itself if they don't already exist; keep each <50 KB and commit them.

## Definition of done

- [ ] `cargo test -p engine` — green (all non-ignored tests pass).
- [ ] `cargo test -p engine -- --ignored` — green locally with `soffice` installed.
- [ ] `cargo clippy -p engine --all-targets -- -D warnings` — clean.
- [ ] `cargo fmt --check` — clean.
- [ ] `cargo doc -p engine --no-deps` — no warnings.
- [ ] `LibreOfficeEngine` is `Send + Sync + Clone` (asserted via `static_assertions`).
- [ ] No `unsafe`. No `unwrap`/`expect` outside `#[cfg(test)]`.
- [ ] No leaked tempdirs (verified by inspecting `tempfile::TempDir` usage — it must Drop-cleanup).
- [ ] Every `[ ]` box in the spec's *Acceptance* section is satisfied.

## Boundaries

- **Touch only:** `crates/engine/Cargo.toml`, `crates/engine/src/libreoffice/**`, `crates/engine/src/lib.rs` (just `pub mod` + re-exports), `crates/engine/tests/libreoffice.rs`, `crates/engine/tests/fixtures/office/**`.
- **Do not touch:** `crates/engine/src/types.rs`, `crates/engine/src/chromium/**` (another agent may be working there), `crates/engine/src/pdfops/**` (same).
- **Do not touch:** `crates/cli/`, `crates/server/`, `crates/py/`, `crates/js/`.

## Important behavioral note

Section *Behavior › `convert_many`* of the spec is explicit that
`OfficeOptions` does **not** carry a `merge` flag — merging multiple
PDFs into one is the server/CLI layer's job, calling `pdfops::merge`
(spec 13). Do not add `merge` to `OfficeOptions`.

## Commit style

Conventional commits scoped by spec ID. Examples:

- `chore(engine/12): add tempfile dep`
- `feat(engine/12): implement soffice discovery and probe`
- `feat(engine/12): implement convert with isolated UserInstallation`
- `feat(engine/12): implement convert_many with semaphore-bounded parallelism`
- `feat(engine/12): export-filter table for writer/calc/impress/draw`
- `test(engine/12): integration tests against system soffice`

One commit per logical chunk. Tests pass on every commit.

## When you're done

Report:

1. Branch name (`feat/spec-12-libreoffice`).
2. Commit count and final SHA.
3. Any deviations from the spec, with rationale.
4. Whether ignored integration tests passed locally, and on what host.
5. Confirm the no-`merge`-flag invariant is upheld.

Begin by reading the three files listed above. Do not start coding before you can articulate how `UserInstallation` isolation works and which CLI flags `--convert-to` accepts.
