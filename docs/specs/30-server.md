# Spec 30 — `server` (`folio-server` binary)

> Gotenberg-compatible HTTP service. Outline.

## Goal

Expose a Gotenberg-compatible HTTP API (mirroring routes from
`@/Users/__deesh_reddy__/projects/personal_git/rust_builds/folio/docs/gotenberg-spec.md:48-90`)
backed by the engine crate, so existing Gotenberg clients can switch
endpoints with no code changes.

## Routes (Phase 2 minimum)

| Method | Path                                  | Backed by             |
|--------|---------------------------------------|-----------------------|
| GET    | `/health`                             | `ChromiumEngine::healthy` |
| GET    | `/version`                            | `env!("CARGO_PKG_VERSION")` |
| POST   | `/forms/chromium/convert/html`        | `html_to_pdf`          |
| POST   | `/forms/chromium/convert/url`         | `url_to_pdf`           |
| POST   | `/forms/chromium/convert/markdown`    | `markdown_to_pdf`      |
| POST   | `/forms/libreoffice/convert`          | spec 12                |
| POST   | `/forms/pdfengines/merge`             | spec 13                |
| POST   | `/forms/pdfengines/split`             | spec 13                |
| POST   | `/forms/pdfengines/flatten`           | spec 13                |
| POST   | `/forms/pdfengines/metadata/read`     | spec 13                |
| POST   | `/forms/pdfengines/metadata/write`    | spec 13                |

## Form-field mapping

Form fields follow Gotenberg's contract, all camelCase. Mapping rules:

- Fields → `PdfOptions` via `serde_urlencoded` then JSON re-deserialise.
- Files: `multer` / `axum::extract::Multipart`. Stored in a per-request
  `tempfile::TempDir`.
- `RequestContext` populated from `extraHttpHeaders`, `cookies`,
  `userAgent`, `failOnHttpStatusCodes`.

## Architecture

```
AppState {
    engine: Arc<ChromiumEngine>,         // Phase 2
    libreoffice: Arc<LibreOfficeEngine>, // Phase 3 (Option)
    semaphore: Arc<Semaphore>,           // global concurrency limit
    config: ServerConfig,
}
```

- Single browser shared across requests; per-request `Permit` from
  `Semaphore`.
- A per-request span (`tracing::info_span!`) propagates a `request_id`.
- Timeouts: per-route via `tower::timeout`, configurable.
- 4xx/5xx mapping:
  - `EngineError::InvalidOption | InvalidPageRange` → `400`.
  - `EngineError::Navigation { reason }` → `502`.
  - `EngineError::Timeout` → `504`.
  - `EngineError::ChromeNotFound | ChromeLaunch` → `500` + structured log.
  - All other → `500`.

## CLI surface

`folio-server serve [--host 0.0.0.0] [--port 3000] [--concurrency N]
[--chrome <path>] [--max-body <bytes>] [--log-level info]`.

Environment variables override flags
(`FOLIO_HOST`, `FOLIO_PORT`, etc.) via `clap`'s `env` attribute.

## To expand before implementation

- [ ] Exhaustive form-field table per route mirroring Gotenberg spec.
- [ ] Trace exporter selection (OTLP / stdout).
- [ ] Webhook route — deferred follow-up.
- [ ] Test plan: `axum::Router::oneshot` integration tests + a golden
      curl-based smoke test against a launched binary in CI.
