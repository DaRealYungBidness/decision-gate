// decision-gate-core/src/core/disclosure.rs
// ============================================================================
// Module: Decision Gate Disclosure Model
// Description: Packet envelopes, payloads, and dispatch targets.
// Purpose: Define controlled disclosure artifacts emitted by Decision Gate.
// Dependencies: crate::core::{hashing, identifiers, time}, serde, serde_json
// ============================================================================

//! ## Overview
//! Packets are the atomic disclosure unit in Decision Gate. Each packet has a stable
//! envelope and an associated payload. Dispatch targets are backend-agnostic
//! recipients for controlled disclosure.
//!
//! Security posture: disclosure payloads may contain sensitive data; see
//! `Docs/security/threat_model.md`.

// ============================================================================
// SECTION: Imports
// ============================================================================

use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;

use crate::core::hashing::HashDigest;
use crate::core::identifiers::CorrelationId;
use crate::core::identifiers::DecisionId;
use crate::core::identifiers::PacketId;
use crate::core::identifiers::RunId;
use crate::core::identifiers::ScenarioId;
use crate::core::identifiers::SchemaId;
use crate::core::identifiers::StageId;
use crate::core::time::Timestamp;

// ============================================================================
// SECTION: Visibility Policy
// ============================================================================

/// Disclosure visibility policy for packets.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VisibilityPolicy {
    /// Classification or policy tags controlling disclosure.
    pub labels: Vec<String>,
    /// Optional policy identifiers applied during dispatch.
    pub policy_tags: Vec<String>,
}

impl VisibilityPolicy {
    /// Creates a visibility policy with the provided labels.
    #[must_use]
    pub const fn new(labels: Vec<String>, policy_tags: Vec<String>) -> Self {
        Self {
            labels,
            policy_tags,
        }
    }
}

// ============================================================================
// SECTION: Dispatch Targets
// ============================================================================

/// Dispatch target for packet delivery.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DispatchTarget {
    /// Agent-specific target.
    Agent {
        /// Agent identifier.
        agent_id: String,
    },
    /// Session-specific target.
    Session {
        /// Session identifier.
        session_id: String,
    },
    /// External system target.
    External {
        /// External system name.
        system: String,
        /// Target identifier within the system.
        target: String,
    },
    /// Broadcast channel target.
    Channel {
        /// Channel identifier.
        channel: String,
    },
}

// ============================================================================
// SECTION: Payloads
// ============================================================================

/// Payload reference to external storage.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContentRef {
    /// URI or handle to the content blob.
    pub uri: String,
    /// Content hash for the referenced payload.
    pub content_hash: HashDigest,
    /// Optional encryption metadata string.
    pub encryption: Option<String>,
}

/// Packet payload content.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PacketPayload {
    /// Inline JSON payload.
    Json {
        /// JSON payload value.
        value: Value,
    },
    /// Inline binary payload.
    Bytes {
        /// Raw payload bytes.
        bytes: Vec<u8>,
    },
    /// External content reference.
    External {
        /// Reference to externally stored content.
        content_ref: ContentRef,
    },
}

// ============================================================================
// SECTION: Packet Envelopes
// ============================================================================

/// Packet envelope with stable metadata and content hash.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PacketEnvelope {
    /// Scenario identifier.
    pub scenario_id: ScenarioId,
    /// Run identifier.
    pub run_id: RunId,
    /// Stage identifier.
    pub stage_id: StageId,
    /// Packet identifier.
    pub packet_id: PacketId,
    /// Packet schema identifier.
    pub schema_id: SchemaId,
    /// Content type for payload decoding.
    pub content_type: String,
    /// Canonical hash of the payload bytes.
    pub content_hash: HashDigest,
    /// Visibility policy for this packet.
    pub visibility: VisibilityPolicy,
    /// Optional expiry timestamp.
    pub expiry: Option<Timestamp>,
    /// Optional correlation identifier.
    pub correlation_id: Option<CorrelationId>,
    /// Time the packet was issued.
    pub issued_at: Timestamp,
}

/// Packet delivery receipt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DispatchReceipt {
    /// Dispatch identifier for idempotency.
    pub dispatch_id: String,
    /// Dispatch target.
    pub target: DispatchTarget,
    /// Receipt hash for verification.
    pub receipt_hash: HashDigest,
    /// Dispatch timestamp.
    pub dispatched_at: Timestamp,
    /// Dispatcher identifier.
    pub dispatcher: String,
}

/// Packet record logged in run state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PacketRecord {
    /// Packet envelope.
    pub envelope: PacketEnvelope,
    /// Payload reference or inline payload.
    pub payload: PacketPayload,
    /// Dispatch receipts for the packet.
    pub receipts: Vec<DispatchReceipt>,
    /// Decision identifier that issued the packet.
    pub decision_id: DecisionId,
}
