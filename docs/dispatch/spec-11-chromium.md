You are implementing **spec 11** of the Folio project: `engine::chromium::ChromiumEngine`.

## Your task

Read these three files in order, then implement spec 11 end to end:

1. `docs/specs/00-overview.md` — process, conventions, definition of done.
2. `docs/specs/10-engine-types.md` — types you will use (already implemented in `crates/engine/src/types.rs` on `dev`).
3. `docs/specs/11-engine-chromium.md` — your work order.

The implementation lives in a new submodule `crates/engine/src/chromium/`. The spec is the source of truth for the public API, behavior, errors, edge cases, test plan, and acceptance criteria.

## Branch and workflow

1. `git checkout -b feat/spec-11-chromium dev`
2. Add `chromiumoxide` and `pulldown-cmark` to `crates/engine/Cargo.toml` (workspace dependencies are already declared in the root `Cargo.toml`; if a feature like `tokio-runtime` is missing add it via `features = [...]`).
3. Create `crates/engine/src/chromium/mod.rs`. You may split into submodules (`launch.rs`, `wait.rs`, `markdown.rs`, etc.) at your discretion — keep `pub use` clean from `mod.rs`.
4. Add `pub mod chromium;` and `pub use chromium::{ChromiumEngine, RequestContext, Cookie};` to `crates/engine/src/lib.rs`.
5. Implement per the spec. Match the public signatures **verbatim** — downstream specs (server, bindings, CLI) depend on them.
6. Land tests:
   - **Unit tests** (no Chrome required): in `#[cfg(test)] mod tests` blocks.
   - **Integration tests** (Chrome required): under `crates/engine/tests/chromium_html.rs`, marked `#[ignore]`. Use `std::env::var("CHROME_PATH").ok()` to locate the executable; default to `BrowserConfig::default()`.

## Definition of done (must all be true before final commit)

- [ ] `cargo test -p engine` — green (all non-ignored tests pass).
- [ ] `cargo test -p engine -- --ignored` — green locally with a system Chrome installed at the path resolved by spec 10's discovery rules.
- [ ] `cargo clippy -p engine --all-targets -- -D warnings` — clean.
- [ ] `cargo fmt --check` — clean.
- [ ] `cargo doc -p engine --no-deps` — no warnings.
- [ ] `ChromiumEngine` is `Send + Sync + Clone` (asserted via `static_assertions::assert_impl_all!`).
- [ ] No `unsafe`. No `panic!` / `unwrap` / `expect` outside `#[cfg(test)]`.
- [ ] Every `[ ]` box in the spec's *Acceptance* section is satisfied.

## Boundaries

- **Touch only:** `crates/engine/Cargo.toml`, `crates/engine/src/chromium/**`, `crates/engine/src/lib.rs` (just add the `pub mod` + re-exports), `crates/engine/tests/chromium_html.rs`, `crates/engine/tests/fixtures/**` if needed.
- **Do not touch:** `crates/engine/src/types.rs` (frozen — if you genuinely need a new `EngineError` variant, stop and report back; do not edit it inline).
- **Do not touch:** `crates/cli/`, `crates/server/`, `crates/py/`, `crates/js/`.

## Commit style

Conventional commits scoped by spec ID. Examples:

- `chore(engine/11): add chromiumoxide and pulldown-cmark deps`
- `feat(engine/11): scaffold ChromiumEngine launch and shutdown`
- `feat(engine/11): implement html_to_pdf and url_to_pdf`
- `feat(engine/11): implement wait conditions`
- `feat(engine/11): implement markdown_to_pdf`
- `test(engine/11): integration tests against system Chrome`

One commit per logical chunk. Tests pass on every commit. Do not squash — leave the history readable.

## When you're done

Report:

1. Branch name (`feat/spec-11-chromium`).
2. Commit count and final SHA.
3. Any deviations from the spec, with rationale.
4. Whether ignored integration tests passed locally, and on what host.

Begin by reading the three files listed above. Do not start coding before you can articulate the public API and the wait-condition implementation strategy.
