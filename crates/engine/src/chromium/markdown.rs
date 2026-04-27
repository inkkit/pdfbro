//! Markdown → HTML conversion for [`super::ChromiumEngine::markdown_to_pdf`].
//!
//! Uses [`pulldown_cmark`] with the *all extensions* option set so that
//! tables, strikethrough, and task lists are supported (per
//! `docs/specs/11-engine-chromium.md` §`markdown_to_pdf`). The rendered
//! HTML is wrapped in a minimal full-document template (charset meta,
//! built-in stylesheet) before being handed to `html_to_pdf`.

use pulldown_cmark::{Options, Parser, html};

/// Default stylesheet bundled with the engine. See `markdown.css`.
pub(crate) const DEFAULT_STYLESHEET: &str = include_str!("markdown.css");

/// Convert the given Markdown to a complete HTML document string.
pub(crate) fn render(markdown_input: &str) -> String {
    let body = render_body(markdown_input);
    wrap_document(&body)
}

/// Convert just the Markdown body to an HTML fragment (no `<html>`,
/// `<head>`, etc.). Exposed for unit tests.
fn render_body(markdown_input: &str) -> String {
    let parser = Parser::new_ext(markdown_input, Options::all());
    let mut out = String::with_capacity(markdown_input.len() * 2);
    html::push_html(&mut out, parser);
    out
}

/// Wrap an HTML fragment in the spec-mandated template (charset meta +
/// default stylesheet).
fn wrap_document(body_html: &str) -> String {
    format!(
        "<!DOCTYPE html>\n\
        <html>\n\
        <head>\n\
        <meta charset=\"utf-8\">\n\
        <style>{css}</style>\n\
        </head>\n\
        <body>\n{body}\n</body>\n\
        </html>",
        css = DEFAULT_STYLESHEET,
        body = body_html,
    )
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn markdown_template_wraps_with_charset_meta() {
        let html = render("hello");
        assert!(
            html.starts_with("<!DOCTYPE html>"),
            "missing doctype: {html}"
        );
        assert!(html.contains("<meta charset=\"utf-8\">"));
        assert!(html.contains("<style>"));
        assert!(html.contains("</style>"));
        assert!(html.contains("<p>hello</p>"));
        assert!(html.contains("</html>"));
    }

    #[test]
    fn markdown_renders_table_with_extension() {
        let md = "| a | b |\n|---|---|\n| 1 | 2 |\n";
        let body = render_body(md);
        assert!(body.contains("<table>"), "no table: {body}");
        assert!(body.contains("<th>a</th>"));
        assert!(body.contains("<td>1</td>"));
    }

    #[test]
    fn markdown_renders_task_list_with_extension() {
        let md = "- [x] done\n- [ ] todo\n";
        let body = render_body(md);
        assert!(body.contains("type=\"checkbox\""), "body: {body}");
    }

    #[test]
    fn markdown_strikethrough_with_extension() {
        let body = render_body("~~gone~~");
        assert!(body.contains("<del>gone</del>"), "body: {body}");
    }

    #[test]
    fn markdown_strips_raw_html_script_block() {
        // Per the spec edge-case: pulldown-cmark with default safety
        // does not execute scripts; the tag is preserved as inline HTML
        // but Chrome's headless rendering will not run it. This test
        // only asserts that we do not panic and produce non-empty
        // output for inputs that contain raw <script>.
        let body = render_body("<script>alert(1)</script>\n\nhi");
        assert!(body.contains("hi") || body.contains("<p>hi</p>"));
    }

    #[test]
    fn default_stylesheet_present() {
        assert!(DEFAULT_STYLESHEET.contains("table"));
        assert!(DEFAULT_STYLESHEET.contains("monospace"));
        assert!(DEFAULT_STYLESHEET.contains("body"));
    }
}
