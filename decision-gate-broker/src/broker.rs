// decision-gate-broker/src/broker.rs
// ============================================================================
// Module: Decision Gate Composite Broker
// Description: Composite dispatcher wiring sources and sinks.
// Purpose: Resolve payloads and deliver disclosures through configured sinks.
// Dependencies: decision-gate-core, url, serde_json
// ============================================================================

//! ## Overview
//! CompositeBroker implements Decision Gate's dispatcher interface by resolving
//! external payloads with sources and delivering them with sinks.

// ============================================================================
// SECTION: Imports
// ============================================================================

use std::collections::BTreeMap;
use std::sync::Arc;

use decision_gate_core::DispatchError;
use decision_gate_core::DispatchReceipt;
use decision_gate_core::DispatchTarget;
use decision_gate_core::Dispatcher;
use decision_gate_core::PacketEnvelope;
use decision_gate_core::PacketPayload;
use decision_gate_core::hashing::HashAlgorithm;
use decision_gate_core::hashing::HashDigest;
use decision_gate_core::hashing::hash_bytes;
use decision_gate_core::hashing::hash_canonical_json;
use serde_json::Value;
use thiserror::Error;
use url::Url;

use crate::payload::Payload;
use crate::payload::PayloadBody;
use crate::sink::Sink;
use crate::sink::SinkError;
use crate::source::Source;
use crate::source::SourceError;

// ============================================================================
// SECTION: Broker Errors
// ============================================================================

/// Errors returned by the composite broker.
#[derive(Debug, Error)]
pub enum BrokerError {
    /// Broker is missing a required sink.
    #[error("broker sink is not configured")]
    MissingSink,
    /// Broker could not determine a source for the URI scheme.
    #[error("missing source for scheme: {0}")]
    MissingSource(String),
    /// URI failed to parse.
    #[error("invalid uri: {0}")]
    InvalidUri(String),
    /// Payload hash mismatch.
    #[error("payload hash mismatch (expected {expected}, got {actual})")]
    HashMismatch {
        /// Expected hash value.
        expected: String,
        /// Actual hash value.
        actual: String,
    },
    /// Payload hash computation failed.
    #[error("payload hash failure: {0}")]
    Hashing(String),
    /// JSON parsing failed.
    #[error("json parse failure: {0}")]
    JsonParse(String),
    /// Source failed to resolve payload.
    #[error("source failure: {0}")]
    Source(#[from] SourceError),
    /// Sink failed to deliver payload.
    #[error("sink failure: {0}")]
    Sink(#[from] SinkError),
}

impl From<BrokerError> for DispatchError {
    fn from(err: BrokerError) -> Self {
        DispatchError::DispatchFailed(err.to_string())
    }
}

// ============================================================================
// SECTION: Composite Broker
// ============================================================================

/// Builder for a composite broker.
#[derive(Default)]
pub struct CompositeBrokerBuilder {
    sources: BTreeMap<String, Arc<dyn Source>>,
    sink: Option<Arc<dyn Sink>>,
}

impl CompositeBrokerBuilder {
    /// Registers a source for the provided URI scheme.
    #[must_use]
    pub fn source(mut self, scheme: impl Into<String>, source: impl Source + 'static) -> Self {
        self.sources.insert(scheme.into(), Arc::new(source));
        self
    }

    /// Registers the sink used for dispatch.
    #[must_use]
    pub fn sink(mut self, sink: impl Sink + 'static) -> Self {
        self.sink = Some(Arc::new(sink));
        self
    }

    /// Builds the composite broker.
    ///
    /// # Errors
    ///
    /// Returns [`BrokerError::MissingSink`] when no sink is configured.
    pub fn build(self) -> Result<CompositeBroker, BrokerError> {
        Ok(CompositeBroker {
            sources: self.sources,
            sink: self.sink.ok_or(BrokerError::MissingSink)?,
        })
    }
}

/// Composite dispatcher wiring sources and a sink.
pub struct CompositeBroker {
    sources: BTreeMap<String, Arc<dyn Source>>,
    sink: Arc<dyn Sink>,
}

impl CompositeBroker {
    /// Returns a builder for the composite broker.
    #[must_use]
    pub fn builder() -> CompositeBrokerBuilder {
        CompositeBrokerBuilder::default()
    }

    /// Resolves the configured source for a content URI.
    fn resolve_source(&self, uri: &str) -> Result<Arc<dyn Source>, BrokerError> {
        let scheme = Url::parse(uri)
            .map_err(|err| BrokerError::InvalidUri(err.to_string()))?
            .scheme()
            .to_string();
        if let Some(source) = self.sources.get(&scheme) {
            return Ok(Arc::clone(source));
        }
        if let Some((base, _)) = scheme.split_once('+') {
            if let Some(source) = self.sources.get(base) {
                return Ok(Arc::clone(source));
            }
        }
        Err(BrokerError::MissingSource(scheme))
    }

    /// Resolves a packet payload into a broker payload with validation.
    fn resolve_payload(
        &self,
        envelope: &PacketEnvelope,
        payload: &PacketPayload,
    ) -> Result<Payload, BrokerError> {
        match payload {
            PacketPayload::Json {
                value,
            } => {
                let body = PayloadBody::Json(value.clone());
                self.validate_payload_hash(
                    &body,
                    envelope.content_hash.algorithm,
                    &envelope.content_hash,
                )?;
                Ok(Payload {
                    envelope: envelope.clone(),
                    body,
                })
            }
            PacketPayload::Bytes {
                bytes,
            } => {
                let body = PayloadBody::Bytes(bytes.clone());
                self.validate_payload_hash(
                    &body,
                    envelope.content_hash.algorithm,
                    &envelope.content_hash,
                )?;
                Ok(Payload {
                    envelope: envelope.clone(),
                    body,
                })
            }
            PacketPayload::External {
                content_ref,
            } => {
                if envelope.content_hash != content_ref.content_hash {
                    return Err(BrokerError::HashMismatch {
                        expected: content_ref.content_hash.value.clone(),
                        actual: envelope.content_hash.value.clone(),
                    });
                }
                let source = self.resolve_source(&content_ref.uri)?;
                let resolved = source.fetch(content_ref)?;
                let body = self.build_body(&resolved.bytes, envelope.content_type.as_str())?;
                self.validate_payload_hash(
                    &body,
                    content_ref.content_hash.algorithm,
                    &content_ref.content_hash,
                )?;
                Ok(Payload {
                    envelope: envelope.clone(),
                    body,
                })
            }
        }
    }

    /// Builds a payload body from raw bytes and content type.
    fn build_body(&self, bytes: &[u8], content_type: &str) -> Result<PayloadBody, BrokerError> {
        if is_json_content_type(content_type) {
            let value = serde_json::from_slice::<Value>(bytes)
                .map_err(|err| BrokerError::JsonParse(err.to_string()))?;
            Ok(PayloadBody::Json(value))
        } else {
            Ok(PayloadBody::Bytes(bytes.to_vec()))
        }
    }

    /// Validates a payload hash against an expected digest.
    fn validate_payload_hash(
        &self,
        body: &PayloadBody,
        algorithm: HashAlgorithm,
        expected: &HashDigest,
    ) -> Result<(), BrokerError> {
        let actual = compute_payload_hash(body, algorithm)?;
        if actual.value != expected.value {
            return Err(BrokerError::HashMismatch {
                expected: expected.value.clone(),
                actual: actual.value,
            });
        }
        Ok(())
    }
}

impl Dispatcher for CompositeBroker {
    fn dispatch(
        &self,
        target: &DispatchTarget,
        envelope: &PacketEnvelope,
        payload: &PacketPayload,
    ) -> Result<DispatchReceipt, DispatchError> {
        let resolved = self.resolve_payload(envelope, payload)?;
        let receipt = self.sink.deliver(target, &resolved).map_err(BrokerError::from)?;
        Ok(receipt)
    }
}

// ============================================================================
// SECTION: Helpers
// ============================================================================

/// Returns true when the content type indicates JSON.
fn is_json_content_type(content_type: &str) -> bool {
    let content_type = content_type.split(';').next().unwrap_or(content_type).trim();
    content_type == "application/json" || content_type.ends_with("+json")
}

/// Computes a payload hash for the provided body and algorithm.
fn compute_payload_hash(
    body: &PayloadBody,
    algorithm: HashAlgorithm,
) -> Result<HashDigest, BrokerError> {
    match body {
        PayloadBody::Json(value) => hash_canonical_json(algorithm, value)
            .map_err(|err| BrokerError::Hashing(err.to_string())),
        PayloadBody::Bytes(bytes) => Ok(hash_bytes(algorithm, bytes)),
    }
}
