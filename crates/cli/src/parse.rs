//! Value parsers for CLI option strings (`--paper`, `--margin`, `--wait`,
//! `--cookie`, `--fail-on-status`, durations).
//!
//! Each parser returns `Result<T, String>` so it can plug straight into
//! clap's `value_parser` slot.

use std::time::Duration;

use engine::{Cookie, Margins, PageRanges, PaperSize, WaitCondition};

// ---------------------------------------------------------------------------
// --paper
// ---------------------------------------------------------------------------

/// Parse `a4` / `letter` / `legal` / `a3` / `a5` / `WxH` into a [`PaperSize`].
pub(crate) fn parse_paper(s: &str) -> Result<PaperSize, String> {
    let trimmed = s.trim();
    match trimmed.to_ascii_lowercase().as_str() {
        "a4" => return Ok(PaperSize::A4),
        "letter" => return Ok(PaperSize::LETTER),
        "legal" => return Ok(PaperSize::LEGAL),
        "a3" => return Ok(PaperSize::A3),
        "a5" => return Ok(PaperSize::A5),
        _ => {}
    }
    parse_paper_dimensions(trimmed)
}

fn parse_paper_dimensions(s: &str) -> Result<PaperSize, String> {
    let lower = s.to_ascii_lowercase();
    let (w_str, h_str) = lower
        .split_once('x')
        .ok_or_else(|| format!("invalid paper '{s}': expected NAME or WxH (inches)"))?;
    let w: f32 = w_str
        .trim()
        .parse()
        .map_err(|e| format!("invalid paper width '{w_str}': {e}"))?;
    let h: f32 = h_str
        .trim()
        .parse()
        .map_err(|e| format!("invalid paper height '{h_str}': {e}"))?;
    PaperSize::new(w, h).map_err(|e| format!("invalid paper: {e}"))
}

// ---------------------------------------------------------------------------
// --margin
// ---------------------------------------------------------------------------

/// Parse a margin spec: either a single inch value (uniform) or four
/// comma-separated values (`top,right,bottom,left`).
pub(crate) fn parse_margin(s: &str) -> Result<Margins, String> {
    let parts: Vec<&str> = s.split(',').map(str::trim).collect();
    match parts.as_slice() {
        [single] => {
            let v: f32 = single
                .parse()
                .map_err(|e| format!("invalid margin '{single}': {e}"))?;
            if !v.is_finite() || v < 0.0 {
                return Err(format!("invalid margin '{single}': must be >= 0"));
            }
            Ok(Margins::uniform(v))
        }
        [t, r, b, l] => {
            let top = parse_margin_component("top", t)?;
            let right = parse_margin_component("right", r)?;
            let bottom = parse_margin_component("bottom", b)?;
            let left = parse_margin_component("left", l)?;
            Ok(Margins {
                top,
                right,
                bottom,
                left,
            })
        }
        _ => Err(format!(
            "invalid margin '{s}': expected single inch value or 'TOP,RIGHT,BOTTOM,LEFT'"
        )),
    }
}

fn parse_margin_component(label: &str, raw: &str) -> Result<f32, String> {
    let v: f32 = raw
        .parse()
        .map_err(|e| format!("invalid {label} margin '{raw}': {e}"))?;
    if !v.is_finite() || v < 0.0 {
        return Err(format!("invalid {label} margin '{raw}': must be >= 0"));
    }
    Ok(v)
}

// ---------------------------------------------------------------------------
// --wait
// ---------------------------------------------------------------------------

/// Parse a `--wait` spec into a [`WaitCondition`].
pub(crate) fn parse_wait(s: &str) -> Result<WaitCondition, String> {
    let trimmed = s.trim();
    match trimmed {
        "load" => return Ok(WaitCondition::Load),
        "domcontentloaded" => return Ok(WaitCondition::DomContentLoaded),
        "networkidle" => return Ok(WaitCondition::NetworkIdle),
        _ => {}
    }
    if let Some(rest) = trimmed.strip_prefix("selector:") {
        if rest.is_empty() {
            return Err("invalid wait 'selector:': selector cannot be empty".into());
        }
        return Ok(WaitCondition::Selector {
            selector: rest.to_string(),
        });
    }
    if let Some(rest) = trimmed.strip_prefix("expr:") {
        if rest.is_empty() {
            return Err("invalid wait 'expr:': expression cannot be empty".into());
        }
        return Ok(WaitCondition::Expression {
            expression: rest.to_string(),
        });
    }
    if let Some(rest) = trimmed.strip_prefix("delay:") {
        let d = parse_duration(rest)?;
        return Ok(WaitCondition::Delay { duration: d });
    }
    Err(format!(
        "invalid wait '{s}': expected one of \
         load|domcontentloaded|networkidle|selector:CSS|expr:JS|delay:DUR"
    ))
}

// ---------------------------------------------------------------------------
// --cookie
// ---------------------------------------------------------------------------

/// Parse a single `--cookie` value: `name=value[;Domain=...;Path=...;Secure;HttpOnly]`.
pub(crate) fn parse_cookie(s: &str) -> Result<Cookie, String> {
    let mut parts = s.split(';');
    let head = parts
        .next()
        .ok_or_else(|| format!("invalid cookie '{s}': empty"))?
        .trim();
    let (name, value) = head
        .split_once('=')
        .ok_or_else(|| format!("invalid cookie '{head}': missing '='"))?;
    let name = name.trim();
    if name.is_empty() {
        return Err(format!("invalid cookie '{s}': empty name"));
    }
    let mut cookie = Cookie {
        name: name.to_string(),
        value: value.trim().to_string(),
        domain: None,
        path: None,
        secure: false,
        http_only: false,
    };
    for attr in parts {
        let attr = attr.trim();
        if attr.is_empty() {
            continue;
        }
        if let Some((k, v)) = attr.split_once('=') {
            match k.trim().to_ascii_lowercase().as_str() {
                "domain" => cookie.domain = Some(v.trim().to_string()),
                "path" => cookie.path = Some(v.trim().to_string()),
                _ => {} // ignore unknown attrs
            }
        } else {
            match attr.to_ascii_lowercase().as_str() {
                "secure" => cookie.secure = true,
                "httponly" => cookie.http_only = true,
                _ => {} // ignore unknown attrs
            }
        }
    }
    Ok(cookie)
}

// ---------------------------------------------------------------------------
// --fail-on-status
// ---------------------------------------------------------------------------

/// Parse a `--fail-on-status` value. Accepts:
///
/// - A single status: `500`.
/// - A wildcard family: `5xx`, `4xx`, etc.
/// - A closed range: `500-503`.
pub(crate) fn parse_fail_on_status(s: &str) -> Result<Vec<u16>, String> {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        return Err("invalid --fail-on-status: empty".into());
    }
    let lower = trimmed.to_ascii_lowercase();

    // Wildcard form (e.g. "5xx").
    if let Some(prefix) = lower.strip_suffix("xx") {
        let digit: u16 = prefix
            .parse()
            .map_err(|_| format!("invalid status family '{s}': expected DIGITxx"))?;
        if !(1..=9).contains(&digit) {
            return Err(format!("invalid status family '{s}': leading digit 1..=9"));
        }
        let base = digit * 100;
        return Ok((base..=base + 99).collect());
    }

    // Range form (e.g. "500-503").
    if let Some((lo_str, hi_str)) = lower.split_once('-') {
        let lo: u16 = lo_str
            .trim()
            .parse()
            .map_err(|_| format!("invalid status range start '{lo_str}'"))?;
        let hi: u16 = hi_str
            .trim()
            .parse()
            .map_err(|_| format!("invalid status range end '{hi_str}'"))?;
        if lo > hi {
            return Err(format!("invalid status range '{s}': start > end"));
        }
        if !(100..=599).contains(&lo) || !(100..=599).contains(&hi) {
            return Err(format!("invalid status range '{s}': must be in 100..=599"));
        }
        return Ok((lo..=hi).collect());
    }

    // Single status.
    let code: u16 = lower
        .parse()
        .map_err(|_| format!("invalid status '{s}': expected NNN, NNN-NNN, or DIGITxx"))?;
    if !(100..=599).contains(&code) {
        return Err(format!("invalid status '{s}': must be in 100..=599"));
    }
    Ok(vec![code])
}

// ---------------------------------------------------------------------------
// Duration
// ---------------------------------------------------------------------------

/// Parse a duration via `humantime` (e.g. `5s`, `2m`, `500ms`).
pub(crate) fn parse_duration(s: &str) -> Result<Duration, String> {
    humantime::parse_duration(s.trim()).map_err(|e| format!("invalid duration '{s}': {e}"))
}

// ---------------------------------------------------------------------------
// --pages
// ---------------------------------------------------------------------------

/// Parse a [`PageRanges`] expression. Thin wrapper around the engine's
/// canonical parser to surface a `String` error type for clap.
pub(crate) fn parse_page_ranges(s: &str) -> Result<PageRanges, String> {
    PageRanges::parse(s).map_err(|e| format!("invalid --pages '{s}': {e}"))
}

// ---------------------------------------------------------------------------
// --header "Name: Value"
// ---------------------------------------------------------------------------

/// Parse a `Name: Value` header pair.
pub(crate) fn parse_header(s: &str) -> Result<(String, String), String> {
    let (name, value) = s
        .split_once(':')
        .ok_or_else(|| format!("invalid header '{s}': expected 'Name: Value'"))?;
    let name = name.trim();
    if name.is_empty() {
        return Err(format!("invalid header '{s}': empty name"));
    }
    Ok((name.to_string(), value.trim().to_string()))
}

// ---------------------------------------------------------------------------
// metadata --set KEY=VALUE
// ---------------------------------------------------------------------------

/// Parse a `KEY=VALUE` pair (value may be empty, meaning "delete this key").
pub(crate) fn parse_set_kv(s: &str) -> Result<(String, String), String> {
    let (k, v) = s
        .split_once('=')
        .ok_or_else(|| format!("invalid --set '{s}': expected KEY=VALUE"))?;
    let k = k.trim();
    if k.is_empty() {
        return Err(format!("invalid --set '{s}': empty key"));
    }
    Ok((k.to_string(), v.to_string()))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_close(a: f32, b: f32) {
        assert!((a - b).abs() < 1e-4, "{a} != {b}");
    }

    #[test]
    fn parse_paper_named() {
        let p = parse_paper("a4").unwrap();
        assert_close(p.width_in, 8.27);
        assert_close(p.height_in, 11.69);

        let p = parse_paper("Letter").unwrap();
        assert_close(p.width_in, 8.5);
        assert_close(p.height_in, 11.0);

        assert!(parse_paper("LEGAL").is_ok());
        assert!(parse_paper("a3").is_ok());
        assert!(parse_paper("A5").is_ok());
    }

    #[test]
    fn parse_paper_dimensions() {
        let p = parse_paper("8.5x11").unwrap();
        assert_close(p.width_in, 8.5);
        assert_close(p.height_in, 11.0);

        let p = parse_paper("4X6").unwrap();
        assert_close(p.width_in, 4.0);
        assert_close(p.height_in, 6.0);
    }

    #[test]
    fn parse_paper_invalid() {
        assert!(parse_paper("bogus").is_err());
        assert!(parse_paper("8.5xnope").is_err());
        assert!(parse_paper("0x0").is_err());
    }

    #[test]
    fn parse_margin_single_value_uniform() {
        let m = parse_margin("0.5").unwrap();
        assert_eq!(m, Margins::uniform(0.5));
    }

    #[test]
    fn parse_margin_four_values_in_order() {
        let m = parse_margin("1,0.5,1,0.25").unwrap();
        assert_close(m.top, 1.0);
        assert_close(m.right, 0.5);
        assert_close(m.bottom, 1.0);
        assert_close(m.left, 0.25);
    }

    #[test]
    fn parse_margin_wrong_count() {
        assert!(parse_margin("1,2").is_err());
        assert!(parse_margin("1,2,3").is_err());
        assert!(parse_margin("1,2,3,4,5").is_err());
    }

    #[test]
    fn parse_margin_negative_rejected() {
        assert!(parse_margin("-0.1").is_err());
        assert!(parse_margin("0,-0.1,0,0").is_err());
    }

    #[test]
    fn parse_wait_simple_keywords() {
        assert_eq!(parse_wait("load").unwrap(), WaitCondition::Load);
        assert_eq!(
            parse_wait("domcontentloaded").unwrap(),
            WaitCondition::DomContentLoaded
        );
        assert_eq!(
            parse_wait("networkidle").unwrap(),
            WaitCondition::NetworkIdle
        );
    }

    #[test]
    fn parse_wait_selector() {
        let w = parse_wait("selector:#root .ready").unwrap();
        match w {
            WaitCondition::Selector { selector } => assert_eq!(selector, "#root .ready"),
            other => panic!("expected Selector, got {other:?}"),
        }
        assert!(parse_wait("selector:").is_err());
    }

    #[test]
    fn parse_wait_expression() {
        let w = parse_wait("expr:window.ready === true").unwrap();
        match w {
            WaitCondition::Expression { expression } => {
                assert_eq!(expression, "window.ready === true");
            }
            other => panic!("expected Expression, got {other:?}"),
        }
        assert!(parse_wait("expr:").is_err());
    }

    #[test]
    fn parse_wait_delay() {
        let w = parse_wait("delay:500ms").unwrap();
        match w {
            WaitCondition::Delay { duration } => {
                assert_eq!(duration, Duration::from_millis(500));
            }
            other => panic!("expected Delay, got {other:?}"),
        }
        assert_eq!(
            parse_wait("delay:2s").unwrap(),
            WaitCondition::Delay {
                duration: Duration::from_secs(2)
            }
        );
        assert!(parse_wait("delay:not-a-duration").is_err());
    }

    #[test]
    fn parse_wait_unknown_kind_rejected() {
        assert!(parse_wait("bogus").is_err());
        assert!(parse_wait("idle:5s").is_err());
    }

    #[test]
    fn parse_cookie_with_attrs() {
        let c = parse_cookie("session=abc;Domain=example.com;Path=/;Secure;HttpOnly").unwrap();
        assert_eq!(c.name, "session");
        assert_eq!(c.value, "abc");
        assert_eq!(c.domain.as_deref(), Some("example.com"));
        assert_eq!(c.path.as_deref(), Some("/"));
        assert!(c.secure);
        assert!(c.http_only);
    }

    #[test]
    fn parse_cookie_minimal() {
        let c = parse_cookie("foo=bar").unwrap();
        assert_eq!(c.name, "foo");
        assert_eq!(c.value, "bar");
        assert_eq!(c.domain, None);
        assert_eq!(c.path, None);
        assert!(!c.secure);
        assert!(!c.http_only);
    }

    #[test]
    fn parse_cookie_unknown_attrs_ignored() {
        let c = parse_cookie("k=v;Foo=bar;Baz").unwrap();
        assert_eq!(c.name, "k");
        assert_eq!(c.value, "v");
        assert!(!c.secure);
    }

    #[test]
    fn parse_cookie_missing_value() {
        assert!(parse_cookie("novalue").is_err());
        assert!(parse_cookie("=v").is_err());
    }

    #[test]
    fn parse_fail_on_status_codes_and_wildcards() {
        assert_eq!(parse_fail_on_status("500").unwrap(), vec![500]);
        let fives = parse_fail_on_status("5xx").unwrap();
        assert_eq!(fives.first().copied(), Some(500));
        assert_eq!(fives.last().copied(), Some(599));
        assert_eq!(fives.len(), 100);
        assert_eq!(
            parse_fail_on_status("500-502").unwrap(),
            vec![500, 501, 502]
        );
        assert_eq!(parse_fail_on_status("4XX").unwrap().len(), 100);
    }

    #[test]
    fn parse_fail_on_status_rejects_garbage() {
        assert!(parse_fail_on_status("").is_err());
        assert!(parse_fail_on_status("0xx").is_err());
        assert!(parse_fail_on_status("99").is_err());
        assert!(parse_fail_on_status("500-499").is_err());
        assert!(parse_fail_on_status("nope").is_err());
    }

    #[test]
    fn parse_header_basic() {
        let (k, v) = parse_header("X-Test: hello").unwrap();
        assert_eq!(k, "X-Test");
        assert_eq!(v, "hello");
    }

    #[test]
    fn parse_header_value_can_contain_colons() {
        let (k, v) = parse_header("Authorization: Bearer abc:def").unwrap();
        assert_eq!(k, "Authorization");
        assert_eq!(v, "Bearer abc:def");
    }

    #[test]
    fn parse_header_missing_colon() {
        assert!(parse_header("nope").is_err());
    }

    #[test]
    fn parse_set_kv_empty_value_keeps_key() {
        let (k, v) = parse_set_kv("Title=").unwrap();
        assert_eq!(k, "Title");
        assert_eq!(v, "");
    }

    #[test]
    fn parse_set_kv_missing_eq() {
        assert!(parse_set_kv("Title").is_err());
    }
}
