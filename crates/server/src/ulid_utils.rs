//! ULID utility functions for validation and parsing.
//!
//! ULIDs provide lexicographic sorting, URL-safety, and 26-character
//! lowercase encoding using Crockford's base32 alphabet.

use crate::error::ApiError;

/// Crockford's base32 alphabet valid characters.
const VALID_ULID_CHARS: &[u8] = b"0123456789abcdefghjkmnpqrstvwxyz";

/// Validate a string is a valid ULID format.
///
/// ULIDs are exactly 26 characters using Crockford base32 encoding.
/// Valid characters: 0-9, a-z (excluding i, l, o, u for readability).
///
/// # Examples
///
/// ```
/// use server::ulid_utils::is_valid_ulid;
///
/// assert!(is_valid_ulid("01hqrqhp6qw2v3c5x7z9abcd8e"));
/// assert!(!is_valid_ulid("invalid"));
/// assert!(!is_valid_ulid("550e8400-e29b-41d4-a716-446655440000")); // UUID
/// ```
pub fn is_valid_ulid(s: &str) -> bool {
    if s.len() != 26 {
        return false;
    }

    s.bytes().all(|b| VALID_ULID_CHARS.contains(&b))
}

/// Parse ULID from string with proper error handling.
///
/// # Errors
///
/// Returns `ApiError::InvalidField` if the string is not a valid ULID.
pub fn parse_ulid(s: &str) -> Result<ulid::Ulid, ApiError> {
    if !is_valid_ulid(s) {
        return Err(ApiError::InvalidField {
            field: "id",
            message: format!(
                "Invalid ULID format: '{}' (expected 26 lowercase chars, got {} chars)",
                s,
                s.len()
            ),
        });
    }

    s.parse::<ulid::Ulid>().map_err(|e| ApiError::InvalidField {
        field: "id",
        message: format!("Failed to parse ULID: {}", e),
    })
}

/// Extract ULID from a prefixed string (e.g., "batch_01hqr...").
pub fn extract_ulid_from_prefixed(s: &str, prefix: &str) -> Result<ulid::Ulid, ApiError> {
    if !s.starts_with(prefix) {
        return Err(ApiError::InvalidField {
            field: "id",
            message: format!("Expected prefix '{}' in '{}'", prefix, s),
        });
    }

    let ulid_part = &s[prefix.len()..];
    parse_ulid(ulid_part)
}

/// Generate a new ULID string in lowercase.
pub fn generate_ulid() -> String {
    ulid::Ulid::new().to_string().to_lowercase()
}

/// Generate a new ULID with a prefix (e.g., "batch_01hqr...").
pub fn generate_prefixed_ulid(prefix: &str) -> String {
    format!("{}{}", prefix, generate_ulid())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ulid_generation_is_lowercase() {
        let id = generate_ulid();
        assert!(id.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit()));
        assert_eq!(id.len(), 26);
    }

    #[test]
    fn ulid_sorting_is_chronological() {
        let id1 = generate_ulid();
        std::thread::sleep(std::time::Duration::from_millis(2));
        let id2 = generate_ulid();
        assert!(id1 < id2, "ULIDs should sort chronologically");
    }

    #[test]
    fn valid_ulid_passes() {
        assert!(is_valid_ulid("01hqrqhp6qw2v3c5x7z9abcd8e"));
        assert!(is_valid_ulid("00000000000000000000000000"));
        assert!(is_valid_ulid("7zzzzzzzzzzzzzzzzzzzzzzzzz"));
    }

    #[test]
    fn invalid_ulid_rejected() {
        // Wrong length
        assert!(!is_valid_ulid("tooshort"));
        assert!(!is_valid_ulid("waytoolongwaytoolongwaytoolong"));

        // Invalid characters
        assert!(!is_valid_ulid("01hqrqhp6qw2v3c5x7z9abcd8i")); // 'i' not allowed
        assert!(!is_valid_ulid("01hqrqhp6qw2v3c5x7z9abcd8l")); // 'l' not allowed
        assert!(!is_valid_ulid("01hqrqhp6qw2v3c5x7z9abcd8o")); // 'o' not allowed
        assert!(!is_valid_ulid("01hqrqhp6qw2v3c5x7z9abcd8u")); // 'u' not allowed

        // Uppercase (should be lowercase)
        assert!(!is_valid_ulid("01HQRQHP6QW2V3C5X7Z9ABCD8E"));
    }

    #[test]
    fn parse_ulid_valid() {
        let result = parse_ulid("01hqrqhp6qw2v3c5x7z9abcd8e");
        assert!(result.is_ok());
    }

    #[test]
    fn parse_ulid_invalid() {
        let result = parse_ulid("invalid");
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("Invalid ULID format"));
    }

    #[test]
    fn extract_from_prefixed() {
        let result = extract_ulid_from_prefixed("batch_01hqrqhp6qw2v3c5x7z9abcd8e", "batch_");
        assert!(result.is_ok());

        let result = extract_ulid_from_prefixed("wrongprefix_01hqrqhp6qw2v3c5x7z9abcd8e", "batch_");
        assert!(result.is_err());
    }

    #[test]
    fn generate_prefixed() {
        let id = generate_prefixed_ulid("batch_");
        assert!(id.starts_with("batch_"));
        assert_eq!(id.len(), 32); // 6 + 26
        assert!(id.chars().skip(6).all(|c| c.is_ascii_lowercase() || c.is_ascii_digit()));
    }
}
