# Spec 49 — Template Library#

> Pre-built document templates for common use cases.
> Users don't need to write HTML from scratch - just
> pick a template, fill in data, and get a perfect PDF.
> Unique to Folio (Gotenberg doesn't have this).

## Goal#

Create a library of professional document templates
that users can customize with their data. Solves the
"I don't know how to write HTML invoices" problem.

## Problem Analysis#

### Current State (Painful)#

**User workflows:**
1. User needs an invoice PDF
2. Searches web for "HTML invoice template"
3. Downloads sketchy HTML from questionable sites
4. Struggles to customize it
5. Converts to PDF → "Why does it look bad?"

**Quote from Gotenberg Discussion:**
> "I wish there was an invoice template. I spent 3 hours
> tweaking HTML/CSS before getting a decent PDF."
> — Gotenberg Discussion #850

### Desired State (Easy)#

1. User picks "Invoice Standard" template
2. Fills in JSON data: `{"company": "Acme", "amount": 1000}`
3. Gets perfect PDF in 2 seconds

## Scope#

**In:**

- Template library at `GET /templates`
- Pre-built templates:
  - Invoice (3 variants)
  - Report (2 variants)
  - Receipt (compact, thermal-printer friendly)
  - Letter (business, personal)
  - Certificate (award, completion)
- Template preview images at `GET /templates/{id}/preview`
- Data injection via JSON: `POST /forms/templates/{id}/render`
- Custom templates support (user-provided HTML)
- Template variables validation

**Out:**

- Template editor (too complex, use external tools)
- Drag-and-drop builder (separate product)
- Template marketplace (legal concerns)

## Implementation#

### 1. Template Definition#

```rust
// crates/server/src/templates/mod.rs#

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Template {
    pub id: String,
    pub name: String,
    pub description: String,
    pub category: TemplateCategory,
    pub thumbnail: String,      // URL to preview image
    pub fields: Vec<TemplateField>,
    pub html_template: String,  // Mustache/Handlebars template
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TemplateCategory {
    Invoice,
    Report,
    Receipt,
    Letter,
    Certificate,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateField {
    pub name: String,
    pub label: String,
    pub field_type: FieldType,
    pub required: bool,
    pub default: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FieldType {
    String,
    Number,
    Date,
    Boolean,
    Image,  // Base64 or URL
}
```

### 2. Built-in Templates#

```rust
// crates/server/src/templates/builtin.rs#

pub fn get_templates() -> Vec<Template> {
    vec![
        Template {
            id: "invoice-standard".into(),
            name: "Standard Invoice".into(),
            description: "Professional invoice with company logo, items table, totals".into(),
            category: TemplateCategory::Invoice,
            thumbnail: "/templates/invoice-standard/preview.png".into(),
            fields: vec![
                TemplateField {
                    name: "company_name".into(),
                    label: "Company Name".into(),
                    field_type: FieldType::String,
                    required: true,
                    default: None,
                },
                TemplateField {
                    name: "company_logo".into(),
                    label: "Company Logo URL".into(),
                    field_type: FieldType::Image,
                    required: false,
                    default: None,
                },
                TemplateField {
                    name: "invoice_number".into(),
                    label: "Invoice #".into(),
                    field_type: FieldType::String,
                    required: true,
                    default: Some(Value::String("INV-001".into())),
                },
                TemplateField {
                    name: "items".into(),
                    label: "Line Items (JSON array)".into(),
                    field_type: FieldType::String,  // JSON string
                    required: true,
                    default: None,
                },
                TemplateField {
                    name: "total".into(),
                    label: "Total Amount".into(),
                    field_type: FieldType::Number,
                    required: true,
                    default: None,
                },
            ],
            html_template: include_str!("templates/invoice-standard.html").into(),
        },
        // ... more templates
    ]
}
```

### 3. Template HTML Example#

```html
<!-- crates/server/assets/templates/invoice-standard.html -->

<!DOCTYPE html>
<html>
<head>
    <style>
        body { font-family: Arial, sans-serif; margin: 40px; }
        .header { display: flex; justify-content: space-between; }
        .logo { max-height: 80px; }
        .invoice-title { font-size: 32px; color: #1a1a2e; }
        table { width: 100%; border-collapse: collapse; margin: 20px 0; }
        th { background: #7c3aed; color: white; padding: 12px; text-align: left; }
        td { padding: 10px; border-bottom: 1px solid #e5e7eb; }
        .total { font-size: 24px; font-weight: bold; text-align: right; }
    </style>
</head>
<body>
    {{#company_logo}}
    <div class="header">
        <img src="{{company_logo}}" class="logo" alt="{{company_name}}">
    {{/company_logo}}
    <h1 class="invoice-title">Invoice {{invoice_number}}</h1>
    
    <div class="details">
        <p><strong>{{company_name}}</strong></p>
        <p>Date: {{date}}</p>
    </div>
    
    <table>
        <thead>
            <tr>
                <th>Description</th>
                <th>Qty</th>
                <th>Price</th>
                <th>Total</th>
            </tr>
        </thead>
        <tbody>
            {{#items}}
            <tr>
                <td>{{description}}</td>
                <td>{{qty}}</td>
                <td>${{price}}</td>
                <td>${{total}}</td>
            </tr>
            {{/items}}
        </tbody>
    </table>
    
    <div class="total">Total: ${{total}}</div>
</body>
</html>
```

### 4. Render Endpoint#

```rust
// crates/server/src/routes/templates.rs#

use handlebars::Handlebars;

/// List all available templates.
pub async fn list_templates() -> Json<Vec<Template>> {
    Json(templates::get_templates())
}

/// Render a template with user data.
pub async fn render_template(
    Path(template_id): Path<String>,
    Json(data): Json<Value>,
) -> ApiResult<impl IntoResponse> {
    let templates = templates::get_templates();
    let template = templates
        .iter()
        .find(|t| t.id == template_id)
        .ok_or_else(|| ApiError::InvalidOption(
            format!("Template not found: {}", template_id)
        ))?;

    // Render HTML from template
    let reg = Handlebars::new();
    let html = reg.render_template(&template.html_template, &data)
        .map_err(|e| ApiError::InvalidOption(
            format!("Template render error: {}", e)
        ))?;

    // Convert to PDF using Chromium
    let state = /* get state */;
    let opts = PdfOptions::default();
    let ctx = RequestContext::default();
    
    let pdf = state
        .chromium
        .as_ref()
        .unwrap()
        .html_to_pdf(&html, None, &opts, &ctx)
        .await?;

    pdf_response(pdf, &format!("{}.pdf", template_id))
}
```

### 5. Template Preview#

```rust
/// Get template preview image.
pub async fn preview_template(
    Path(template_id): Path<String>,
) -> ApiResult<impl IntoResponse> {
    let path = format!("assets/templates/{}/preview.png", template_id);
    
    if !Path::new(&path).exists() {
        return Err(ApiError::InvalidOption("Preview not found".into()));
    }
    
    let bytes = tokio::fs::read(&path).await
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    
    Ok((
        [(header::CONTENT_TYPE, HeaderValue::from_static("image/png"))],
        bytes,
    ))
}
```

## Form Fields#

| Field | Type | Description |
|-------|------|-------------|
| `data` | JSON | Template data to inject |
| `template_id` | string | Template identifier |

## Expected Behaviour#

### List Templates#

```bash
curl http://localhost:3000/templates
```

```json
[
  {
    "id": "invoice-standard",
    "name": "Standard Invoice",
    "description": "Professional invoice with company logo, items table, totals",
    "category": "invoice",
    "thumbnail": "/templates/invoice-standard/preview.png"
  }
]
```

### Render Template#

```bash
curl -X POST http://localhost:3000/forms/templates/invoice-standard/render \
  -H "Content-Type: application/json" \
  -d '{
    "company_name": "Acme Corp",
    "invoice_number": "INV-001",
    "items": [
      {"description": "Web Development", "qty": 1, "price": 1000}
    ],
    "total": 1000
  }' \
  -o invoice.pdf
```

## Test Plan#

### Unit Tests#

- `render_invoice_template`
- `template_variable_validation`
- `handlebars_renders_correctly`

### Integration Tests#

- `list_templates_returns_all`
- `render_template_returns_pdf`
- `invalid_template_returns_404`
- `preview_image_loads`

## Acceptance#

- [ ] `GET /templates` lists all templates
- [ ] 5+ built-in templates (invoice, report, receipt, etc.)
- [ ] `POST /forms/templates/{id}/render` endpoint
- [ ] Template preview images
- [ ] Handlebars templating engine
- [ ] JSON data validation
- [ ] Unit tests for all templates
- [ ] Integration tests for render endpoint
- [ ] `cargo clippy -p server -- -D warnings` clean

## References#

- Handlebars Rust: https://docs.rs/handlebars/
- HTML template examples: https://freehtml5.co/html5-templates/invoice/
- Gotenberg discussions: https://github.com/gotenberg/gotenberg/discussions
