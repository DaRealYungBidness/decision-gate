// decision-gate-broker/src/sink/channel.rs
// ============================================================================
// Module: Decision Gate Channel Sink
// Description: Channel-based sink for asynchronous delivery.
// Purpose: Send resolved payloads through a Tokio mpsc channel.
// Dependencies: decision-gate-core, tokio
// ============================================================================

//! ## Overview
//! [`ChannelSink`] delivers payloads by sending dispatch messages into a
//! `tokio::sync::mpsc` channel.
//! Invariants:
//! - Successful deliveries enqueue exactly one [`crate::sink::DispatchMessage`].
//!
//! Security posture: channel receivers are external sinks; treat payloads as
//! sensitive per `Docs/security/threat_model.md`.

// ============================================================================
// SECTION: Imports
// ============================================================================

use decision_gate_core::DispatchReceipt;
use decision_gate_core::DispatchTarget;
use tokio::sync::mpsc::Sender;

use crate::payload::Payload;
use crate::sink::DispatchMessage;
use crate::sink::ReceiptFactory;
use crate::sink::Sink;
use crate::sink::SinkError;

// ============================================================================
// SECTION: Channel Sink
// ============================================================================

/// Channel-based payload sink.
///
/// # Invariants
/// - Each successful delivery emits a message with a matching receipt.
#[derive(Debug)]
pub struct ChannelSink {
    /// Sender used to dispatch messages.
    sender: Sender<DispatchMessage>,
    /// Receipt factory for deterministic dispatch IDs.
    receipts: ReceiptFactory,
}

impl ChannelSink {
    /// Creates a channel sink with the default dispatcher name.
    #[must_use]
    pub fn new(sender: Sender<DispatchMessage>) -> Self {
        Self {
            sender,
            receipts: ReceiptFactory::new("channel"),
        }
    }

    /// Creates a channel sink with a custom dispatcher name.
    #[must_use]
    pub fn with_dispatcher(sender: Sender<DispatchMessage>, dispatcher: impl Into<String>) -> Self {
        Self {
            sender,
            receipts: ReceiptFactory::new(dispatcher),
        }
    }
}

impl Sink for ChannelSink {
    fn deliver(
        &self,
        target: &DispatchTarget,
        payload: &Payload,
    ) -> Result<DispatchReceipt, SinkError> {
        let receipt = self.receipts.next(target, payload);
        let message = DispatchMessage {
            target: target.clone(),
            payload: payload.clone(),
            receipt: receipt.clone(),
        };
        self.sender.try_send(message).map_err(|err| SinkError::DeliveryFailed(err.to_string()))?;
        Ok(receipt)
    }
}
