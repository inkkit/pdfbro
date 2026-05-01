//! Webhook URL validation (SSRF protection).

use std::net::{Ipv4Addr, Ipv6Addr};

use url::Url;

/// Validation errors.
#[derive(Debug, thiserror::Error)]
pub enum ValidationError {
    /// Invalid URL format.
    #[error("Invalid URL: {0}")]
    InvalidUrl(String),
    /// URL scheme is not http or https.
    #[error("URL scheme must be https or http: {0}")]
    InvalidScheme(String),
    /// Private IP address (RFC 1918) not allowed.
    #[error("Private IP addresses not allowed: {0}")]
    PrivateIp(String),
    /// Loopback address not allowed.
    #[error("Loopback addresses not allowed: {0}")]
    Loopback(String),
    /// Link-local address not allowed.
    #[error("Link-local addresses not allowed: {0}")]
    LinkLocal(String),
    /// Multicast address not allowed.
    #[error("Multicast addresses not allowed: {0}")]
    Multicast(String),
    /// Broadcast address not allowed.
    #[error("Broadcast addresses not allowed: {0}")]
    Broadcast(String),
    /// Hostname resolved to a blocked IP.
    #[error("Hostname resolved to blocked IP: {0}")]
    BlockedHost(String),
}

/// Validate webhook URL for security (SSRF protection).
///
/// Blocks:
/// - Non-HTTP/HTTPS schemes
/// - Private IP ranges (10.0.0.0/8, 172.16.0.0/12, 192.168.0.0/16)
/// - Loopback (127.0.0.0/8, ::1)
/// - Link-local (169.254.0.0/16, fe80::/10)
/// - Multicast (224.0.0.0/4, ff00::/8)
/// - Broadcast (255.255.255.255)
/// - localhost hostname
///
/// Returns Ok(()) if URL is safe, Err(ValidationError) otherwise.
pub fn validate_webhook_url(url_str: &str) -> Result<(), ValidationError> {
    // Parse URL
    let url = Url::parse(url_str)
        .map_err(|e| ValidationError::InvalidUrl(format!("Failed to parse URL: {}", e)))?;

    // Check scheme
    let scheme = url.scheme();
    if scheme != "http" && scheme != "https" {
        return Err(ValidationError::InvalidScheme(scheme.into()));
    }

    // Check for localhost hostname
    if let Some(host) = url.host_str() {
        let host_lower = host.to_lowercase();
        if host_lower == "localhost" || host_lower.ends_with(".localhost") {
            return Err(ValidationError::BlockedHost("localhost".into()));
        }
    }

    // Check IP address if present
    if let Some(host) = url.host() {
        match host {
            url::Host::Ipv4(ip) => validate_ipv4(ip)?,
            url::Host::Ipv6(ip) => validate_ipv6(ip)?,
            url::Host::Domain(_) => {
                // Domain names are allowed, but we should ideally resolve and check
                // For now, allow domains and rely on DNS resolution security
            }
        }
    }

    Ok(())
}

/// Compiled allow/deny regex sets layered on top of [`validate_webhook_url`].
///
/// Order of checks for `is_allowed(url)`:
/// 1. SSRF check via `validate_webhook_url` (private/loopback/etc).
/// 2. Allow-list — if non-empty, the URL must match at least one regex.
/// 3. Deny-list — the URL must not match any regex.
#[derive(Debug, Clone, Default)]
pub struct WebhookUrlValidator {
    allow: Vec<regex::Regex>,
    deny: Vec<regex::Regex>,
}

impl WebhookUrlValidator {
    /// Compile the user-supplied regex strings. Returns the offending
    /// pattern + compile error on first failure so the operator can fix
    /// their config.
    pub fn compile(allow: &[String], deny: &[String]) -> Result<Self, String> {
        let allow = compile_patterns("webhook-allow-list", allow)?;
        let deny = compile_patterns("webhook-deny-list", deny)?;
        Ok(Self { allow, deny })
    }

    /// True if neither list has any entries — used by callers to skip
    /// the regex passes when they're a no-op.
    pub fn is_empty(&self) -> bool {
        self.allow.is_empty() && self.deny.is_empty()
    }

    /// Run SSRF + allow + deny checks against the given URL.
    pub fn is_allowed(&self, url: &str) -> Result<(), ValidationError> {
        validate_webhook_url(url)?;
        if !self.allow.is_empty() && !self.allow.iter().any(|r| r.is_match(url)) {
            return Err(ValidationError::BlockedHost(format!(
                "url did not match any --webhook-allow-list pattern: {url}"
            )));
        }
        if let Some(matched) = self.deny.iter().find(|r| r.is_match(url)) {
            return Err(ValidationError::BlockedHost(format!(
                "url matched --webhook-deny-list pattern `{}`: {url}",
                matched.as_str()
            )));
        }
        Ok(())
    }
}

fn compile_patterns(field: &str, patterns: &[String]) -> Result<Vec<regex::Regex>, String> {
    patterns
        .iter()
        .map(|p| {
            regex::Regex::new(p)
                .map_err(|e| format!("invalid {field} regex `{p}`: {e}"))
        })
        .collect()
}

/// Validate IPv4 address.
fn validate_ipv4(ip: Ipv4Addr) -> Result<(), ValidationError> {
    // Check loopback
    if ip.is_loopback() {
        return Err(ValidationError::Loopback(ip.to_string()));
    }

    // Check private ranges
    if ip.is_private() {
        return Err(ValidationError::PrivateIp(ip.to_string()));
    }

    // Check link-local
    if ip.is_link_local() {
        return Err(ValidationError::LinkLocal(ip.to_string()));
    }

    // Check multicast
    if ip.is_multicast() {
        return Err(ValidationError::Multicast(ip.to_string()));
    }

    // Check broadcast
    if ip == Ipv4Addr::new(255, 255, 255, 255) {
        return Err(ValidationError::Broadcast(ip.to_string()));
    }

    Ok(())
}

/// Validate IPv6 address.
fn validate_ipv6(ip: Ipv6Addr) -> Result<(), ValidationError> {
    // Check loopback (::1)
    if ip == Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 1) {
        return Err(ValidationError::Loopback(ip.to_string()));
    }

    // Check link-local (fe80::/10)
    if (ip.segments()[0] & 0xffc0) == 0xfe80 {
        return Err(ValidationError::LinkLocal(ip.to_string()));
    }

    // Check multicast (ff00::/8)
    if ip.segments()[0] & 0xff00 == 0xff00 {
        return Err(ValidationError::Multicast(ip.to_string()));
    }

    // Check unique local addresses (fc00::/7) - private
    if (ip.segments()[0] & 0xfe00) == 0xfc00 {
        return Err(ValidationError::PrivateIp(ip.to_string()));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allows_public_https_url() {
        assert!(validate_webhook_url("https://example.com/webhook").is_ok());
    }

    #[test]
    fn allows_public_http_url() {
        assert!(validate_webhook_url("http://api.example.com/v1/webhook").is_ok());
    }

    #[test]
    fn rejects_ftp_scheme() {
        assert!(validate_webhook_url("ftp://example.com/webhook").is_err());
    }

    #[test]
    fn rejects_localhost() {
        assert!(validate_webhook_url("http://localhost/webhook").is_err());
    }

    #[test]
    fn rejects_loopback_ip() {
        assert!(validate_webhook_url("http://127.0.0.1/webhook").is_err());
    }

    #[test]
    fn rejects_private_ip_10() {
        assert!(validate_webhook_url("http://10.0.0.1/webhook").is_err());
    }

    #[test]
    fn rejects_private_ip_192_168() {
        assert!(validate_webhook_url("http://192.168.1.1/webhook").is_err());
    }

    #[test]
    fn rejects_link_local() {
        assert!(validate_webhook_url("http://169.254.1.1/webhook").is_err());
    }

    #[test]
    fn rejects_multicast() {
        assert!(validate_webhook_url("http://224.0.0.1/webhook").is_err());
    }

    #[test]
    fn rejects_broadcast() {
        assert!(validate_webhook_url("http://255.255.255.255/webhook").is_err());
    }

    #[test]
    fn rejects_ipv6_loopback() {
        assert!(validate_webhook_url("http://[::1]/webhook").is_err());
    }

    #[test]
    fn validator_empty_allows_public_url() {
        let v = WebhookUrlValidator::compile(&[], &[]).unwrap();
        assert!(v.is_allowed("https://hooks.example.com/x").is_ok());
        assert!(v.is_empty());
    }

    #[test]
    fn validator_allow_list_blocks_unmatched() {
        let v =
            WebhookUrlValidator::compile(&["^https://hooks\\.example\\.com/".into()], &[]).unwrap();
        assert!(v.is_allowed("https://hooks.example.com/x").is_ok());
        assert!(v.is_allowed("https://other.example.org/x").is_err());
    }

    #[test]
    fn validator_deny_list_blocks_matched() {
        let v = WebhookUrlValidator::compile(&[], &["evil\\.example\\.com".into()]).unwrap();
        assert!(v.is_allowed("https://safe.example.com/x").is_ok());
        assert!(v.is_allowed("https://evil.example.com/x").is_err());
    }

    #[test]
    fn validator_ssrf_check_runs_first() {
        // Even with a permissive allow-list, private IPs must be rejected
        // because SSRF is the first check.
        let v = WebhookUrlValidator::compile(&[".*".into()], &[]).unwrap();
        assert!(v.is_allowed("http://10.0.0.1/x").is_err());
    }

    #[test]
    fn validator_compile_returns_pattern_in_error() {
        let err = WebhookUrlValidator::compile(&["[".into()], &[]).unwrap_err();
        assert!(err.contains("webhook-allow-list"), "msg was: {err}");
        assert!(err.contains('['), "msg was: {err}");
    }

    #[test]
    fn validator_deny_takes_precedence_when_both_match() {
        let v = WebhookUrlValidator::compile(
            &["example\\.com".into()],
            &["evil\\.example\\.com".into()],
        )
        .unwrap();
        assert!(v.is_allowed("https://safe.example.com/x").is_ok());
        // matches both, but deny wins
        assert!(v.is_allowed("https://evil.example.com/x").is_err());
    }
}
