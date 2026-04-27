# Spec 40 — Python bindings (`py` crate)

> PyO3 wrapper exposing `import folio`. Outline.

## Goal

Allow Python users to call into the engine without an external service:

```python
import folio
engine = folio.ChromiumEngine()
pdf = engine.html_to_pdf("<h1>Hi</h1>")
```

## Public surface

```python
class ChromiumEngine:
    def __init__(self, *, chrome: str | None = None, headless: bool = True,
                 no_sandbox: bool | None = None, timeout_secs: int = 60): ...
    def html_to_pdf(self, html: str, *, base_url: str | None = None,
                    options: PdfOptions | dict | None = None,
                    request: RequestContext | dict | None = None) -> bytes: ...
    def url_to_pdf(self, url: str, **kwargs) -> bytes: ...
    def markdown_to_pdf(self, md: str, **kwargs) -> bytes: ...
    def shutdown(self) -> None: ...

class PdfOptions: ...           # @dataclass-like; mirrors spec 10
class RequestContext: ...
class FolioError(Exception): ...
```

## Behavior

- Internally owns one `tokio::runtime::Runtime` (multi-threaded). Every
  Python call uses `runtime.block_on(...)`.
- Errors converted to a single `FolioError` exception subclass per
  `EngineError` variant via `__cause__`.
- `bytes` returned without copies via `PyBytes::new`.

## Build

- `crates/py/Cargo.toml` declares `[lib] crate-type = ["cdylib"]`,
  `name = "folio"`. `pyo3 = { workspace = true }`.
- `pyproject.toml` at workspace root configured for `maturin`.
- Wheels published from CI for cp3.9..cp3.13 on linux/macos.

## To expand before implementation

- [ ] PyO3 `#[pyclass]` definitions and `#[pymethods]` for each entrypoint.
- [ ] GIL-release strategy during render (`Python::allow_threads`).
- [ ] Type stubs (`folio.pyi`) checked into repo.
- [ ] `pytest`-based test plan.
