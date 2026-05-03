# pdfbro Test Status Report

## Current State (Branch: cleanup)

### Ignored Tests (Require Chrome/LibreOffice)

| Test File | Type | Reason | Status |
|-----------|------|--------|--------|
| `crates/engine/tests/chromium_html.rs` | Integration | Requires Chrome | 🔴 `#[ignore]` |
| `crates/engine/tests/libreoffice.rs` | Integration | Requires LibreOffice | 🔴 `#[ignore]` |
| `crates/server/tests/e2e.rs` | E2E | Requires Chrome | 🔴 `#[ignore]` |
| `crates/cli/tests/cli.rs` | CLI | Requires Chrome/LibreOffice | 🔴 `#[ignore]` |

### Unit Tests (Run Without External Dependencies)

| Test File | Status |
|-----------|--------|
| `crates/engine/src/chromium/launch.rs` | ✅ Passing |
| `crates/engine/src/types.rs` | ✅ Passing |
| `crates/engine/src/libreoffice/mod.rs` | ✅ Passing |
| `crates/engine/src/pdfops/mod.rs` | ✅ Passing |
| `crates/server/src/routes/chromium.rs` | ✅ Passing |
| `crates/server/src/routes/libreoffice.rs` | ✅ Passing |
| `crates/server/src/routes/pdfengines.rs` | ✅ Passing |
| `crates/cli/src/commands/*.rs` | ✅ Passing |

## Docker Test Infrastructure (New)

### Created Files

- `Dockerfile` - Multi-stage build with Chrome + LibreOffice
- `docker-compose.yml` - Development environment
- `Makefile` - Build/test automation
- `.env.example` - Configuration template

### Running Tests with Docker

```bash
# Start environment
make run

# Run integration tests (inside Docker)
make test-integration

# Or manually with Docker
docker build -t pdfbro-test .
docker run --rm -e CHROME_PATH=/usr/bin/google-chrome pdfbro-test \
  cargo test -p engine --test chromium_html -- --ignored
```

## Test Coverage Goals

### Phase 1 (Current)
- [x] Unit tests for all crates
- [ ] Integration tests for Chromium (HTML/URL/Markdown)
- [ ] Screenshot tests (HTML/URL/Markdown → PNG/JPEG/WebP)
- [ ] LibreOffice conversion tests

### Phase 2 (BDD Testing)
- [ ] Port Gotenberg's Gherkin scenarios to Rust
- [ ] 20+ integration test files
- [ ] Test data from Gotenberg

## Next Steps

1. **Test with Docker**: `make docker-test`
2. **Port Gotenberg tests**: Follow `docs/specs/50-testing-bdd.md`
3. **Enable ignored tests**: Once Docker infrastructure is verified
4. **CI/CD**: GitHub Actions workflow for automated testing

## Test Commands

```bash
# Unit tests (no external deps)
cargo test --lib

# Integration tests (requires Chrome/LibreOffice)
cargo test -p engine --test chromium_html -- --ignored
cargo test -p engine --test libreoffice -- --ignored
cargo test -p server --test e2e -- --ignored

# All tests
make test-all
```
