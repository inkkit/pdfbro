# Spec 46 — PDF Size Estimator

> Proactively warn users about PDF size before conversion.
> Solves the #1 complaint: "PDFs 8x larger than
> wkhtmltopdf" (Gotenberg issues #521, #1056, #1067).

## Goal

Create a pre-flight estimation system that analyses
HTML/CSS/fonts/images and predicts output PDF size.
Gives users actionable warnings BEFORE they waste
time converting a document that will be too large.

## Problem Analysis#

### User Quotes (Gotenberg Issues)

> "Gotenberg generates larger PDFs than Chromium, AthenaPDF
> and Firefox... noticed a significant increase of file
> size... This unfortunately broke our integration with other
> tools, which enforce a file size limit"
> — Issue #521

> "HTML to PDF file size 8X larger than wkhtmltopdf...
> We recently switched from wkhtmltopdf to Gotenberg..."
> — Issue #1056

> "Generated PDF sizes with v8.x are ~2-3x larger
> than same generated PDF on v7.x... 286kb vs 795kb"
> — Issue #1067

### Root Causes Identified#

| Factor | Size Impact | Detection Method |
|--------|------------|-------------------|
| Web fonts (Google Fonts) | +200% | Scan CSS for @font-face |
| White background paths (Chromium bug) | +50% | Check printBackground=false |
| Images not optimised | +300% | Check image dimensions |
| Font not installed locally | +100% | Compare with system fonts |
| No compression applied | +400% | Check if Ghostscript needed |

## Scope#

**In:**

- `POST /estimate` - Analyse HTML/URL and return size prediction
- `POST /estimate/batch` - Estimate multiple URLs
- Size breakdown: fonts, images, markup, overhead
- Warning thresholds: 5MB (warn), 10MB (error)
- Suggestions: install fonts, optimise images, use Ghostscript
- Factor analysis: what contributes most to size
- Comparison: vs Gotenberg, vs wkhtmltopdf

**Out:**

- Actual conversion (that's other endpoints)
- File size limits (policy, not estimation)
- Automatic optimisation (see spec-42)

## Implementation#

### 1. Estimation Endpoint#

```rust
// crates/server/src/routes/estimate.rs

#[derive(Deserialize)]
struct EstimateRequest {
    url: Option<String>,
    html: Option<String>,
    files: Option<Vec<String>>,
}

#[derive(Serialize)]
struct EstimateResponse {
    estimated_size_mb: f64,
    confidence: String,  // "high", "medium", "low"
    breakdown: SizeBreakdown,
    warnings: Vec<String>,
    suggestions: Vec<String>,
    comparison: Option<Comparison>,
}

#[derive(Serialize)]
struct SizeBreakdown {
    fonts_mb: f64,
    images_mb: f64,
    markup_mb: f64,
    overhead_mb: f64,
}

pub async fn estimate(
    State(state): State<AppState>,
    Json(req): Json<EstimateRequest>,
) -> ApiResult<impl IntoResponse> {
    let mut breakdown = SizeBreakdown {
        fonts_mb: 0.0,
        images_mb: 0.0,
        markup_mb: 0.0,
        overhead_mb: 0.5,  // Base PDF overhead
    };

    let mut warnings = Vec::new();
    let mut suggestions = Vec::new();

    // Analyse HTML/CSS
    if let Some(ref html) = req.html {
        let analysis = analyse_html(html).await?;
        breakdown.markup_mb += analysis.markup_size_mb;
        breakdown.fonts_mb += analysis.font_size_mb;
        breakdown.images_mb += analysis.image_size_mb;

        if analysis.has_web_fonts {
            warnings.push(
                "Uses web fonts - may increase size by 200%".into()
            );
            suggestions.push(
                "Install fonts locally: apt-get install ttf-mscorefonts-installer".into()
            );
        }

        if analysis.large_images {
            warnings.push(
                "Contains large images - consider optimisation".into()
            );
        }
    }

    // Estimate total
    let estimated_mb = breakdown.fonts_mb
        + breakdown.images_mb
        + breakdown.markup_mb
        + breakdown.overhead_mb;

    // Add warnings based on thresholds
    if estimated_mb > 10.0 {
        warnings.push(format!(
            "Estimated size {:.1} MB exceeds 10 MB limit",
            estimated_mb
        ));
        suggestions.push(
            "Consider POST /forms/pdfengines/optimise after conversion".into()
        );
    } else if estimated_mb > 5.0 {
        warnings.push(format!(
            "Estimated size {:.1} MB is quite large",
            estimated_mb
        ));
    }

    Ok(Json(EstimateResponse {
        estimated_size_mb: estimated_mb,
        confidence: "medium".into(),
        breakdown,
        warnings,
        suggestions,
        comparison: None,  // TODO: compare with Gotenberg
    }))
}
```

### 2. HTML Analysis#

```rust
// crates/server/src/analysis/html.rs

struct HtmlAnalysis {
    markup_size_mb: f64,
    font_size_mb: f64,
    image_size_mb: f64,
    has_web_fonts: bool,
    large_images: bool,
}

async fn analyse_html(html: &str) -> Result<HtmlAnalysis, EngineError> {
    let mut result = HtmlAnalysis {
        markup_size_mb: (html.len() as f64) / 1_000_000.0,
        font_size_mb: 0.0,
        image_size_mb: 0.0,
        has_web_fonts: false,
        large_images: false,
    };

    // Check for web fonts
    if html.contains("@font-face") {
        result.has_web_fonts = true;
        // Estimate: each web font ~500KB
        let font_count = html.matches("@font-face").count();
        result.font_size_mb += font_count as f64 * 0.5;
    }

    // Check for images
    let img_pattern = regex::Regex::new(r#"img[^>]+src="([^"]+)""#).unwrap();
    for cap in img_pattern.captures_iter(html) {
        let src = &cap[1];
        if src.starts_with("http") || src.starts_with("data:") {
            result.large_images = true;
            result.image_size_mb += 1.0;  // Estimate
        }
    }

    Ok(result)
}
```

### 3. Batch Estimation#

```rust
/// Estimate multiple URLs at once.
pub async fn estimate_batch(
    State(state): State<AppState>,
    Json(req): Json<Vec<String>>,
) -> ApiResult<impl IntoResponse> {
    let mut results = Vec::new();

    for url in req {
        let estimate = estimate_single_url(&state, &url).await;
        results.push((url, estimate));
    }

    Ok(Json(BatchEstimateResponse { results }))
}
```

## Expected Behaviour#

### Estimation Request#

```json
POST /estimate
{
  "html": "<html><head><style>@font-face { font-family: 'Comic Sans'; src: url(font.woff2); }</style></head><body><p>Hello</p><img src=\"large.jpg\"></body></html>"
}
```

### Estimation Response#

```json
{
  "estimated_size_mb": 3.5,
  "confidence": "medium",
  "breakdown": {
    "fonts_mb": 2.0,
    "images_mb": 1.0,
    "markup_mb": 0.002,
    "overhead_mb": 0.5
  },
  "warnings": [
    "Uses web fonts - may increase size by 200%",
    "Contains large images - consider optimisation"
  ],
  "suggestions": [
    "Install fonts locally: apt-get install ttf-mscorefonts-installer",
    "Consider POST /forms/pdfengines/optimise after conversion"
  ]
}
```

### Size Thresholds#

| Estimated Size | Action |
|---------------|--------|
| <5 MB | ✅ Proceed (no warning) |
| 5-10 MB | ⚠️ Warning in response |
| >10 MB | 🔥 Error suggestion + optimisation tip |

## Test Plan#

### Unit Tests#

- `estimate_html_with_web_fonts`
- `estimate_html_with_large_images`
- `breakdown_calculates_correctly`
- `threshold_warnings_triggered`

### Integration Tests#

- `estimate_url_returns_valid_prediction`
- `batch_estimate_handles_10_urls`
- `web_fonts_warning_included`
- `optimisation_suggestion_provided`

## Acceptance#

- [ ] `POST /estimate` endpoint
- [ ] `POST /estimate/batch` endpoint
- [ ] Size breakdown: fonts/images/markup/overhead
- [ ] Warning thresholds: 5MB/10MB
- [ ] Web font detection
- [ ] Large image detection
- [ ] Suggestions for optimisation
- [ ] Unit tests for analysis functions
- [ ] Integration tests with real HTML
- [ ] `cargo clippy -p server -- -D warnings` clean

## References#

- Gotenberg issue #521: https://github.com/gotenberg/gotenberg/issues/521
- Gotenberg issue #1056: https://github.com/gotenberg/gotenberg/issues/1056
- Gotenberg issue #1067: https://github.com/gotenberg/gotenberg/issues/1067
- Web font size impact: https://github.com/puppeteer/puppeteer/issues/3939
