//! `read_metadata` / `write_metadata` — manipulate the `/Info` dictionary.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::types::{EngineError, EngineResult};

/// PDF document metadata (the `/Info` dictionary plus arbitrary custom
/// entries).
///
/// Wire format for the date fields is the PDF date string,
/// `D:YYYYMMDDhhmmss±hh'mm'`. All standard fields are optional.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "PascalCase")]
pub struct Metadata {
    /// Document title.
    pub title: Option<String>,
    /// Document author.
    pub author: Option<String>,
    /// Document subject.
    pub subject: Option<String>,
    /// Search keywords.
    pub keywords: Option<String>,
    /// Original creator application.
    pub creator: Option<String>,
    /// Producer string. `write_metadata` overrides this with `folio/<ver>`.
    pub producer: Option<String>,
    /// PDF date string (`D:YYYYMMDDhhmmss±hh'mm'`).
    pub creation_date: Option<String>,
    /// PDF date string for last modification.
    pub mod_date: Option<String>,
    /// Custom info-dict entries; keys must match `^[A-Za-z][A-Za-z0-9_-]{0,127}$`.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub custom: BTreeMap<String, String>,
}

/// Read the `/Info` dictionary into a [`Metadata`] struct.
///
/// Returns [`Metadata::default()`] when the document has no `/Info` entry.
///
/// # Errors
///
/// [`EngineError::Internal`] if the input fails to parse or is encrypted.
pub fn read_metadata(pdf: &[u8]) -> EngineResult<Metadata> {
    super::parse_input(pdf)?;
    Err(EngineError::Internal(
        "read_metadata: not yet implemented".into(),
    ))
}

/// Merge `meta` into the document's `/Info` dictionary.
///
/// Fields set to `None` are left untouched; fields set to `Some("")` are
/// removed. Custom keys must match `^[A-Za-z][A-Za-z0-9_-]{0,127}$`.
///
/// # Errors
///
/// - [`EngineError::InvalidOption`] for malformed custom keys.
/// - [`EngineError::Internal`] if the input fails to parse, is encrypted,
///   or the result fails to save.
pub fn write_metadata(pdf: &[u8], meta: &Metadata) -> EngineResult<Vec<u8>> {
    validate_custom_keys(&meta.custom)?;
    super::parse_input(pdf)?;
    Err(EngineError::Internal(
        "write_metadata: not yet implemented".into(),
    ))
}

fn validate_custom_keys(custom: &BTreeMap<String, String>) -> EngineResult<()> {
    for key in custom.keys() {
        if !is_valid_custom_key(key) {
            return Err(EngineError::InvalidOption(format!(
                "invalid custom info-dict key: {key:?}"
            )));
        }
    }
    Ok(())
}

fn is_valid_custom_key(key: &str) -> bool {
    let bytes = key.as_bytes();
    if bytes.is_empty() || bytes.len() > 128 {
        return false;
    }
    if !bytes[0].is_ascii_alphabetic() {
        return false;
    }
    bytes
        .iter()
        .skip(1)
        .all(|&b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pdfops::test_support::make_blank_pdf;

    #[test]
    fn write_metadata_rejects_invalid_custom_key() {
        let pdf = make_blank_pdf();
        let mut meta = Metadata::default();
        meta.custom.insert("bad name!".into(), "value".into());
        let err = write_metadata(&pdf, &meta).unwrap_err();
        assert!(matches!(err, EngineError::InvalidOption(_)));
    }

    #[test]
    fn custom_key_validator_accepts_letters_digits_dashes_underscores() {
        assert!(is_valid_custom_key("Foo"));
        assert!(is_valid_custom_key("My-Custom_Key1"));
        assert!(!is_valid_custom_key("1Numeric"));
        assert!(!is_valid_custom_key(""));
        assert!(!is_valid_custom_key("has space"));
        assert!(!is_valid_custom_key("has!bang"));
    }
}
