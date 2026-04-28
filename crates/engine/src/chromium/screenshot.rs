//! Screenshot capture using Chromium.
//!
//! Implements screenshot functionality via Chrome DevTools Protocol.

use std::collections::HashMap;

use chromiumoxide::cdp::browser_protocol::page::{CaptureScreenshotFormat, CaptureScreenshotParams};
use chromiumoxide::page::Page;

use crate::types::{EngineError, EngineResult};
pub use crate::types::WaitCondition;

/// Screenshot image format.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScreenshotFormat {
    /// PNG format (lossless).
    Png,
    /// JPEG format with quality 0-100.
    Jpeg { 
        /// JPEG quality (0-100).
        quality: u8 
    },
}

impl ScreenshotFormat {
    fn to_cdp_format(&self) -> CaptureScreenshotFormat {
        match self {
            ScreenshotFormat::Png => CaptureScreenshotFormat::Png,
            ScreenshotFormat::Jpeg { .. } => CaptureScreenshotFormat::Jpeg,
        }
    }

    fn quality(&self) -> Option<i64> {
        match self {
            ScreenshotFormat::Png => None,
            ScreenshotFormat::Jpeg { quality } => Some(*quality as i64),
        }
    }

    /// Content-Type header value.
    pub fn content_type(&self) -> &'static str {
        match self {
            ScreenshotFormat::Png => "image/png",
            ScreenshotFormat::Jpeg { .. } => "image/jpeg",
        }
    }

    /// File extension.
    pub fn extension(&self) -> &'static str {
        match self {
            ScreenshotFormat::Png => "png",
            ScreenshotFormat::Jpeg { .. } => "jpg",
        }
    }
}

/// Screenshot capture mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptureMode {
    /// Capture visible viewport only.
    Viewport,
    /// Capture full page (entire scrollable area).
    FullPage,
}

/// Screenshot capture options.
#[derive(Debug, Clone)]
pub struct ScreenshotOptions {
    /// Output format.
    pub format: ScreenshotFormat,
    /// Capture mode.
    pub mode: CaptureMode,
    /// Viewport width in pixels.
    pub width: u32,
    /// Viewport height in pixels.
    pub height: u32,
    /// Device scale factor (1.0 = standard, 2.0 = retina).
    pub device_scale_factor: f32,
    /// Wait condition before capture.
    pub wait_condition: WaitCondition,
    /// Custom HTTP headers.
    pub extra_headers: HashMap<String, String>,
    /// Background CSS color (e.g., "white", "#ffffff").
    pub background_color: Option<String>,
}

impl Default for ScreenshotOptions {
    fn default() -> Self {
        Self {
            format: ScreenshotFormat::Png,
            mode: CaptureMode::Viewport,
            width: 1920,
            height: 1080,
            device_scale_factor: 1.0,
            wait_condition: WaitCondition::Load,
            extra_headers: HashMap::new(),
            background_color: None,
        }
    }
}

/// Capture screenshot of a page.
///
/// Screenshot an HTML string to image bytes.
///
/// High-level function that creates a page, sets HTML content, and captures.
pub async fn html_to_screenshot(
    engine: &super::ChromiumEngine,
    html: &str,
    opts: &ScreenshotOptions,
) -> EngineResult<Vec<u8>> {
    // Get browser lock
    let browser_guard = engine.inner.browser.lock().await;
    let browser = browser_guard.as_ref()
        .ok_or_else(|| EngineError::ChromeLaunch("Browser not available".into()))?;

    // Create new page
    let page = browser.new_page("about:blank").await
        .map_err(|e| EngineError::ChromeLaunch(format!("Failed to create page: {}", e)))?;

    // Set HTML content using data URL
    let data_url = format!("data:text/html;charset=utf-8,{}"
        , urlencoding::encode(html));
    page.goto(&data_url).await
        .map_err(|e| EngineError::ChromeLaunch(format!("Failed to set HTML: {}", e)))?;

    // Capture screenshot
    let data = capture_screenshot(&page, opts).await?;

    // Page will be closed when dropped
    Ok(data)
}

/// Screenshot a URL to image bytes.
///
/// High-level function that navigates to URL and captures.
pub async fn url_to_screenshot(
    engine: &super::ChromiumEngine,
    url: &str,
    opts: &ScreenshotOptions,
) -> EngineResult<Vec<u8>> {
    // Get browser lock
    let browser_guard = engine.inner.browser.lock().await;
    let browser = browser_guard.as_ref()
        .ok_or_else(|| EngineError::ChromeLaunch("Browser not available".into()))?;

    // Create new page and navigate
    let page = browser.new_page(url).await
        .map_err(|e| EngineError::ChromeLaunch(format!("Failed to navigate: {}", e)))?;

    // Capture screenshot
    let data = capture_screenshot(&page, opts).await?;

    Ok(data)
}

/// This is the internal implementation used by ChromiumEngine.
pub async fn capture_screenshot(
    page: &Page,
    opts: &ScreenshotOptions,
) -> EngineResult<Vec<u8>> {
    // Set viewport
    set_viewport(page, opts).await?;

    // Apply background color if specified
    if let Some(color) = &opts.background_color {
        set_background_color(page, color).await?;
    }

    // Wait for condition
    wait_for_condition(page, &opts.wait_condition).await?;

    // Capture screenshot
    let data = match opts.mode {
        CaptureMode::Viewport => capture_viewport_screenshot(page, opts).await?,
        CaptureMode::FullPage => capture_fullpage_screenshot(page, opts).await?,
    };

    Ok(data)
}

/// Set the page viewport.
async fn set_viewport(page: &Page, opts: &ScreenshotOptions) -> EngineResult<()> {
    use chromiumoxide::cdp::browser_protocol::emulation::SetDeviceMetricsOverrideParams;

    let params = SetDeviceMetricsOverrideParams::builder()
        .width(opts.width as i64)
        .height(opts.height as i64)
        .device_scale_factor(opts.device_scale_factor as f64)
        .mobile(false)
        .build()
        .map_err(|e| EngineError::ChromeLaunch(format!("Viewport params error: {}", e)))?;

    page.execute(params).await
        .map_err(|e| EngineError::ChromeLaunch(format!("Failed to set viewport: {}", e)))?;

    Ok(())
}

/// Set background color via Emulation.
async fn set_background_color(page: &Page, color: &str) -> EngineResult<()> {
    // Parse CSS color - simplified, supports "white", "#ffffff", etc.
    let rgba = parse_css_color(color).unwrap_or((255, 255, 255, 1.0));

    use chromiumoxide::cdp::browser_protocol::emulation::SetDefaultBackgroundColorOverrideParams;

    let color_param = chromiumoxide::cdp::browser_protocol::dom::Rgba {
        r: rgba.0 as i64,
        g: rgba.1 as i64,
        b: rgba.2 as i64,
        a: Some(rgba.3),
    };

    let params = SetDefaultBackgroundColorOverrideParams::builder()
        .color(color_param)
        .build();

    page.execute(params).await
        .map_err(|e| EngineError::ChromeLaunch(format!("Failed to set background: {}", e)))?;

    Ok(())
}

/// Wait for specified condition.
async fn wait_for_condition(page: &Page, condition: &WaitCondition) -> EngineResult<()> {
    match condition {
        WaitCondition::Load => {
            // Wait for load event is handled by page.goto()
            Ok(())
        }
        WaitCondition::DomContentLoaded => {
            // Wait for DOMContentLoaded
            page.wait_for_navigation().await
                .map_err(|e| EngineError::ChromeLaunch(format!("Navigation failed: {}", e)))?;
            Ok(())
        }
        WaitCondition::NetworkIdle => {
            // Wait for network idle (simplified - wait fixed time)
            tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
            Ok(())
        }
        WaitCondition::Expression { expression } => {
            // Wait for JavaScript expression
            let _ = page.evaluate(expression.as_str()).await
                .map_err(|e| EngineError::ChromeLaunch(format!("Wait expression failed: {}", e)))?;
            Ok(())
        }
        WaitCondition::Selector { selector } => {
            // Poll for selector (simplified - just wait fixed time)
            let _ = selector;
            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
            Ok(())
        }
        WaitCondition::Delay { duration } => {
            tokio::time::sleep(*duration).await;
            Ok(())
        }
    }
}

/// Capture viewport screenshot.
async fn capture_viewport_screenshot(page: &Page, opts: &ScreenshotOptions) -> EngineResult<Vec<u8>> {
    let params = CaptureScreenshotParams::builder()
        .format(opts.format.to_cdp_format());
        
    let params = if let Some(q) = opts.format.quality() {
        params.quality(q)
    } else {
        params
    }
    .from_surface(true)
    .build();

    let result = page.execute(params).await
        .map_err(|e| EngineError::ChromeLaunch(format!("Screenshot failed: {}", e)))?;

    // Decode base64 data - Binary is a wrapper around base64 string
    let data = base64::decode(result.data.as_ref())
        .map_err(|e| EngineError::Internal(format!("Base64 decode failed: {}", e)))?;

    Ok(data)
}

/// Capture full page screenshot.
async fn capture_fullpage_screenshot(page: &Page, opts: &ScreenshotOptions) -> EngineResult<Vec<u8>> {
    // Get page full metrics
    use chromiumoxide::cdp::browser_protocol::page::GetLayoutMetricsParams;

    let metrics = page.execute(GetLayoutMetricsParams::default()).await
        .map_err(|e| EngineError::ChromeLaunch(format!("Failed to get layout metrics: {}", e)))?;

    // Use css_layout_viewport for full page dimensions
    let css_viewport = &metrics.css_layout_viewport;
    let full_width = css_viewport.client_width as u32;
    let full_height = css_viewport.client_height as u32;

    // Temporarily expand viewport to full page
    let orig_width = opts.width;
    let orig_height = opts.height;

    let temp_opts = ScreenshotOptions {
        width: full_width,
        height: full_height,
        ..opts.clone()
    };

    set_viewport(page, &temp_opts).await?;

    // Allow time for layout
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // Capture
    let result = capture_viewport_screenshot(page, opts).await?;

    // Restore viewport (optional - page will be closed anyway)
    let restore_opts = ScreenshotOptions {
        width: orig_width,
        height: orig_height,
        ..opts.clone()
    };
    let _ = set_viewport(page, &restore_opts).await;

    Ok(result)
}

/// Parse simple CSS color.
fn parse_css_color(color: &str) -> Option<(u8, u8, u8, f64)> {
    match color.to_lowercase().as_str() {
        "white" | "#ffffff" | "#fff" => Some((255, 255, 255, 1.0)),
        "black" | "#000000" | "#000" => Some((0, 0, 0, 1.0)),
        "transparent" => Some((0, 0, 0, 0.0)),
        _ => {
            // Try hex color
            if color.starts_with('#') {
                let hex = &color[1..];
                if hex.len() == 6 {
                    let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
                    let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
                    let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
                    return Some((r, g, b, 1.0));
                }
            }
            None
        }
    }
}

// Simple base64 decoder for screenshot data
mod base64 {
    pub fn decode(s: &str) -> Result<Vec<u8>, String> {
        use std::collections::HashMap;

        let chars: Vec<char> = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/".chars().collect();
        let char_to_value: HashMap<char, u8> = chars.iter().enumerate().map(|(i, &c)| (c, i as u8)).collect();

        let mut result = Vec::new();
        let mut buffer: u32 = 0;
        let mut bits_collected = 0;

        for c in s.chars() {
            if c == '=' {
                break;
            }

            let value = char_to_value.get(&c).ok_or("Invalid base64 character")?;
            buffer = (buffer << 6) | (*value as u32);
            bits_collected += 6;

            if bits_collected >= 8 {
                bits_collected -= 8;
                let byte = ((buffer >> bits_collected) & 0xFF) as u8;
                result.push(byte);
            }
        }

        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn screenshot_format_content_type() {
        assert_eq!(ScreenshotFormat::Png.content_type(), "image/png");
        assert_eq!(ScreenshotFormat::Jpeg { quality: 80 }.content_type(), "image/jpeg");
    }

    #[test]
    fn screenshot_format_extension() {
        assert_eq!(ScreenshotFormat::Png.extension(), "png");
        assert_eq!(ScreenshotFormat::Jpeg { quality: 80 }.extension(), "jpg");
    }

    #[test]
    fn parse_css_color_basic() {
        assert_eq!(parse_css_color("white"), Some((255, 255, 255, 1.0)));
        assert_eq!(parse_css_color("#ffffff"), Some((255, 255, 255, 1.0)));
        assert_eq!(parse_css_color("#000000"), Some((0, 0, 0, 1.0)));
        assert_eq!(parse_css_color("transparent"), Some((0, 0, 0, 0.0)));
    }

    #[test]
    fn base64_decode_simple() {
        assert_eq!(base64::decode("SGVsbG8=").unwrap(), b"Hello");
        assert_eq!(base64::decode("V29ybGQ=").unwrap(), b"World");
    }
}
