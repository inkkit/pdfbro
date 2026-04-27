You are implementing **spec 30** of the Folio project: the `folio-server` HTTP service.

## Your task

Read these five files in order, then implement spec 30 end to end:

1. `docs/specs/00-overview.md` — process, conventions, definition of done.
2. `docs/specs/10-engine-types.md` — engine value types you will deserialise multipart fields into. Already implemented on `dev`.
3. `docs/specs/11-engine-chromium.md` — backend for the `/forms/chromium/*` routes. Already implemented on `dev`.
4. `docs/specs/12-engine-libreoffice.md` — backend for the `/forms/libreoffice/*` route. Already implemented on `dev`.
5. `docs/specs/13-engine-pdfops.md` — backend for the `/forms/pdfengines/*` routes. Already implemented on `dev`.

Then your work order: `docs/specs/30-server.md`.

You should also skim `crates/engine/src/lib.rs` to confirm the public re-exports.

The implementation lives in the existing `crates/server/` scaffold (already producing the `folio-server` binary). The spec is the source of truth for routes, multipart field maps, error → status mapping, middleware order, and graceful-shutdown semantics.

## Branch and workflow

You are already on `feat/spec-30-server`, branched from `dev`, in the `folio-spec30` worktree. Do **not** branch again.

1. Add the workspace dependencies the spec needs to `crates/server/Cargo.toml`:
   - Runtime: `axum` (workspace, with `multipart` feature already enabled), `tower` (workspace), `tower-http` (workspace), `tokio` (workspace), `tracing` (workspace), `tracing-subscriber` (workspace), `serde` (workspace), `serde_json` (workspace), `engine` (workspace), `clap` (workspace), `anyhow` (workspace).
   - Spec-30-specific (declare in workspace `Cargo.toml` first if missing, then reference `{ workspace = true }`): `multer`, `tempfile`, `uuid` (with `v4` feature), `serde_urlencoded`, `humantime`.
   - You will likely need to extend `tower-http`'s feature list to include `request-id`, `limit`, `timeout` in addition to the existing `trace`, `cors`. Update the workspace declaration accordingly.
   - Dev: `tower` (with `util` feature for `ServiceExt::oneshot`), `reqwest`.
   Place new entries alphabetically in the workspace's `[workspace.dependencies]`.
2. Structure code under `crates/server/src/` as `main.rs` + submodules — suggested split: `state.rs` (the `AppState` struct), `config.rs` (CLI / env resolution), `routes/` (one file per route), `multipart.rs` (form-field helpers), `error.rs` (the `EngineError` → response mapping), `shutdown.rs` (graceful-shutdown plumbing).
3. Implement per the spec. Match the documented routes, multipart field names, and error JSON shape **verbatim** — the spec's tests assert on the exact wire contract.
4. Land tests:
   - **Unit tests** (no engines, in-source `#[cfg(test)]`): config resolution, multipart-field parsing, error mapping table.
   - **Router tests** (no real engines): introduce a `PdfBackend` trait inside the server crate that `ChromiumEngine` implements; tests use a mock impl. Drive with `tower::ServiceExt::oneshot` against the `Router`.
   - **Integration tests** (`#[ignore]`d, real engines): under `crates/server/tests/e2e.rs`. Start the server on an ephemeral port, POST multipart bodies via `reqwest`, assert on PDF/zip/JSON responses.
   - **Graceful shutdown** test: start a long render, send SIGINT, assert the in-flight request completes (or 503s) and the process exits within 35s. Mark `#[ignore]` since it needs Chrome.

## Definition of done

- [ ] `cargo test -p server` — green (all non-ignored tests pass).
- [ ] `cargo test -p server -- --ignored` — green locally with Chrome and `soffice` available.
- [ ] `cargo clippy -p server --all-targets -- -D warnings` — clean.
- [ ] `cargo fmt --check` — clean.
- [ ] `folio-server serve --help` matches the documented option list (host/port/concurrency/max-body-bytes/request-timeout/chrome/no-sandbox/sandbox/soffice/log-level/log-format).
- [ ] Every documented route returns the documented status + body shape on the documented error path. Verified by the dedicated unit test.
- [ ] `Content-Disposition: attachment; filename="..."` set on success responses.
- [ ] Graceful shutdown verified by the dedicated integration test.
- [ ] CLI flags > env vars > defaults (precedence) verified by a unit test on `ServerConfig::resolve`.
- [ ] No `unwrap`/`expect` outside `#[cfg(test)]`.
- [ ] No `unsafe`.
- [ ] Every `[ ]` box in the spec's *Acceptance* section is satisfied.

## Boundaries

- **Touch only:** `Cargo.toml` (workspace dep additions), `crates/server/**`, `crates/server/tests/fixtures/**`.
- **Do not touch:** `crates/engine/**`. The engine API is frozen; if you genuinely need a new variant on `EngineError` or a new engine method, **stop and report back** — do not edit `engine/` from this branch.
- **Do not touch:** `crates/cli/`, `crates/py/`, `crates/js/`. The `cli` worktree is being implemented in parallel; merging will resolve dep conflicts on root `Cargo.toml`.

## Important behavioral notes

- One `ChromiumEngine` and one `LibreOfficeEngine` per process, in `Arc`. Built in parallel with `tokio::join!` at startup.
- Outer concurrency cap via a global `Arc<Semaphore>`. Permit acquired in handler prelude, dropped on completion (success or error).
- pdf-ops over inputs > 1 MiB go through `tokio::task::spawn_blocking` (CPU-bound).
- Multipart parsing buffers files into a per-request `tempfile::TempDir`. Reject path-traversal in filenames.
- `Content-Type` for responses: `application/pdf` (single PDF), `application/zip` (multi), `application/json` (metadata read).
- The `/forms/chromium/convert/markdown` route accepts both wrapper-template form (with `<link rel="markdown" href="...">`) and the simpler "first .md file" form. Wrapper takes precedence.
- The `/forms/libreoffice/convert` route accepts a `merge=true` flag; when true and there are >1 inputs, post-process via `engine::pdfops::merge`. **The engine's `OfficeOptions` does NOT carry a merge flag — merging is the server's job.**
- The `/forms/pdfengines/split` route accepts both `splitMode` and `mode` field names (Gotenberg quirk).

## Commit style

Conventional commits scoped by spec ID. Examples:

- `chore(server/30): add multer, uuid, serde_urlencoded workspace deps`
- `feat(server/30): scaffold AppState and config resolution`
- `feat(server/30): EngineError -> HTTP response mapping`
- `feat(server/30): /forms/chromium/convert/{html,url,markdown}`
- `feat(server/30): /forms/libreoffice/convert with optional merge`
- `feat(server/30): /forms/pdfengines/{merge,split,flatten,metadata}`
- `feat(server/30): middleware stack (request-id, body-limit, timeout, cors)`
- `feat(server/30): graceful shutdown via tokio::signal`
- `test(server/30): router-level tests with PdfBackend mock`
- `test(server/30): e2e integration smoke against real engines`

One commit per logical chunk. Tests pass on every commit.

## When you're done

Report:

1. Branch name (`feat/spec-30-server`).
2. Commit count and final SHA.
3. Any deviations from the spec, with rationale.
4. Whether ignored integration tests passed locally, and on what host.
5. Confirm the `Content-Disposition` and `Content-Type` invariants by listing the values used per route.

Begin by reading the five files listed above. Do not start coding before you can articulate the full route table, the multipart field maps, and the middleware ordering.
