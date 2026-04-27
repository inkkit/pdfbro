# Known Test Issues

## E2E Test Failures

### `e2e_pdfengines_merge_split_round_trip`

**Status:** 🔴 Failing

**File:** `crates/server/tests/e2e.rs:277`

**Issue:**
Test merges 3 PDFs (each 1 page), expects merged PDF to have 3 pages, then splits with `splitSpan=1` expecting 3 output PDFs. But merged PDF only has 2 pages.

**Debug Output:**
```
DEBUG: merged PDF has 2 pages
DEBUG: input PDF 0 has 1 pages
DEBUG: input PDF 1 has 1 pages
DEBUG: input PDF 2 has 1 pages
DEBUG merge: total pages collected: 3
DEBUG merge: pages_in_order IDs: [(2, 0), (19, 0), (36, 0)]
DEBUG merge: Kids array has 3 elements
```

**Root Cause Analysis:**
The merge function in `crates/engine/src/pdfops/merge.rs`:
1. Collects 3 pages correctly (`pages_in_order` has 3 elements)
2. Builds `/Pages` tree with 3 kids (correct)
3. BUT: `merged.renumber_objects()` is called before `finalize()`
   - This renumbers ALL objects in the merged doc
   - The `/Pages` tree still references OLD object IDs
   - After renumbering, one page reference becomes invalid
   - When loading the saved PDF, `get_pages()` only finds 2 valid pages

**Attempted Fixes:**
1. Removed `merged.renumber_objects()` - made it worse (0 pages)
2. Added debug output - confirmed the issue is in renumbering

**Proper Fix Needed:**
Option A: Rebuild `/Pages` tree AFTER `renumber_objects()`
Option B: Don't call `renumber_objects()` and ensure object IDs are managed correctly
Option C: Use lopdf's `renumber_objects()` which should update references (might be buggy)

**Next Steps:**
1. Investigate how `lopdf::Document::renumber_objects()` works
2. Check if it updates references in the `/Pages` dictionary
3. Apply proper fix (likely Option A)

---

## Test Results Summary

### ✅ Passing (43+10+10+4 = 67 tests)
- Unit tests: 43/43 ✅
- Chromium integration: 10/10 ✅
- LibreOffice integration: 10/10 ✅
- E2E tests: 4/5 ✅

### 🔴 Failing (1 test)
- `e2e_pdfengines_merge_split_round_trip` - merge produces 2 pages instead of 3

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
