# Spec 20 — `cli` (`folio` binary)

> User-facing command line for one-off conversions and PDF post-processing,
> built on `clap` derive and the `engine` crate.

## Goal

Provide a `folio` binary that exercises the engine for HTML / URL /
Markdown / Office conversions and basic PDF ops (merge / split), matching
the README usage in
`@/Users/__deesh_reddy__/projects/personal_git/rust_builds/folio/README.md:69-83`,
without needing the HTTP server.

## Scope

**In:**

- `convert` — single-file or single-input-source conversion.
- `batch` — directory walker that converts many files in parallel.
- `merge`, `split`, `flatten`, `metadata` — direct wrappers over spec 13.
- Shell-completion generation.
- Stdin/stdout streaming for pipelines.
- Structured logging behind `RUST_LOG`.

**Out:**

- `serve` subcommand. Users invoke `folio-server` directly. (CLI may
  later gain a thin `serve` shim, but not in the MVP.)
- Watermark / rotate / encrypt — exposed via the server first; CLI
  follow-up once the server fronting them is solid.

## Public surface

```
folio <COMMAND>

Global options (apply to every command):
  -v, --verbose        Increase log verbosity (-v info, -vv debug, -vvv trace)
  -q, --quiet          Suppress log output (overrides -v)
      --log-format <FORMAT>
                       text | json. Default: text on a TTY, json otherwise.
      --chrome <PATH>  Override Chrome executable (BrowserConfig::executable)
      --no-sandbox     Pass --no-sandbox to Chrome (default true on Linux)
      --sandbox        Force sandbox on (overrides --no-sandbox / Linux default)
      --timeout <DUR>  Per-render timeout, e.g. "60s", "2m". Default 60s.
  -h, --help
  -V, --version
```

### `folio convert`

Exactly one of `--html`, `--url`, `--markdown`, `--office`, `--stdin`.
Exactly one `--output` (path or `-` for stdout).

```
folio convert
  (--html <FILE> | --url <URL> | --markdown <FILE> | --office <FILE>
                  | --stdin --as <html|markdown>)
  --output <FILE>            FILE or '-' for stdout. Required.

  PdfOptions (apply to html/url/markdown; ignored for office):
  --paper <SIZE>             a4 | letter | legal | a3 | a5 | "WxH"
  --landscape
  --margin <SPEC>            inches (e.g. "0.5") or "TOP,RIGHT,BOTTOM,LEFT"
                             default 0.39in (~1cm)
  --scale <FLOAT>            0.1..=2.0
  --no-print-background
  --emulate <print|screen>
  --pages <RANGES>           e.g. "1-3,5,7-"
  --header-template <FILE>   Path to HTML file
  --footer-template <FILE>
  --prefer-css-page-size
  --wait <SPEC>              load | domcontentloaded | networkidle
                             | selector:CSS | expr:JS | delay:DUR

  RequestContext (html/url/markdown):
  --user-agent <STR>
  --header "Name: Value"     Repeatable
  --cookie "name=value;..."  Repeatable; ;-separated attrs
  --fail-on-status <SPEC>    Repeatable. e.g. "500", "5xx", "400-404"
  --base-url <URL>           For --html / --markdown / --stdin; ignored otherwise

  Office-only:
  --pdf-a <a1b|a2b|a3b>
  --pdf-ua
  --quality <1..=100>
  --max-image-resolution <DPI>
```

### `folio batch`

```
folio batch
  --input-dir <DIR>            Required. Walked recursively.
  --output-dir <DIR>           Required. Mirrors input directory tree.
  --pattern <GLOB>             Default: "**/*.{html,htm,md,markdown}"
  --concurrency <N>            Default: number of CPUs
  --on-error <stop|skip>       Default: skip
  --dry-run                    Print planned conversions, do nothing

  + every PdfOptions / RequestContext flag from `convert`

Each input is converted individually, with extension switched to .pdf
in the output tree. Office files are accepted iff `--pattern` includes
them; choose `--pattern "**/*.{docx,xlsx,pptx}"` etc.
```

### `folio merge`

```
folio merge --output <FILE> <INPUT>...

INPUT may be a path or '-' (read PDF bytes from stdin). Order is preserved.
```

### `folio split`

```
folio split <INPUT>
  --output-dir <DIR>           Required.
  --prefix <STR>               Default: input basename without extension.
  --mode <SPEC>                ranges:1-3,5,7- | every-n:5 | one-per-page
                               Default: one-per-page

Outputs: <prefix>-<NNN>.pdf, zero-padded. e.g. report-001.pdf.
```

### `folio flatten`

```
folio flatten <INPUT> --output <FILE>
INPUT or FILE may be '-' for stdio.
```

### `folio metadata`

```
folio metadata read <INPUT>            # JSON to stdout
folio metadata write <INPUT> --output <FILE> [--from-json <FILE> | --set KEY=VALUE]...
```

`--set` repeatable. Special keys: `Title`, `Author`, `Subject`,
`Keywords`, `Creator`, `Producer`, `CreationDate`, `ModDate`. Anything
else lands in `Metadata::custom`. Empty value (`--set Title=`) deletes.

### `folio completions <SHELL>`

Emits completion script to stdout. SHELL ∈ `bash | zsh | fish | powershell`.

## Behavior

### Process model

- One `tokio::runtime::Builder::new_multi_thread().enable_all().build()`
  built in `main`.
- All commands are short-lived; the runtime is dropped at exit.
- Logging configured with `tracing_subscriber::fmt()` with the chosen
  format. `--quiet` sets the level filter to `off`. `-v` to `info`,
  `-vv` to `debug`, `-vvv` to `trace`. `RUST_LOG`, when set, takes
  precedence (parsed by `tracing_subscriber::EnvFilter`).

### Engine reuse

- `convert`: launches one `ChromiumEngine` (or `LibreOfficeEngine`),
  performs one render, calls `shutdown` on success path, returns.
- `batch`: launches one engine, gates renders with
  `tokio::sync::Semaphore::new(concurrency)`, fans out via
  `tokio::task::JoinSet`, calls `shutdown` once all are joined.
- `merge`, `split`, `flatten`, `metadata`: no engine launch — pdfops are
  pure functions on byte buffers.

### Stdin / stdout

- `--stdin` reads raw bytes from `tokio::io::stdin` until EOF.
  `--as html` (default) treats them as a single HTML document; `--as markdown`
  feeds them to `markdown_to_pdf`.
- `--output -` writes PDF bytes to `stdout` *unbuffered* and disables
  any other stdout output (including the success log line) — so
  callers can pipe directly.
- `merge` accepts `-` as an input meaning "next chunk of bytes from
  stdin". Multiple `-`s are not allowed; stdin can only be consumed once.

### Option parsing helpers

- `--paper`: `a4`/`letter`/`legal`/`a3`/`a5` map to `PaperSize` constants.
  `WxH` parsed as two `f32`s separated by `x` (case-insensitive); both
  values are inches.
- `--margin`: a single value sets all four; `T,R,B,L` sets each in turn.
  Unit is inches. Examples: `--margin 0.5`, `--margin 1,0.5,1,0.5`.
- `--wait`:
  - `load` / `domcontentloaded` / `networkidle` map to the matching
    `WaitCondition` variant.
  - `selector:<CSS>` → `WaitCondition::Selector { selector }`.
  - `expr:<JS>` → `WaitCondition::Expression { expression }`.
  - `delay:<DUR>` → `WaitCondition::Delay { duration: parse_dur }`.
- `--cookie`: `name=value` followed by `;`-separated attributes
  `Domain=`, `Path=`, `Secure`, `HttpOnly`. Unknown attributes ignored.
- `--fail-on-status`: parses individual codes (`500`), wildcard families
  (`5xx`, `4xx`), or ranges (`500-503`). Resolved into `Vec<u16>`.
- All durations parsed by `humantime::parse_duration` (e.g. `5s`, `2m`,
  `500ms`).

### Logging fields

For each completed conversion:

```
INFO render
  source = "html|url|markdown|office"
  bytes_in = <usize>     (skipped for url)
  bytes_out = <usize>
  duration_ms = <u64>
  pages = <Option<u32>>  (extracted via `lopdf` after the fact)
```

For each error: `error.code = "<EngineError variant>"` and `error.message`.

### Exit codes

| Code | Meaning                                                |
|------|--------------------------------------------------------|
| 0    | Success.                                               |
| 1    | Generic / unexpected error (last-resort fallthrough).  |
| 2    | Usage / parse error (delegated to clap).               |
| 3    | Engine error (anything mapping to `EngineError`).      |
| 4    | Timeout (`EngineError::Timeout`).                      |
| 5    | I/O error reading inputs / writing outputs.            |
| 6    | Multiple errors in `batch` with `--on-error skip`.     |

In `--on-error skip` mode, a non-zero count of failures yields exit code
6 and a one-line summary on stderr.

### `batch` ordering

Walks via `walkdir::WalkDir`, collects matching paths into a stable
sorted order, schedules conversions in that order. Reported errors carry
the input path so users can correlate.

### `merge` / `split` correctness

- `merge` reads each input fully into memory before delegating to
  `engine::pdfops::merge`. Inputs validated as PDFs upon read; bad input
  fails fast with the path in the error.
- `split` filenames are zero-padded to fit the chunk count
  (`width = chunk_count.to_string().len()`, min 3).

## Errors

Mapped to exit codes per the table above. Error messages on stderr
follow this shape:

```
error: <one-line summary>
  caused by: <next layer>
  caused by: <leaf>
```

`anyhow`'s `{:#}` formatter is used. The error's source chain MUST
reach the originating `EngineError` variant.

## Edge cases

| Scenario                                                     | Required behavior                                                  |
|--------------------------------------------------------------|--------------------------------------------------------------------|
| `convert --html foo.html --url ...`                          | clap mutex group rejects → exit 2.                                  |
| `convert --output -` on a TTY                                | Allowed. Bytes go to stdout. Stderr still receives logs.            |
| `convert --output existing.pdf`                              | Overwrites. No prompt.                                              |
| `batch --input-dir A --output-dir A` (same directory)        | Refused; exit 2 with explanation.                                   |
| `batch --output-dir <does-not-exist>`                        | Created recursively (`fs::create_dir_all`).                         |
| `batch --concurrency 0`                                      | Treated as 1.                                                       |
| `--paper 0x0`                                                | Caught by spec 10 `PaperSize::new`; exit 3.                         |
| `--margin "1, 2"` (only two values)                          | Exit 2 with usage hint.                                             |
| `--cookie "novalue"` (no `=`)                                | Exit 2.                                                             |
| `--wait selector:` (empty)                                   | Exit 2.                                                             |
| `merge` with one input                                       | Allowed; bytes round-tripped through pdfops.                        |
| `merge` with zero inputs                                     | Exit 2 (clap requires at least one).                                |
| `merge -` consumed twice                                     | Exit 2 ("stdin can only be used once").                             |
| `metadata read` on encrypted PDF                             | Exit 3 with the documented engine message.                          |
| Long-running conversion → SIGINT                             | Engine receives `shutdown` via `tokio::signal`; exit 130.           |

## Test plan

Tests live in `crates/cli/tests/cli.rs` using `assert_cmd`,
`predicates`, and `tempfile`. Network-bound and Chrome-bound tests are
`#[ignore]`d by default.

### Unit tests (option parsers)

Exposed as `pub(crate)` for direct testing.

- `parse_paper_named`, `parse_paper_dimensions`, `parse_paper_invalid`.
- `parse_margin_single_value_uniform`.
- `parse_margin_four_values_in_order`.
- `parse_margin_wrong_count`.
- `parse_wait_simple_keywords`.
- `parse_wait_selector`.
- `parse_wait_expression`.
- `parse_wait_delay`.
- `parse_cookie_with_attrs`.
- `parse_cookie_missing_value`.
- `parse_fail_on_status_codes_and_wildcards`.

### Command-level tests (`assert_cmd`)

Without engine:

- `version_subcommand_outputs_semver_string`.
- `convert_requires_one_input_source`.
- `convert_requires_output`.
- `merge_with_no_inputs_exits_2`.
- `split_default_mode_one_per_page` — using a tiny canned PDF.
- `metadata_read_round_trips_via_write` — pure pdfops.
- `flatten_idempotent_via_cli` — pure pdfops.
- `completions_emits_bash_script` — output starts with `_folio()`.
- `usage_error_exits_2`.
- `engine_error_path_exits_3` — invoke `convert --html nonexistent.html`.

With Chrome (`#[ignore]`):

- `convert_html_to_stdout_pipes_bytes` — `… --output -` produces
  bytes starting with `%PDF-`.
- `convert_url_to_pdf_against_local_axum`.
- `batch_smoke_two_files_into_two_pdfs`.
- `batch_skip_on_error_exits_6_with_summary`.

With LibreOffice (`#[ignore]`):

- `convert_office_writer_doc`.
- `convert_office_with_pdf_a_2b`.

### Logging / output golden tests

- `log_format_json_emits_valid_json_per_line` — capture stderr, parse
  each line via `serde_json::from_str`.
- `log_format_text_does_not_emit_color_when_piped`.

## Acceptance

- [ ] `crates/cli/src/main.rs` compiles, plus `commands/`, `args/`,
      `parse/` submodules as needed.
- [ ] `clap = { workspace = true, features = ["derive", "env"] }`,
      `clap_complete`, `assert_cmd`, `predicates`, `humantime`,
      `walkdir`, `tracing-subscriber` wired via `workspace.dependencies`.
- [ ] Top-level binary name is `folio` (already set in `crates/cli/Cargo.toml`).
- [ ] `folio convert --help` matches the surface above (golden test
      against the rendered help).
- [ ] All listed unit tests pass.
- [ ] All non-ignored integration tests pass.
- [ ] `--ignored` integration tests pass on a host with Chrome and `soffice`.
- [ ] `cargo clippy -p cli -- -D warnings` clean.
- [ ] No `unwrap`/`expect` outside test code.
- [ ] Exit codes match the documented table (assert via `assert_cmd`).

## Out of scope / follow-ups

- `serve` subcommand fronting `folio-server`.
- Interactive TUI mode.
- Configuration file support (e.g. `folio.toml` discovered by ancestor
  walk) — defer until users ask.
- Watermark / rotate / encrypt CLI subcommands — once spec 13 covers
  them server-side.
- Progress bars in `batch` — defer; logs cover it.
