# Spec 43 — Font Doctor

> Diagnose and fix font-related rendering issues, the #2
> complaint across PDF generation tools. Provides endpoints to
> detect missing fonts, suggest fixes, and validate font loading.

## Goal

Create a comprehensive font diagnostics system that detects,
diagnoses, and helps fix font-related issues in PDF
generation. Addresses Gotenberg issues #921, #1371, #861
where users struggle with deformed numbers, missing fonts, and
intermittent rendering failures.

## Problem Analysis

### Real User Quotes (Gotenberg Issues)

> "Numbers 6 and 8 get a bigger font size than other
> numbers after conversion... The problem isn't with the HTML,
> everything renders just fine. After conversion the resulted
> PDF file shows this problem."
> — Issue #921

> "Every so often a PDF generated with Gotenberg 8 will
> lack all fonts loaded with CSS @font-face... It seems
> standard fonts work, the header and footer are both using
> font-family: 'Helvetica Neue', Helvetica, Roboto, Arial,
> sans-serif; I suppose a workaround could be to rebuild
> the Docker container"
> — Discussion #861

> "Custom fonts not working on versions >8.21.1...
> After upgrading to 8.30.0: The font stack was
> simplified from 30+ packages to 8. Documents relying on
> Microsoft Core Fonts now use metric-compatible replacements."
> — Issue #1371

### Root Causes

| Problem | Impact | Detection Method |
|----------|--------|-------------------|
| Font not installed in container | Deformed text, wrong fonts | Check system fonts |
| Web fonts not loaded in time | Missing text | `waitForSelector` + font check |
| Chromium font cache issues | Intermittent failures | Clear cache, retry |
| Fallback fonts used | Layout shifts | Compare requested vs actual |
| Large web fonts | 10x PDF size | Check font file sizes |

## Scope

**In:**

- `GET /debug/fonts` - List all system fonts
- `POST /debug/validate-fonts` - Check if fonts will render
- `POST /debug/diagnose-html` - Full font diagnostics for HTML
- Font loading wait mechanism (extend spec-36)
- Auto-suggestion for missing fonts
- Dockerfile generator for custom fonts

**Out:**

- Font installation via API (security risk)
- Automatic font downloading (copyright concerns)
- Font substitution algorithm (too complex)

## Implementation

### 1. Font Detection (`GET /debug/fonts`)

```rust
// crates/server/src/routes/debug.rs

use font_kit::source::SystemSource;

/// List all system fonts with metadata.
pub async fn list_fonts() -> ApiResult<impl IntoResponse> {
    let source = SystemSource::new();
    let fonts = source.all_fonts().map_err(|e| {
        ApiError::Internal(format!("Failed to list fonts: {}", e))
    })?;

    let font_list: Vec<FontInfo> = fonts
        .iter()
        .map(|(path, font)| FontInfo {
            name: font.name().to_string(),
            family: font.family_name().to_string(),
            style: format!("{:?}", font.style()),
            path: path.to_string_lossy().to_string(),
            size_bytes: std::fs::metadata(path)
                .map(|m| m.len())
                .unwrap_or(0),
        })
        .collect();

    Ok(Json(FontList { fonts: font_list }))
}

#[derive(Serialize)]
struct FontInfo {
    name: String,
    family: String,
    style: String,
    path: String,
    size_bytes: u64,
}

#[derive(Serialize)]
struct FontList {
    fonts: Vec<FontInfo>,
}
```

### 2. Font Validation (`POST /debug/validate-fonts`)

```rust
// Validate that fonts in CSS will render correctly

pub async fn validate_fonts(
    mp: Multipart,
) -> ApiResult<impl IntoResponse> {
    let form = parse_multipart(mp).await?;

    let mut html = form.get("html").cloned();
    let mut css = form.get("css").cloned();
    let url = form.get("url").cloned();

    // Extract font families from CSS/HTML
    let font_families = extract_font_families(html, css, url).await?;

    // Check each font
    let mut results = Vec::new();
    for family in font_families {
        let status = check_font_availability(&family).await;
        results.push(FontValidation {
            family: family.clone(),
            available: status.available,
            installed_font: status.installed_font,
            suggestion: status.suggestion,
        });
    }

    Ok(Json(FontValidationResponse { fonts: results }))
}

struct FontAvailability {
    available: bool,
    installed_font: Option<String>,
    suggestion: Option<String>,
}

async fn check_font_availability(family: &str) -> FontAvailability {
    let source = SystemSource::new();

    // Check if font is installed
    if let Ok(fonts) = source.select_family_by_name(family) {
        if !fonts.is_empty() {
            return FontAvailability {
                available: true,
                installed_font: Some(fonts[0].name().to_string()),
                suggestion: None,
            };
        }
    }

    // Not installed - suggest similar or default
    let suggestion = find_similar_font(family);

    FontAvailability {
        available: false,
        installed_font: None,
        suggestion: Some(format!(
            "Font '{}' not installed. {}",
            family,
            suggestion.unwrap_or_else(|| "Install via: apt-get install ttf-mscorefonts-installer".into())
        )),
    }
}
```

### 3. HTML Diagnostics (`POST /debug/diagnose-html`)

```rust
// Full diagnostics for an HTML file

pub async fn diagnose_html(
    State(state): State<AppState>,
    mp: Multipart,
) -> ApiResult<impl IntoResponse> {
    let form = parse_multipart(mp).await?;

    let html = form.get("html").ok_or_else(|| {
        ApiError::InvalidOption("html field required".into())
    })?;

    let mut diagnostics = HtmlDiagnostics {
        fonts: Vec::new(),
        warnings: Vec::new(),
        suggestions: Vec::new(),
    };

    // 1. Extract all font families
    let font_families = extract_font_families_from_html(&html);
    for family in font_families {
        let available = check_font_availability(&family).await;
        if !available.available {
            diagnostics.warnings.push(format!(
                "Font '{}' not installed",
                family
            ));
            if let Some(suggestion) = available.suggestion {
                diagnostics.suggestions.push(suggestion);
            }
        }
        diagnostics.fonts.push(FontDetail {
            family: family,
            installed: available.available,
            path: available.installed_font,
        });
    }

    // 2. Check for web fonts (will bloat PDF)
    if has_web_fonts(&html) {
        diagnostics.warnings.push(
            "HTML uses web fonts - PDF size may increase by 200%".into()
        );
        diagnostics.suggestions.push(
            "Install fonts locally in Docker: apt-get install ttf-mscorefonts-installer".into()
        );
    }

    // 3. Validate CSS @font-face declarations
    let font_face_issues = validate_font_face(&html).await?;
    diagnostics.warnings.extend(font_face_issues);

    Ok(Json(diagnostics))
}

#[derive(Serialize)]
struct HtmlDiagnostics {
    fonts: Vec<FontDetail>,
    warnings: Vec<String>,
    suggestions: Vec<String>,
}
```

### 4. Font Wait Mechanism (Chromium)

```rust
// Extend spec-36: wait for fonts to load

// In chromium/mod.rs render function
if let Some(ref font_wait) = opts.wait_for_fonts {
    // Wait for fonts to be loaded
    let js = format!(
        r#"
        const fontsLoaded = await document.fonts.ready;
        return fontsLoaded;
        "#
    );

    page.evaluate(&js).await.map_err(|e| {
        EngineError::Navigation {
            url: "font-wait".into(),
            reason: format!("Font loading timeout: {}", e),
        }
    })?;
}
```

### 5. Dockerfile Generator

```bash
# Generated Dockerfile for custom fonts

# Usage: POST /debug/generate-dockerfile
# Body: { "fonts": ["Comic Sans", "Helvetica Neue"] }

pub async fn generate_dockerfile(
    Json(request): Json<DockerfileRequest>,
) -> ApiResult<impl IntoResponse> {
    let mut dockerfile = vec![
        "FROM gotenberg/gotenberg:latest".to_string(),
    ];

    for font in &request.fonts {
        match font.as_str() {
            "Comic Sans" => {
                dockerfile.push("RUN apt-get update && apt-get install -y fonts-comic-sans".into());
            }
            "Helvetica Neue" => {
                dockerfile.push(
                    "COPY helvetica-neue.ttf /usr/share/fonts/truetype/".into()
                );
            }
            _ => {
                dockerfile.push(format!(
                    "# TODO: Add installation command for {}",
                    font
                ));
            }
        }
    }

    Ok(TextResponse(dockerfile.join("\n")))
}
```

## Expected Behaviour

### `GET /debug/fonts`

```json
{
  "fonts": [
    {
      "name": "Arial",
      "family": "Arial",
      "style": "Normal",
      "path": "/usr/share/fonts/truetype/arial.ttf",
      "size_bytes": 786432
    }
  ]
}
```

### `POST /debug/validate-fonts`

```json
{
  "fonts": [
    {
      "family": "Comic Sans",
      "available": false,
      "installed_font": null,
      "suggestion": "Font 'Comic Sans' not installed. Install via: apt-get install fonts-comic-sans"
    }
  ]
}
```

### `POST /debug/diagnose-html`

```json
{
  "fonts": [
    {"family": "Arial", "installed": true, "path": "/usr/share/fonts/arial.ttf"}
  ],
  "warnings": [
    "Font 'Helvetica Neue' not installed",
    "HTML uses web fonts - PDF size may increase by 200%"
  ],
  "suggestions": [
    "Install fonts locally in Docker: apt-get install ttf-mscorefonts-installer"
  ]
}
```

## Test Plan

### Unit Tests

- `list_fonts_returns_system_fonts`
- `check_font_availability_detects_missing`
- `extract_font_families_from_css`
- `validate_font_face_returns_errors`

### Integration Tests

- `diagnose_html_finds_missing_fonts`
- `validate_fonts_returns_suggestions`
- `dockerfile_generator_creates_valid_dockerfile`

## Acceptance

- [ ] `GET /debug/fonts` endpoint
- [ ] `POST /debug/validate-fonts` endpoint
- [ ] `POST /debug/diagnose-html` endpoint
- [ ] Font availability checking with suggestions
- [ ] Web font detection and warnings
- [ ] Dockerfile generator for custom fonts
- [ ] Unit tests for all font functions
- [ ] Integration tests with real HTML/CSS
- [ ] `cargo clippy -p server -- -D warnings` clean

## References

- Gotenberg issue #921: https://github.com/gotenberg/gotenberg/issues/921
- Gotenberg issue #1371: https://github.com/gotenberg/gotenberg/issues/1371
- Gotenberg discussion #861: https://github.com/gotenberg/gotenberg/discussions/861
- font-kit crate: https://docs.rs/font-kit/
- CSS @font-face spec: https://developer.mozilla.org/en-US/docs/Web/CSS/@font-face
