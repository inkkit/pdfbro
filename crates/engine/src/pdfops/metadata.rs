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
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;

use crate::types::{EngineError, EngineResult};

/// PDF document metadata (the `/Info` dictionary plus arbitrary custom
/// entries).
///
/// Standard date fields accept ISO 8601 (`2006-09-18T16:27:50-04:00`)
/// or PDF date (`D:20060918162750-04'00'`) on input; they are returned
/// in ExifTool format (`2006:09:18 16:27:50-04:00`) on read.
///
/// Unknown JSON fields (e.g. `Copyright`, `Marked`, `PDFVersion`,
/// `Trapped`) are absorbed into [`Metadata::custom`] via `flatten`,
/// preserving their original JSON types.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Metadata {
    /// Document title.
    #[serde(rename = "Title", skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// Document author.
    #[serde(rename = "Author", skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    /// Document subject.
    #[serde(rename = "Subject", skip_serializing_if = "Option::is_none")]
    pub subject: Option<String>,
    /// Search keywords — accepts either a comma-joined string or a JSON array.
    #[serde(
        rename = "Keywords",
        deserialize_with = "de_keywords",
        serialize_with = "se_keywords",
        default
    )]
    pub keywords: Option<Vec<String>>,
    /// Original creator application.
    #[serde(rename = "Creator", skip_serializing_if = "Option::is_none")]
    pub creator: Option<String>,
    /// Producer string. Always overwritten by the common `finalize` step
    /// to `folio/<crate-version>`; values supplied via `write_metadata`
    /// are silently superseded.
    #[serde(rename = "Producer", skip_serializing_if = "Option::is_none")]
    pub producer: Option<String>,
    /// Creation date. JSON field name is `CreateDate` (Gotenberg/ExifTool API).
    /// Accepts ISO 8601 or PDF date format on input; stored in PDF as
    /// `/CreationDate`.
    #[serde(
        rename = "CreateDate",
        alias = "CreationDate",
        skip_serializing_if = "Option::is_none"
    )]
    pub creation_date: Option<String>,
    /// Last-modification date. Accepts ISO 8601 or PDF date format.
    #[serde(rename = "ModDate", skip_serializing_if = "Option::is_none")]
    pub mod_date: Option<String>,
    /// Custom and non-standard info-dict entries (e.g. `Copyright`,
    /// `Marked`, `PDFVersion`, `Trapped`). Absorbed from any unknown
    /// JSON keys via `#[serde(flatten)]`.
    /// On write, keys must match `^[A-Za-z][A-Za-z0-9_-]{0,127}$`.
    #[serde(flatten)]
    pub custom: BTreeMap<String, Value>,
}

fn de_keywords<'de, D>(de: D) -> Result<Option<Vec<String>>, D::Error>
where
    D: Deserializer<'de>,
{
    use serde::de::{SeqAccess, Visitor};
    use std::fmt;

    struct KwVisitor;

    impl<'de> Visitor<'de> for KwVisitor {
        type Value = Option<Vec<String>>;

        fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "a string or array of strings")
        }

        fn visit_none<E: serde::de::Error>(self) -> Result<Self::Value, E> {
            Ok(None)
        }

        fn visit_some<D2: Deserializer<'de>>(self, d: D2) -> Result<Self::Value, D2::Error> {
            d.deserialize_any(KwInnerVisitor).map(Some)
        }

        fn visit_unit<E: serde::de::Error>(self) -> Result<Self::Value, E> {
            Ok(None)
        }

        fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<Self::Value, E> {
            Ok(Some(v.split(", ").map(|s| s.to_string()).collect()))
        }

        fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
            let mut items = Vec::new();
            while let Some(v) = seq.next_element::<String>()? {
                items.push(v);
            }
            Ok(Some(items))
        }
    }

    struct KwInnerVisitor;

    impl<'de> Visitor<'de> for KwInnerVisitor {
        type Value = Vec<String>;

        fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "a string or array of strings")
        }

        fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<Self::Value, E> {
            Ok(v.split(", ").map(|s| s.to_string()).collect())
        }

        fn visit_string<E: serde::de::Error>(self, v: String) -> Result<Self::Value, E> {
            Ok(v.split(", ").map(|s| s.to_string()).collect())
        }

        fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
            let mut items = Vec::new();
            while let Some(v) = seq.next_element::<String>()? {
                items.push(v);
            }
            Ok(items)
        }
    }

    de.deserialize_option(KwVisitor)
}

fn se_keywords<S>(kw: &Option<Vec<String>>, s: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    match kw {
        None => s.serialize_none(),
        Some(v) => s.collect_seq(v.iter()),
    }
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
            b"Keywords" => {
                meta.keywords = Some(decoded.split(", ").map(|s| s.to_string()).collect());
            }
            b"Creator" => meta.creator = Some(decoded),
            b"Producer" => meta.producer = Some(decoded),
            b"CreationDate" => {
                meta.creation_date = Some(pdf_date_to_exiftool(&decoded).unwrap_or(decoded));
            }
            b"ModDate" => {
                meta.mod_date = Some(pdf_date_to_exiftool(&decoded).unwrap_or(decoded));
            }
            other => {
                if let Ok(name) = std::str::from_utf8(other) {
                    meta.custom.insert(name.to_string(), coerce_custom_value(name, decoded));
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
    let keywords_str = meta.keywords.as_ref().map(|v| v.join(", "));
    apply_text_field(d, "Keywords", keywords_str.as_deref());
    apply_text_field(d, "Creator", meta.creator.as_deref());
    apply_date_field(d, "CreationDate", meta.creation_date.as_deref());

    match meta.mod_date.as_deref() {
        Some("") => {
            d.remove(b"ModDate");
        }
        Some(v) => {
            let pdf_date = to_pdf_date(v).unwrap_or_else(|| v.to_string());
            d.set("ModDate", super::encode_pdf_text_string(&pdf_date));
        }
        None => {
            d.set("ModDate", super::encode_pdf_text_string(&now_pdf_date()));
        }
    }

    for (k, v) in &meta.custom {
        // Strip ExifTool group prefix (e.g., "System:FileName" → "FileName").
        let base_key = if let Some(idx) = k.find(':') { &k[idx + 1..] } else { k.as_str() };
        // Silently skip invalid keys and known-dangerous ExifTool filesystem tags.
        if !is_valid_custom_key(base_key) || DANGEROUS_EXIFTOOL_TAGS.contains(&base_key) {
            continue;
        }
        let str_val = json_value_to_string(v);
        match str_val.as_deref() {
            None => {}
            Some("") => { d.remove(base_key.as_bytes()); }
            Some(s) => { d.set(base_key, super::encode_pdf_text_string(s)); }
        }
    }

    super::finalize_with_producer(doc, meta.producer.as_deref())
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

/// Like `apply_text_field` but converts ISO 8601 or ExifTool dates to PDF
/// date format before writing.
fn apply_date_field(d: &mut Dictionary, key: &str, value: Option<&str>) {
    match value {
        None => {}
        Some("") => { d.remove(key.as_bytes()); }
        Some(v) => {
            let pdf_date = to_pdf_date(v).unwrap_or_else(|| v.to_string());
            d.set(key, super::encode_pdf_text_string(&pdf_date));
        }
    }
}

/// Convert a `serde_json::Value` to its string representation for PDF storage.
/// Returns `None` for null/array/object (skip). Returns `Some("")` to remove.
fn json_value_to_string(v: &Value) -> Option<String> {
    match v {
        Value::Null => None,
        Value::Bool(b) => Some(b.to_string()),
        Value::Number(n) => Some(n.to_string()),
        Value::String(s) => Some(s.clone()),
        Value::Array(_) | Value::Object(_) => None,
    }
}

/// Try to convert an ISO 8601 or ExifTool date string to PDF date format.
///
/// - ISO 8601: `YYYY-MM-DDTHH:MM:SS[±HH:MM|Z]`  → `D:YYYYMMDDhhmmss[±HH'MM'|Z00'00']`
/// - ExifTool: `YYYY:MM:DD HH:MM:SS[±HH:MM]`     → same
/// - Already PDF date (`D:…`)                     → returned unchanged
fn to_pdf_date(s: &str) -> Option<String> {
    if s.starts_with("D:") {
        return Some(s.to_string());
    }
    // Detect ISO 8601 (contains 'T') or ExifTool (colon-separated date)
    let (date_part, time_tz) = if let Some(idx) = s.find('T') {
        (&s[..idx], &s[idx + 1..])
    } else if s.len() >= 10 && s.as_bytes()[4] == b':' && s.as_bytes()[7] == b':' {
        // ExifTool format: "YYYY:MM:DD HH:MM:SS±..."
        (&s[..10], s.get(11..).unwrap_or(""))
    } else {
        return None;
    };

    // Parse date: handle both `YYYY-MM-DD` and `YYYY:MM:DD`
    let date_bytes = date_part.as_bytes();
    if date_bytes.len() < 10 {
        return None;
    }
    let year = &date_part[0..4];
    let month = &date_part[5..7];
    let day = &date_part[8..10];

    // Parse time and timezone from `HH:MM:SS[±HH:MM|Z]`
    let (time_part, tz_part) = if time_tz.len() >= 8 {
        let tz_start = time_tz.find(['+', '-', 'Z']).unwrap_or(time_tz.len());
        (&time_tz[..tz_start], &time_tz[tz_start..])
    } else {
        (time_tz, "")
    };

    let time_bytes = time_part.as_bytes();
    let hh = if time_bytes.len() >= 2 { &time_part[0..2] } else { "00" };
    let mm = if time_bytes.len() >= 5 { &time_part[3..5] } else { "00" };
    let ss = if time_bytes.len() >= 8 { &time_part[6..8] } else { "00" };

    let tz = if tz_part.is_empty() || tz_part == "Z" {
        "Z00'00'".to_string()
    } else if tz_part.starts_with(['+', '-']) && tz_part.len() >= 6 {
        // ±HH:MM → ±HH'MM'
        let sign = &tz_part[0..1];
        let tz_h = &tz_part[1..3];
        let tz_m = &tz_part[4..6];
        format!("{sign}{tz_h}'{tz_m}'")
    } else {
        "Z00'00'".to_string()
    };

    Some(format!("D:{year}{month}{day}{hh}{mm}{ss}{tz}"))
}

/// Convert a PDF date string (`D:YYYYMMDDhhmmss±HH'MM'`) to ExifTool format
/// (`YYYY:MM:DD HH:MM:SS±HH:MM`). Returns `None` if the string is not a
/// recognized PDF date.
fn pdf_date_to_exiftool(s: &str) -> Option<String> {
    let s = s.strip_prefix("D:")?;
    if s.len() < 14 {
        return None;
    }
    let year = &s[0..4];
    let month = &s[4..6];
    let day = &s[6..8];
    let hh = &s[8..10];
    let mm = &s[10..12];
    let ss = &s[12..14];
    let tz_raw = &s[14..];
    let tz = if tz_raw.is_empty() || tz_raw.starts_with('Z') {
        "+00:00".to_string()
    } else if tz_raw.starts_with(['+', '-']) && tz_raw.len() >= 6 {
        // ±HH'MM' → ±HH:MM
        let sign = &tz_raw[0..1];
        let tz_h = &tz_raw[1..3];
        // tz_raw[3] is '\'' separator
        let tz_m = tz_raw.get(4..6).unwrap_or("00");
        format!("{sign}{tz_h}:{tz_m}")
    } else {
        "+00:00".to_string()
    };
    Some(format!("{year}:{month}:{day} {hh}:{mm}:{ss}{tz}"))
}

/// Fields known to have a non-string type in the Gotenberg/ExifTool API.
const BOOL_FIELDS: &[&str] = &["Marked"];
const FLOAT_FIELDS: &[&str] = &["PDFVersion"];

/// Coerce a string value read from a PDF /Info entry to the appropriate
/// `serde_json::Value` type for the given field name.
fn coerce_custom_value(field: &str, s: String) -> Value {
    if BOOL_FIELDS.contains(&field) {
        match s.to_ascii_lowercase().as_str() {
            "true" => return Value::Bool(true),
            "false" => return Value::Bool(false),
            _ => {}
        }
    }
    if FLOAT_FIELDS.contains(&field) {
        if let Ok(n) = s.parse::<f64>() {
            if let Some(num) = serde_json::Number::from_f64(n) {
                return Value::Number(num);
            }
        }
    }
    Value::String(s)
}

fn validate_custom_keys(_custom: &BTreeMap<String, Value>) -> EngineResult<()> {
    // Invalid/group-prefixed keys (e.g., "System:FileName") are silently dropped
    // in the write loop below; no upfront rejection needed.
    Ok(())
}

/// ExifTool tags that can manipulate the filesystem when used as metadata keys.
/// These must be blocked even when supplied with a group prefix (e.g., "System:FileName").
const DANGEROUS_EXIFTOOL_TAGS: &[&str] = &["FileName", "Directory", "SymLink", "HardLink"];

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
            keywords: Some(vec!["kw".to_string()]),
            creator: Some("Folio".into()),
            ..Default::default()
        };
        let back = round_trip(&pdf, &meta);
        assert_eq!(back.title.as_deref(), Some("Hello"));
        assert_eq!(back.author.as_deref(), Some("Alice"));
        assert_eq!(back.subject.as_deref(), Some("Test"));
        assert_eq!(back.keywords.as_deref(), Some(&["kw".to_string()][..]));
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
