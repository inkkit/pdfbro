# Gap Analysis: Obscura → Gotenberg Chromium Replacement

> Obscura as a drop-in replacement for Chromium inside Gotenberg - mapping required CDP methods.

## Context

Obscura is being developed to **replace Chromium** inside Gotenberg. This means Obscura needs to implement the CDP methods that Gotenberg's Chromium module calls.

**Current Obscura**: Supports navigation, JS execution, basic CDP
**Target**: Replace Chromium in Gotenberg's PDF generation pipeline

---

## CDP Methods Gotenberg Uses

Gotenberg's Chromium module uses these CDP methods (from `chromium/tasks.go` and `chromium/events.go`):

### Navigation & Page

| CDP Method | Used In | Obscura Status | Priority |
|------------|--------|---------------|-------------|
| `Page.navigate` | URL fetching | ✓ Implemented | CRITICAL |
| `Page.getLayoutMetrics` | PDF sizing | ✗ MISSING | CRITICAL |
| `Page.printToPDF` | PDF generation | ✗ MISSING | CRITICAL |
| `Page.captureScreenshot` | Screenshot | ✗ MISSING | HIGH |

### Emulation

| CDP Method | Used In | Obscura Status | Priority |
|------------|--------|---------------|-------------|
| `Emulation.setDeviceMetricsOverride` | Viewport size | ? Verify | HIGH |
| `Emulation.setUserAgentOverride` | Custom UA | ✓ Implemented | HIGH |
| `Emulation.setScriptExecutionDisabled` | JS disable | ? Verify | MEDIUM |
| `Emulation.setDefaultBackgroundColorOverride` | Transparent BG | ? Verify | MEDIUM |
| `Emulation.setEmulatedMedia` | Print media | ? Verify | LOW |

### Network

| CDP Method | Used In | Obscura Status | Priority |
|------------|--------|---------------|-------------|
| `Network.clearBrowserCache` | Cache clear | ? Verify | LOW |
| `Network.clearBrowserCookies` | Cookie clear | ? Verify | LOW |
| `Network.setCookie` | Cookie setting | ✓ Implemented | HIGH |

### Runtime/Evaluation

| CDP Method | Used In | Obscura Status | Priority |
|------------|--------|---------------|-------------|
| `Runtime.evaluate` | JS execution | ✓ Implemented | CRITICAL |
| DOM selectors | Wait visible | ✓ Implemented | HIGH |

### Events (ListenForEvent)

| Event | Used In | Obscura Status | Priority |
|-------|--------|---------------|-------------|
| `Page.domContentEventFired` | Navigation | ✓ Implemented | CRITICAL |
| `Page.loadEventFired` | Navigation | ✓ Implemented | CRITICAL |
| `Page.lifecycleEvent` (networkIdle) | Wait network | ✓ Implemented | CRITICAL |
| `Page.lifecycleEvent` (networkIdle2) | Wait network | ✓ Implemented | CRITICAL |
| `Page.loadingFinished` | Navigation | ✓ Implemented | HIGH |