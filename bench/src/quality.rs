use lopdf::Document;

pub fn validate_pdf(bytes: &[u8], expected_pages: Option<usize>) -> anyhow::Result<()> {
    if !bytes.starts_with(b"%PDF") {
        anyhow::bail!("response does not start with %PDF magic bytes");
    }
    if bytes.len() < 64 {
        anyhow::bail!("response body too small to be a valid PDF ({} bytes)", bytes.len());
    }
    if let Some(pages) = expected_pages {
        let doc = Document::load_mem(bytes)
            .map_err(|e| anyhow::anyhow!("failed to parse PDF: {e}"))?;
        let actual = doc.get_pages().len();
        if actual != pages {
            anyhow::bail!("expected {pages} pages, got {actual}");
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_non_pdf() {
        let result = validate_pdf(b"not a pdf", None);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("%PDF"));
    }

    #[test]
    fn rejects_short_body() {
        let result = validate_pdf(b"%PDF-1.4", None);
        assert!(result.is_err());
    }
}
