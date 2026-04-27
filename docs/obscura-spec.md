# Obscura - Technical Specification

> A lightweight headless browser for AI agents and web scraping, built in Rust.

## Overview

Obscura is a V8-based headless browser designed for automation at scale. It implements the Chrome DevTools Protocol (CDP) and serves as a drop-in replacement for headless Chrome with Puppeteer and Playwright.

## Architecture

### Crate Structure

| Crate | Purpose |
|------|---------|
| `obscura-dom` | HTML parsing, DOM tree management, CSS selectors |
| `obscura-net` | HTTP client, cookies, robots.txt, request interception |
| `obscura-browser` | Page lifecycle, browser context management |
| `obscura-cdp` | Chrome DevTools Protocol implementation |
| `obscura-js` | JavaScript runtime (V8 integration) |
| `obscura-cli` | CLI application entry point |

### Key Dependencies

- **V8** via `obscura-js` - JavaScript engine
- **Tokio** - Async runtime
- **Reqwest** - HTTP client
- **html5ever** - HTML parser
- **clap** - CLI argument parsing

## Performance Metrics

| Metric | Obscura | Headless Chrome |
|--------|--------|--------------|
| Memory | **30 MB** | 200+ MB |
| Binary size | **70 MB** | 300+ MB |
| Page load (static) | **51 ms** | ~500 ms |
| Page load (JS+XHR) | **84 ms** | ~800 ms |
| Startup | **Instant** | ~2s |

## Features

### Current Capabilities

1. **CDP Server** (`obscura serve`)
   - WebSocket-based CDP server on port 9222
   - Supports Puppeteer and Playwright connections
   - Multiple worker processes for parallelism

2. **Page Fetch** (`obscura fetch`)
   - Fetch and render single pages
   - Output formats: HTML, text, links
   - JavaScript evaluation support
   - Configurable wait conditions

3. **Parallel Scrape** (`obscura scrape`)
   - Concurrent URL scraping
   - Configurable concurrency
   - JSON/text output

4. **Stealth Mode**
   - Anti-fingerprinting (GPU, screen, canvas, audio, battery randomization)
   - Tracker blocking (3,520 domains blocklist)
   - `navigator.webdriver` masking
   - Event `isTrusted` emulation

### CDP Domains Implemented

| Domain | Implemented Methods |
|--------|-------------------|
| **Target** | createTarget, closeTarget, attachToTarget, createBrowserContext, disposeBrowserContext |
| **Page** | navigate, getFrameTree, addScriptToEvaluateOnNewDocument, lifecycleEvents |
| **Runtime** | evaluate, callFunctionOn, getProperties, addBinding |
| **DOM** | getDocument, querySelector, querySelectorAll, getOuterHTML, resolveNode |
| **Network** | enable, setCookies, getCookies, setExtraHTTPHeaders, setUserAgentOverride |
| **Fetch** | enable, continueRequest, fulfillRequest, failRequest |
| **Storage** | getCookies, setCookies, deleteCookies |
| **Input** | dispatchMouseEvent, dispatchKeyEvent |
| **LP** | getMarkdown (DOM-to-Markdown conversion) |

### Missing CDP Domains (for Gotenberg parity)

The following Chrome DevTools Protocol domains are **NOT YET IMPLEMENTED** in Obscura:

| Domain | Required For | Priority |
|--------|-------------|----------|
| **Page.printToPDF** | PDF generation | CRITICAL |
| **Page.captureScreenshot** | Screenshot capture | HIGH |
| **Emulation** | Device emulation, viewport control | HIGH |
| **Security** | Security details | MEDIUM |
| **Performance** | Performance metrics | MEDIUM |
| **Log** | Console/log management | LOW |
| **Accessibility** | AX tree | LOW |
| **CSS** | Computed styles, CSS rules | LOW |

## CLI Reference

### `obscura serve`

Start a CDP WebSocket server.

| Flag | Default | Description |
|------|---------|-------------|
| `--port` | 9222 | WebSocket port |
| `--proxy` | — | HTTP/SOCKS5 proxy URL |
| `--stealth` | off | Enable anti-detection + tracker blocking |
| `--workers` | 1 | Number of parallel worker processes |
| `--obey-robots` | off | Respect robots.txt |

### `obscura fetch <URL>`

Fetch and render a single page.

| Flag | Default | Description |
|------|---------|-------------|
| `--dump` | html | Output: html, text, or links |
| `--eval` | — | JavaScript expression to evaluate |
| `--wait-until` | load | Wait: load, domcontentloaded, networkidle0 |
| `--selector` | — | Wait for CSS selector |
| `--stealth` | off | Anti-detection mode |
| `--quiet` | off | Suppress banner |

### `obscura scrape <URL...>`

Scrape multiple URLs in parallel.

| Flag | Default | Description |
|------|---------|-------------|
| `--concurrency` | 10 | Parallel workers |
| `--eval` | — | JS expression per page |
| `--format` | json | Output: json or text |

## Code Organization

### Main Entry Point

`obscura-cli/src/main.rs` defines:
- `Args` struct with global flags
- `Command` enum: Serve, Fetch, Scrape
- Banner printing
- Worker process spawning

### Browser Context

`obscura-browser/src/context.rs`:
- Manages page lifecycle
- HTTP client configuration
- Cookie jar management
- Stealth mode settings

### Page Structure

`obscura-browser/src/page.rs`:
- `Page` struct with id, frame_id, url, dom, js_runtime
- Navigation methods
- Network event tracking
- Intercept configuration

### CDP Dispatch

`obscura-cdp/src/dispatch.rs`:
- Routes CDP methods to handlers
- Session management
- Event emission

### Domain Handlers

`obscura-cdp/src/domains/`:
- `target.rs` - Target management
- `page.rs` - Page navigation
- `runtime.rs` - JavaScript execution
- `dom.rs` - DOM operations
- `network.rs` - Network monitoring
- `fetch.rs` - Request interception
- `storage.rs` - Cookie storage
- `input.rs` - Mouse/keyboard events
- `browser.rs` - Browser control
- `lp.rs` - Markdown conversion

## Integration with External Tools

### Puppeteer

```javascript
import puppeteer from 'puppeteer-core';

const browser = await puppeteer.connect({
  browserWSEndpoint: 'ws://127.0.0.1:9222/devtools/browser',
});
```

### Playwright

```javascript
import { chromium } from 'playwright-core';

const browser = await chromium.connectOverCDP({
  endpointURL: 'ws://127.0.0.1:9222',
});
```

## Build Configuration

### Features

- `stealth` - Enables anti-detection and tracker blocking
- Default build: lightweight without stealth

### Cargo.toml Structure

```toml
[workspace]
members = [
    "crates/obscura-dom",
    "crates/obscura-net", 
    "crates/obscura-browser",
    "crates/obscura-cdp",
    "crates/obscura-js",
    "crates/obscura-cli",
]
```

## Limitations

1. **No built-in PDF generation** - Requires external PDF engine
2. **No REST API** - CLI-only interface
3. **Limited browser features** - Optimized for scraping, not full browser testing
4. **No extension support** - No Chrome extension APIs
5. **Basic print styling** - No advanced print CSS support

## Roadmap for Gotenberg Parity

To match Gotenberg's functionality, Obscura needs:

1. **Page.printToPDF implementation**
2. **Page.captureScreenshot implementation**
3. **REST API layer** (HTTP server)
4. **PDF operations** (merge, split, watermark, encrypt)
5. **LibreOffice integration** (for Office → PDF)

## License

Apache 2.0