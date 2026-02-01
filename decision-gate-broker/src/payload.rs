// decision-gate-broker/src/payload.rs
// ============================================================================
// Module: Decision Gate Broker Payload
// Description: Resolved payloads with Decision Gate envelope metadata.
// Purpose: Carry resolved disclosure content to broker sinks.
// Dependencies: decision-gate-core, serde_json
// ============================================================================

//! ## Overview
//! Payloads represent resolved disclosure content paired with the originating
//! [`decision_gate_core::PacketEnvelope`]. Sinks receive payloads after sources resolve any
//! external references.
//! Security posture: payload bodies originate from untrusted inputs and should
//! only be constructed after validation; see `Docs/security/threat_model.md`.

// ============================================================================
// SECTION: Imports
// ============================================================================

use decision_gate_core::PacketEnvelope;
use serde_json::Value;

// ============================================================================
// SECTION: Payload Types
// ============================================================================

/// Resolved payload content returned by broker sources.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PayloadBody {
    /// JSON payload value.
    Json(Value),
    /// Raw payload bytes.
    Bytes(Vec<u8>),
}

/// Resolved payload with the originating packet envelope.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Payload {
    /// Decision Gate envelope metadata.
    pub envelope: PacketEnvelope,
    /// Payload content.
    pub body: PayloadBody,
}

impl Payload {
    /// Returns the payload length in bytes when available.
    #[must_use]
    pub fn len(&self) -> usize {
        match &self.body {
            PayloadBody::Json(value) => serde_json::to_vec(value).map_or(0, |bytes| bytes.len()),
            PayloadBody::Bytes(bytes) => bytes.len(),
        }
    }

    /// Returns true when the payload has zero length.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}
