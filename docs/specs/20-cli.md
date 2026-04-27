# Spec 20 — `cli` (`folio` binary)

> User-facing command line for one-off conversions. Outline.

## Goal

Provide a `folio` binary that exercises the engine for HTML / URL /
Markdown / Office / PDF-ops conversions, matching the README usage in
`@/Users/__deesh_reddy__/projects/personal_git/rust_builds/folio/README.md:69-83`.

## Surface

```
folio [--config <path>] [-v|-vv|-q] <command>

Commands:
  convert     One-off conversion
  batch       Recursive conversion over a directory
  merge       Merge PDFs
  split       Split PDF
  serve       Forward to `folio-server` (convenience; spec 30)

`folio convert` subcommands / flags:
  --html <FILE>            HTML file input
  --url <URL>              Remote URL input
  --markdown <FILE>        Markdown file input
  --office <FILE>          Office document input
  --stdin                  Read HTML/MD from stdin (per --as flag)
  --as html|markdown       Stdin format (default html)
  --output <FILE>          Output PDF path; `-` for stdout

  PdfOptions surface (mirrors spec 10, kebab-case):
  --paper <a4|letter|legal|a3|a5|WxH>
  --landscape
  --margin <inches | "top,right,bottom,left">
  --scale <0.1..2.0>
  --no-print-background
  --emulate <print|screen>
  --pages <ranges>          e.g. "1-3,5,7-"
  --header-template <FILE>
  --footer-template <FILE>
  --wait <load|domcontentloaded|networkidle|selector:CSS|expr:JS|delay:DUR>
  --timeout <DUR>

  Browser config:
  --chrome <path>
  --no-sandbox / --sandbox

  Network (forwarded as RequestContext):
  --user-agent <STR>
  --header "Name: Value" (repeatable)
  --cookie "name=value;Domain=...;Path=..." (repeatable)
  --fail-on-status 4xx,5xx (repeatable)

`folio batch`:
  --input-dir <DIR> --output-dir <DIR>
  --pattern <glob>          default "**/*.{html,htm,md}"
  --concurrency <N>         default num_cpus
  + all `convert` PdfOptions flags
```

## Behavior

- Each subcommand maps directly to a single engine call.
- `batch` shares one `ChromiumEngine` across all renders, gated by a
  `tokio::sync::Semaphore(concurrency)`.
- Exit codes: `0` success, `2` usage error, `3` engine error, `4` timeout,
  `5` IO error. Errors print `{:#}` of the underlying `anyhow::Error`.

## To expand before implementation

- [ ] Confirm the exact clap derive struct hierarchy.
- [ ] Decide between `--stdin` only-for-html vs. supporting all input modes.
- [ ] Test plan with `assert_cmd` + `predicates` for golden CLI behavior.
- [ ] Shell-completion generation under `--shell-completion <bash|zsh|fish>`.
