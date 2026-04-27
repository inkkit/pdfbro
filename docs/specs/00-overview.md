# Folio Specs — Overview

> Spec-driven, sub-agent-friendly development plan for the Folio workspace.

## Why this exists

Each spec under `docs/specs/` is a **self-contained work order** for a single
crate or module. An implementing agent must be able to:

1. Read **only** the spec (plus the cited docs/links inside it),
2. Produce code that satisfies every item in the spec's *Acceptance* section,
3. Run the *Test plan* and have it pass.

This decouples authoring from implementation, lets multiple agents work in
parallel on independent specs, and gives reviewers a single source of truth
to compare a PR against.

## Source-of-truth hierarchy

When specs and other docs disagree:

```
docs/specs/* (this directory)   <- highest priority, authoritative
docs/proposal.md                <- design intent, may be stale
docs/gotenberg-spec.md          <- Gotenberg API contract we mirror
README.md                       <- user-facing summary
docs/gap-analysis.md            <- background / context only
docs/obscura-spec.md            <- background / context only
```

If a spec needs to override `proposal.md`, do it explicitly in the spec body
and call it out in the PR.

## Spec template

Every spec MUST contain these sections in this order:

1. **Goal** — one sentence, present tense.
2. **Scope** — what's in / out.
3. **Public API** — exact Rust signatures (or HTTP routes / CLI surface).
4. **Behavior** — stepwise pseudocode for each public entrypoint.
5. **Errors** — every error variant the code can produce + when.
6. **Edge cases** — concrete adversarial inputs and the required response.
7. **Test plan** — list of unit + integration tests with input → expected.
8. **Acceptance** — bullet checklist; every box must be tickable to merge.
9. **Out of scope / follow-ups** — explicitly deferred work.

## Dispatch ledger

| ID  | Spec                       | Crate            | Depends on    | Phase |
|-----|----------------------------|------------------|---------------|-------|
| 10  | engine-types               | `engine`         | —             | 1     |
| 11  | engine-chromium            | `engine`         | 10            | 1     |
| 12  | engine-libreoffice         | `engine`         | 10            | 3     |
| 13  | engine-pdfops              | `engine`         | 10            | 4     |
| 20  | cli                        | `cli`            | 10, 11        | 1/5   |
| 30  | server                     | `server`         | 10, 11(+12,13)| 2     |
| 40  | bindings-py                | `py`             | 10, 11        | 6     |
| 41  | bindings-js                | `js`             | 10, 11        | 6     |

Phases mirror `@docs/proposal.md` *Implementation Phases*. Anything in the
same phase with no shared dependency can be worked in parallel by separate
sub-agents.

## Conventions

### Rust

- Edition: `2024` (set at workspace level).
- Errors: each crate exports a `thiserror` enum; binaries/bindings convert
  to `anyhow::Error` only at the top of `main` / FFI boundary.
- All public async fns take `&self`, never `&mut self`. Internal mutability
  goes through `tokio::sync` primitives.
- Public types implement `Debug` + `Clone` where it doesn't break invariants.
- No `unsafe` outside FFI shims (`py`, `js`).
- `#![deny(rust_2018_idioms, missing_docs)]` on every published crate's lib.
- Public functions documented with `///`; doc examples compile (`cargo test --doc`).

### Imports / lib names

The `engine` crate's package is `engine`; importable path is `engine::…`.
The `py` and `js` crates produce a `cdylib` with `[lib] name = "folio"` so
their respective host languages see a module called `folio`.

### Tests

- **Unit tests** colocated in `src/` via `#[cfg(test)] mod tests`.
- **Integration tests** under each crate's `tests/`.
- **End-to-end** Chrome-bound tests gated behind `#[ignore]` and run by CI
  with `cargo test -- --ignored` after Chrome is provisioned. They never
  block local `cargo test`.
- Test PDFs are validated by:
  - Byte-stream contains `%PDF-1.` header and `%%EOF` trailer.
  - `lopdf::Document::load_mem(&bytes)` round-trips successfully.
  - Page count matches expectation.

### Commits

Conventional commits, scoped by spec ID where applicable, e.g.:

- `feat(engine/11): implement ChromiumEngine::html_to_pdf`
- `test(engine/11): add networkidle wait condition tests`
- `docs(specs): expand 13-engine-pdfops`

### Definition of Done (per spec)

A spec is **done** when:

1. Every box in *Acceptance* is checked,
2. `cargo fmt --check` and `cargo clippy --workspace -- -D warnings` pass,
3. `cargo test --workspace` passes (excluding `--ignored` E2E),
4. Public API matches *Public API* section verbatim,
5. The spec file itself is updated if any deviation was necessary, with
   rationale in the commit message.
