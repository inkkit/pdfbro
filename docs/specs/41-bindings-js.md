# Spec 41 — Node bindings (`js` crate)

> Self-contained napi-rs wrapper exposing `require('folio')` (or
> `import folio from 'folio'`) to Node.js users.

## Goal

Allow Node.js users to convert HTML / URL / Markdown to PDF in-process
via the same `engine` crate, returning real `Promise`s without
`block_on`, matching the README example in
`@/Users/__deesh_reddy__/projects/personal_git/rust_builds/folio/README.md:125-137`.

## Scope

**In:**

- `ChromiumEngine` JS class with async methods `htmlToPdf`, `urlToPdf`,
  `markdownToPdf`, `healthy`, `shutdown`.
- Plain TS objects (interfaces) for `PdfOptions`, `RequestContext`,
  `BrowserConfig`, `Cookie`, `WaitCondition` (discriminated union).
- Auto-generated `.d.ts` shipped in the npm package.
- Prebuilt binaries on darwin-x64, darwin-arm64, linux-x64-gnu,
  linux-arm64-gnu, win32-x64-msvc.
- Node ≥ 18 (`napi8`).

**Out:**

- LibreOffice and pdfops surfaces — Node users use the HTTP server today.
  Follow-up.
- ESM-first published surface — package supports both CJS and ESM via
  `"exports"`, default export is the `ChromiumEngine` class.
- Streaming output / chunked Buffer responses — return one `Buffer` for MVP.
- Worker-thread isolation helpers — out of MVP.

## Public API

### TypeScript surface (auto-generated `index.d.ts`)

```ts
export type EmulateMedia = 'print' | 'screen';

export interface PaperSize {
    widthIn: number;
    heightIn: number;
}
export const PAPER_A4: PaperSize;
export const PAPER_LETTER: PaperSize;
export const PAPER_LEGAL: PaperSize;
export const PAPER_A3: PaperSize;
export const PAPER_A5: PaperSize;

export interface Margins {
    top: number; right: number; bottom: number; left: number;
}
export const MARGINS_ZERO: Margins;
export const MARGINS_DEFAULT: Margins;

export type WaitCondition =
    | { kind: 'load' }
    | { kind: 'domContentLoaded' }
    | { kind: 'networkIdle' }
    | { kind: 'selector'; selector: string }
    | { kind: 'expression'; expression: string }
    | { kind: 'delay'; durationMs: number };

export interface PdfOptions {
    paper?: PaperSize;
    margin?: Margins;
    landscape?: boolean;
    scale?: number;
    printBackground?: boolean;
    preferCssPageSize?: boolean;
    emulateMedia?: EmulateMedia;
    pageRanges?: string;
    headerTemplate?: string;
    footerTemplate?: string;
    wait?: WaitCondition;
}

export interface Cookie {
    name: string;
    value: string;
    domain?: string;
    path?: string;
    secure?: boolean;
    httpOnly?: boolean;
}

export interface RequestContext {
    userAgent?: string;
    extraHeaders?: Record<string, string>;
    cookies?: Cookie[];
    failOnStatus?: number[];
}

export interface BrowserConfig {
    executable?: string;
    headless?: boolean;
    extraArgs?: string[];
    noSandbox?: boolean;
    timeoutMs?: number;
}

export class ChromiumEngine {
    constructor(config?: BrowserConfig);
    htmlToPdf(html: string, opts?: { baseUrl?: string; options?: PdfOptions; request?: RequestContext }): Promise<Buffer>;
    urlToPdf(url: string, opts?: { options?: PdfOptions; request?: RequestContext }): Promise<Buffer>;
    markdownToPdf(markdown: string, opts?: { options?: PdfOptions; request?: RequestContext }): Promise<Buffer>;
    healthy(): Promise<boolean>;
    shutdown(): Promise<void>;
}

export class FolioError extends Error {
    code: string;        // e.g. 'INVALID_OPTION', 'TIMEOUT', 'NAVIGATION'
    /** Present only when code === 'NAVIGATION'. */
    url?: string;
    /** Present only when code === 'NAVIGATION'. */
    reason?: string;
    /** Present only when code === 'CHROME_NOT_FOUND'. */
    searched?: string[];
}

export const VERSION: string;
```

### Rust surface (`crates/js/src/lib.rs`)

```rust
use napi_derive::napi;

#[napi]
pub struct ChromiumEngine { /* Arc<engine::ChromiumEngine> */ }

#[napi]
impl ChromiumEngine {
    #[napi(constructor)]
    pub fn new(config: Option<BrowserConfigJs>) -> napi::Result<Self>;

    #[napi]
    pub async fn html_to_pdf(
        &self,
        html: String,
        opts: Option<HtmlToPdfArgs>,
    ) -> napi::Result<napi::bindgen_prelude::Buffer>;

    #[napi]
    pub async fn url_to_pdf(
        &self,
        url: String,
        opts: Option<UrlToPdfArgs>,
    ) -> napi::Result<napi::bindgen_prelude::Buffer>;

    #[napi]
    pub async fn markdown_to_pdf(
        &self,
        markdown: String,
        opts: Option<MarkdownToPdfArgs>,
    ) -> napi::Result<napi::bindgen_prelude::Buffer>;

    #[napi]
    pub async fn healthy(&self) -> bool;

    #[napi]
    pub async fn shutdown(&self) -> napi::Result<()>;
}
```

`BrowserConfigJs`, `PdfOptionsJs`, etc. are `#[napi(object)]` plain
structs that map directly to the TS interfaces above. Field names are
camelCase via `#[napi(js_name = "...")]` where rename is needed.

## Behavior

### Runtime / async

napi-rs ships with a built-in tokio integration: any `async fn`
annotated with `#[napi]` is converted into a JS `Promise` automatically.
**No** `block_on` is needed — napi-rs schedules futures on its own
runtime and resolves the JS Promise when the future completes.

To use the same engine across many calls efficiently we keep an
`Arc<engine::ChromiumEngine>` inside the napi class.

### `ChromiumEngine.constructor`

The constructor cannot be `async` in napi-rs; instead:

1. Build `engine::types::BrowserConfig` from the provided `BrowserConfigJs`.
2. Synchronously call `engine::ChromiumEngine::launch_with` via a small
   helper that uses `napi::tokio::block_on` (napi-rs exposes this for
   construction-time work).
3. Store the resulting engine in `Arc`.

If launch fails, throw a `FolioError` (see *Error mapping*). JS callers
see a thrown error from `new ChromiumEngine(...)`.

### `htmlToPdf` / `urlToPdf` / `markdownToPdf`

Each:

1. Convert `Option<*Args>` into the engine's owned types
   (`PdfOptions`, `RequestContext`, optional `base_url`).
2. Validate: `opts.options.validate()?`. Validation errors throw a
   `FolioError` with code `INVALID_OPTION`.
3. Call the corresponding `engine::ChromiumEngine` method.
4. Wrap the resulting `Vec<u8>` in `napi::bindgen_prelude::Buffer` (this
   is zero-copy: napi-rs hands ownership of the Rust `Vec` to V8).

### `healthy` / `shutdown`

- `healthy` mirrors the engine's method.
- `shutdown` is idempotent. Subsequent calls return `Ok(())` quickly.
  After shutdown, other methods reject with `FolioError(code = 'INTERNAL', message = 'engine shut down')`.

### Error mapping

Each `EngineError` variant produces a `napi::Error` with both:

- A `code` (also exposed as a property on the JS `Error` object).
- A `reason` string (used as the JS `Error.message`).

Mapping table:

| `EngineError`                | `code` (string)        | Extra props on `Error`        |
|------------------------------|------------------------|--------------------------------|
| `InvalidOption`              | `INVALID_OPTION`       | —                              |
| `InvalidPageRange`           | `INVALID_PAGE_RANGE`   | —                              |
| `ChromeNotFound { searched }`| `CHROME_NOT_FOUND`     | `searched: string[]`           |
| `ChromeLaunch(msg)`          | `CHROME_LAUNCH`        | —                              |
| `Cdp(msg)`                   | `CDP`                  | —                              |
| `Navigation { url, reason }` | `NAVIGATION`           | `url: string`, `reason: string`|
| `Timeout(d)`                 | `TIMEOUT`              | `seconds: number`              |
| `Io(_)`                      | `IO`                   | —                              |
| `Internal(msg)`              | `INTERNAL`             | —                              |

A small helper `into_napi_err(e: engine::EngineError) -> napi::Error`
handles this. Extra properties are attached via
`napi::Error::with_status` / `napi_create_error` and a JS-side wrapper
(`makeFolioError(rawErr)`) that copies fields onto a real `FolioError`
class instance. The JS wrapper lives in `crates/js/index.js` (or the
generated stub augmented post-build).

### Concurrency

A single `ChromiumEngine` instance is safe to use from any number of
concurrent JS calls (the underlying engine handles parallelism). Workers
created via `worker_threads` each get their own native instance — they
do not share state across the Worker boundary (this matches V8 isolation
guarantees and napi-rs's runtime model).

### Module shape

`require('folio')` returns the auto-generated module object with:

- `ChromiumEngine` class.
- `FolioError` class (defined in JS to allow `instanceof`).
- Constants (`PAPER_A4`, `MARGINS_DEFAULT`, etc.).
- `VERSION` string.

Distribution:

- `crates/js/package.json` is the published npm package, name `folio`.
- The Rust artifact is loaded via `@napi-rs/cli`'s host loader pattern;
  prebuilt binaries are downloaded by the post-install script per
  platform.

## Errors

Every public method throws (sync) or rejects (async) only with
`FolioError` instances. Type errors arising from incorrect JS argument
shapes produce `TypeError` (napi-rs default).

## Edge cases

| Scenario                                                     | Required behavior                                                  |
|--------------------------------------------------------------|--------------------------------------------------------------------|
| `new ChromiumEngine()` with no Chrome installed              | Throws `FolioError(code='CHROME_NOT_FOUND', searched=[...])`.       |
| `htmlToPdf("")`                                              | Resolves with a valid PDF Buffer.                                   |
| `htmlToPdf` after `await shutdown()`                         | Rejects with `FolioError(code='INTERNAL')`.                         |
| Many parallel `htmlToPdf` from event loop                    | All resolve; engine handles concurrency.                            |
| Caller passes `delay: { durationMs: -1 }`                    | `INVALID_OPTION` error.                                             |
| Caller passes `paper: { widthIn: 0, heightIn: 11 }`          | `INVALID_OPTION` error.                                             |
| User cancels by dropping the Promise                         | The render runs to completion (engine doesn't cancel mid-render in MVP); response is dropped harmlessly. |
| Large PDF (>1 GiB)                                           | Buffer transfer succeeds but allocation may fail; rejects with `INTERNAL`. Not optimised for in MVP. |
| GC of `ChromiumEngine` without `await shutdown()`            | The `Arc` keeps Chrome alive until last clone drops; emits a `tracing::warn!`. |
| Use from a `worker_thread`                                   | Each worker has its own instance; no cross-worker sharing.          |

## Test plan

### Rust unit tests (`crates/js/src/...`)

- `browser_config_js_to_native_round_trip`.
- `pdf_options_js_to_native_round_trip` — every field defaulted vs set.
- `wait_condition_discriminated_union_to_native` — every variant.
- `cookie_js_to_native_round_trip`.
- `error_mapping_table` — for each `EngineError` variant, build a
  `napi::Error`, assert `code` string and extra fields.

### JS integration tests (`crates/js/__tests__/folio.test.ts`)

Run via `vitest` against the built native module.

Without Chrome (skipped if absent):

- `module exports VERSION as semver`.
- `paper and margin constants frozen`.
- `creates ChromiumEngine and reports CHROME_NOT_FOUND when path is bogus`.
- `pdfOptions with invalid scale rejects`.

With Chrome (`describe.skipIf(!hasChrome())`):

- `htmlToPdf returns a Buffer starting with %PDF-`.
- `urlToPdf against a local http server`.
- `markdownToPdf renders a table`.
- `parallel calls all resolve`.
- `failOnStatus rejects with NAVIGATION carrying url and reason`.
- `selector wait timeout rejects with TIMEOUT carrying seconds`.
- `shutdown is idempotent and subsequent calls reject with INTERNAL`.
- `error.instanceof FolioError`.

### Type-level tests

- `tsd` snapshots assert that the generated `.d.ts` types match the
  documented surface; CI fails if the snapshot drifts.

### Build sanity

A CI job per platform builds the addon and runs the test suite.
Prebuilt binaries are uploaded via `@napi-rs/cli artifacts`.

## Acceptance

- [ ] `crates/js/Cargo.toml` declares `[lib] crate-type = ["cdylib"]`,
      `name = "folio"`, depends on `napi`, `napi-derive`, `engine`.
- [ ] `crates/js/package.json` is configured for `@napi-rs/cli` build,
      with platform-specific optional dependencies (`@folio/folio-darwin-arm64`
      style scoped sub-packages, or whatever the chosen distribution
      pattern is — to be finalised before publish).
- [ ] Auto-generated `index.d.ts` matches the documented surface
      (verified by `tsd` snapshot).
- [ ] All Rust unit tests pass with `cargo test -p js`.
- [ ] All JS tests pass with `npm test`.
- [ ] `cargo clippy -p js -- -D warnings` clean.
- [ ] `FolioError` JS class has subclass-friendly `instanceof` semantics
      (verified by test).
- [ ] No `unsafe` outside what `#[napi]` macros generate.
- [ ] Released package publishes a CJS entry point (`require('folio')`)
      and an ESM entry point (`import folio from 'folio'`).
- [ ] Wheel/binary size is reasonable (< 30 MiB per platform).

## Out of scope / follow-ups

- LibreOffice + pdfops surfaces — separate spec.
- AbortSignal cancellation of in-flight renders.
- Worker-thread shared engine handles via SharedArrayBuffer / message
  passing.
- Streaming output: writable-stream-friendly responses.
- ESM-only re-architecture once Node 22 is the floor.
- Direct N-API zero-copy when the engine learns to write into a
  pre-allocated buffer.
