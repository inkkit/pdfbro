//! `read_metadata` / `write_metadata` — manipulate the `/Info` dictionary.
//!
//! The implementation rides on lopdf's canonical text-string codec
//! (`text_string` / `decode_text_string`) so unicode round-trips through
//! UTF-16BE with BOM, ASCII rides as a literal, and PDFDocEncoding inputs
//! decode losslessly. `/ModDate` is auto-stamped to the current UTC time
//! when not supplied; the date formatter is hand-rolled (Howard Hinnant's
//! `civil_from_days`) to avoid a calendar dep.

use std::collections::BTreeMap;
use std::time::{SystemTime, UNIX_EPOCH};

use lopdf::{Dictionary, Object};
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
    /// Producer string. Always overwritten by the common `finalize` step
    /// to `folio/<crate-version>`; values supplied via `write_metadata`
    /// are silently superseded.
    pub producer: Option<String>,
    /// PDF date string (`D:YYYYMMDDhhmmss±hh'mm'`).
    pub creation_date: Option<String>,
    /// PDF date string for last modification.
    pub mod_date: Option<String>,
    /// Custom info-dict entries; keys must match
    /// `^[A-Za-z][A-Za-z0-9_-]{0,127}$`.
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
    let doc = super::parse_input(pdf)?;

    let Ok(info_id) = doc.trailer.get(b"Info").and_then(|o| o.as_reference()) else {
        return Ok(Metadata::default());
    };
    let Ok(info) = doc.get_object(info_id).and_then(|o| o.as_dict()) else {
        return Ok(Metadata::default());
    };

    let mut meta = Metadata::default();
    for (key_bytes, value) in info.iter() {
        let Some(decoded) = super::decode_pdf_text_string(value) else {
            continue;
        };
        match key_bytes.as_slice() {
            b"Title" => meta.title = Some(decoded),
            b"Author" => meta.author = Some(decoded),
            b"Subject" => meta.subject = Some(decoded),
            b"Keywords" => meta.keywords = Some(decoded),
            b"Creator" => meta.creator = Some(decoded),
            b"Producer" => meta.producer = Some(decoded),
            b"CreationDate" => meta.creation_date = Some(decoded),
            b"ModDate" => meta.mod_date = Some(decoded),
            other => {
                if let Ok(name) = std::str::from_utf8(other) {
                    meta.custom.insert(name.to_string(), decoded);
                }
            }
        }
    }
    Ok(meta)
}

/// Merge `meta` into the document's `/Info` dictionary.
///
/// `None` fields are left untouched; `Some("")` fields are removed. Custom
/// keys must match `^[A-Za-z][A-Za-z0-9_-]{0,127}$`. `/ModDate` is auto-
/// stamped to the current UTC time unless `meta.mod_date` is supplied.
///
/// # Errors
///
/// - [`EngineError::InvalidOption`] for malformed custom keys.
/// - [`EngineError::Internal`] if the input fails to parse, is encrypted,
///   or the result fails to save.
pub fn write_metadata(pdf: &[u8], meta: &Metadata) -> EngineResult<Vec<u8>> {
    validate_custom_keys(&meta.custom)?;
    let mut doc = super::parse_input(pdf)?;
    let info_id = super::ensure_info_dict(&mut doc);

    let Ok(Object::Dictionary(d)) = doc.get_object_mut(info_id) else {
        return Err(EngineError::Internal(
            "/Info object is not a dictionary".into(),
        ));
    };

    apply_text_field(d, "Title", meta.title.as_deref());
    apply_text_field(d, "Author", meta.author.as_deref());
    apply_text_field(d, "Subject", meta.subject.as_deref());
    apply_text_field(d, "Keywords", meta.keywords.as_deref());
    apply_text_field(d, "Creator", meta.creator.as_deref());
    // `/Producer` is set by `finalize`; user-supplied value is honored
    // here for round-trip semantics but will be overwritten before save.
    apply_text_field(d, "Producer", meta.producer.as_deref());
    apply_text_field(d, "CreationDate", meta.creation_date.as_deref());

    match meta.mod_date.as_deref() {
        Some("") => {
            d.remove(b"ModDate");
        }
        Some(v) => {
            d.set("ModDate", super::encode_pdf_text_string(v));
        }
        None => {
            d.set("ModDate", super::encode_pdf_text_string(&now_pdf_date()));
        }
    }

    for (k, v) in &meta.custom {
        if v.is_empty() {
            d.remove(k.as_bytes());
        } else {
            d.set(k.clone(), super::encode_pdf_text_string(v));
        }
    }

    super::finalize(doc)
}

fn apply_text_field(d: &mut Dictionary, key: &str, value: Option<&str>) {
    match value {
        None => {} // leave untouched
        Some("") => {
            d.remove(key.as_bytes());
        }
        Some(v) => {
            d.set(key, super::encode_pdf_text_string(v));
        }
    }
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

/// Format the current UTC instant as a PDF date string.
fn now_pdf_date() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let (y, m, d, hh, mm, ss) = unix_to_utc_calendar(secs);
    format!("D:{y:04}{m:02}{d:02}{hh:02}{mm:02}{ss:02}Z00'00'")
}

/// Convert UNIX seconds (UTC) to `(year, month, day, hour, minute, second)`.
/// Uses Howard Hinnant's `civil_from_days` algorithm
/// (<https://howardhinnant.github.io/date_algorithms.html>).
fn unix_to_utc_calendar(secs: i64) -> (i64, u32, u32, u32, u32, u32) {
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    let h = (rem / 3600) as u32;
    let mi = ((rem / 60) % 60) as u32;
    let s = (rem % 60) as u32;

    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = (z - era * 146_097) as u64; // [0, 146_096]
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365; // [0, 399]
    let y_civil = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let y = if m <= 2 { y_civil + 1 } else { y_civil };
    (y, m, d, h, mi, s)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pdfops::test_support::make_blank_pdf;
    use lopdf::Document;

    fn round_trip(pdf: &[u8], meta: &Metadata) -> Metadata {
        let out = write_metadata(pdf, meta).unwrap();
        read_metadata(&out).unwrap()
    }

    #[test]
    fn write_metadata_rejects_invalid_custom_key() {
        let pdf = make_blank_pdf();
        let meta = Metadata {
            custom: BTreeMap::from([("bad name!".to_string(), "value".to_string())]),
            ..Default::default()
        };
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

    #[test]
    fn metadata_default_when_info_dict_missing() {
        // Build a doc, strip /Info from the trailer, then read.
        let pdf = make_blank_pdf();
        let mut doc = Document::load_mem(&pdf).unwrap();
        doc.trailer.remove(b"Info");
        let mut bytes = Vec::new();
        doc.save_to(&mut bytes).unwrap();

        let meta = read_metadata(&bytes).unwrap();
        // /Info absent ⇒ all fields default. (The finalize step in the
        // builder may have left /Info absent; if not, our default is
        // permissive about unknown values.)
        assert_eq!(meta.title, None);
        assert_eq!(meta.author, None);
        assert!(meta.custom.is_empty());
    }

    #[test]
    fn write_metadata_empty_string_removes_key() {
        let pdf = make_blank_pdf();
        let m = Metadata {
            title: Some("Hello".into()),
            ..Default::default()
        };
        let with_title = write_metadata(&pdf, &m).unwrap();
        assert_eq!(
            read_metadata(&with_title).unwrap().title.as_deref(),
            Some("Hello")
        );

        let clear = Metadata {
            title: Some(String::new()),
            ..Default::default()
        };
        let cleared = write_metadata(&with_title, &clear).unwrap();
        assert_eq!(read_metadata(&cleared).unwrap().title, None);
    }

    #[test]
    fn write_metadata_round_trips_ascii_fields() {
        let pdf = make_blank_pdf();
        let meta = Metadata {
            title: Some("Hello".into()),
            author: Some("Alice".into()),
            subject: Some("Test".into()),
            keywords: Some("kw".into()),
            creator: Some("Folio".into()),
            ..Default::default()
        };
        let back = round_trip(&pdf, &meta);
        assert_eq!(back.title.as_deref(), Some("Hello"));
        assert_eq!(back.author.as_deref(), Some("Alice"));
        assert_eq!(back.subject.as_deref(), Some("Test"));
        assert_eq!(back.keywords.as_deref(), Some("kw"));
        assert_eq!(back.creator.as_deref(), Some("Folio"));
    }

    #[test]
    fn write_metadata_round_trips_unicode_title() {
        let pdf = make_blank_pdf();
        let meta = Metadata {
            title: Some("héllo, 世界".into()),
            ..Default::default()
        };
        let back = round_trip(&pdf, &meta);
        assert_eq!(back.title.as_deref(), Some("héllo, 世界"));
    }

    #[test]
    fn write_metadata_custom_keys_round_trip() {
        let pdf = make_blank_pdf();
        let meta = Metadata {
            custom: BTreeMap::from([
                ("Foo".to_string(), "bar".to_string()),
                ("My-Key_1".to_string(), "value 2".to_string()),
            ]),
            ..Default::default()
        };
        let back = round_trip(&pdf, &meta);
        assert_eq!(back.custom.get("Foo").map(String::as_str), Some("bar"));
        assert_eq!(
            back.custom.get("My-Key_1").map(String::as_str),
            Some("value 2")
        );
    }

    #[test]
    fn write_metadata_auto_stamps_mod_date() {
        let pdf = make_blank_pdf();
        let meta = Metadata {
            title: Some("X".into()),
            ..Default::default()
        };
        let back = round_trip(&pdf, &meta);
        let mod_date = back.mod_date.expect("ModDate should be auto-stamped");
        assert!(mod_date.starts_with("D:"), "mod_date: {mod_date}");
        assert!(mod_date.ends_with("'00'"), "mod_date: {mod_date}");
    }

    #[test]
    fn write_metadata_honors_user_supplied_mod_date() {
        let pdf = make_blank_pdf();
        let meta = Metadata {
            mod_date: Some("D:20240101000000Z00'00'".into()),
            ..Default::default()
        };
        let back = round_trip(&pdf, &meta);
        assert_eq!(back.mod_date.as_deref(), Some("D:20240101000000Z00'00'"));
    }

    #[test]
    fn unix_to_utc_calendar_known_dates() {
        assert_eq!(unix_to_utc_calendar(0), (1970, 1, 1, 0, 0, 0));
        // 2024-02-29 12:34:56 UTC
        assert_eq!(unix_to_utc_calendar(1709210096), (2024, 2, 29, 12, 34, 56));
        // 2025-01-01 00:00:00 UTC
        assert_eq!(unix_to_utc_calendar(1735689600), (2025, 1, 1, 0, 0, 0));
    }

    #[test]
    fn now_pdf_date_format_is_pdf_date() {
        let s = now_pdf_date();
        assert!(s.starts_with("D:"));
        assert!(s.ends_with("Z00'00'"));
        assert_eq!(s.len(), "D:YYYYMMDDHHMMSSZ00'00'".len());
    }
}
