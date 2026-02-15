// crates/decision-gate-providers/src/http.rs
// ============================================================================
// Module: HTTP Evidence Provider
// Description: Evidence provider for HTTP endpoint checks.
// Purpose: Provide status and body-hash evidence with strict limits.
// Dependencies: decision-gate-core, reqwest, serde_json
// ============================================================================

//! ## Overview
//! The HTTP provider issues bounded GET requests and returns status codes or
//! body hashes. It enforces scheme restrictions, host allowlists, redirects
//! disabled by default, and size limits to preserve fail-closed behavior.
//! Security posture: evidence inputs are untrusted; see `Docs/security/threat_model.md`.

// ============================================================================
// SECTION: Imports
// ============================================================================

use std::collections::BTreeSet;
use std::io::Read;
use std::net::IpAddr;
use std::net::SocketAddr;
use std::net::ToSocketAddrs;
use std::time::Duration;

use decision_gate_core::EvidenceAnchor;
use decision_gate_core::EvidenceContext;
use decision_gate_core::EvidenceError;
use decision_gate_core::EvidenceProvider;
use decision_gate_core::EvidenceQuery;
use decision_gate_core::EvidenceRef;
use decision_gate_core::EvidenceResult;
use decision_gate_core::EvidenceValue;
use decision_gate_core::HashAlgorithm;
use decision_gate_core::ProviderMissingError;
use decision_gate_core::ScenarioSpec;
use decision_gate_core::TrustLane;
use decision_gate_core::hashing::DEFAULT_HASH_ALGORITHM;
use decision_gate_core::hashing::hash_bytes;
use reqwest::Url;
use reqwest::blocking::Client;
use reqwest::blocking::Response;
use reqwest::redirect::Policy;
use serde::Deserialize;
use serde_json::Number;
use serde_json::Value;

// ============================================================================
// SECTION: Configuration
// ============================================================================

/// Configuration for the HTTP provider.
///
/// # Invariants
/// - `allow_http = false` blocks cleartext `http://` URLs.
/// - `max_response_bytes` is enforced as a hard upper bound on response bodies.
/// - If `allowed_hosts` is set, only listed hosts are permitted.
/// - `allow_private_networks = false` blocks private/link-local/loopback targets.
/// - URLs with embedded credentials are rejected.
/// - `timeout_ms` applies to the full request lifecycle.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct HttpProviderConfig {
    /// Allow cleartext HTTP (disabled by default).
    pub allow_http: bool,
    /// Request timeout in milliseconds.
    pub timeout_ms: u64,
    /// Maximum response size allowed, in bytes.
    pub max_response_bytes: usize,
    /// Optional host allowlist.
    pub allowed_hosts: Option<BTreeSet<String>>,
    /// Allow requests to private/link-local/loopback addresses.
    pub allow_private_networks: bool,
    /// User agent string for outbound requests.
    pub user_agent: String,
    /// Hash algorithm used for body hash responses.
    pub hash_algorithm: HashAlgorithm,
}

impl Default for HttpProviderConfig {
    fn default() -> Self {
        Self {
            allow_http: false,
            timeout_ms: 5_000,
            max_response_bytes: 1024 * 1024,
            allowed_hosts: None,
            allow_private_networks: false,
            user_agent: "decision-gate/0.1".to_string(),
            hash_algorithm: DEFAULT_HASH_ALGORITHM,
        }
    }
}

// ============================================================================
// SECTION: Provider Implementation
// ============================================================================

/// Evidence provider for HTTP endpoint checks.
///
/// # Invariants
/// - Only `status` and `body_hash` checks are supported.
/// - Redirects are not followed.
/// - Responses exceeding configured limits fail closed.
pub struct HttpProvider {
    /// Provider configuration, including limits and policy.
    config: HttpProviderConfig,
    /// HTTP client used for outbound requests.
    client: Client,
}

impl HttpProvider {
    /// Creates a new HTTP provider with the given configuration.
    ///
    /// # Errors
    ///
    /// Returns [`EvidenceError`] when the HTTP client cannot be created.
    pub fn new(config: HttpProviderConfig) -> Result<Self, EvidenceError> {
        let client = build_http_client(&config, None)?;
        Ok(Self {
            config,
            client,
        })
    }
}

impl EvidenceProvider for HttpProvider {
    fn query(
        &self,
        query: &EvidenceQuery,
        _ctx: &EvidenceContext,
    ) -> Result<EvidenceResult, EvidenceError> {
        let url = extract_url(query.params.as_ref())?;
        let resolved = resolve_request_host(&url, &self.config)?;

        match query.check_id.as_str() {
            "status" => {
                let response = self.send_pinned_request(&url, &resolved)?;
                let status = response.status().as_u16();
                Ok(EvidenceResult {
                    value: Some(EvidenceValue::Json(Value::Number(Number::from(status)))),
                    lane: TrustLane::Verified,
                    error: None,
                    evidence_hash: None,
                    evidence_ref: Some(EvidenceRef {
                        uri: url.to_string(),
                    }),
                    evidence_anchor: Some(EvidenceAnchor {
                        anchor_type: "url".to_string(),
                        anchor_value: url.to_string(),
                    }),
                    signature: None,
                    content_type: Some("application/json".to_string()),
                })
            }
            "body_hash" => {
                let mut response = self.send_pinned_request(&url, &resolved)?;
                let body = read_response_limited(&mut response, self.config.max_response_bytes)?;
                let digest = hash_bytes(self.config.hash_algorithm, &body);
                let hash_value = serde_json::to_value(digest).map_err(|_| {
                    EvidenceError::Provider("hash serialization failed".to_string())
                })?;
                Ok(EvidenceResult {
                    value: Some(EvidenceValue::Json(hash_value)),
                    lane: TrustLane::Verified,
                    error: None,
                    evidence_hash: None,
                    evidence_ref: Some(EvidenceRef {
                        uri: url.to_string(),
                    }),
                    evidence_anchor: Some(EvidenceAnchor {
                        anchor_type: "url".to_string(),
                        anchor_value: url.to_string(),
                    }),
                    signature: None,
                    content_type: Some("application/json".to_string()),
                })
            }
            _ => Err(EvidenceError::Provider("unsupported http check".to_string())),
        }
    }

    fn validate_providers(&self, _spec: &ScenarioSpec) -> Result<(), ProviderMissingError> {
        Ok(())
    }
}

impl HttpProvider {
    /// Sends a request using pinned DNS resolution for the selected host.
    ///
    /// # Errors
    ///
    /// Returns [`EvidenceError`] when all resolved peers fail or policy checks fail.
    fn send_pinned_request(
        &self,
        url: &Url,
        resolved: &ResolvedHost,
    ) -> Result<Response, EvidenceError> {
        let mut last_error: Option<EvidenceError> = None;
        for ip in &resolved.ips {
            let client = match self.client_for_ip(resolved, *ip) {
                Ok(client) => client,
                Err(err) => {
                    last_error = Some(err);
                    continue;
                }
            };
            let Ok(response) = client.get(url.as_str()).send() else {
                last_error = Some(EvidenceError::Provider("http request failed".to_string()));
                continue;
            };
            if response.url() != url {
                return Err(EvidenceError::Provider("http redirect not allowed".to_string()));
            }
            enforce_ip_policy(&resolved.host_label, *ip, self.config.allow_private_networks)?;
            return Ok(response);
        }
        Err(last_error
            .unwrap_or_else(|| EvidenceError::Provider("http request failed".to_string())))
    }

    /// Builds a client pinned to a specific resolved IP when needed.
    ///
    /// # Errors
    ///
    /// Returns [`EvidenceError`] when the pinned HTTP client cannot be built.
    fn client_for_ip(&self, resolved: &ResolvedHost, ip: IpAddr) -> Result<Client, EvidenceError> {
        if !resolved.is_domain {
            return Ok(self.client.clone());
        }
        let socket_addr = SocketAddr::new(ip, resolved.port);
        build_http_client(&self.config, Some((&resolved.host, socket_addr)))
    }
}

// ============================================================================
// SECTION: Helpers
// ============================================================================

/// Extracts the URL from query parameters.
fn extract_url(params: Option<&Value>) -> Result<Url, EvidenceError> {
    let params =
        params.ok_or_else(|| EvidenceError::Provider("http check requires params".to_string()))?;
    let Value::Object(map) = params else {
        return Err(EvidenceError::Provider("http params must be an object".to_string()));
    };
    let Value::String(url) =
        map.get("url").ok_or_else(|| EvidenceError::Provider("missing url param".to_string()))?
    else {
        return Err(EvidenceError::Provider("url param must be a string".to_string()));
    };
    Url::parse(url).map_err(|_| EvidenceError::Provider("invalid url".to_string()))
}

/// Validates URL scheme and allowlist policy.
fn validate_url(url: &Url, config: &HttpProviderConfig) -> Result<(), EvidenceError> {
    match url.scheme() {
        "https" => {}
        "http" if config.allow_http => {}
        _ => return Err(EvidenceError::Provider("unsupported url scheme".to_string())),
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err(EvidenceError::Provider("url credentials are not allowed".to_string()));
    }
    if let Some(allowlist) = &config.allowed_hosts {
        let host = normalize_host_label(
            url.host_str()
                .ok_or_else(|| EvidenceError::Provider("url host required".to_string()))?,
        );
        let allowed = allowlist.iter().any(|entry| normalize_host_label(entry.as_str()) == host);
        if !allowed {
            return Err(EvidenceError::Provider("url host not allowed".to_string()));
        }
    }
    Ok(())
}

/// Resolves host metadata and validates address policy before requests.
fn resolve_request_host(
    url: &Url,
    config: &HttpProviderConfig,
) -> Result<ResolvedHost, EvidenceError> {
    validate_url(url, config)?;
    let host =
        url.host_str().ok_or_else(|| EvidenceError::Provider("url host required".to_string()))?;
    let host_label = normalize_host_label(host);
    let host_for_resolution =
        host.strip_prefix('[').and_then(|inner| inner.strip_suffix(']')).unwrap_or(host);
    let port = url
        .port_or_known_default()
        .ok_or_else(|| EvidenceError::Provider("url port required".to_string()))?;
    let mut ips = resolve_host_ips(host_for_resolution, port)?;
    if ips.is_empty() {
        return Err(EvidenceError::Provider("url host has no resolved addresses".to_string()));
    }
    for ip in &ips {
        enforce_ip_policy(&host_label, *ip, config.allow_private_networks)?;
    }
    dedupe_ips(&mut ips);
    Ok(ResolvedHost {
        host: host_for_resolution.to_string(),
        host_label,
        port,
        ips,
        is_domain: host_for_resolution.parse::<IpAddr>().is_err(),
    })
}

/// Builds an HTTP client with optional DNS pinning override.
fn build_http_client(
    config: &HttpProviderConfig,
    resolve: Option<(&str, SocketAddr)>,
) -> Result<Client, EvidenceError> {
    let mut builder = Client::builder()
        .timeout(Duration::from_millis(config.timeout_ms))
        .user_agent(config.user_agent.clone())
        .redirect(Policy::none());
    if let Some((host, socket_addr)) = resolve {
        builder = builder.resolve(host, socket_addr);
    }
    builder.build().map_err(|_| EvidenceError::Provider("http client build failed".to_string()))
}

/// Resolves hostnames to peer IPs used for policy checks and pinning.
fn resolve_host_ips(host: &str, port: u16) -> Result<Vec<IpAddr>, EvidenceError> {
    if let Ok(ip) = host.parse::<IpAddr>() {
        return Ok(vec![ip]);
    }
    (host, port)
        .to_socket_addrs()
        .map(|iter| iter.map(|addr| addr.ip()).collect::<Vec<IpAddr>>())
        .map_err(|_| EvidenceError::Provider("url host resolution failed".to_string()))
}

/// Enforces private/link-local restrictions for resolved peer IPs.
fn enforce_ip_policy(
    host_label: &str,
    ip: IpAddr,
    allow_private_networks: bool,
) -> Result<(), EvidenceError> {
    if allow_private_networks {
        return Ok(());
    }
    if is_private_or_link_local(&ip) {
        return Err(EvidenceError::Provider(format!(
            "url host resolves to private or link-local address: {host_label}"
        )));
    }
    Ok(())
}

/// Returns true when an IP is private, loopback, link-local, or otherwise local.
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

/// Normalizes host labels for allowlist comparisons.
fn normalize_host_label(host: &str) -> String {
    let trimmed = host.trim_end_matches('.');
    let trimmed =
        trimmed.strip_prefix('[').and_then(|inner| inner.strip_suffix(']')).unwrap_or(trimmed);
    trimmed.to_ascii_lowercase()
}

/// Deduplicates IP addresses while preserving insertion order.
fn dedupe_ips(ips: &mut Vec<IpAddr>) {
    let mut unique = Vec::with_capacity(ips.len());
    for ip in ips.drain(..) {
        if !unique.contains(&ip) {
            unique.push(ip);
        }
    }
    *ips = unique;
}

/// Resolved host metadata for pinned outbound requests.
///
/// # Invariants
/// - `ips` is non-empty and deduplicated.
/// - `port` is the effective request port.
struct ResolvedHost {
    /// Host string as it appears in the URL.
    host: String,
    /// Normalized host label used in policy messages.
    host_label: String,
    /// Effective request port.
    port: u16,
    /// Resolved candidate peer IPs.
    ips: Vec<IpAddr>,
    /// True when host represents a DNS domain name.
    is_domain: bool,
}

/// Reads the response body while enforcing a byte limit.
fn read_response_limited(
    response: &mut reqwest::blocking::Response,
    max_bytes: usize,
) -> Result<Vec<u8>, EvidenceError> {
    let expected_len = response.content_length();
    let max_bytes_u64 = u64::try_from(max_bytes)
        .map_err(|_| EvidenceError::Provider("response size limit exceeds u64".to_string()))?;
    if let Some(expected) = expected_len
        && expected > max_bytes_u64
    {
        return Err(EvidenceError::Provider("http response exceeds size limit".to_string()));
    }
    let mut buf = Vec::new();
    let limit = max_bytes_u64.saturating_add(1);
    let mut handle = response.take(limit);
    handle
        .read_to_end(&mut buf)
        .map_err(|_| EvidenceError::Provider("failed to read response".to_string()))?;
    if buf.len() > max_bytes {
        return Err(EvidenceError::Provider("http response exceeds size limit".to_string()));
    }
    if let Some(expected) = expected_len {
        let expected = usize::try_from(expected)
            .map_err(|_| EvidenceError::Provider("invalid response length".to_string()))?;
        if buf.len() < expected {
            return Err(EvidenceError::Provider("http response truncated".to_string()));
        }
    }
    Ok(buf)
}
