// crates/decision-gate-broker/src/source/http.rs
// ============================================================================
// Module: Decision Gate HTTP Source
// Description: HTTP-backed source for external payload resolution.
// Purpose: Fetch payload bytes via HTTP GET.
// Dependencies: decision-gate-core, reqwest, url
// ============================================================================

//! ## Overview
//! [`HttpSource`] resolves `http://` and `https://` URIs into payload bytes.
//! Non-success status codes fail closed.
//! Invariants:
//! - Host policy checks are enforced before any request is sent.
//! - Redirects are rejected.
//! - Payload bytes are capped at [`crate::source::MAX_SOURCE_BYTES`].
//! - DNS resolution is pinned per request and re-validated before accepting responses.
//!
//! Security posture: treats remote content as untrusted; see
//! `Docs/security/threat_model.md`.

// ============================================================================
// SECTION: Imports
// ============================================================================

use std::io::Read;
use std::net::IpAddr;
use std::net::SocketAddr;
use std::net::ToSocketAddrs;
use std::time::Duration;

use decision_gate_core::ContentRef;
use reqwest::blocking::Client;
use reqwest::header::CONTENT_TYPE;
use reqwest::redirect::Policy;
use url::Host;
use url::Url;

use crate::source::Source;
use crate::source::SourceError;
use crate::source::SourcePayload;
use crate::source::enforce_max_bytes;
use crate::source::max_source_bytes_u64;

// ============================================================================
// SECTION: HTTP Source
// ============================================================================

/// Host allowlist + denylist policy for HTTP sources.
///
/// # Invariants
/// - Denylist rules are evaluated before allowlist rules.
/// - Private/link-local IPs are denied unless explicitly allowed.
#[derive(Debug, Clone, Default)]
pub struct HttpSourcePolicy {
    /// Optional allowlist of hosts. When set, only matching hosts are allowed.
    allowlist: Option<Vec<HostPattern>>,
    /// Explicitly denied hosts (matched before allowlist).
    denylist: Vec<HostPattern>,
    /// Whether private and link-local IP ranges are allowed.
    allow_private_networks: bool,
}

impl HttpSourcePolicy {
    /// Creates a default policy (public hosts only, private ranges denied).
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Replaces the allowlist with the provided hosts.
    #[must_use]
    pub fn allow_hosts<I, S>(mut self, hosts: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let patterns = parse_host_patterns(hosts);
        self.allowlist = Some(patterns);
        self
    }

    /// Replaces the denylist with the provided hosts.
    #[must_use]
    pub fn deny_hosts<I, S>(mut self, hosts: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        self.denylist = parse_host_patterns(hosts);
        self
    }

    /// Allows requests to private/link-local IP ranges.
    #[must_use]
    pub const fn allow_private_networks(mut self) -> Self {
        self.allow_private_networks = true;
        self
    }

    /// Validates the provided URL against the policy and resolves host IPs.
    fn resolve_request(&self, url: &Url) -> Result<ResolvedHost, SourceError> {
        let host = url.host().ok_or_else(|| SourceError::InvalidUri("missing host".to_string()))?;
        let host_label = normalize_host_label(&host);
        if self.is_denied(&host_label) {
            return Err(SourceError::Policy(format!("host denied: {host_label}")));
        }
        if let Some(allowlist) = &self.allowlist
            && !allowlist.iter().any(|pattern| pattern.matches(&host_label))
        {
            return Err(SourceError::Policy(format!("host not in allowlist: {host_label}")));
        }
        let port = url.port_or_known_default().ok_or_else(|| {
            SourceError::InvalidUri("missing port for host resolution".to_string())
        })?;
        let mut ips = resolve_host_ips(&host, port)?;
        if ips.is_empty() {
            return Err(SourceError::Policy(format!("host has no resolved IPs: {host_label}")));
        }
        if !self.allow_private_networks {
            for ip in &ips {
                self.enforce_ip_policy(&host_label, *ip)?;
            }
        }
        dedupe_ips(&mut ips);
        Ok(ResolvedHost {
            host_label,
            host: owned_host(&host),
            port,
            ips,
        })
    }

    /// Validates a peer IP against the policy.
    fn enforce_ip_policy(&self, host_label: &str, ip: IpAddr) -> Result<(), SourceError> {
        if self.allow_private_networks {
            return Ok(());
        }
        if is_private_or_link_local(&ip) {
            return Err(SourceError::Policy(format!(
                "host resolves to private or link-local address: {host_label}"
            )));
        }
        Ok(())
    }

    /// Returns true when a host matches the denylist.
    fn is_denied(&self, host: &str) -> bool {
        self.denylist.iter().any(|pattern| pattern.matches(host))
    }
}

/// Host allow/deny pattern.
#[derive(Debug, Clone)]
enum HostPattern {
    /// Exact host match.
    Exact(String),
    /// Wildcard suffix match (for example: *.example.com).
    WildcardSuffix(String),
}

impl HostPattern {
    /// Parses a host pattern string into a normalized matcher.
    fn parse(raw: &str) -> Option<Self> {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return None;
        }
        let normalized = normalize_host_string(trimmed);
        if let Some(suffix) = normalized.strip_prefix("*.") {
            if suffix.is_empty() {
                return None;
            }
            return Some(Self::WildcardSuffix(suffix.to_string()));
        }
        Some(Self::Exact(normalized))
    }

    /// Returns true when the pattern matches the provided host.
    fn matches(&self, host: &str) -> bool {
        match self {
            Self::Exact(value) => host == value,
            Self::WildcardSuffix(suffix) => {
                if host.len() <= suffix.len() || !host.ends_with(suffix) {
                    return false;
                }
                let boundary = host.len() - suffix.len() - 1;
                host.as_bytes().get(boundary) == Some(&b'.')
            }
        }
    }
}

/// Parses an iterable of host patterns into normalized matchers.
fn parse_host_patterns<I, S>(hosts: I) -> Vec<HostPattern>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    hosts.into_iter().filter_map(|host| HostPattern::parse(host.as_ref())).collect()
}

/// Normalizes a host label into a lowercase string for matching.
fn normalize_host_label(host: &Host<&str>) -> String {
    match host {
        Host::Domain(domain) => normalize_host_string(domain),
        Host::Ipv4(ip) => ip.to_string(),
        Host::Ipv6(ip) => ip.to_string(),
    }
}

/// Normalizes raw host strings by trimming trailing dots and brackets.
fn normalize_host_string(host: &str) -> String {
    let trimmed = host.trim_end_matches('.');
    let trimmed =
        trimmed.strip_prefix('[').and_then(|inner| inner.strip_suffix(']')).unwrap_or(trimmed);
    trimmed.to_ascii_lowercase()
}

/// Resolves hostnames to IP addresses for private-range validation.
fn resolve_host_ips(host: &Host<&str>, port: u16) -> Result<Vec<IpAddr>, SourceError> {
    match host {
        Host::Ipv4(ip) => Ok(vec![IpAddr::V4(*ip)]),
        Host::Ipv6(ip) => Ok(vec![IpAddr::V6(*ip)]),
        Host::Domain(domain) => (*domain, port)
            .to_socket_addrs()
            .map(|iter| iter.map(|addr| addr.ip()).collect::<Vec<IpAddr>>())
            .map_err(|err| SourceError::Policy(format!("dns lookup failed: {err}"))),
    }
}

/// Converts a host reference into an owned host.
fn owned_host(host: &Host<&str>) -> Host<String> {
    match host {
        Host::Domain(domain) => Host::Domain(domain.to_string()),
        Host::Ipv4(ip) => Host::Ipv4(*ip),
        Host::Ipv6(ip) => Host::Ipv6(*ip),
    }
}

/// Removes duplicate IPs while preserving order.
fn dedupe_ips(ips: &mut Vec<IpAddr>) {
    let mut unique = Vec::with_capacity(ips.len());
    for ip in ips.drain(..) {
        if !unique.contains(&ip) {
            unique.push(ip);
        }
    }
    *ips = unique;
}

/// Returns true if the IP is private, link-local, loopback, or unspecified.
#[allow(
    clippy::option_if_let_else,
    reason = "Option::map_or is not const-callable on current toolchain."
)]
const fn is_private_or_link_local(ip: &IpAddr) -> bool {
    match ip {
        IpAddr::V4(addr) => {
            addr.is_private()
                || addr.is_loopback()
                || addr.is_link_local()
                || addr.is_unspecified()
                || addr.is_multicast()
                || addr.is_broadcast()
        }
        IpAddr::V6(addr) => {
            let mapped_private = if let Some(mapped) = addr.to_ipv4_mapped() {
                mapped.is_private()
                    || mapped.is_loopback()
                    || mapped.is_link_local()
                    || mapped.is_unspecified()
                    || mapped.is_multicast()
                    || mapped.is_broadcast()
            } else {
                false
            };
            mapped_private
                || addr.is_loopback()
                || addr.is_unique_local()
                || addr.is_unicast_link_local()
                || addr.is_unspecified()
                || addr.is_multicast()
        }
    }
}

/// HTTP-backed payload source.
///
/// # Invariants
/// - Redirects are rejected.
/// - Responses exceeding [`crate::source::MAX_SOURCE_BYTES`] are rejected.
/// - Host policy is enforced on each request with pinned DNS resolution.
#[derive(Debug, Clone)]
pub struct HttpSource {
    /// HTTP client used for fetch requests.
    client: Client,
    /// Host policy enforcement for outbound requests.
    policy: HttpSourcePolicy,
}

impl HttpSource {
    /// Builds an HTTP source with a default client.
    ///
    /// # Errors
    ///
    /// Returns [`SourceError`] when the HTTP client cannot be constructed.
    pub fn new() -> Result<Self, SourceError> {
        Self::with_policy(HttpSourcePolicy::default())
    }

    /// Builds an HTTP source with a specific host policy.
    ///
    /// # Errors
    ///
    /// Returns [`SourceError`] when the HTTP client cannot be constructed.
    pub fn with_policy(policy: HttpSourcePolicy) -> Result<Self, SourceError> {
        let client = Client::builder()
            .redirect(Policy::none())
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|err| SourceError::Http(err.to_string()))?;
        Ok(Self {
            client,
            policy,
        })
    }

    /// Creates an HTTP source with a preconfigured client.
    #[must_use]
    pub const fn with_client(client: Client) -> Self {
        Self {
            client,
            policy: HttpSourcePolicy {
                allowlist: None,
                denylist: Vec::new(),
                allow_private_networks: false,
            },
        }
    }

    /// Creates an HTTP source with a preconfigured client and policy.
    #[must_use]
    pub const fn with_client_and_policy(client: Client, policy: HttpSourcePolicy) -> Self {
        Self {
            client,
            policy,
        }
    }
}

impl Source for HttpSource {
    fn fetch(&self, content_ref: &ContentRef) -> Result<SourcePayload, SourceError> {
        let url =
            Url::parse(&content_ref.uri).map_err(|err| SourceError::InvalidUri(err.to_string()))?;
        match url.scheme() {
            "http" | "https" => {}
            scheme => return Err(SourceError::UnsupportedScheme(scheme.to_string())),
        }
        let resolved = self.policy.resolve_request(&url)?;

        let mut last_error = None;
        for ip in resolved.ips.iter().copied() {
            let client = match self.client_for_ip(&resolved, ip) {
                Ok(client) => client,
                Err(err) => {
                    last_error = Some(err);
                    continue;
                }
            };
            let response = match client.get(url.as_str()).send() {
                Ok(response) => response,
                Err(err) => {
                    last_error = Some(SourceError::Http(err.to_string()));
                    continue;
                }
            };
            if response.url() != &url {
                return Err(SourceError::Http(format!(
                    "redirected from {} to {}",
                    url,
                    response.url()
                )));
            }
            if !response.status().is_success() {
                return Err(SourceError::Http(format!("http status {}", response.status())));
            }
            // Re-validate the pinned peer IP before accepting the response.
            self.policy.enforce_ip_policy(&resolved.host_label, ip)?;

            let max_bytes = max_source_bytes_u64()?;
            if let Some(length) = response.content_length()
                && length > max_bytes
            {
                let actual_bytes = usize::try_from(length).unwrap_or(usize::MAX);
                return Err(SourceError::TooLarge {
                    max_bytes: crate::source::MAX_SOURCE_BYTES,
                    actual_bytes,
                });
            }
            let content_type = response
                .headers()
                .get(CONTENT_TYPE)
                .and_then(|value| value.to_str().ok())
                .map(str::to_string);
            let limit = max_bytes.checked_add(1).ok_or(SourceError::LimitOverflow {
                limit: crate::source::MAX_SOURCE_BYTES,
            })?;
            let mut limited = response.take(limit);
            let mut bytes = Vec::new();
            limited.read_to_end(&mut bytes).map_err(|err| SourceError::Http(err.to_string()))?;
            enforce_max_bytes(bytes.len())?;
            return Ok(SourcePayload {
                bytes,
                content_type,
            });
        }

        Err(last_error.unwrap_or_else(|| {
            SourceError::Http("request failed for all resolved IPs".to_string())
        }))
    }
}

impl HttpSource {
    /// Builds a client pinned to the provided IP when required.
    fn client_for_ip(&self, resolved: &ResolvedHost, ip: IpAddr) -> Result<Client, SourceError> {
        let socket_addr = SocketAddr::new(ip, resolved.port);
        match &resolved.host {
            Host::Domain(domain) => Client::builder()
                .redirect(Policy::none())
                .timeout(Duration::from_secs(30))
                .resolve(domain.as_str(), socket_addr)
                .build()
                .map_err(|err| SourceError::Http(err.to_string())),
            Host::Ipv4(_) | Host::Ipv6(_) => Ok(self.client.clone()),
        }
    }
}

// ============================================================================
// SECTION: Resolved Host
// ============================================================================

/// Resolved host metadata for pinned HTTP requests.
///
/// # Invariants
/// - `ips` is non-empty and contains unique entries.
/// - `port` is the effective request port.
struct ResolvedHost {
    /// Normalized host label used for policy reporting.
    host_label: String,
    /// Resolved host for request construction.
    host: Host<String>,
    /// Effective port for the request.
    port: u16,
    /// Resolved IPs for the host.
    ips: Vec<IpAddr>,
}
