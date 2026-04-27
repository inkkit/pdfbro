# Spec 40 — Python bindings (`py` crate)

> Self-contained PyO3 wrapper exposing `import folio` to Python users.
> No external HTTP service required at runtime.

## Goal

Allow Python users to convert HTML / URL / Markdown to PDF in-process via
the same `engine` crate the server uses, matching the README example in
`@/Users/__deesh_reddy__/projects/personal_git/rust_builds/folio/README.md:99-114`.

## Scope

**In:**

- `ChromiumEngine` Python class with `html_to_pdf`, `url_to_pdf`,
  `markdown_to_pdf`, `shutdown`, `healthy`.
- Exception hierarchy mapping each `EngineError` variant.
- `PdfOptions`, `RequestContext`, `BrowserConfig` exposed as Python
  `@dataclass`-style classes (constructed positionally or via kwargs).
- Type stubs (`folio.pyi`) shipped with the wheel.
- Wheels built by CI for cp3.9..cp3.13 on linux-x64/aarch64,
  macos-x64/arm64, win-x64.

**Out:**

- LibreOffice and pdfops surfaces — Python users for those use the HTTP
  server today. Follow-up spec.
- Async Python (`async def`) — Python remains synchronous; we
  `block_on` internally. Async support is a follow-up.
- Streaming PDF output (chunked writes) — return a single `bytes` for
  MVP.

## Public API

### Python surface (excerpt of `folio.pyi`)

```python
from typing import Any, Optional, Mapping, Sequence

class FolioError(Exception):
    """Base class for all engine errors raised by folio."""
    code: str        # e.g. "INVALID_OPTION", "TIMEOUT", "NAVIGATION", ...

class InvalidOptionError(FolioError): ...
class InvalidPageRangeError(FolioError): ...
class ChromeNotFoundError(FolioError): ...
class ChromeLaunchError(FolioError): ...
class CdpError(FolioError): ...
class NavigationError(FolioError):
    url: str
    reason: str
class TimeoutError(FolioError): ...
class IoError(FolioError): ...
class InternalError(FolioError): ...

class PaperSize:
    A4: "PaperSize"
    LETTER: "PaperSize"
    LEGAL: "PaperSize"
    A3: "PaperSize"
    A5: "PaperSize"
    def __init__(self, width_in: float, height_in: float) -> None: ...
    width_in: float
    height_in: float

class Margins:
    ZERO: "Margins"
    DEFAULT: "Margins"
    @staticmethod
    def uniform(inches: float) -> "Margins": ...
    def __init__(self, top: float, right: float, bottom: float, left: float) -> None: ...
    top: float
    right: float
    bottom: float
    left: float

class WaitCondition:
    @staticmethod
    def load() -> "WaitCondition": ...
    @staticmethod
    def dom_content_loaded() -> "WaitCondition": ...
    @staticmethod
    def network_idle() -> "WaitCondition": ...
    @staticmethod
    def selector(css: str) -> "WaitCondition": ...
    @staticmethod
    def expression(js: str) -> "WaitCondition": ...
    @staticmethod
    def delay(seconds: float) -> "WaitCondition": ...

class PdfOptions:
    def __init__(
        self, *,
        paper: PaperSize = ...,
        margin: Margins = ...,
        landscape: bool = False,
        scale: float = 1.0,
        print_background: bool = True,
        prefer_css_page_size: bool = False,
        emulate_media: str = "print",   # "print" | "screen"
        page_ranges: Optional[str] = None,
        header_template: Optional[str] = None,
        footer_template: Optional[str] = None,
        wait: WaitCondition = ...,
    ) -> None: ...

class Cookie:
    def __init__(
        self, name: str, value: str, *,
        domain: Optional[str] = None,
        path: Optional[str] = None,
        secure: bool = False,
        http_only: bool = False,
    ) -> None: ...

class RequestContext:
    def __init__(
        self, *,
        user_agent: Optional[str] = None,
        extra_headers: Optional[Mapping[str, str]] = None,
        cookies: Optional[Sequence[Cookie]] = None,
        fail_on_status: Optional[Sequence[int]] = None,
    ) -> None: ...

class BrowserConfig:
    def __init__(
        self, *,
        executable: Optional[str] = None,
        headless: bool = True,
        extra_args: Sequence[str] = (),
        no_sandbox: Optional[bool] = None,    # None = platform default
        timeout_secs: float = 60.0,
    ) -> None: ...

class ChromiumEngine:
    def __init__(self, config: Optional[BrowserConfig] = None) -> None: ...

    def html_to_pdf(
        self, html: str, *,
        base_url: Optional[str] = None,
        options: Optional[PdfOptions] = None,
        request: Optional[RequestContext] = None,
    ) -> bytes: ...

    def url_to_pdf(
        self, url: str, *,
        options: Optional[PdfOptions] = None,
        request: Optional[RequestContext] = None,
    ) -> bytes: ...

    def markdown_to_pdf(
        self, markdown: str, *,
        options: Optional[PdfOptions] = None,
        request: Optional[RequestContext] = None,
    ) -> bytes: ...

    def healthy(self) -> bool: ...

    def shutdown(self) -> None: ...

    # Context manager support (calls shutdown on exit):
    def __enter__(self) -> "ChromiumEngine": ...
    def __exit__(self, *exc_info: Any) -> None: ...

__version__: str
```

### Rust surface (`crates/py/src/lib.rs`)

```rust
use pyo3::prelude::*;

#[pymodule]
fn folio(py: Python<'_>, m: &PyModule) -> PyResult<()> {
    m.add("__version__", env!("CARGO_PKG_VERSION"))?;
    m.add_class::<py_types::PaperSize>()?;
    m.add_class::<py_types::Margins>()?;
    m.add_class::<py_types::WaitCondition>()?;
    m.add_class::<py_types::PdfOptions>()?;
    m.add_class::<py_types::Cookie>()?;
    m.add_class::<py_types::RequestContext>()?;
    m.add_class::<py_types::BrowserConfig>()?;
    m.add_class::<py_engine::ChromiumEngine>()?;
    py_errors::register(py, m)?;
    Ok(())
}
```

Internal modules:

- `py_types` — `#[pyclass]` wrappers around the engine's value types.
- `py_engine::ChromiumEngine` — wraps `Arc<engine::ChromiumEngine>` and a
  shared `tokio::runtime::Runtime`.
- `py_errors` — defines and registers the exception hierarchy.

## Behavior

### Runtime ownership

A single multi-thread tokio runtime is built lazily on first use and
reused across all engines in the process:

```rust
static RUNTIME: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
fn rt() -> &'static tokio::runtime::Runtime {
    RUNTIME.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .thread_name("folio-py")
            .build()
            .expect("tokio runtime build")
    })
}
```

Rationale: PyO3 modules are loaded once per process, so a `OnceLock` is
the standard idiom; multiple `ChromiumEngine` instances all share the
runtime.

### `ChromiumEngine.__init__`

1. Resolve config: `config or BrowserConfig()`.
2. Convert to `engine::types::BrowserConfig`.
3. `rt().block_on(engine::ChromiumEngine::launch_with(cfg))`.
4. Store `Arc<engine::ChromiumEngine>` inside the `#[pyclass]`.

### `html_to_pdf` / `url_to_pdf` / `markdown_to_pdf`

```rust
fn html_to_pdf(
    &self,
    py: Python<'_>,
    html: &str,
    base_url: Option<&str>,
    options: Option<&PdfOptions>,
    request: Option<&RequestContext>,
) -> PyResult<Py<PyBytes>> {
    let opts = options.map(|o| o.to_native()).unwrap_or_default();
    let req = request.map(|r| r.to_native()).unwrap_or_default();
    let engine = self.inner.clone();
    let html_owned = html.to_owned();
    let base = base_url.map(str::to_owned);

    py.allow_threads(|| {
        rt().block_on(async move {
            engine.html_to_pdf(&html_owned, base.as_deref(), &opts, &req).await
        })
    })
    .map_err(into_py_err)
    .map(|bytes| PyBytes::new(py, &bytes).into())
}
```

Critical points:

- `Python::allow_threads` releases the GIL during the async work.
- All inputs cloned into owned `String`s so the closure is `Send`.
- Output `Vec<u8>` re-acquires the GIL and is wrapped in `PyBytes`
  (`PyBytes::new` copies; that's acceptable in MVP).

### `markdown_to_pdf`

Same pattern as `html_to_pdf` but no `base_url` parameter.

### `healthy()`

`rt().block_on(self.inner.healthy())`. Holds the GIL across the call —
acceptable since `healthy` is bounded by `BrowserConfig::timeout`.

### `shutdown()` and context manager

- `shutdown` is idempotent. After the first successful call, subsequent
  calls raise nothing.
- `__exit__` calls `shutdown` and never re-raises engine errors when
  another exception is already in flight (logs at `warn` instead).

### Error mapping

All `EngineError`s convert to a corresponding Python exception. Each
exception class:

- Inherits from `FolioError`.
- Carries a string `code` attribute equal to the variant name (e.g.
  `"INVALID_OPTION"`).
- Preserves source-chain text in `__cause__` via
  `PyErr::set_cause` when the engine error has a `source()`.

Mapping table:

| `EngineError`                | Python class              | Extra attributes  |
|------------------------------|---------------------------|-------------------|
| `InvalidOption`              | `InvalidOptionError`      | —                 |
| `InvalidPageRange`           | `InvalidPageRangeError`   | —                 |
| `ChromeNotFound { searched }`| `ChromeNotFoundError`     | `searched: list[str]` |
| `ChromeLaunch(msg)`          | `ChromeLaunchError`       | —                 |
| `Cdp(msg)`                   | `CdpError`                | —                 |
| `Navigation { url, reason }` | `NavigationError`         | `url`, `reason`   |
| `Timeout(d)`                 | `TimeoutError`            | `seconds: float`  |
| `Io(_)`                      | `IoError`                 | —                 |
| `Internal(msg)`              | `InternalError`           | —                 |

Note: `folio.TimeoutError` shadows Python's builtin name *only* inside
the `folio` module's namespace; users who do `from folio import
TimeoutError` accept that. The class is importable as
`folio.TimeoutError`.

### Python type conversion

| Engine Rust type       | Python wrapper                  | Conversion            |
|------------------------|---------------------------------|-----------------------|
| `PaperSize`            | `PaperSize` `#[pyclass(frozen)]`| `to_native` cheap copy |
| `Margins`              | `Margins`                       | same                  |
| `WaitCondition`        | tagged enum mirrored in Python  | factory functions      |
| `MediaType`            | string ("print"/"screen")       | parsed in `PdfOptions::__init__` |
| `PageRanges`           | `Optional[str]`                  | parsed via spec 10's `PageRanges::parse` and re-stringified |
| `Cookie`               | `Cookie`                         | direct field copy     |
| `RequestContext`       | `RequestContext`                 | dict-like              |
| `BrowserConfig`        | `BrowserConfig`                  | direct                 |

Wrapper types implement `__repr__` returning a stable form like
`PaperSize(width_in=8.27, height_in=11.69)` and `__eq__` based on
field equality. They are NOT mutable from Python (`#[pyclass(frozen)]`).

### Threading

- Python instances are safe to share across threads (the wrapped
  `Arc<ChromiumEngine>` is `Sync`).
- The wrapper class is annotated with `#[pyclass(unsendable = false)]`
  and asserted via `static_assertions::assert_impl_all!`.

### Cleanup

- `__del__` is **not** implemented (avoids the GIL/destructor pitfall).
- `__exit__` covers the deterministic-cleanup path.
- If a `ChromiumEngine` is dropped without `shutdown`, the underlying
  Chrome process exits when the last `Arc` clone drops (chromiumoxide
  semantics). A `tracing::warn!` records this.

## Errors

Every public Python method only raises subclasses of `FolioError`,
`TypeError` (for misused kwargs caught by PyO3 type extraction), or
`ValueError` (for `PaperSize.__init__` etc. failures translated from
`EngineError::InvalidOption`).

## Edge cases

| Scenario                                                     | Required behavior                                                  |
|--------------------------------------------------------------|--------------------------------------------------------------------|
| `ChromiumEngine()` while no Chrome is on PATH                | Raises `ChromeNotFoundError(searched=[...])`.                      |
| `html_to_pdf("")` with default options                        | Returns valid PDF bytes (delegates to engine).                     |
| Calling `html_to_pdf` after `shutdown()`                      | Raises `InternalError` with the documented engine message.         |
| Multiple Python threads calling concurrently                  | Allowed; GIL released during each call; engine handles concurrency.|
| `with ChromiumEngine(...) as e: raise RuntimeError`           | `__exit__` runs shutdown but does not mask the user exception.     |
| Garbage collection while a render is in flight                | The wrapper holds an `Arc` so the engine is alive until the future resolves. |
| `PdfOptions(emulate_media="invalid")`                         | `ValueError("emulate_media must be 'print' or 'screen'")`.         |
| `Cookie(name="", value="x")`                                  | `ValueError("cookie name must not be empty")`.                     |
| Passing a dict where a wrapper class is expected              | Allowed in MVP only for `RequestContext.extra_headers`. Other params require typed instances. |

## Test plan

### Rust unit tests (`crates/py/src/...`)

- `paper_size_constants_match_engine`.
- `wait_condition_factory_round_trip`.
- `request_context_extra_headers_dict_to_native`.
- `error_conversion_table` — for each `EngineError` variant, build a
  `PyErr` and assert its class name and `code` attribute.

### Python integration tests (`crates/py/tests/test_folio.py`)

Run via `pytest` against the built wheel (or `maturin develop`).

Without Chrome (skipped if absent):

- `test_module_has_version`.
- `test_paper_size_constants`.
- `test_pdf_options_kwargs_round_trip`.
- `test_invalid_emulate_media_raises_valueerror`.
- `test_chromium_engine_constructs_and_reports_chrome_not_found_when_path_unset`
  (sets a bogus `LIBREOFFICE_PATH` is irrelevant; uses a bogus
  `BrowserConfig(executable="/no/such")`).

With Chrome (`pytest.mark.skipif(not has_chrome())`):

- `test_html_to_pdf_returns_pdf_bytes` — bytes start with `b"%PDF-"`.
- `test_url_to_pdf_against_local_http_server`.
- `test_markdown_to_pdf_renders_table`.
- `test_concurrent_calls_from_threads`.
- `test_context_manager_shuts_down_on_exit`.
- `test_shutdown_is_idempotent`.
- `test_navigation_error_carries_url_and_reason`.
- `test_timeout_error_raised_when_selector_never_appears`.

### Stub validation

- `mypy --strict crates/py/python/folio/__init__.pyi` runs as part of CI.
- `pyright` smoke check against the same stubs.

## Acceptance

- [ ] `crates/py/Cargo.toml` declares `[lib] crate-type = ["cdylib"]`,
      `name = "folio"`, depends on `pyo3` and `engine` (workspace).
- [ ] `crates/py/pyproject.toml` configures `maturin` builds with the
      target Python ABIs and platform list.
- [ ] `crates/py/python/folio/__init__.pyi` shipped in the wheel,
      exact signatures matching *Public API*.
- [ ] All listed Rust unit tests pass with `cargo test -p py`.
- [ ] All Python tests pass with `maturin develop` + `pytest`.
- [ ] `mypy --strict` passes against the stub.
- [ ] `cargo clippy -p py -- -D warnings` clean.
- [ ] No `unsafe` outside what PyO3 macros generate.
- [ ] `__version__` matches the workspace package version.
- [ ] Wheel size < 30 MiB on linux-x64 (sanity).

## Out of scope / follow-ups

- LibreOffice + pdfops Python surfaces — separate spec.
- Async Python API (`async def html_to_pdf`) — likely a `pyo3-async`
  follow-up; non-trivial because of the GIL/runtime dance.
- Streaming output via a Python file-like protocol.
- Type protocol exports for non-engine types (e.g. `Sequence[Cookie]`
  Protocols).
- Deeper structural typing (`TypedDict` for headers) once API stabilises.
