//! Webhook URL validation (SSRF protection).

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

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
}
