# Known Test Issues

## E2E Test Failures

### `e2e_pdfengines_merge_split_round_trip`

**Status:** 🔴 Failing

**File:** `crates/server/tests/e2e.rs:277`

**Issue:**
Test merges 3 PDFs (expects 3 pages), then splits by intervals (span=1). Expects 3 split PDFs but only gets 2.

**Error:**
```
assertion `left == right` failed
  left: 2  (actual split files)
 right: 3  (expected)
```

**Hypothesis:**
- Split logic with `splitMode=intervals` and `splitSpan=1` may have off-by-one error
- Or the merged PDF doesn't have 3 pages as expected

**Next Steps:**
1. Debug the split logic in `crates/engine/src/pdfops/split.rs`
2. Verify the merged PDF actually has 3 pages before splitting
3. Check if the split interval calculation is correct

---

## Test Results Summary (Docker)

### ✅ Passing (20 tests)
- Unit tests: 43/43 ✅
- Chromium integration: 10/10 ✅
- LibreOffice integration: 10/10 ✅
- E2E tests: 4/5 ✅

### 🔴 Failing (1 test)
- `e2e_pdfengines_merge_split_round_trip` - split produces 2 instead of 3 PDFs

---

## Docker Infrastructure Status

✅ **Working:**
- Dockerfile builds successfully
- Chromium (Chrome) installed and functional
- LibreOffice installed and functional
- All unit tests pass in container
- All Chromium integration tests pass (with `--test-threads=1`)
- All LibreOffice integration tests pass
- Most E2E tests pass

**Command to run all tests:**
```bash
docker build -f Dockerfile.test -t folio-test-runner .
docker run --rm folio-test-runner cargo test --release -- --ignored --test-threads=1
```
