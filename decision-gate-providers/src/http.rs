// decision-gate-providers/src/http.rs
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
use reqwest::redirect::Policy;
use serde::Deserialize;
use serde_json::Number;
use serde_json::Value;

// ============================================================================
// SECTION: Configuration
// ============================================================================

/// Configuration for the HTTP provider.
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
            user_agent: "decision-gate/0.1".to_string(),
            hash_algorithm: DEFAULT_HASH_ALGORITHM,
        }
    }
}

// ============================================================================
// SECTION: Provider Implementation
// ============================================================================

/// Evidence provider for HTTP endpoint checks.
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
        let client = Client::builder()
            .timeout(Duration::from_millis(config.timeout_ms))
            .user_agent(config.user_agent.clone())
            .redirect(Policy::none())
            .build()
            .map_err(|_| EvidenceError::Provider("http client build failed".to_string()))?;
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
        validate_url(&url, &self.config)?;

        match query.check_id.as_str() {
            "status" => {
                let response = self
                    .client
                    .get(url.clone())
                    .send()
                    .map_err(|_| EvidenceError::Provider("http request failed".to_string()))?;
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
                let mut response = self
                    .client
                    .get(url.clone())
                    .send()
                    .map_err(|_| EvidenceError::Provider("http request failed".to_string()))?;
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
    if let Some(allowlist) = &config.allowed_hosts {
        let host = url
            .host_str()
            .ok_or_else(|| EvidenceError::Provider("url host required".to_string()))?;
        if !allowlist.contains(host) {
            return Err(EvidenceError::Provider("url host not allowed".to_string()));
        }
    }
    Ok(())
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
