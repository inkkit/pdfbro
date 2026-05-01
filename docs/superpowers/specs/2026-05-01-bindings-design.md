# Folio Python & Node bindings — design

**Status:** approved (v1 = phase B; phase A documented, deferred)
**Date:** 2026-05-01
**Owner:** __deesh__

## Goal

Make Folio embeddable from Python and Node.js with a small, idiomatic API.
The user runs conversions inside their own process — no Folio HTTP server, no
Docker. The bindings reuse the existing `engine` crate, which already speaks
Chromium and LibreOffice behind cargo features.

Two phases:

- **Phase B (v1, this spec ships):** conversion only — HTML / URL / Markdown / Office → PDF.
- **Phase A (v2, designed here, deferred):** full parity — screenshots and every PDF op
  (merge, split, rotate, flatten, watermark, metadata, optimise, encrypt, bookmarks, PDF/A).

Phase A is documented end-to-end so v2 is mechanical.

## Why this shape

The user's stated call site is:

```python
folio = Folio(...supported options..., engines=["chromium", "office"])
pdf = folio.html_to_pdf(...)
```

That implies: in-process, configurable engine subset, on-demand Chrome if
absent. The design follows that literally for v1. Phase A widens the same
object with namespaced PDF operations.

## Non-goals

- Replacing the HTTP server. The server stays the supported deployment for
  multi-tenant workloads.
- Async Node sync wrappers. Node is async-native; users wrap with their own
  helpers if they want sync.
- Bundling Chrome inside the wheel/npm package. Chrome is fetched on first use
  and cached on disk — keeps package size sane.

## Architecture

```
crates/engine          existing — source of truth (async, tokio)
   └─ chrome_fetch     NEW: detect a usable Chrome; download + cache if missing
crates/py              PyO3 cdylib — sync Folio + async AsyncFolio
crates/js              napi-rs cdylib — async Folio
bindings/
   ├─ python/          maturin project: pyproject.toml, README, examples, tests
   └─ node/            napi-rs project: package.json, README, examples, tests
```

Why two roots? `crates/*` are Rust glue compiling to a `cdylib`. `bindings/*`
hold the language-side package metadata (pyproject.toml / package.json),
publishing scaffolding, examples, and tests in their native idiom. Keeps the
Rust workspace clean and gives each ecosystem an idiomatic project root.

### Module ownership

| Concern | Owner |
|---|---|
| Engine launch, Chromium / LibreOffice rendering | `engine` (unchanged) |
| Detect / download / cache Chrome | `engine::chrome_fetch` (new) |
| Type marshalling (Py ↔ engine types) | `crates/py` |
| Type marshalling (JS ↔ engine types) | `crates/js` |
| Tokio runtime ownership in sync Python | `crates/py` |
| Coroutine bridging in async Python | `crates/py` (via `pyo3-async-runtimes`) |
| Promise bridging in Node | `crates/js` (via `napi::tokio`) |
| Distribution / packaging | `bindings/python`, `bindings/node` |

## Engine selection at install time

The bindings forward to the engine's existing `chromium` / `libreoffice` cargo
features. Users opt into the subset they want.

| Install                             | Chromium | LibreOffice |
|-------------------------------------|----------|-------------|
| `pip install folio`                 | ✓        | ✓           |
| `pip install folio[chromium]`       | ✓        | ✗           |
| `pip install folio[office]`         | ✗        | ✓           |
| `npm install @folio/folio`          | ✓        | ✓           |
| `npm install @folio/folio-chromium` | ✓        | ✗           |
| `npm install @folio/folio-office`   | ✗        | ✓           |

Implementation: separate wheels / npm packages built with different
`--features` flags, published under variant names. Python uses extras + a
default metapackage; Node uses sibling packages.

**v1 ships only the default variant for both ecosystems** — both engines
included. Engine-subset variants land in v2 once the build matrix is proven.
This is documented so the absence isn't surprising.

## Chrome auto-download

A new module `engine::chrome_fetch`, behind a `chrome-fetch` cargo feature
(default-on for the bindings, off for the server which expects ops to
provision Chrome). Steps on first use:

1. **Detect a usable Chrome**, in order:
   - `BROWSER_PATH` / `CHROME_PATH` environment variable
   - `which chromium-browser`, `which chromium`, `which google-chrome`, `which chrome`
   - Platform default paths:
     - macOS: `/Applications/Google Chrome.app/Contents/MacOS/Google Chrome`,
       `/Applications/Chromium.app/Contents/MacOS/Chromium`
     - Linux: `/usr/bin/google-chrome`, `/usr/bin/chromium`,
       `/snap/bin/chromium`
     - Windows: `%PROGRAMFILES%\Google\Chrome\Application\chrome.exe` and the
       `(x86)` variant
   - The engine already does most of this in `BrowserConfig::executable`;
     `chrome_fetch::detect()` extracts and reuses that logic.
2. **If none found and `auto_download_chrome=true`** (default), fetch a
   pinned Chrome-for-Testing build for the current `(os, arch)` from
   Google's official manifest at
   `https://googlechromelabs.github.io/chrome-for-testing/known-good-versions-with-downloads.json`.
   The pinned version is recorded in `bindings/CHROME_VERSION` (one source of
   truth) and bumped on each Folio release.
3. **Cache** the extracted Chrome at:
   - macOS: `~/Library/Caches/folio/chromium/<version>/`
   - Linux: `${XDG_CACHE_HOME:-$HOME/.cache}/folio/chromium/<version>/`
   - Windows: `%LOCALAPPDATA%\folio\chromium\<version>\`
   The directory is overridable via `chrome_cache_dir` constructor argument
   and `FOLIO_CHROME_CACHE` env var (constructor wins).
4. **Verify** the SHA-256 from the manifest. Extract under a temp directory,
   then atomic-rename into place — never expose a half-extracted Chrome.
5. **Reuse** on subsequent runs: if `<cache>/<version>/chrome[.exe]` exists
   and is executable, skip the network entirely.
6. **Opt out**: `auto_download_chrome=False` (or `autoDownloadChrome: false`)
   raises `ChromeNotFoundError` if detection fails — never downloads silently.

The downloader stays in `engine` so the CLI and server can opt into it later
if useful. Server keeps it disabled by default to avoid surprise downloads in
containers.

## Phase B (v1): public API

### Python

Sync `Folio` and async `AsyncFolio`. They share the same engine; we expose
both because forcing one shape onto callers is painful in either direction:

- Async-only forces sync callers into `asyncio.run(...)`, which both adds loop
  setup overhead per call **and breaks inside an already-running loop**
  (Jupyter, FastAPI handlers, etc.) — a confusing failure mode for users who
  don't yet know they're inside a loop.
- Sync-only forces async callers into `loop.run_in_executor(None, ...)`,
  which works but ties up a thread per concurrent call.

Cost of providing both: ~50 extra lines of binding code. No engine
duplication.

```python
from folio import Folio, AsyncFolio, PdfOptions, OfficeOptions

# Sync — scripts, Django views, notebooks.
with Folio(
    engines=["chromium", "office"],   # subset of what's installed
    chrome_path=None,                  # explicit override
    auto_download_chrome=True,
    chrome_cache_dir=None,             # default: platform cache
) as folio:
    pdf: bytes = folio.html_to_pdf("<h1>hi</h1>", options=PdfOptions(landscape=True))
    pdf: bytes = folio.url_to_pdf("https://example.com")
    pdf: bytes = folio.markdown_to_pdf("# hello")
    pdf: bytes = folio.office_to_pdf("/path/to/deck.pptx", options=OfficeOptions(...))

# Async — FastAPI, asyncio apps.
async with AsyncFolio(...) as folio:
    pdf = await folio.html_to_pdf("<h1>hi</h1>")
```

`Folio` owns a tokio `Runtime` and `block_on`s engine futures. `AsyncFolio`
returns Python awaitables bound to the caller's running loop via
`pyo3-async-runtimes`. Both implement context-manager and explicit `close()`.

#### Sync GIL handling

All sync methods release the GIL around the engine call (`Python::allow_threads`)
so other Python threads make progress during long renders.

### Node

Async-only.

```js
import { Folio } from '@folio/folio';

const folio = await Folio.create({
  engines: ['chromium', 'office'],
  chromePath: null,
  autoDownloadChrome: true,
  chromeCacheDir: null,
});

const pdf = await folio.htmlToPdf('<h1>hi</h1>', { landscape: true });   // Buffer
const pdf2 = await folio.urlToPdf('https://example.com');
const pdf3 = await folio.markdownToPdf('# hello');
const pdf4 = await folio.officeToPdf('/path/to/deck.pptx');

await folio.close();
```

`napi::tokio` bridges Rust futures to JS Promises. The engine's `tokio::Runtime`
is shared with napi's runtime — no second runtime spun up.

### Type mapping

Engine types cross the FFI boundary as plain dicts/objects, validated on the
Rust side and turned back into typed engine structs.

| Engine type | Python | Node |
|---|---|---|
| `PdfOptions` | dataclass `PdfOptions` (snake_case) | object literal (camelCase), `PdfOptions` TS interface |
| `OfficeOptions` | dataclass | object literal + interface |
| `Margins`, `PaperSize`, `PageRanges`, `MediaType`, `WaitCondition` | dataclasses / enums | object literals / string-union types |
| `Vec<u8>` (PDF bytes) | `bytes` | `Buffer` |
| `EngineError` | `FolioError` exception hierarchy | typed `Error` subclasses |

#### Error hierarchy (both languages)

```
FolioError                       (base)
 ├─ ChromeNotFoundError          (no Chrome and auto-download disabled / failed)
 ├─ ChromeFetchError             (network / extract / verify failed)
 ├─ ChromiumError                (render failed inside Chrome)
 ├─ OfficeError                  (LibreOffice failed)
 ├─ EngineDisabledError          (called a method whose engine wasn't installed)
 ├─ TimeoutError                 (render exceeded configured timeout)
 └─ ValidationError              (bad PdfOptions etc., caught before engine call)
```

Mapping is defined once in the engine (`From<EngineError>`) and the bindings
just route to the right Python class / JS subclass.

### Lifecycle

- Construction launches Chromium and/or LibreOffice eagerly so failures
  surface immediately — not on the first render.
- `close()` shuts both down. Idempotent. Logged at warn if called twice.
- Process exit without `close()`: best-effort `Drop` impl on the Rust side
  kills child processes; non-fatal if it fails (the OS will reap them).

## Phase A (v2): designed, deferred

Same `Folio` / `AsyncFolio` object grows methods grouped into namespaces,
each backed by an existing engine module. **None of these exist on the v1
object** — clean type surface, no half-features.

```
folio.screenshot.html(...) / url(...) / markdown(...)         # chromium::screenshot
folio.pdf.merge([...])                                        # pdfops::merge
folio.pdf.split(input, mode=...)                              # pdfops::split
folio.pdf.rotate(input, degrees, pages=...)                   # pdfops::rotate
folio.pdf.flatten(input)                                      # pdfops::flatten
folio.pdf.watermark(input, kind=..., options=...)             # pdfops::watermark
folio.pdf.optimise(input, preset=...)                         # pdfops::optimise_pdf
folio.pdf.metadata.read(input) / .write(input, meta)          # pdfops metadata
folio.pdf.bookmarks.read(input) / .write(input, bookmarks)    # bookmarks
folio.pdf.encrypt(input, password, ...) / .decrypt(...)       # encrypt
folio.pdf.to_pdfa(input, profile=...)                         # pdfa
```

For each: signatures match the Rust engine, types are mirrored as
dataclasses / TS interfaces. Implementation is a per-method binding shim —
no new engine work.

External tool dependencies for v2 (already required by the engine today):

- `optimise` ⇒ Ghostscript or qpdf (engine picks via `OptimiseBackend`)
- `encrypt` / `decrypt` ⇒ qpdf
- `to_pdfa` ⇒ Ghostscript

These are detected at runtime; missing tools raise `EngineDisabledError`
with a clear message naming the missing binary. Bindings do **not**
auto-install Ghostscript or qpdf — too many platform-specific footguns.
v2 docs include install instructions per platform.

v2 also adds the engine-subset packages (`folio[chromium]`, etc.) once CI
matrix is proven.

## Distribution

### Python

- Build with `maturin`. Wheel matrix:
  - Linux x86_64 + aarch64 (manylinux2014)
  - macOS x86_64 + aarch64
  - Windows x86_64
- Python ABI: abi3 against py38+ (via PyO3's `abi3-py38` feature) — one wheel
  serves Python 3.8 through current.
- CI: GitHub Actions matrix driving `maturin build`. Use `cibuildwheel` only
  if abi3 alone proves insufficient.
- Publish via `maturin publish` to PyPI.

### Node

- Build with `@napi-rs/cli`. Prebuilt `.node` files per `(os, arch, libc)`:
  - linux-x64-gnu, linux-x64-musl, linux-arm64-gnu, linux-arm64-musl
  - darwin-x64, darwin-arm64
  - win32-x64
- Loader package picks the right one at install time via
  `optionalDependencies` — the standard napi-rs pattern.
- Node ≥ 18 (napi8 / N-API 8 available).
- Publish to npm under `@folio/folio` (and the engine-subset siblings in v2).

## Testing

| Layer | Tool | What it covers |
|---|---|---|
| Rust unit | `cargo test -p py -p js` | Pure type-mapping conversions, error mapping. No Python/Node runtime needed. |
| Rust unit | `cargo test -p engine --features chrome-fetch` | Chrome detection logic against fixture PATH; download path mocked. |
| Python | `pytest bindings/python/tests/` | Every public method on `Folio` and `AsyncFolio` against fixture HTML / Markdown / a tiny .docx. |
| Node | `vitest bindings/node/tests/` | Every public method on `Folio`. |
| E2E | One job in CI, gated on `FOLIO_E2E=1` | Real Chrome download → render fixture HTML → assert PDF magic bytes. Single matrix entry to keep CI cheap. |

Fixtures live in `bindings/fixtures/` and are shared by both languages.

## Decisions & open calls

These were made under auto mode; flag any to revisit.

1. **PyO3 0.22** (workspace pin). Fine for v1; bumping is a follow-up.
2. **napi-rs 2 with `napi8`** (workspace pin). Requires Node ≥ 18.
3. **Chrome version pin** = latest Chrome-for-Testing **Stable** at the time
   of each Folio release. Pin recorded in `bindings/CHROME_VERSION`.
4. **GIL released** around every sync engine call.
5. **No Chrome bundling** — fetched + cached on first use.
6. **v1 = default variant only** for both ecosystems; subset variants in v2.
7. **No auto-install of Ghostscript / qpdf** — v2 docs show how, runtime
   detection raises clear errors.

## Out of scope

- WebSocket / SSE streaming progress to language clients (server feature).
- Multi-tenant concurrency limits (use the server for that).
- A Folio-managed `LibreOffice` download. LibreOffice is too large and
  varied across distros; users install it themselves. The error when missing
  is explicit.

## Success criteria

- `pip install folio && python -c "import folio; folio.Folio().html_to_pdf('<h1>hi</h1>')"`
  produces a valid PDF on a clean machine without Chrome installed.
- The same flow on Node.
- Engine and server crates are unchanged in behaviour. `cargo test --workspace`
  stays green.
- Spec for Phase A is detailed enough that v2 is implementation, not design.
