// crates/decision-gate-mcp/src/namespace_authority.rs
// ============================================================================
// Module: Namespace Authority
// Description: Namespace validation backends for Decision Gate MCP.
// Purpose: Enforce namespace scoping with optional Asset Core catalog checks.
// Dependencies: decision-gate-core, reqwest
// ============================================================================

//! ## Overview
//! Namespace authority checks validate that namespaces are known and permitted.
//! Asset Core integration uses the write-daemon namespace endpoints to verify
//! namespace existence and authorization without coupling to ASC internals.
//! Security posture: namespace checks are a trust boundary; fail closed. See
//! `Docs/security/threat_model.md`.

// ============================================================================
// SECTION: Imports
// ============================================================================

use std::time::Duration;

use async_trait::async_trait;
use decision_gate_core::NamespaceId;
use decision_gate_core::TenantId;
use reqwest::Client;
use reqwest::StatusCode;
use reqwest::header::HeaderMap;
use reqwest::header::HeaderValue;
use thiserror::Error;

use crate::correlation::sanitize_client_correlation_id;

// ============================================================================
// SECTION: Public Types
// ============================================================================

/// Namespace authority interface.
#[async_trait]
pub trait NamespaceAuthority: Send + Sync {
    /// Ensures the namespace is known and allowed.
    ///
    /// # Errors
    ///
    /// Returns [`NamespaceAuthorityError`] when validation fails.
    async fn ensure_namespace(
        &self,
        tenant_id: Option<&TenantId>,
        namespace_id: &NamespaceId,
        request_id: Option<&str>,
    ) -> Result<(), NamespaceAuthorityError>;
}

/// No-op authority for standalone deployments.
///
/// # Invariants
/// - Always allows namespace access.
pub struct NoopNamespaceAuthority;

#[async_trait]
impl NamespaceAuthority for NoopNamespaceAuthority {
    async fn ensure_namespace(
        &self,
        _tenant_id: Option<&TenantId>,
        _namespace_id: &NamespaceId,
        _request_id: Option<&str>,
    ) -> Result<(), NamespaceAuthorityError> {
        Ok(())
    }
}

/// Asset Core-backed namespace authority.
///
/// # Invariants
/// - Base URL is normalized without a trailing slash.
pub struct AssetCoreNamespaceAuthority {
    /// Asset Core base URL (no trailing slash).
    base_url: String,
    /// Optional bearer token for Asset Core requests.
    auth_token: Option<String>,
    /// HTTP client configured with timeouts.
    client: Client,
}

impl AssetCoreNamespaceAuthority {
    /// Builds a new Asset Core namespace authority.
    ///
    /// # Errors
    ///
    /// Returns [`NamespaceAuthorityError`] when the HTTP client cannot be built.
    pub fn new(
        mut base_url: String,
        auth_token: Option<String>,
        connect_timeout: Duration,
        request_timeout: Duration,
    ) -> Result<Self, NamespaceAuthorityError> {
        let client = Client::builder()
            .connect_timeout(connect_timeout)
            .timeout(request_timeout)
            .build()
            .map_err(|err| NamespaceAuthorityError::Unavailable(err.to_string()))?;
        let trimmed_len = base_url.trim_end_matches('/').len();
        base_url.truncate(trimmed_len);
        Ok(Self {
            base_url,
            auth_token,
            client,
        })
    }

    /// Builds headers for Asset Core namespace authority requests.
    fn build_headers(
        &self,
        request_id: Option<&str>,
    ) -> Result<HeaderMap, NamespaceAuthorityError> {
        let mut headers = HeaderMap::new();
        if let Some(token) = &self.auth_token {
            let value = HeaderValue::from_str(&format!("Bearer {token}")).map_err(|_| {
                NamespaceAuthorityError::InvalidNamespace("invalid auth token".to_string())
            })?;
            headers.insert(reqwest::header::AUTHORIZATION, value);
        }
        let request_id = sanitize_client_correlation_id(request_id).map_err(|_| {
            NamespaceAuthorityError::InvalidNamespace("invalid request id".to_string())
        })?;
        if let Some(request_id) = request_id {
            headers.insert(
                "x-correlation-id",
                HeaderValue::from_str(&request_id).map_err(|_| {
                    NamespaceAuthorityError::InvalidNamespace("invalid request id".to_string())
                })?,
            );
        }
        Ok(headers)
    }
}

#[async_trait]
impl NamespaceAuthority for AssetCoreNamespaceAuthority {
    async fn ensure_namespace(
        &self,
        _tenant_id: Option<&TenantId>,
        namespace_id: &NamespaceId,
        request_id: Option<&str>,
    ) -> Result<(), NamespaceAuthorityError> {
        let url = format!("{}/v1/write/namespaces/{}", self.base_url, namespace_id.get());
        let headers = self.build_headers(request_id)?;
        let response = self
            .client
            .get(url)
            .headers(headers)
            .send()
            .await
            .map_err(|err| NamespaceAuthorityError::Unavailable(err.to_string()))?;
        match response.status() {
            StatusCode::OK => Ok(()),
            StatusCode::NOT_FOUND => {
                Err(NamespaceAuthorityError::Denied("namespace not found".to_string()))
            }
            StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => {
                Err(NamespaceAuthorityError::Denied("namespace not authorized".to_string()))
            }
            status => Err(NamespaceAuthorityError::Unavailable(format!(
                "namespace authority error: status {status}"
            ))),
        }
    }
}

// ============================================================================
// SECTION: Errors
// ============================================================================

/// Namespace authority failures.
///
/// # Invariants
/// - Variants are stable for error classification.
#[derive(Debug, Error)]
pub enum NamespaceAuthorityError {
    /// Namespace identifier is invalid for the configured authority.
    #[error("invalid namespace: {0}")]
    InvalidNamespace(String),
    /// Namespace is unknown or unauthorized.
    #[error("namespace denied: {0}")]
    Denied(String),
    /// Namespace authority is unavailable.
    #[error("namespace authority unavailable: {0}")]
    Unavailable(String),
}

// (helpers removed; correlation sanitization lives in crate::correlation)
