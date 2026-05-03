# pdfbro bindings

This directory ships pdfbro as embeddable libraries.

- `bindings/python/` — maturin project producing the `pdfbro` PyPI package.
- `bindings/node/` — napi-rs project producing the `@vel/pdfbro` npm package.
- `bindings/fixtures/` — shared HTML/Office fixtures used by tests.
- `CHROME_VERSION` — pinned Chrome-for-Testing version. Bumped per release.

The Rust glue lives in `crates/py` and `crates/js`. The pdfbro engine
itself is unchanged; bindings reuse `crates/engine` plus the new
`engine::chrome_fetch` module.

See `docs/superpowers/specs/2026-05-01-bindings-design.md` for the full
design (v1 + v2).
