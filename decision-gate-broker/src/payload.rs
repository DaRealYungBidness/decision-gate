// decision-gate-broker/src/payload.rs
// ============================================================================
// Module: Decision Gate Broker Payload
// Description: Resolved payloads with Decision Gate envelope metadata.
// Purpose: Carry resolved disclosure content to broker sinks.
// Dependencies: decision-gate-core, serde_json
// ============================================================================

//! ## Overview
//! Payloads represent resolved disclosure content paired with the originating
//! Decision Gate envelope. Sinks receive payloads after sources resolve any
//! external references.

// ============================================================================
// SECTION: Imports
// ============================================================================

use decision_gate_core::PacketEnvelope;
use serde_json::Value;

// ============================================================================
// SECTION: Payload Types
// ============================================================================

/// Resolved payload content returned by broker sources.
#[derive(Debug, Clone, PartialEq)]
pub enum PayloadBody {
    /// JSON payload value.
    Json(Value),
    /// Raw payload bytes.
    Bytes(Vec<u8>),
}

/// Resolved payload with the originating packet envelope.
#[derive(Debug, Clone, PartialEq)]
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
