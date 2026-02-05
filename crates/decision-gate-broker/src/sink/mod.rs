// crates/decision-gate-broker/src/sink/mod.rs
// ============================================================================
// Module: Decision Gate Broker Sinks
// Description: Sink traits and reference implementations for dispatch delivery.
// Purpose: Deliver resolved payloads to concrete targets.
// Dependencies: decision-gate-core, thiserror, std
// ============================================================================

//! ## Overview
//! Sinks deliver resolved payloads to [`decision_gate_core::DispatchTarget`] values and return
//! [`decision_gate_core::DispatchReceipt`] values for auditing. Implementations must fail closed
//! on delivery errors.
//! Invariants:
//! - Receipts are returned only after successful delivery.
//! - Delivery failures must not emit partial side effects.
//!
//! Security posture: dispatch targets are external systems; treat payloads as
//! sensitive and see `Docs/security/threat_model.md`.

// ============================================================================
// SECTION: Imports
// ============================================================================

use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;

use decision_gate_core::DispatchReceipt;
use decision_gate_core::DispatchTarget;
use decision_gate_core::Timestamp;
use thiserror::Error;

use crate::payload::Payload;

// ============================================================================
// SECTION: Sink Errors
// ============================================================================

/// Errors emitted by broker sinks.
///
/// # Invariants
/// - Variants are stable for programmatic handling.
#[derive(Debug, Error)]
pub enum SinkError {
    /// Sink delivery failed.
    #[error("sink delivery failed: {0}")]
    DeliveryFailed(String),
    /// Log sink failed to write.
    #[error("log write failed: {0}")]
    LogWriteFailed(String),
}

// ============================================================================
// SECTION: Sink Trait
// ============================================================================

/// Delivers resolved payloads to a dispatch target.
pub trait Sink: Send + Sync {
    /// Delivers the payload to the target.
    ///
    /// # Errors
    ///
    /// Returns [`SinkError`] when delivery fails.
    fn deliver(
        &self,
        target: &DispatchTarget,
        payload: &Payload,
    ) -> Result<DispatchReceipt, SinkError>;
}

// ============================================================================
// SECTION: Dispatch Message
// ============================================================================

/// Dispatch message emitted by channel-based sinks.
///
/// # Invariants
/// - `receipt` must correspond to the provided `target` and `payload`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DispatchMessage {
    /// Dispatch target.
    pub target: DispatchTarget,
    /// Resolved payload.
    pub payload: Payload,
    /// Dispatch receipt.
    pub receipt: DispatchReceipt,
}

// ============================================================================
// SECTION: Receipt Helpers
// ============================================================================

/// Builds deterministic dispatch receipts.
#[derive(Debug)]
pub(crate) struct ReceiptFactory {
    /// Dispatcher identifier embedded in receipts.
    dispatcher: String,
    /// Monotonic counter used for deterministic IDs.
    counter: AtomicU64,
}

impl ReceiptFactory {
    /// Creates a receipt factory with the provided dispatcher name.
    pub(crate) fn new(dispatcher: impl Into<String>) -> Self {
        Self {
            dispatcher: dispatcher.into(),
            counter: AtomicU64::new(0),
        }
    }

    /// Returns the next receipt for the provided payload.
    pub(crate) fn next(&self, target: &DispatchTarget, payload: &Payload) -> DispatchReceipt {
        let seq = self.counter.fetch_add(1, Ordering::Relaxed) + 1;
        DispatchReceipt {
            dispatch_id: format!("{}-{}", self.dispatcher, seq),
            target: target.clone(),
            receipt_hash: payload.envelope.content_hash.clone(),
            dispatched_at: Timestamp::Logical(seq),
            dispatcher: self.dispatcher.clone(),
        }
    }
}

// ============================================================================
// SECTION: Implementations
// ============================================================================

pub mod callback;
pub mod channel;
pub mod log;

pub use callback::CallbackSink;
pub use channel::ChannelSink;
pub use log::LogSink;
