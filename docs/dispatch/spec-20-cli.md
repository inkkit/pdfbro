You are implementing **spec 20** of the Folio project: the `folio` CLI binary.

## Your task

Read these four files in order, then implement spec 20 end to end:

1. `docs/specs/00-overview.md` — process, conventions, definition of done.
2. `docs/specs/10-engine-types.md` — the engine value types you will pass through (`PdfOptions`, `RequestContext`, `WaitCondition`, `PageRanges`, etc.). Already implemented on `dev`.
3. `docs/specs/11-engine-chromium.md` — the engine API you'll consume for HTML / URL / Markdown conversion. Already implemented on `dev`.
4. `docs/specs/20-cli.md` — your work order.

You should also skim the public surface of `engine` by reading `crates/engine/src/lib.rs` to confirm what's actually re-exported.

The implementation lives in the existing `crates/cli/` scaffold (already producing the `folio` binary). The spec is the source of truth for the public API, behavior, errors, edge cases, test plan, and acceptance criteria.

## Branch and workflow

You are already on `feat/spec-20-cli`, branched from `dev`, in the `folio-spec20` worktree. Do **not** branch again.

1. Add the workspace dependencies the spec needs to `crates/cli/Cargo.toml`:
   - Runtime: `clap` (workspace, derive feature already enabled), `engine` (workspace), `tokio` (workspace), `tracing` (workspace), `tracing-subscriber` (workspace), `anyhow` (workspace).
   - Spec-20-specific (declare in workspace `Cargo.toml` first if missing, then reference `{ workspace = true }`): `clap_complete`, `humantime`, `walkdir`.
   - Dev: `assert_cmd`, `predicates`, `tempfile` (workspace).
   Place new entries alphabetically in the workspace's `[workspace.dependencies]`.
2. Structure code under `crates/cli/src/` as `main.rs` + submodules — suggested split: `args/` (clap derive structs and parsers), `commands/` (one module per subcommand), `parse/` (option-value parsers like `--paper`, `--margin`, `--wait`, `--cookie`, `--fail-on-status`).
3. Implement per the spec. Match the documented CLI surface **verbatim** — the test plan asserts on `--help` output and exit codes.
4. Land tests:
   - **Unit tests** (no Chrome / soffice required): in-source `#[cfg(test)] mod tests` for each parser helper. Public via `pub(crate)` so they're directly testable.
   - **Integration tests** (`assert_cmd`-driven, no engine): under `crates/cli/tests/cli.rs`. Cover usage errors, exit codes, `merge`/`split`/`flatten`/`metadata` over canned PDFs (use the same fixtures from spec 13's tests if helpful — copy under `crates/cli/tests/fixtures/`).
   - **Integration tests requiring Chrome / soffice**: marked `#[ignore]`. Resolve Chrome via `BrowserConfig::default()` discovery.

## Definition of done

- [ ] `cargo test -p cli` — green (all non-ignored tests pass).
- [ ] `cargo test -p cli -- --ignored` — green locally with Chrome and `soffice` available.
- [ ] `cargo clippy -p cli --all-targets -- -D warnings` — clean.
- [ ] `cargo fmt --check` — clean.
- [ ] `folio --help`, `folio convert --help`, `folio batch --help`, `folio merge --help`, `folio split --help`, `folio flatten --help`, `folio metadata --help`, `folio completions bash` all produce non-empty output and match the documented command surface.
- [ ] Exit codes match the table in the spec (verified by `assert_cmd` tests).
- [ ] Tracing logs go to stderr; PDF bytes (when `--output -`) go cleanly to stdout.
- [ ] No `unwrap`/`expect` outside `#[cfg(test)]`.
- [ ] No `unsafe`.
- [ ] Every `[ ]` box in the spec's *Acceptance* section is satisfied.

## Boundaries

- **Touch only:** `Cargo.toml` (workspace dep additions), `crates/cli/**`, `crates/cli/tests/fixtures/**`.
- **Do not touch:** `crates/engine/**`. The engine API is frozen; if you genuinely need a new variant on `EngineError` or a new engine method, **stop and report back** — do not edit `engine/` from this branch.
- **Do not touch:** `crates/server/`, `crates/py/`, `crates/js/`. The `server` worktree is being implemented in parallel; merging will resolve dep conflicts on root `Cargo.toml`.
- **Do not implement** the `serve` subcommand — out of scope per the spec; users invoke `folio-server` directly.

## Important behavioral notes

- A single `tokio::runtime::Builder::new_multi_thread().enable_all().build()` in `main`. All commands run inside it.
- Reuse one engine handle per `convert` / `batch`. Do **not** launch a Chrome per file in `batch`.
- `batch` concurrency is bounded with `tokio::sync::Semaphore::new(concurrency)`.
- `--output -` to stdout disables every other stdout write (logs always go to stderr).
- `merge` accepts `-` as one input meaning "stdin"; only allowed once.
- Engine-error → exit-code mapping is enumerated in the spec's *Errors* and *Edge cases* sections; map via `IntoExitCode for EngineError` (or equivalent helper).

## Commit style

Conventional commits scoped by spec ID. Examples:

- `chore(cli/20): add clap_complete, humantime, walkdir workspace deps`
- `feat(cli/20): scaffold args module and global options`
- `feat(cli/20): implement convert subcommand`
- `feat(cli/20): implement batch with semaphore-bounded concurrency`
- `feat(cli/20): implement merge/split/flatten/metadata via pdfops`
- `feat(cli/20): exit-code mapping for EngineError`
- `feat(cli/20): completions subcommand for bash/zsh/fish/powershell`
- `test(cli/20): integration tests via assert_cmd`

One commit per logical chunk. Tests pass on every commit.

## When you're done

Report:

1. Branch name (`feat/spec-20-cli`).
2. Commit count and final SHA.
3. Any deviations from the spec, with rationale.
4. Whether ignored integration tests passed locally, and on what host.
5. The full output of `folio --help` (sanity check).

Begin by reading the four files listed above. Do not start coding before you can articulate the full subcommand tree and the exit-code mapping.
