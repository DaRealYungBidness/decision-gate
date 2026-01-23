// decision-gate-broker/src/source/http.rs
// ============================================================================
// Module: Decision Gate HTTP Source
// Description: HTTP-backed source for external payload resolution.
// Purpose: Fetch payload bytes via HTTP GET.
// Dependencies: reqwest, url
// ============================================================================

//! ## Overview
//! `HttpSource` resolves `http://` and `https://` URIs into payload bytes.
//! Non-success status codes fail closed.
//! Security posture: treats remote content as untrusted; see
//! `Docs/security/threat_model.md`.

// ============================================================================
// SECTION: Imports
// ============================================================================

use std::io::Read;
use std::time::Duration;

use decision_gate_core::ContentRef;
use reqwest::blocking::Client;
use reqwest::header::CONTENT_TYPE;
use reqwest::redirect::Policy;
use url::Url;

use crate::source::Source;
use crate::source::SourceError;
use crate::source::SourcePayload;
use crate::source::enforce_max_bytes;

// ============================================================================
// SECTION: HTTP Source
// ============================================================================

/// HTTP-backed payload source.
#[derive(Debug, Clone)]
pub struct HttpSource {
    /// HTTP client used for fetch requests.
    client: Client,
}

impl HttpSource {
    /// Builds an HTTP source with a default client.
    ///
    /// # Errors
    ///
    /// Returns [`SourceError`] when the HTTP client cannot be constructed.
    pub fn new() -> Result<Self, SourceError> {
        let client = Client::builder()
            .redirect(Policy::none())
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|err| SourceError::Http(err.to_string()))?;
        Ok(Self {
            client,
        })
    }

    /// Creates an HTTP source with a preconfigured client.
    #[must_use]
    pub const fn with_client(client: Client) -> Self {
        Self {
            client,
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

        let response = self
            .client
            .get(url.as_str())
            .send()
            .map_err(|err| SourceError::Http(err.to_string()))?;
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
        if let Some(length) = response.content_length() {
            if length > crate::source::MAX_SOURCE_BYTES as u64 {
                return Err(SourceError::TooLarge {
                    max_bytes: crate::source::MAX_SOURCE_BYTES,
                    actual_bytes: length as usize,
                });
            }
        }
        let content_type = response
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .map(str::to_string);
        let mut limited = response.take((crate::source::MAX_SOURCE_BYTES + 1) as u64);
        let mut bytes = Vec::new();
        limited.read_to_end(&mut bytes).map_err(|err| SourceError::Http(err.to_string()))?;
        enforce_max_bytes(bytes.len())?;
        Ok(SourcePayload {
            bytes,
            content_type,
        })
    }
}
