# Spec 30 — `server` (`folio-server` binary)

> Gotenberg-compatible HTTP service backed by the `engine` crate.
> Drop-in replacement for Gotenberg's `/forms/chromium/*`,
> `/forms/libreoffice/*`, and `/forms/pdfengines/*` routes.

## Goal

Expose an HTTP API that mirrors Gotenberg's wire contract from
`@/Users/__deesh_reddy__/projects/personal_git/rust_builds/folio/docs/gotenberg-spec.md:48-90`,
so existing Gotenberg clients can switch by changing only the base URL.

## Scope

**In:**

- The Phase-1/2 routes listed below (chromium html/url/markdown/screenshot,
  libreoffice convert, pdfengines merge/split/flatten/metadata).
- Form-multipart parsing (matching Gotenberg field names).
- One shared `ChromiumEngine` and `LibreOfficeEngine` per process.
- Concurrency limit via global `Semaphore`.
- Per-request `request_id`, structured `tracing` logs, `/health`, `/version`.
- Graceful shutdown on `SIGINT` / `SIGTERM`: drain in-flight, close
  engines, exit 0.

**Out:**

- Webhook routes (`/forms/webhook`).
- Screenshot routes (`/forms/chromium/screenshot/*`) — follow-up.
- Encrypt / watermark / stamp / rotate routes — wired once spec 13's
  follow-ups land.
- Metrics (Prometheus / OpenTelemetry) — separate optional feature.
- Auth — none in MVP. Operators are expected to front this with a
  reverse proxy when exposed publicly.

## Public API

### Routes

| Method | Path                                     | Handler                          |
|--------|------------------------------------------|----------------------------------|
| GET    | `/health`                                | `health`                         |
| GET    | `/version`                               | `version`                        |
| POST   | `/forms/chromium/convert/html`           | `chromium_html`                  |
| POST   | `/forms/chromium/convert/url`            | `chromium_url`                   |
| POST   | `/forms/chromium/convert/markdown`       | `chromium_markdown`              |
| POST   | `/forms/chromium/screenshot/html`        | `chromium_screenshot_html`       |
| POST   | `/forms/chromium/screenshot/url`         | `chromium_screenshot_url`        |
| POST   | `/forms/chromium/screenshot/markdown`    | `chromium_screenshot_markdown`   |
| POST   | `/forms/libreoffice/convert`             | `libreoffice_convert`            |
| POST   | `/forms/pdfengines/merge`                | `pdfengines_merge`               |
| POST   | `/forms/pdfengines/split`                | `pdfengines_split`               |
| POST   | `/forms/pdfengines/flatten`              | `pdfengines_flatten`             |
| POST   | `/forms/pdfengines/metadata/read`        | `pdfengines_metadata_read`       |
| POST   | `/forms/pdfengines/metadata/write`       | `pdfengines_metadata_write`      |

All POST routes are `multipart/form-data`. JSON bodies are not accepted
(matches Gotenberg).

### CLI surface

```
folio-server serve [OPTIONS]

Options (env-overridable; flags take precedence):
  --host <HOST>            Default 0.0.0.0          (env FOLIO_HOST)
  --port <PORT>            Default 3000             (env FOLIO_PORT)
  --concurrency <N>        Default num_cpus         (env FOLIO_CONCURRENCY)
  --max-body-bytes <N>     Default 50 MiB           (env FOLIO_MAX_BODY)
  --request-timeout <DUR>  Default 120s             (env FOLIO_REQUEST_TIMEOUT)
  --chrome <PATH>          Override Chrome path     (env CHROME_PATH)
  --no-sandbox / --sandbox                          (env FOLIO_NO_SANDBOX)
  --soffice <PATH>         Override soffice path    (env LIBREOFFICE_PATH)
  --log-level <LEVEL>      Default "info"           (env RUST_LOG)
  --log-format <FORMAT>    text | json. Default text on TTY, else json.
                                                    (env FOLIO_LOG_FORMAT)
```

## Behavior

### App state

```rust
pub struct AppState {
    chromium: Arc<ChromiumEngine>,
    libreoffice: Arc<LibreOfficeEngine>,
    sem: Arc<tokio::sync::Semaphore>,   // global concurrency cap
    config: ServerConfig,               // ports, timeouts, max body
    started_at: std::time::Instant,
}
```

`AppState` is `Clone` (its fields are all `Arc`/`Copy`-friendly) and
attached via `axum::extract::State`.

### Engine lifecycle

1. On startup, build `ChromiumEngine::launch_with(BrowserConfig::from(cfg))`
   in parallel with `LibreOfficeEngine::launch(LibreOfficeConfig::from(cfg))`
   via `tokio::join!`.
2. On either engine failing to launch, log the error and exit 1.
3. On `SIGINT`/`SIGTERM` (`tokio::signal::ctrl_c()` + Unix signals):
   1. Stop accepting new connections (`axum::serve` graceful shutdown).
   2. Wait for in-flight requests up to a 30-second drain budget.
   3. `chromium.shutdown().await` and drop `libreoffice`.
   4. Exit 0.

### Form-field parsing

Files are extracted from the multipart body into a per-request
`tempfile::TempDir`. Non-file fields are collected into a
`HashMap<String, String>` (last wins on duplicates).

Then a pure helper deserialises the map into the relevant request
struct using `serde_urlencoded` (after the map has been re-serialised).
This gives us camelCase field names for free via spec 10's `#[serde]`
annotations.

### `chromium_html`

Multipart fields:

| Field name                | Type     | Maps to                              |
|---------------------------|----------|--------------------------------------|
| `files` (one .html file)  | file     | inlined as the HTML string            |
| `files` (additional)      | file(s)  | written next to `index.html` for relative resolution; `base_url` set to a `file://<tmpdir>/index.html` |
| `paperWidth`              | float    | `PdfOptions::paper.width_in`          |
| `paperHeight`             | float    | `PdfOptions::paper.height_in`         |
| `marginTop` ... `marginRight` | float | `PdfOptions::margin.*`               |
| `landscape`               | bool     | `PdfOptions::landscape`               |
| `scale`                   | float    | `PdfOptions::scale`                   |
| `printBackground`         | bool     | `PdfOptions::print_background`        |
| `pageRanges`              | string   | `PdfOptions::page_ranges`             |
| `headerTemplate`          | string   | `PdfOptions::header_template`         |
| `footerTemplate`          | string   | `PdfOptions::footer_template`         |
| `preferCssPageSize`       | bool     | `PdfOptions::prefer_css_page_size`    |
| `emulateMediaType`        | string   | `PdfOptions::emulate_media`           |
| `waitDelay`               | duration | `WaitCondition::Delay`                |
| `waitForSelector`         | string   | `WaitCondition::Selector`             |
| `waitForExpression`       | string   | `WaitCondition::Expression`           |
| `userAgent`               | string   | `RequestContext::user_agent`          |
| `extraHttpHeaders`        | json     | `RequestContext::extra_headers`       |
| `cookies`                 | json     | `RequestContext::cookies`             |
| `failOnHttpStatusCodes`   | json     | `RequestContext::fail_on_status`      |

Steps:

1. Acquire a permit from `state.sem` (await; this is the back-pressure
   point).
2. Parse multipart; require a file named `index.html` (Gotenberg
   convention).
3. Build `PdfOptions` and `RequestContext` from the form map.
4. Validate via `PdfOptions::validate()`.
5. Call `state.chromium.html_to_pdf(html, base_url, &opts, &ctx)`.
6. Stream the bytes back as `application/pdf` with
   `Content-Disposition: attachment; filename="result.pdf"` (matches
   Gotenberg). Set `X-Request-Id` echo.

### `chromium_url`

Same as `chromium_html`, except instead of `files` there's a `url`
field (string, required), and the engine call is `url_to_pdf`.

### `chromium_markdown`

Multipart accepts:

- An `index.html` file (a wrapper template).
- One or more `.md` files referenced by `<link rel="markdown" href="...">`
  inside the wrapper.

Implementation:

1. Read all files into the per-request tempdir.
2. Read the wrapper `index.html`. Find all
   `<link rel="markdown" href="...">` (or the simpler convention of
   reading the *first* `.md` file when no wrapper is provided — both
   supported, wrapper takes precedence).
3. For each referenced markdown, render via the engine's markdown→html
   conversion (delegating to spec 11) and inline into the wrapper.
4. Send the resulting HTML to `html_to_pdf` with `base_url` set to the
   tempdir.

### `chromium_screenshot_html`

Multipart fields:

| Field name                | Type     | Maps to                              |
|---------------------------|----------|--------------------------------------|
| `files` (one .html file)  | file     | inlined as the HTML string            |
| `format`                  | string   | `ScreenshotOptions::format` (png/jpeg/webp) |
| `quality`                 | int      | `ScreenshotOptions::quality` (0-100) |
| `fullPage`                | bool     | `ScreenshotOptions::full_page`       |
| `clip.x`, `clip.y`        | float    | Clip rectangle position               |
| `clip.width`, `clip.height` | float | Clip rectangle dimensions             |
| `viewport.width`          | int      | `ScreenshotOptions::viewport_width`  |
| `viewport.height`         | int      | `ScreenshotOptions::viewport_height`  |
| `viewport.scale`          | float    | `ScreenshotOptions::scale`           |
| `waitDelay`               | duration | `WaitCondition::Delay`                |
| `waitForSelector`         | string   | `WaitCondition::Selector`             |
| `waitForExpression`       | string   | `WaitCondition::Expression`           |
| `userAgent`               | string   | `RequestContext::user_agent`          |
| `extraHttpHeaders`        | json     | `RequestContext::extra_headers`       |
| `cookies`                 | json     | `RequestContext::cookies`             |
| `failOnHttpStatusCodes`   | json     | `RequestContext::fail_on_status`      |

Steps:

1. Acquire semaphore permit.
2. Parse multipart; require a file named `index.html`.
3. Build `ScreenshotOptions` and `RequestContext` from form map.
4. Call `state.chromium.screenshot_html(html, base_url, &opts, &ctx)`.
5. Return bytes as `image/png`, `image/jpeg`, or `image/webp` with
   `Content-Disposition: attachment; filename="result.{png|jpg|webp}"`.

### `chromium_screenshot_url`

Same as `chromium_screenshot_html`, except uses `url` field instead of
`files`, and calls `screenshot_url`.

### `chromium_screenshot_markdown`

Same pattern as `chromium_markdown` but renders to screenshot instead of
PDF. Calls `screenshot_markdown`.

### `libreoffice_convert`

Multipart fields:

| Field                  | Type     | Maps to                            |
|------------------------|----------|------------------------------------|
| `files`                | file(s)  | input documents                    |
| `landscape`            | bool     | `OfficeOptions::landscape`         |
| `pageRanges`           | string   | `OfficeOptions::page_ranges`       |
| `pdfa`                 | string   | `OfficeOptions::pdf_a`             |
| `pdfua`                | bool     | `OfficeOptions::pdf_ua`            |
| `merge`                | bool     | post-process via `pdfops::merge`   |
| `quality`              | int      | `OfficeOptions::quality`           |
| `maxImageResolution`   | int      | `OfficeOptions::max_image_resolution` |
| `nativePageRanges`     | string   | alias of `pageRanges` (Gotenberg)  |

Steps:

1. Permit + tempdir.
2. Save each `files` part to `tempdir/<name>`.
3. Call `libreoffice.convert_many(...)`.
4. If `merge = true`, pipe results into `pdfops::merge` (spec 13).
5. Return the single-file or zip-of-files response (when not merging
   with multiple inputs, ZIP up the outputs as
   `application/zip` — this matches Gotenberg's behavior).

### `pdfengines_merge`

Multipart `files`: two or more PDFs, in field order. Other fields:
`metadata` (json) — optional, applied via `pdfops::write_metadata`
after merge.

### `pdfengines_split`

Fields:

- `files`: exactly one PDF.
- `splitMode`: `intervals` | `pages`. (Gotenberg uses `mode` — accept
  both names.)
- `splitSpan`: integer for `intervals`.
- `splitUnify`: bool — when true and mode is `pages`, merge the chunks
  back into a single PDF (matches Gotenberg quirk).
- `splitPages`: comma list of page-range chunks for `pages` mode.

Returns:

- Single chunk: `application/pdf`.
- Multiple chunks: `application/zip` containing
  `result-001.pdf`, `result-002.pdf`, ...

### `pdfengines_flatten`

Fields: `files` — one or more PDFs. Each is flattened independently;
returns single PDF or ZIP per the same rule.

### `pdfengines_metadata_read`

Fields: `files` — one or more PDFs. Returns `application/json`:

```json
{
  "input-1.pdf": { "title": "...", "author": "...", "custom": {...} },
  "input-2.pdf": { ... }
}
```

### `pdfengines_metadata_write`

Fields:

- `files`: one or more PDFs.
- `metadata`: required JSON. Merged into each input.

Returns single PDF / ZIP per the standard rule.

### `health`

Returns `200 OK` with body:

```json
{
  "status": "up",
  "uptime_secs": 1234,
  "chromium": "up" | "down",
  "libreoffice": "up" | "down"
}
```

`chromium` reflects `ChromiumEngine::healthy().await`; same for
`libreoffice`. If either is `down`, the overall HTTP status is still
`200` (matches Gotenberg convention) but the body indicates the issue.

### `version`

Returns `text/plain` body with `env!("CARGO_PKG_VERSION")`.

### Middleware stack (outer → inner)

1. `tower_http::trace::TraceLayer` with a custom span
   (`request_id`, `method`, `uri`, `status`, `latency_ms`).
2. `tower_http::request_id::SetRequestIdLayer` (use `X-Request-Id` if
   incoming, else generate a UUIDv4).
3. `tower_http::limit::RequestBodyLimitLayer::new(max_body_bytes)`.
4. `tower::timeout::TimeoutLayer::new(request_timeout)` — bypassed for
   `/health` and `/version`.
5. `tower_http::cors::CorsLayer::permissive()` (operator-overridable
   later via flag, MVP keeps it permissive).
6. The router.

### Error mapping

Single `IntoResponse` for `EngineError`:

| Variant                          | Status | Body                                  |
|----------------------------------|--------|---------------------------------------|
| `InvalidOption`                  | 400    | `{ "error": "...", "code": "INVALID_OPTION" }` |
| `InvalidPageRange`               | 400    | `{ "error": "...", "code": "INVALID_PAGE_RANGE" }` |
| `Navigation { url, reason }`     | 502    | `{ "error": "...", "code": "NAVIGATION", "url": "...", "reason": "..." }` |
| `Timeout(d)`                     | 504    | `{ "error": "...", "code": "TIMEOUT" }` |
| `ChromeNotFound | ChromeLaunch` | 500    | `{ "error": "...", "code": "ENGINE_UNAVAILABLE" }` |
| `Cdp | Internal | Io`            | 500    | `{ "error": "...", "code": "INTERNAL" }` |

All error responses are `application/json`. The originating
`EngineError` `Display` text becomes the `error` field; the chain (when
present) joins via `: `.

### Concurrency model

- Outer cap: `Semaphore::new(concurrency)`. Permit acquired in handler
  prelude, dropped when the handler future ends (success or error).
- Inner: `ChromiumEngine` opens a fresh page per request (spec 11
  guarantees safe concurrency).
- LibreOffice: each `convert*` call serialises through the engine's
  internal semaphore (spec 12).
- PDF ops are pure CPU; offload via `tokio::task::spawn_blocking` for
  any input larger than 1 MiB so we don't block the runtime.

## Errors

See "Error mapping" above. The server's surface contains no error
variants of its own — every failure ultimately maps to an `EngineError`
or to one of the standard HTTP errors:

- 400 — multipart parse failure, missing required field.
- 405 — wrong HTTP method on a known path.
- 413 — body exceeds `--max-body-bytes` (returned by tower-http layer).
- 415 — non-multipart `Content-Type`.

## Edge cases

| Scenario                                                            | Required behavior                                                       |
|---------------------------------------------------------------------|-------------------------------------------------------------------------|
| Multipart missing required `files`                                  | 400 with `{"error":"missing required file 'index.html'"}`.              |
| `files` includes a `..` path traversal                              | Reject; 400.                                                             |
| Body exactly at `--max-body-bytes`                                  | Accepted.                                                                |
| Body 1 byte over                                                    | 413, structured error code `BODY_TOO_LARGE`.                            |
| Chrome dies mid-request                                             | `EngineError::Cdp` → 500. Server keeps running; next request triggers re-launch attempt (out of MVP — for now we exit). |
| `/health` while engines are down                                    | 200 with `{ "status": "up", "chromium": "down" }`. Operator's monitor decides. |
| SIGINT during slow render                                            | Graceful shutdown waits up to 30s, then forces engine shutdown and exits. In-flight client receives 503 (TimeoutLayer) or connection close. |
| Concurrent identical requests                                       | Each gets its own page; results returned independently.                 |
| `extraHttpHeaders` not valid JSON                                   | 400 `{"code":"INVALID_OPTION","error":"extraHttpHeaders is not valid JSON"}`. |
| `cookies` JSON has unknown attributes                               | Unknown attrs ignored; documented in OpenAPI later.                     |
| Output too large to fit in 4 GiB Vec                                | Hypothetical; 500 with internal error. Not optimised for in MVP.        |

## Test plan

### Unit tests (`crates/server/src/...`)

- `app_state_clone_is_cheap` — `static_assertions` for `Clone + Send + Sync`.
- `parse_pdf_options_from_form_map_round_trip`.
- `parse_request_context_from_form_map_round_trip`.
- `extra_http_headers_invalid_json_returns_invalid_option`.
- `cookies_with_attrs_parse`.
- `fail_on_status_codes_parse`.
- `error_mapping_table` — for each `EngineError` variant produce the
  documented HTTP status + JSON shape.

### Router-level tests (`tower::ServiceExt::oneshot` against `Router`)

These do not launch real engines; they use a test double
(`ChromiumEngine` mocked behind a trait `PdfBackend`). The trait is
introduced *only* for the server's unit tests; production code uses the
concrete engine.

- `health_returns_200_when_engines_up`.
- `version_returns_pkg_version`.
- `chromium_html_returns_pdf_bytes_on_success` — mock returns
  `b"%PDF-1.7..."`.
- `chromium_html_400_on_missing_index_html`.
- `chromium_url_400_on_missing_url_field`.
- `chromium_html_504_when_backend_returns_timeout`.
- `chromium_html_502_when_backend_returns_navigation_error`.
- `chromium_screenshot_html_returns_png_on_success` — mock returns
  PNG bytes (`\x89PNG`).
- `chromium_screenshot_url_returns_jpeg_when_format_set` — mock returns
  JPEG bytes (`0xFF 0xD8`).
- `chromium_screenshot_markdown_returns_webp` — mock returns WebP.
- `body_too_large_returns_413`.
- `nonexistent_route_returns_404`.

### Integration tests (`crates/server/tests/`)

Marked `#[ignore]`, require Chrome and `soffice` on the host:

- `e2e_chromium_html` — start server on ephemeral port, POST a tiny
  HTML, assert PDF bytes returned.
- `e2e_chromium_url_against_local_axum_app`.
- `e2e_libreoffice_docx`.
- `e2e_pdfengines_merge_split_round_trip`.
- `graceful_shutdown_drains_inflight` — start a long render, send
  SIGINT, assert the in-flight request completes (or 503s cleanly) and
  the process exits within 35s.

### Smoke

A `crates/server/tests/smoke.sh` (or Rust harness) script `curl`s every
documented route against a launched server and asserts non-error
responses for a small fixture set. Runs in CI on Linux runners only.

## Acceptance

- [ ] All routes implemented per the table (including screenshot routes).
- [ ] Multipart parser handles repeated fields and named files
      (Gotenberg-style: `files` repeated; the *file name* matters for
      `index.html`).
- [ ] `axum`, `tower`, `tower-http`, `multer`, `tempfile`, `uuid`,
      `serde`, `serde_json`, `serde_urlencoded`, `tracing-subscriber`
      added via `workspace.dependencies`.
- [ ] Error mapping matches the table; covered by the dedicated unit test.
- [ ] CLI flags + env vars resolve in the documented precedence order
      (flag > env > default). Verified by a unit test on
      `ServerConfig::resolve(args, env)`.
- [ ] Graceful shutdown verified by the integration test.
- [ ] `cargo clippy -p server -- -D warnings` clean.
- [ ] No `unsafe`. No `unwrap`/`expect` outside `#[cfg(test)]`.
- [ ] Output content types: `application/pdf` for single PDFs,
      `application/zip` for multi, `application/json` for metadata read,
      `image/png`/`image/jpeg`/`image/webp` for screenshots.
- [ ] `Content-Disposition: attachment; filename="result.pdf"` (or
      `result.zip` / `result.json` / `result.{png|jpg|webp}`) on success.
- [ ] Screenshot routes return correct image format based on `format` field.

## Out of scope / follow-ups

- Webhook routes (`/forms/webhook`) — Gotenberg has them; defer until
  user demand.
- Full route set behind `/forms/pdfengines/*` (encrypt, watermark,
  stamp, rotate, embed, bookmarks) — wired as their backing `pdfops`
  functions land.
- Prometheus / OpenTelemetry exporters — separate optional feature.
- Multi-tenant API keys / quotas — assume reverse-proxy fronting.
- Hot-restart of crashed engines (today the process exits on engine
  death; a supervisor is expected externally).
