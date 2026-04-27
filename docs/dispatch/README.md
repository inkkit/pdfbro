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

## Status

| Spec | Status      | Worktree            | Branch                     |
|------|-------------|---------------------|----------------------------|
| 10   | merged      | —                   | (on `dev`)                 |
| 11   | merged      | —                   | (was `feat/spec-11-chromium`) |
| 12   | merged      | —                   | (was `feat/spec-12-libreoffice`) |
| 13   | merged      | —                   | (was `feat/spec-13-pdfops`) |
| 20   | dispatchable | `../folio-spec20`  | `feat/spec-20-cli`         |
| 30   | dispatchable | `../folio-spec30`  | `feat/spec-30-server`      |
| 40   | deferred    | —                   | —                          |
| 41   | deferred    | —                   | —                          |

## Currently dispatchable

| File                              | Spec | Crate    | External deps to test         |
|-----------------------------------|------|----------|-------------------------------|
| `spec-20-cli.md`                  | 20   | cli      | Chrome and/or `soffice` for the `#[ignore]`d tests |
| `spec-30-server.md`               | 30   | server   | Chrome and `soffice` for the `#[ignore]`d e2e tests |

Specs 20 and 30 share the engine API surface (already on `dev`) and
touch disjoint files (`crates/cli/`, `crates/server/`). They will both
add workspace dependencies to the root `Cargo.toml`; conflicts are
expected only in the `[workspace.dependencies]` block and are
mechanical to resolve (alphabetical union).

## Deferred

`spec-40-bindings-py.md` and `spec-41-bindings-js.md` will be revived
once specs 20 and 30 are stable on `dev`.

## Already merged (historical)

The following dispatch files describe completed work on `dev`. Keep
them around as references; do not re-dispatch them.

- `spec-11-chromium.md`
- `spec-12-libreoffice.md`
- `spec-13-pdfops.md`

## Coordination rules for parallel agents

1. **Branch off `dev`.** Each agent works on a `feat/spec-NN-<slug>`
   branch in its own worktree.
2. **Stay inside your spec's crate.** Touch only the crate listed in
   your dispatch file (and the workspace `Cargo.toml` for dependency
   additions). Do not edit other crates.
3. **The engine API is frozen.** Specs 20+ consume `engine::*` as a
   black box. If you genuinely need a new variant on `EngineError` or
   a new engine method, **stop and report back** rather than editing
   `crates/engine/` from a downstream branch.
4. **Cargo.toml conflicts:** when adding workspace dependencies, keep
   entries alphabetical inside `[workspace.dependencies]`. The merger
   resolves overlaps via union.
5. **Tests must pass before commit.** Run `cargo test -p <crate>`,
   `cargo clippy -p <crate> --all-targets -- -D warnings`,
   `cargo fmt --check`. Integration tests that need Chrome / soffice
   are `#[ignore]` and not required to run on commit; document how to
   run them in the PR description.
6. **One spec, one PR / merge.** Don't bundle multiple specs together.
