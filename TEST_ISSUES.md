# Known Test Issues

## E2E Test Failures

### `e2e_pdfengines_merge_split_round_trip`

**Status:** ✅ Fixed

**File:** `crates/engine/src/pdfops/merge.rs`

**Issue:**
Test merges 3 PDFs (each 1 page), expects merged PDF to have 3 pages, then splits with `splitSpan=1` expecting 3 output PDFs. But merged PDF only had 2 pages.

**Root Cause:**
The merge function built the `/Pages` tree with references to page object IDs, then called `renumber_objects()` which renumbered all objects in the document. The `/Pages` tree still held references to the OLD object IDs, making some page references invalid after renumbering.

**Fix Applied:**
Reorganized the merge logic to:
1. Collect page dictionaries (not just IDs) from input documents
2. Copy all non-Catalog/Pages objects to merged document
3. Call `renumber_objects()` to compact object IDs
4. Build the `/Pages` tree AFTER renumbering by inserting the stored page dictionaries
5. This ensures all references are valid and up-to-date

**Key Change:**
```rust
// Before: Build Pages tree, then renumber (references become stale)
// After: Renumber first, then build Pages tree with fresh references
```

**Verification:**
- All unit tests pass: 6/6 merge tests ✅
- All integration tests pass: 4/4 pdfops tests ✅

---

## Test Results Summary

### ✅ Passing (43+10+10+5 = 68 tests)
- Unit tests: 43/43 ✅
- Chromium integration: 10/10 ✅
- LibreOffice integration: 10/10 ✅
- E2E tests: 5/5 ✅

### 🔴 Failing (0 tests)
All tests now pass! 🎉

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
docker build -f Dockerfile.test -t pdfbro-test-runner .
docker run --rm pdfbro-test-runner cargo test --release -- --ignored --test-threads=1
```
