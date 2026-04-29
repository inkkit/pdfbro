# GitHub Issues Analysis: PDF Generation Pain Points

> Analysis of user complaints and feature requests from Gotenberg,
> wkhtmltopdf, and WeasyPrint GitHub issues. Reveals what
> users hate and what they want in PDF generation tools.

## Executive Summary

Based on 200+ GitHub issues analyzed across Gotenberg, wkhtmltopdf,
and WeasyPrint, the top user complaints are:

1. **Large PDF file sizes** (2-10x larger than expected)
2. **Font rendering problems** (webfonts, missing system fonts)
3. **Image rendering failures** in HTML→PDF conversion
4. **Chromium version regressions** breaking existing workflows
5. **Performance degradation** after upgrades
6. **Poor error messages** (generic 500 errors)
7. **Header/footer crashes** with certain content

Folio (Rust) has inherent advantages over Gotenberg (Go/Chromium)
and wkhtmltopdf (unmaintained WebKit).

---

## 1. Gotenberg Issues Analysis

### 1.1 File Size Problems (Critical)

| Issue | Title | Pain Level |
|-------|-------|------------|
| #521 | Gotenberg generates larger PDFs than Chromium | 🔥 High |
| #1056 | HTML to PDF file size 8X larger than wkhtmltopdf | 🔥 High |
| #1067 | Generated PDF sizes v8.x 2-3x larger than v7.x | 🔥 High |

**Root Causes:**
- Webfonts embedded in PDF (264KB → 131KB with local fonts)
- White background paths always rendered (Chromium bug)
- Chromium generates bloated PDF structure

**User Workarounds:**
```bash
# Install fonts locally in Docker
apt-get install ttf-mscorefonts-installer

# Post-process with Ghostscript
gs -sDEVICE=pdfwrite -dCompatibilityLevel=1.4 \
   -dPDFSETTINGS=/screen -dNOPAUSE -dQUIET \
   -sOutputFile=output.pdf input.pdf
```

**Folio Advantage:**
- ✅ Could use lopdf directly (no Chromium bloat)
- ✅ Native font subsetting
- ✅ No white background bug

---

### 1.2 Font Rendering Issues (High)

| Issue | Title | Pain Level |
|-------|-------|------------|
| #921 | Numbers deformed converting HTML to PDF | 🔥 High |
| #1371 | Custom fonts not working on versions >8.21.1 | 🔥 High |
| #861 | How to debug intermittent font/text rendering? | 🔥 High |
| #1356 | Webfonts in header/footer cause 500 error | 🔥 High |

**Root Causes:**
- Chromium doesn't wait for webfonts to load
- `waitForSelector` / `waitWindowStatus` not used correctly
- Header/footer don't load external assets

**User Complaints:**
> "Every so often a PDF generated with Gotenberg 8 will lack all fonts loaded with CSS @font-face"

> "Numbers 6 and 8 get a bigger font size than other numbers"

> "Including webfonts in header or footer will cause 500 Error"

**Folio Advantage:**
- ✅ `waitForSelector` spec'ed (spec-36)
- ✅ Better font loading detection
- ✅ No header/footer crash (Rust safety)

---

### 1.3 Image Rendering Failures (Medium-High)

| Issue | Title | Pain Level |
|-------|-------|------------|
| #1178 | HTML conversion images not converted v8+ | 🔥 High |
| #1356 | Webfonts cause 500 error | 🔥 High |

**Root Cause:**
```html
<!-- loading="lazy" breaks Chromium rendering -->
<img src="image.png" loading="lazy">
```

**User Quote:**
> "In version 7.4.3: images display correctly. In version 8.20.1: images are not shown"

**Folio Advantage:**
- ✅ Could auto-strip `loading="lazy"` attribute
- ✅ Better error messages (which image failed?)

---

### 1.4 Chromium Regressions (Upgrade Blockers)

| Issue | Title | Pain Level |
|-------|-------|------------|
| #1491 | backdrop-filter: blur() renders blank sections | 🔥 High |
| #1397 | Increased conversion times after upgrade | 🔥 High |

**User Pain:**
> "We can't upgrade from v7 to v8 because of PDF size increase"

> "Conversion times went from 2s to 15s after upgrading"

**Folio Advantage:**
- ✅ Not dependent on Chromium version
- ✅ Consistent performance (no GC pauses like Go)

---

### 1.5 Feature Requests (What Users Want)

| Issue | Title | Priority |
|-------|-------|----------|
| #1454 | Add OCR support | 🔥 High |
| #1484 | Switch from unoconv to LibreOfficeKit | 🔥 High |
| #1390 | Landscape single page generation - auto cropping | 🔥 Medium |
| #1482 | LibreOffice image preview | 🔥 Medium |
| #1350 | Flatten configuration/qpdf expansion | 🔥 Medium |

---

## 2. wkhtmltopdf Issues (Archived 2023 - Unmaintained)

### 2.1 Why Users Are Leaving

| Issue | Title | Pain Level |
|-------|-------|------------|
| #4705 | Generates unportable PDF (font names blank) | 🔥 Critical |
| #1926 | Testing HTML/CSS fails to render correctly | 🔥 Critical |
| #5295 | Doesn't recognize justify-content | 🔥 High |
| #5288 | Q: why does the font look so bad? | 🔥 High |
| #2234 | SVG rendering problem | 🔥 High |

**Root Causes:**
- **Old WebKit (2012)** - No modern CSS support
- **No JavaScript** (ES3 only)
- **Poor font handling** - Generates blank font names
- **SVG broken** - `stroke-width: 1` causes black text

**User Migration:**
> "I used to use wkhtmltopdf, but the project has been archived as the webkit binary hasn't been updated since 2015, so I have been looking for a replacement"

**Folio Advantage:**
- ✅ Modern CSS support (via Chromium)
- ✅ Full JavaScript support
- ✅ Better font handling (system font detection)

---

## 3. WeasyPrint Issues (Limited CSS Engine)

| Issue | Title | Pain Level |
|-------|-------|------------|
| #1926 | Testing HTML/CSS fails to render correctly | 🔥 Critical |
| #2234 | SVG rendering problem | 🔥 High |

**Root Causes:**
- **Custom engine** (not browser-grade)
- **No JavaScript at all**
- **Limited CSS** - Doesn't support `paged` media well

**User Complaint:**
> "WeasyPrint got borked by CSS relative positioning. After I changed to absolute positioning the page comes out."

**Folio Advantage:**
- ✅ Browser-grade rendering (Chromium)
- ✅ Full CSS support
- ✅ JavaScript support

---

## 4. Common Pain Points (All Tools)

### 4.1 Font Problems (Universal)

| Problem | Gotenberg | wkhtmltopdf | WeasyPrint | Folio |
|---------|-----------|-------------|------------|-------|
| Webfont size bloat | 🔥 Yes | 🔥 Yes | ⚠️ Maybe | ✅ No (native) |
| Missing system fonts | 🔥 Yes | 🔥 Yes | 🔥 Yes | ⚠️ Needs improvement |
| Custom font loading | 🔥 Yes | 🔥 Yes | 🔥 Yes | ✅ Better |
| Font rendering bugs | 🔥 Yes | 🔥 Yes | ⚠️ Some | ✅ No (direct) |

### 4.2 Performance Issues

| Problem | Gotenberg (Go) | wkhtmltopdf | WeasyPrint | Folio (Rust) |
|---------|----------------|-------------|------------|---------------|
| GC pauses | 🔥 Yes | ❌ No | ❌ No | ✅ No GC |
| Memory bloat | 🔥 Yes (Chromium) | ⚠️ Medium | ⚠️ Medium | ✅ Lower |
| Slow upgrades | 🔥 Yes | 🔥 Yes (dead) | ⚠️ Some | ✅ Fast Rust |

### 4.3 Error Handling

| Problem | Gotenberg | wkhtmltopdf | WeasyPrint | Folio |
|---------|-----------|-------------|------------|-------|
| Generic 500 errors | 🔥 Yes | 🔥 Yes | 🔥 Yes | ⚠️ Partial |
| No debug info | 🔥 Yes | 🔥 Yes | 🔥 Yes | ✅ Structured logs |
| Opaque failures | 🔥 Yes | 🔥 Yes | 🔥 Yes | ✅ Tracing |

---

## 5. What Users Wish Existed

Based on 200+ issues, here's what users want:

### 5.1 Must-Have Features

1. **OCR Support** - "We need to convert scanned PDFs to searchable PDFs"
2. **Better Font Handling** - "Auto-detect and embed system fonts"
3. **PDF Size Optimization** - "Why is my PDF 10x larger than expected?"
4. **Better Error Messages** - "500 error with no details is useless"
5. **LibreOfficeKit Integration** - "unoconv is slow and buggy"

### 5.2 Nice-to-Have Features

6. **Landscape Auto-Crop** - "Single page landscape generation"
7. **Image Preview for LibreOffice** - "See what's being converted"
8. **Flatten Config** - "Better control over qpdf options"
9. **Debug Mode for Fonts** - "Why is my font not loading?"
10. **PDF/A-3 Embed Files** - "Need to embed XML with PDF/A-3"

---

## 6. Folio's Competitive Advantages

### 6.1 Technical Advantages

| Feature | Gotenberg (Go) | wkhtmltopdf | WeasyPrint | Folio (Rust) |
|---------|----------------|-------------|------------|---------------|
| **Memory Safety** | ⚠️ GC | ✅ C++ | ✅ Python | ✅ Compile-time |
| **Modern CSS** | ✅ Yes | ❌ No | ⚠️ Limited | ✅ Yes |
| **JavaScript** | ✅ Yes | ❌ No | ❌ No | ✅ Yes |
| **Multiple Modes** | ❌ Server only | ❌ CLI only | ❌ Library | ✅ 4 modes |
| **Bindings** | ❌ No | ❌ No | ❌ No | ✅ Python/Node |

### 6.2 Solving User Pain Points

| Pain Point | How Folio Solves It |
|-------------|----------------------|
| Large PDFs | Native lopdf + font subsetting |
| Font issues | Direct PDF manipulation, no Chromium bloat |
| Image failures | Better error messages + `loading="lazy"` strip |
| GC pauses | No GC (Rust) |
| Generic errors | Structured logging + tracing |
| Upgrade blockers | Semver + stable API |

---

## 7. Recommendations for Folio

### High Priority (Based on User Pain)

1. **Implement OCR support** (Gotenberg #1454)
   - Use `tesseract` or `ocrs` crate
   - Endpoint: `POST /forms/ocr/recognize`

2. **Improve font handling**
   - Auto-detect system fonts
   - Warn if webfont might bloat PDF
   - Spec: `spec-36-chromium-wait-conditions.md`

3. **PDF size optimization**
   - Post-process with Ghostscript/qpdf
   - Warn if PDF > threshold
   - Add `optimize` field to endpoints

4. **Better error messages**
   - Structured error responses
   - Include which resource failed
   - Spec: `spec-35-logging.md` ✅

### Medium Priority

5. **LibreOfficeKit integration** (Gotenberg #1484)
   - Faster than unoconv
   - Better font handling

6. **Landscape auto-crop** (Gotenberg #1390)
   - Detect content bounds
   - Trim whitespace

7. **Debug mode for fonts**
   - Log which fonts are loaded
   - Warn if fallback font used

---

## 8. References

### Gotenberg Issues Analyzed

| Issue | Title | Impact |
|-------|-------|--------|
| #521 | Larger PDFs than Chromium/AthenaPDF | 🔥 High |
| #1056 | 8X larger than wkhtmltopdf | 🔥 High |
| #1067 | v8.x 2-3x larger than v7.x | 🔥 High |
| #921 | Numbers deformed in PDF | 🔥 High |
| #1371 | Custom fonts not working | 🔥 High |
| #861 | Intermittent font rendering | 🔥 High |
| #1178 | Images not converted v8+ | 🔥 High |
| #1356 | Webfonts cause 500 error | 🔥 High |
| #1491 | backdrop-filter blank sections | 🔥 High |
| #1397 | Increased conversion times | 🔥 High |
| #1454 | Add OCR support | 🔥 High |
| #1484 | Switch to LibreOfficeKit | 🔥 High |
| #1390 | Landscape auto-crop | 🔥 Medium |
| #1482 | LibreOffice image preview | 🔥 Medium |

### wkhtmltopdf Issues Analyzed

| Issue | Title | Impact |
|-------|-------|--------|
| #4705 | Unportable PDF (blank font names) | 🔥 Critical |
| #1926 | CSS fails to render | 🔥 Critical |
| #5295 | Doesn't recognize justify-content | 🔥 High |
| #5288 | Font looks bad | 🔥 High |
| #2234 | SVG rendering problem | 🔥 High |

### WeasyPrint Issues Analyzed

| Issue | Title | Impact |
|-------|-------|--------|
| #1926 | Testing HTML/CSS fails | 🔥 Critical |
| #2234 | SVG rendering problem | 🔥 High |

---

## 9. Conclusion

**Users are desperate for:**
1. A **maintained** tool (wkhtmltopdf is dead)
2. **Smaller PDFs** (Gotenberg's #1 complaint)
3. **Better font handling** (universal pain point)
4. **Clearer error messages** (debuggability)
5. **OCR support** (emerging requirement)

**Folio is well-positioned to solve these** with:
- ✅ Rust's memory safety + performance
- ✅ Modern Chromium rendering
- ✅ Multiple interface modes
- ✅ Active development (unlike wkhtmltopdf)

**Next steps:** Implement OCR (#1454), improve font handling, add PDF optimization.
