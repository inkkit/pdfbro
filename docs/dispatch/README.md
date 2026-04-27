# Parallel agent dispatch

Each file in this directory is a **copy-paste-ready prompt** for a fresh
Cascade (or any agentic) session. The prompts are designed so the
receiving agent can complete the work end-to-end without reading the
rest of the conversation history.

## How to use

1. Pick one of the dispatch files below.
2. Open a fresh Cascade window pointed at this workspace.
3. Paste the **entire contents** of the dispatch file as the first message.
4. The agent will create its own branch, implement the spec, run tests,
   and commit.

## Currently dispatchable (post-spec-10)

| File                              | Spec | Crate    | Branch              | External deps to test         |
|-----------------------------------|------|----------|---------------------|-------------------------------|
| `spec-11-chromium.md`             | 11   | engine   | `feat/spec-11-chromium`     | Chrome / Chromium on PATH     |
| `spec-12-libreoffice.md`          | 12   | engine   | `feat/spec-12-libreoffice`  | `soffice` (LibreOffice)       |
| `spec-13-pdfops.md`               | 13   | engine   | `feat/spec-13-pdfops`       | None (pure Rust + `lopdf`)    |

Specs 11, 12, 13 share only the `engine::types` foundation (already on
`dev`). They touch disjoint files (`crates/engine/src/chromium/`,
`crates/engine/src/libreoffice/`, `crates/engine/src/pdfops/`) and each
crate's `Cargo.toml` additions are dep-only — merge conflicts should be
limited to the dependency block.

## Blocked until 11/12/13 merge

These need the engine APIs they consume:

| Spec | Reason                                                    |
|------|-----------------------------------------------------------|
| 20 (cli)        | Consumes `engine::ChromiumEngine` (spec 11).    |
| 30 (server)     | Consumes specs 11, 12, 13.                      |
| 40 (bindings-py)| Wraps spec 11.                                  |
| 41 (bindings-js)| Wraps spec 11.                                  |

Dispatch files for these will be added once their dependencies merge.

## Coordination rules for parallel agents

1. **Branch off `dev`.** Each agent creates `feat/spec-NN-<slug>`.
2. **Don't touch other crates.** Engine sub-modules only; do not edit
   `crates/cli/`, `crates/server/`, `crates/py/`, `crates/js/`.
3. **Cargo.toml conflicts:** if your spec needs a workspace dependency
   that another agent also adds (unlikely, but possible), keep the entry
   alphabetical inside its section. The merger resolves conflicts.
4. **`engine::types` is frozen.** If your spec genuinely needs a new
   variant on `EngineError`, add it to spec 10 first (a separate
   `docs/specs/10-engine-types.md` PR) and wait for that to merge.
5. **Tests must pass before commit.** `cargo test -p engine`,
   `cargo clippy -p engine --all-targets -- -D warnings`,
   `cargo fmt --check`. Integration tests that need Chrome / soffice
   are `#[ignore]` and not required to run on commit; document how to
   run them in the PR description.
6. **One spec, one PR.** Don't bundle multiple specs together.
