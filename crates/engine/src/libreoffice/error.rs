//! Classifier for LibreOfficeKit document-load failures.
//!
//! LOK reports load failures through opaque `OfficeError::OfficeError(msg)`
//! values whose payloads originate in `framework/source/loadenv/loadenv.cxx`.
//! We map the well-known message strings to actionable engine errors and
//! fall back to a content-sniffer for the ambiguous "Unsupported URL" wrapper
//! that LO uses for both encrypted files and outright corruption.
//!
//! The sniffer mirrors the production logic in
//! `office-convert-server/src/encrypted.rs` (same author as the
//! `libreofficekit` crate); we are not inventing a new heuristic.

use crate::types::EngineError;

/// Map a LOK load-time error message + the input file's first bytes to
/// the most informative `EngineError` variant we can derive.
///
/// The original LOK error string is preserved on the corruption branches
/// (where it tends to carry diagnostic detail like "loadComponentFromURL
/// returned an empty reference"). The encrypted and unsupported-format
/// branches drop it because their meaning is exhausted by the variant
/// name; the message would only add LO internals to the API response.
pub(super) fn classify_load_error(msg: &str, file_prefix: &[u8]) -> EngineError {
    if msg.contains("Unsupported URL") {
        return match sniff_file_condition(file_prefix) {
            FileCondition::Encrypted => EngineError::LibreOfficeEncrypted,
            FileCondition::Corrupted => EngineError::LibreOfficeCorrupted(msg.to_string()),
            FileCondition::Unknown => EngineError::LibreOfficeUnsupportedFormat,
        };
    }
    if msg.contains("loadComponentFromURL returned an empty reference") {
        return EngineError::LibreOfficeCorrupted(msg.to_string());
    }
    if msg.contains("type detection failed") {
        return EngineError::LibreOfficeUnsupportedFormat;
    }
    EngineError::Internal(format!("LOK document_load: {msg}"))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum FileCondition {
    Encrypted,
    Corrupted,
    Unknown,
}

/// Inspect the first ~8 KB of a file and return our best guess at why
/// LO might have refused to open it. This is intentionally heuristic —
/// it never aborts a conversion on its own, only refines the error.
pub(super) fn sniff_file_condition(bytes: &[u8]) -> FileCondition {
    if bytes.len() < 4 {
        return FileCondition::Corrupted;
    }

    // PDF — match `/Encrypt` only when followed by a non-name-character so
    // we don't false-positive on `/EncryptMetadata` (a distinct, unrelated
    // PDF name token that legitimately appears in the trailer of files
    // that aren't actually encrypted).
    if bytes.starts_with(b"%PDF-") {
        if bytes.windows(9).any(|w| {
            w[..8] == *b"/Encrypt"
                && !matches!(w[8], b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'_')
        }) {
            return FileCondition::Encrypted;
        }
        return FileCondition::Unknown;
    }

    // ZIP (OOXML / ODF / EPUB ... )
    if bytes.starts_with(b"PK\x03\x04") {
        return classify_zip(bytes);
    }

    // OLE compound document
    if bytes.starts_with(b"\xD0\xCF\x11\xE0\xA1\xB1\x1A\xE1") {
        return FileCondition::Unknown;
    }

    FileCondition::Unknown
}

/// ZIP-aware sub-classifier. Walks the local-file-header sequence in the
/// prefix and looks for OOXML / ODF marker entries.
fn classify_zip(bytes: &[u8]) -> FileCondition {
    let mut saw_content_types = false;
    let mut saw_mimetype = false;
    let mut saw_encrypted_package = false;

    let mut i = 0usize;
    while i + 30 <= bytes.len() {
        if &bytes[i..i + 4] != b"PK\x03\x04" {
            break;
        }
        let name_len = u16::from_le_bytes([bytes[i + 26], bytes[i + 27]]) as usize;
        let extra_len = u16::from_le_bytes([bytes[i + 28], bytes[i + 29]]) as usize;
        let comp_size = u32::from_le_bytes([
            bytes[i + 18], bytes[i + 19], bytes[i + 20], bytes[i + 21],
        ]) as usize;

        let name_start = i + 30;
        let name_end = name_start.saturating_add(name_len);
        if name_end > bytes.len() {
            return FileCondition::Corrupted;
        }

        let name = &bytes[name_start..name_end];
        if name == b"[Content_Types].xml" {
            saw_content_types = true;
        }
        if name == b"mimetype" {
            saw_mimetype = true;
        }
        if name == b"EncryptedPackage" {
            saw_encrypted_package = true;
        }

        // Advance past extra+payload to the next LFH. Streaming-mode ZIPs
        // (general-purpose-bit-flag 0x08) record `comp_size = 0` here and
        // put the real size in a post-payload data descriptor; in that
        // case the next iteration will see non-LFH magic and break out
        // cleanly with whatever markers we accumulated so far. Use
        // checked_add so a malformed `comp_size = u32::MAX` returns
        // Corrupted instead of relying on the loop predicate to trip.
        i = match name_end
            .checked_add(extra_len)
            .and_then(|v| v.checked_add(comp_size))
        {
            Some(n) => n,
            None => return FileCondition::Corrupted,
        };
    }

    if saw_encrypted_package {
        return FileCondition::Encrypted;
    }
    if saw_content_types || saw_mimetype {
        return FileCondition::Unknown;
    }
    FileCondition::Corrupted
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unsupported_url_with_encrypted_zip_yields_encrypted() {
        let bytes = build_zip_with_entry(b"EncryptedPackage", &[0u8; 16]);
        let err = classify_load_error("Unsupported URL <file:///x>: \"type detection failed\"", &bytes);
        assert!(matches!(err, EngineError::LibreOfficeEncrypted), "got {err:?}");
    }

    #[test]
    fn loadcomponent_failure_yields_corrupted() {
        let err = classify_load_error(
            "loadComponentFromURL returned an empty reference",
            &[],
        );
        assert!(matches!(err, EngineError::LibreOfficeCorrupted(_)), "got {err:?}");
    }

    #[test]
    fn type_detection_failed_alone_yields_unsupported() {
        let err = classify_load_error("type detection failed", &[]);
        assert!(matches!(err, EngineError::LibreOfficeUnsupportedFormat), "got {err:?}");
    }

    #[test]
    fn unknown_message_falls_through_to_internal() {
        let err = classify_load_error("something completely different", &[]);
        assert!(matches!(err, EngineError::Internal(_)), "got {err:?}");
    }

    #[test]
    fn pdf_with_encrypt_is_encrypted() {
        let mut bytes = b"%PDF-1.4\n%blah\n".to_vec();
        bytes.extend_from_slice(b"1 0 obj\n<< /Encrypt 2 0 R >>\nendobj\n");
        assert_eq!(sniff_file_condition(&bytes), FileCondition::Encrypted);
    }

    #[test]
    fn ooxml_zip_without_content_types_is_corrupted() {
        let bytes = build_zip_with_entry(b"someotherfile.xml", b"hello");
        assert_eq!(sniff_file_condition(&bytes), FileCondition::Corrupted);
    }

    #[test]
    fn ooxml_zip_with_content_types_is_unknown() {
        let bytes = build_zip_with_entry(b"[Content_Types].xml", b"<xml/>");
        assert_eq!(sniff_file_condition(&bytes), FileCondition::Unknown);
    }

    #[test]
    fn ooxml_zip_with_encrypted_package_is_encrypted() {
        // Direct sniffer test (sibling to the classifier-level test above)
        // so a future regression in the ZIP walker is localised here.
        let bytes = build_zip_with_entry(b"EncryptedPackage", &[0u8; 16]);
        assert_eq!(sniff_file_condition(&bytes), FileCondition::Encrypted);
    }

    #[test]
    fn pdf_with_encrypt_metadata_only_is_not_encrypted() {
        // /EncryptMetadata is a distinct PDF name token that appears in
        // legitimately-unencrypted PDFs. The anchored matcher must reject
        // it without false-positiving as Encrypted.
        let mut bytes = b"%PDF-1.7\n".to_vec();
        bytes.extend_from_slice(b"<< /EncryptMetadata false >>\n");
        assert_eq!(sniff_file_condition(&bytes), FileCondition::Unknown);
    }

    #[test]
    fn random_bytes_are_unknown() {
        let bytes = b"this is not any known magic".to_vec();
        assert_eq!(sniff_file_condition(&bytes), FileCondition::Unknown);
    }

    #[test]
    fn empty_bytes_are_corrupted() {
        assert_eq!(sniff_file_condition(&[]), FileCondition::Corrupted);
    }

    /// Build a minimal ZIP local-file-header followed by a payload, in the
    /// same shape `classify_zip` walks. No central directory needed because
    /// the classifier only walks LFHs.
    fn build_zip_with_entry(name: &[u8], payload: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(b"PK\x03\x04");
        out.extend_from_slice(&[20, 0]);
        out.extend_from_slice(&[0, 0]);
        out.extend_from_slice(&[0, 0]);
        out.extend_from_slice(&[0, 0, 0, 0]);
        out.extend_from_slice(&[0, 0, 0, 0]);
        let comp = (payload.len() as u32).to_le_bytes();
        out.extend_from_slice(&comp);
        out.extend_from_slice(&comp);
        let nlen = (name.len() as u16).to_le_bytes();
        out.extend_from_slice(&nlen);
        out.extend_from_slice(&[0, 0]);
        out.extend_from_slice(name);
        out.extend_from_slice(payload);
        out
    }
}
