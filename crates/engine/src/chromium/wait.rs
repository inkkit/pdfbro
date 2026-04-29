//! [`WaitCondition`] application against a chromiumoxide [`Page`].
//!
//! Implements the *Wait conditions* table in
//! `docs/specs/11-engine-chromium.md`. Each entrypoint runs without an
//! enclosing timeout — the caller wraps the entire render in
//! `tokio::time::timeout(BrowserConfig::timeout, ...)`.

use std::time::Duration;

use chromiumoxide::Page;
use chromiumoxide::cdp::browser_protocol::page::{EventDomContentEventFired, EventLifecycleEvent};
use futures_util::StreamExt;

use crate::types::{EngineError, EngineResult, WaitCondition};

/// Polling interval for [`WaitCondition::Selector`] /
/// [`WaitCondition::Expression`].
const POLL_INTERVAL: Duration = Duration::from_millis(50);

/// Apply the given wait condition. Returns `Ok(())` once it resolves.
///
/// On any CDP error mid-wait (other than the page closing during
/// shutdown) the error propagates up and is mapped by the caller via
/// [`super::ChromiumEngine::map_cdp_error`].
pub(crate) async fn apply(page: &Page, wait: &WaitCondition) -> EngineResult<()> {
    match wait {
        WaitCondition::Load => {
            // Implicit: callers have already awaited goto/set_content
            // which themselves wait for the load event.
            Ok(())
        }
        WaitCondition::DomContentLoaded => wait_dom_content_loaded(page).await,
        WaitCondition::NetworkIdle => wait_network_idle(page).await,
        WaitCondition::Selector { selector } => wait_selector(page, selector).await,
        WaitCondition::Expression { expression } => wait_expression(page, expression).await,
        WaitCondition::Delay { duration } => {
            tokio::time::sleep(*duration).await;
            Ok(())
        }
        WaitCondition::WindowStatus { status } => wait_window_status(page, status).await,
    }
}

async fn wait_dom_content_loaded(page: &Page) -> EngineResult<()> {
    let mut events = page
        .event_listener::<EventDomContentEventFired>()
        .await
        .map_err(|e| EngineError::Cdp(e.to_string()))?;
    if events.next().await.is_none() {
        return Err(EngineError::Cdp(
            "domContentEventFired stream closed before firing".into(),
        ));
    }
    Ok(())
}

async fn wait_network_idle(page: &Page) -> EngineResult<()> {
    let mut events = page
        .event_listener::<EventLifecycleEvent>()
        .await
        .map_err(|e| EngineError::Cdp(e.to_string()))?;
    while let Some(ev) = events.next().await {
        if ev.name == "networkIdle" {
            return Ok(());
        }
    }
    Err(EngineError::Cdp(
        "lifecycleEvent stream closed before networkIdle".into(),
    ))
}

async fn wait_selector(page: &Page, selector: &str) -> EngineResult<()> {
    let escaped = json_escape(selector);
    let expr = format!("!!document.querySelector(\"{escaped}\")");
    poll_truthy(page, &expr).await
}

async fn wait_expression(page: &Page, expression: &str) -> EngineResult<()> {
    // Coerce to bool with `!!(...)`. The user expression is wrapped in
    // a parenthesised group to preserve the original semantics.
    let expr = format!("!!({expression})");
    poll_truthy(page, &expr).await
}

async fn poll_truthy(page: &Page, expr: &str) -> EngineResult<()> {
    loop {
        // The caller already wraps `expr` in `!!(...)` so the eval
        // result is always a bool. JS exceptions surface as a CDP
        // error which we treat as "not truthy yet" and keep polling
        // until the outer timeout fires.
        let truthy = page
            .evaluate(expr)
            .await
            .ok()
            .and_then(|r| r.into_value::<bool>().ok())
            .unwrap_or(false);
        if truthy {
            return Ok(());
        }
        tokio::time::sleep(POLL_INTERVAL).await;
    }
}

async fn wait_window_status(page: &Page, status: &str) -> EngineResult<()> {
    let escaped = json_escape(status);
    // Poll window.status until it matches the expected value.
    let expr = format!("window.status === \"{escaped}\"");
    poll_truthy(page, &expr).await
}

/// Minimal JSON string escape — enough to safely interpolate a CSS
/// selector into a JS double-quoted string literal.
fn json_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                use std::fmt::Write as _;
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_escape_handles_quotes_and_backslashes() {
        assert_eq!(json_escape("a\"b"), "a\\\"b");
        assert_eq!(json_escape("a\\b"), "a\\\\b");
        assert_eq!(json_escape("\n"), "\\n");
        assert_eq!(json_escape("plain"), "plain");
    }

    #[test]
    fn json_escape_handles_special_chars() {
        assert_eq!(json_escape("\r"), "\\r");
        assert_eq!(json_escape("\t"), "\\t");
        assert_eq!(json_escape("quote\"here"), "quote\\\"here");
        assert_eq!(json_escape("back\\slash"), "back\\\\slash");
    }

    #[test]
    fn json_escape_window_status_special_chars() {
        // Window status might contain various characters that need escaping.
        assert_eq!(json_escape("status:ready"), "status:ready");
        assert_eq!(json_escape("status=ready"), "status=ready");
        assert_eq!(json_escape("ready\"injected"), "ready\\\"injected");
    }
}
