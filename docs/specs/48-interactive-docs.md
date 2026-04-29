# Spec 48 — Interactive Documentation#

> Built-in API explorer and interactive docs. Gotenberg
> has static docs only. Folio should have "Try it now"
> buttons, live testing, and interactive API exploration.

## Goal#

Create an interactive documentation system that lets users
test Folio endpoints directly from the browser.
No external tools needed - just visit `/docs` and start
converting. This dramatically lowers the barrier to entry.

## Problem Analysis#

### Current State (Bad)#

#### Gotenberg#
- Static docs at `gotenberg.dev/docs`
- Users need `curl`/`postman` to test
- No way to "try before install"
- **User complaint**: *"I wish I could test if my HTML works before installing"*

#### Folio (Current)#
- Static docs in `/docs/`
- Same problems as Gotenberg

### Desired State (Good)#

- Visit `http://localhost:3000/docs`
- See all endpoints with examples
- Click "Try it" → auto-fills the form
- Submit → see live response
- Share example URLs with team

## Scope#

**In:**

- `GET /docs` - Interactive API explorer (HTML UI)
- `GET /docs/api/openapi.json` - OpenAPI/Swagger spec
- Live "Try it now" buttons on every endpoint
- Code samples in curl, Python, Node.js
- Response preview (PDF, JSON, image)
- Shareable example URLs
- Dark mode support

**Out:**

- Full Swagger UI (too heavy, build custom)
- API key management (separate feature)
- Rate limiting display (not needed for docs)

## Implementation#

### 1. OpenAPI Spec Generation#

```rust
// crates/server/src/docs/openapi.rs#

use serde::Serialize;

#[derive(Serialize)]
struct OpenApiSpec {
    openapi: String,
    info: Info,
    servers: Vec<Server>,
    paths: HashMap<String, PathItem>,
}

#[derive(Serialize)]
struct Info {
    title: String,
    version: String,
    description: String,
}

/// Generate OpenAPI 3.0 spec.
pub fn generate_openapi() -> OpenApiSpec {
    let mut paths = HashMap::new();

    // Chromium endpoints
    paths.insert(
        "/forms/chromium/convert/url".into(),
        PathItem {
            post: Some(Operation {
                summary: "Convert URL to PDF".into(),
                operation_id: Some("convertUrl".into()),
                request_body: Some(RequestBody {
                    content: hashmap! {
                        "multipart/form-data" => MediaType {
                            schema: Some(schema_for_chromium_convert())
                        }
                    },
                }),
                responses: responses_for_pdf(),
                ..
            }),
        }
    );

    // ... add all endpoints

    OpenApiSpec {
        openapi: "3.0.0".into(),
        info: Info {
            title: "Folio API".into(),
            version: env!("CARGO_PKG_VERSION").into(),
            description: "Gotenberg-compatible PDF generation API".into(),
        },
        servers: vec![
            Server {
                url: "http://localhost:3000".into(),
                description: Some("Local development".into()),
            }
        ],
        paths,
    }
}
```

### 2. Interactive HTML UI#

```html
<!-- crates/server/assets/docs/index.html -->

<!DOCTYPE html>
<html>
<head>
    <title>Folio API Docs</title>
    <style>
        /* Dark mode by default - modern look */
        :root {
            --bg: #1a1a2e;
            --surface: #2d2d44;
            --text: #e2e8f0;
            --accent: #7c3aed;
            --success: #10b981;
        }
    </style>
</head>
<body>
    <h1>📄 Folio API Documentation</h1>
    
    <div class="endpoint">
        <h2>POST /forms/chromium/convert/url</h2>
        <p>Convert any URL to PDF</p>
        
        <button onclick="tryIt('chromium-url')">Try it now</button>
        
        <div id="chromium-url" class="try-it">
            <input type="text" id="url" placeholder="https://example.com" value="https://example.com">
            <button onclick="submitForm('chromium-url')">Convert</button>
            <div id="result"></div>
        </div>
    </div>
    
    <script>
        function tryIt(id) {
            document.getElementById(id).style.display = 'block';
        }
        
        async function submitForm(id) {
            const url = document.getElementById('url').value;
            
            const formData = new FormData();
            formData.append('url', url);
            
            const response = await fetch('/forms/chromium/convert/url', {
                method: 'POST',
                body: formData
            });
            
            if (response.ok) {
                const blob = await response.blob();
                const url = URL.createObjectURL(blob);
                window.open(url);
            } else {
                const text = await response.text();
                document.getElementById('result').innerText = text;
            }
        }
    </script>
</body>
</html>
```

### 3. Endpoint Handler#

```rust
// crates/server/src/routes/docs.rs#

use axum::response::Html;

/// Serve interactive API documentation.
pub async fn docs_handler() -> Html<&'static str> {
    let html = include_str!("../../assets/docs/index.html");
    Html(html)
}

/// Serve OpenAPI spec as JSON.
pub async fn openapi_handler() -> Json<OpenApiSpec> {
    Json(generate_openapi())
}
```

### 4. Router Integration#

```rust
// crates/server/src/app.rs#

Router::new()
    .route("/docs", get(docs_handler))
    .route("/docs/api/openapi.json", get(openapi_handler))
    // ... other routes
```

### 5. "Try it Now" Code Samples#

```javascript
// Code sample generator
function generateCurl(endpoint, fields) {
    let cmd = `curl -X POST http://localhost:3000${endpoint} \\\n`;
    for (let [key, value] of Object.entries(fields)) {
        cmd += `  --form ${key}="${value}" \\\n`;
    }
    return cmd + '  -o output.pdf';
}

function generatePython(endpoint, fields) {
    return `import requests

response = requests.post(
    "http://localhost:3000${endpoint}",
    files={${Object.entries(fields).map(([k,v]) => `"${k}": open("${v}")`).join(', ')}
)
open("output.pdf", "wb").write(response.content)`;
}

function generateNode(endpoint, fields) {
    return `const axios = require('axios');
const fs = require('fs');

const form = new FormData();
${Object.entries(fields).map(([k,v]) => `form.append('${k}', '${v}');`).join('\n')}

axios.post('http://localhost:3000${endpoint}', form)
  .then(response => fs.writeFileSync('output.pdf', response.data));`;
}
```

## Expected Behaviour#

### Visit `/docs`#

```
📄 Folio API Documentation

[Endpoint List]
- POST /forms/chromium/convert/url  [Try it now]
- POST /forms/chromium/convert/html  [Try it now]
- ...

[Interactive Tester]
URL: [https://example.com          ]
[Convert] [View cURL] [View Python] [View Node]
```

### Response Preview#

- PDF: Auto-downloads and opens in new tab
- JSON: Pretty-printed with syntax highlighting
- Image: Rendered inline

### Shareable URLs#

```
http://localhost:3000/docs#endpoint=chromium-url&url=https://example.com
```

## Test Plan#

### Unit Tests#

- `openapi_spec_generates_valid_json`
- `code_sample_generator_curl`
- `code_sample_generator_python`

### Integration Tests#

- `docs_page_loads`
- `try_it_now_returns_pdf`
- `openapi_json_valid`

## Acceptance#

- [ ] `GET /docs` serves interactive UI
- [ ] `GET /docs/api/openapi.json` returns spec
- [ ] "Try it now" buttons on all endpoints
- [ ] Code samples in 3 languages
- [ ] PDF/JSON/image preview
- [ ] Dark mode support
- [ ] Shareable URLs
- [ ] Unit tests for OpenAPI generation
- [ ] Integration tests for docs page
- [ ] `cargo clippy -p server -- -D warnings` clean

## References#

- Swagger UI: https://swagger.io/tools/swagger-ui/
- OpenAPI 3.0: https://spec.openapis.org/oas/v3.0.3
- Gotenberg docs (static): https://gotenberg.dev/docs/
