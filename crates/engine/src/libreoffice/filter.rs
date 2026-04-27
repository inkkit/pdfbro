//! Map document extensions to LibreOffice export filters.
//!
//! See spec 12 § *Export filter* for the authoritative table.

/// Return the `--convert-to` argument value for the given input extension.
///
/// The lookup is case-insensitive and matches on the *extension only* (no
/// content sniffing). Unknown extensions fall back to a generic `"pdf"` so
/// LibreOffice attempts to infer the input format itself.
///
/// # Examples
///
/// ```
/// use engine::libreoffice::filter::for_extension;
/// assert_eq!(for_extension("docx"), "pdf:writer_pdf_Export");
/// assert_eq!(for_extension("XLSX"), "pdf:calc_pdf_Export");
/// assert_eq!(for_extension("zzz"),  "pdf");
/// ```
pub fn for_extension(ext: &str) -> &'static str {
    let lower = ext.trim_start_matches('.').to_ascii_lowercase();
    match lower.as_str() {
        // Writer / text documents
        "doc" | "docx" | "odt" | "rtf" | "txt" | "html" | "htm" => "pdf:writer_pdf_Export",
        // Calc / spreadsheets
        "xls" | "xlsx" | "ods" | "csv" => "pdf:calc_pdf_Export",
        // Impress / presentations
        "ppt" | "pptx" | "odp" => "pdf:impress_pdf_Export",
        // Draw / vector / Visio
        "odg" | "vsd" | "vsdx" => "pdf:draw_pdf_Export",
        // Generic fallback — let LibreOffice infer.
        _ => "pdf",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn for_extension_maps_writer_calc_impress_draw() {
        assert_eq!(for_extension("docx"), "pdf:writer_pdf_Export");
        assert_eq!(for_extension("xlsx"), "pdf:calc_pdf_Export");
        assert_eq!(for_extension("pptx"), "pdf:impress_pdf_Export");
        assert_eq!(for_extension("odg"), "pdf:draw_pdf_Export");
    }

    #[test]
    fn for_extension_is_case_insensitive() {
        assert_eq!(for_extension("DOCX"), "pdf:writer_pdf_Export");
        assert_eq!(for_extension("Xlsx"), "pdf:calc_pdf_Export");
        assert_eq!(for_extension("PPT"), "pdf:impress_pdf_Export");
        // A leading dot is tolerated (defensive: callers may pass `.docx`).
        assert_eq!(for_extension(".docx"), "pdf:writer_pdf_Export");
    }

    #[test]
    fn for_extension_unknown_returns_pdf_fallback() {
        assert_eq!(for_extension("unknown"), "pdf");
        assert_eq!(for_extension(""), "pdf");
        assert_eq!(for_extension("zzz"), "pdf");
    }

    /// Acceptance criterion: filter table covered exhaustively.
    #[test]
    fn for_extension_covers_table() {
        let writer = ["doc", "docx", "odt", "rtf", "txt", "html", "htm"];
        let calc = ["xls", "xlsx", "ods", "csv"];
        let impress = ["ppt", "pptx", "odp"];
        let draw = ["odg", "vsd", "vsdx"];

        for e in writer {
            assert_eq!(for_extension(e), "pdf:writer_pdf_Export", "writer: {e}");
        }
        for e in calc {
            assert_eq!(for_extension(e), "pdf:calc_pdf_Export", "calc: {e}");
        }
        for e in impress {
            assert_eq!(for_extension(e), "pdf:impress_pdf_Export", "impress: {e}");
        }
        for e in draw {
            assert_eq!(for_extension(e), "pdf:draw_pdf_Export", "draw: {e}");
        }
    }
}
