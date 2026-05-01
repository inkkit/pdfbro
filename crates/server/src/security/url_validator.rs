//! SSRF (Server-Side Request Forgery) prevention for URL validation.
//!
//! Blocks access to internal networks, localhost, and other potentially
//! dangerous destinations to prevent attacks via the `url_to_pdf` endpoint.

use std::net::IpAddr;

use crate::error::ApiError;

/// CIDR block for IP range matching.
#[derive(Debug, Clone)]
pub struct IpNet {
    network: IpAddr,
    prefix: u8,
}

impl IpNet {
    /// Parse a CIDR string like "127.0.0.0/8" or "::1/128".
    pub fn parse(s: &str) -> Result<Self, String> {
        let parts: Vec<&str> = s.split('/').collect();
        if parts.len() != 2 {
            return Err(format!("Invalid CIDR format: {}", s));
        }

        let network = parts[0]
            .parse::<IpAddr>()
            .map_err(|e| format!("Invalid IP address: {}", e))?;
        let prefix = parts[1]
            .parse::<u8>()
            .map_err(|e| format!("Invalid prefix: {}", e))?;

        Ok(Self { network, prefix })
    }

    /// Check if an IP address is contained in this network.
    pub fn contains(&self, ip: &IpAddr) -> bool {
        match (self.network, ip) {
            (IpAddr::V4(net), IpAddr::V4(addr)) => {
                let net_bits = u32::from_be_bytes(net.octets());
                let addr_bits = u32::from_be_bytes(addr.octets());
                let mask = if self.prefix == 0 {
                    0
                } else {
                    !0u32 << (32 - self.prefix)
                };
                (net_bits & mask) == (addr_bits & mask)
            }
            (IpAddr::V6(net), IpAddr::V6(addr)) => {
                let net_segments = net.segments();
                let addr_segments = addr.segments();

                let full_segments = self.prefix as usize / 16;
                let partial_bits = self.prefix as usize % 16;

                // Check full segments
                for i in 0..full_segments {
                    if net_segments[i] != addr_segments[i] {
                        return false;
                    }
                }

                // Check partial segment if needed
                if partial_bits > 0 && full_segments < 8 {
                    let mask = !0u16 << (16 - partial_bits);
                    return (net_segments[full_segments] & mask)
                        == (addr_segments[full_segments] & mask);
                }

                true
            }
            _ => false, // IPv4/IPv6 mismatch
        }
    }
}

impl std::str::FromStr for IpNet {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

impl std::fmt::Display for IpNet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}/{}", self.network, self.prefix)
    }
}

/// Configuration for URL validation / SSRF prevention.
#[derive(Debug, Clone)]
pub struct UrlValidationConfig {
    /// Block these CIDR ranges (default: private/reserved).
    pub blocked_cidrs: Vec<IpNet>,
    /// Only allow these schemes (default: http, https).
    pub allowed_schemes: Vec<String>,
    /// Block these hostnames/patterns.
    pub blocked_hosts: Vec<String>,
    /// Require explicit allowlist match (deny-by-default mode).
    pub allowlist_only: bool,
    /// Allowed hostnames/patterns (if allowlist_only).
    pub allowed_hosts: Vec<String>,
}

impl Default for UrlValidationConfig {
    fn default() -> Self {
        Self {
            blocked_cidrs: vec![
                IpNet::parse("127.0.0.0/8").unwrap(),     // Loopback
                IpNet::parse("10.0.0.0/8").unwrap(),      // Private
                IpNet::parse("172.16.0.0/12").unwrap(),   // Private
                IpNet::parse("192.168.0.0/16").unwrap(),  // Private
                IpNet::parse("169.254.0.0/16").unwrap(),  // Link-local
                IpNet::parse("0.0.0.0/8").unwrap(),       // Current network
                IpNet::parse("fc00::/7").unwrap(),        // IPv6 private
                IpNet::parse("fe80::/10").unwrap(),     // IPv6 link-local
                IpNet::parse("::1/128").unwrap(),       // IPv6 loopback
                IpNet::parse("100.64.0.0/10").unwrap(),   // CGNAT
                IpNet::parse("192.0.0.0/24").unwrap(),    // IETF protocol assignments
                IpNet::parse("192.0.2.0/24").unwrap(),    // TEST-NET-1
                IpNet::parse("198.18.0.0/15").unwrap(),   // Benchmark testing
                IpNet::parse("198.51.100.0/24").unwrap(), // TEST-NET-2
                IpNet::parse("203.0.113.0/24").unwrap(),  // TEST-NET-3
                IpNet::parse("224.0.0.0/4").unwrap(),     // Multicast
                IpNet::parse("240.0.0.0/4").unwrap(),     // Reserved
                IpNet::parse("255.255.255.255/32").unwrap(), // Broadcast
            ],
            allowed_schemes: vec!["http".into(), "https".into()],
            blocked_hosts: vec![
                "localhost".into(),
                "*.local".into(),
                "*.internal".into(),
                "*.localhost".into(),
            ],
            allowlist_only: false,
            allowed_hosts: vec![],
        }
    }
}

impl UrlValidationConfig {
    /// Create a strict allowlist-only configuration.
    pub fn allowlist(hosts: Vec<String>) -> Self {
        Self {
            allowed_hosts: hosts,
            allowlist_only: true,
            ..Default::default()
        }
    }
}

/// Validate a URL against SSRF prevention rules.
///
/// # Arguments
///
/// * `url` - The URL to validate
/// * `config` - Validation configuration
///
/// # Errors
///
/// Returns `ApiError::InvalidField` if the URL is blocked.
pub async fn validate_url(url: &str, config: &UrlValidationConfig) -> Result<(), ApiError> {
    let parsed = url::Url::parse(url).map_err(|e| ApiError::InvalidField {
        field: "url",
        message: format!("Invalid URL: {}", e),
    })?;

    // Scheme check
    let scheme = parsed.scheme();
    if !config.allowed_schemes.contains(&scheme.to_string()) {
        return Err(ApiError::InvalidField {
            field: "url",
            message: format!(
                "URL scheme '{}' not allowed (allowed: {})",
                scheme,
                config.allowed_schemes.join(", ")
            ),
        });
    }

    // Host extraction
    let host = parsed.host_str().ok_or_else(|| ApiError::InvalidField {
        field: "url",
        message: "URL missing host".into(),
    })?;

    // Hostname pattern matching (blocked)
    for blocked in &config.blocked_hosts {
        if host_matches_pattern(host, blocked) {
            return Err(ApiError::InvalidField {
                field: "url",
                message: format!(
                    "Host '{}' matches blocked pattern '{}'",
                    host, blocked
                ),
            });
        }
    }

    // Allowlist check
    if config.allowlist_only {
        let mut allowed = false;
        for pattern in &config.allowed_hosts {
            if host_matches_pattern(host, pattern) {
                allowed = true;
                break;
            }
        }
        if !allowed {
            return Err(ApiError::InvalidField {
                field: "url",
                message: format!(
                    "Host '{}' not in allowlist. Allowed hosts: {}",
                    host,
                    config.allowed_hosts.join(", ")
                ),
            });
        }
    }

    // DNS resolution and IP check
    // Note: We resolve the hostname to check if it points to blocked IPs
    let port = parsed.port_or_known_default().unwrap_or(80);
    let addrs = match tokio::net::lookup_host(format!("{}:{}", host, port)).await {
        Ok(addrs) => addrs,
        Err(e) => {
            // DNS lookup failed - this might be a valid domain that just doesn't exist
            // or a network issue. We allow this through and let the actual request fail.
            tracing::warn!("DNS lookup failed for {}: {}", host, e);
            return Ok(());
        }
    };

    for addr in addrs {
        let ip = addr.ip();
        for cidr in &config.blocked_cidrs {
            if cidr.contains(&ip) {
                return Err(ApiError::InvalidField {
                    field: "url",
                    message: format!(
                        "URL '{}' resolves to blocked IP {} (range: {}) - possible SSRF attempt",
                        url, ip, cidr
                    ),
                });
            }
        }
    }

    Ok(())
}

/// Check if a hostname matches a pattern (supports wildcards).
fn host_matches_pattern(host: &str, pattern: &str) -> bool {
    let pattern_lower = pattern.to_lowercase();
    let host_lower = host.to_lowercase();

    if pattern_lower.starts_with("*.") {
        let suffix = &pattern_lower[2..];
        host_lower == suffix || host_lower.ends_with(&format!(".{}", suffix))
    } else {
        host_lower == pattern_lower
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ipnet_ipv4_contains() {
        let net = IpNet::parse("127.0.0.0/8").unwrap();
        assert!(net.contains(&IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1))));
        assert!(net.contains(&IpAddr::V4(Ipv4Addr::new(127, 255, 255, 255))));
        assert!(!net.contains(&IpAddr::V4(Ipv4Addr::new(128, 0, 0, 1))));
    }

    #[test]
    fn ipnet_ipv6_contains() {
        let net = IpNet::parse("::1/128").unwrap();
        assert!(net.contains(&IpAddr::V6(Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 1))));
        assert!(!net.contains(&IpAddr::V6(Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 2))));
    }

    #[test]
    fn host_pattern_matching() {
        assert!(host_matches_pattern("localhost", "localhost"));
        assert!(host_matches_pattern("api.localhost", "*.localhost"));
        assert!(host_matches_pattern("sub.api.localhost", "*.localhost"));
        assert!(!host_matches_pattern("other.com", "*.localhost"));
        assert!(host_matches_pattern("service.internal", "*.internal"));
    }

    #[tokio::test]
    async fn blocks_localhost_url() {
        let config = UrlValidationConfig::default();
        assert!(validate_url("http://localhost/", &config).await.is_err());
        assert!(validate_url("http://localhost:3000/", &config).await.is_err());
        assert!(validate_url("http://127.0.0.1/", &config).await.is_err());
    }

    #[tokio::test]
    async fn blocks_private_ips() {
        let config = UrlValidationConfig::default();
        assert!(validate_url("http://10.0.0.1/", &config).await.is_err());
        assert!(validate_url("http://192.168.1.1/", &config).await.is_err());
        assert!(validate_url("http://172.16.0.1/", &config).await.is_err());
    }

    #[tokio::test]
    async fn allows_public_urls() {
        let config = UrlValidationConfig::default();
        assert!(validate_url("https://example.com/", &config).await.is_ok());
        assert!(validate_url("https://www.google.com/search", &config).await.is_ok());
    }

    #[tokio::test]
    async fn blocks_non_http_schemes() {
        let config = UrlValidationConfig::default();
        assert!(validate_url("file:///etc/passwd", &config).await.is_err());
        assert!(validate_url("ftp://ftp.example.com/", &config).await.is_err());
        assert!(validate_url("gopher://gopher.example.com/", &config).await.is_err());
    }

    #[tokio::test]
    async fn allowlist_mode() {
        let config = UrlValidationConfig::allowlist(vec!["example.com".into()]);

        assert!(validate_url("https://example.com/", &config).await.is_ok());
        assert!(validate_url("https://sub.example.com/", &config).await.is_err());
        assert!(validate_url("https://other.com/", &config).await.is_err());

        let config = UrlValidationConfig::allowlist(vec!["*.example.com".into()]);
        assert!(validate_url("https://sub.example.com/", &config).await.is_ok());
        assert!(validate_url("https://deep.sub.example.com/", &config).await.is_ok());
    }
}
