// decision-gate-broker/src/sink/log.rs
// ============================================================================
// Module: Decision Gate Log Sink
// Description: Log-only sink for audit-grade delivery records.
// Purpose: Persist delivery receipts without dispatching payloads.
// Dependencies: serde_json, std
// ============================================================================

//! ## Overview
//! `LogSink` writes a log record for each dispatch and returns the receipt. It
//! does not deliver payloads to external systems.

// ============================================================================
// SECTION: Imports
// ============================================================================

use std::io::Write;
use std::sync::Mutex;

use decision_gate_core::DispatchReceipt;
use decision_gate_core::DispatchTarget;
use serde_json::json;

use crate::payload::Payload;
use crate::sink::ReceiptFactory;
use crate::sink::Sink;
use crate::sink::SinkError;

// ============================================================================
// SECTION: Log Sink
// ============================================================================

/// Log-only payload sink.
pub struct LogSink<W: Write + Send> {
    /// Output writer for log records.
    writer: Mutex<W>,
    /// Receipt factory for deterministic dispatch IDs.
    receipts: ReceiptFactory,
}

impl<W: Write + Send> LogSink<W> {
    /// Creates a log sink with the default dispatcher name.
    pub fn new(writer: W) -> Self {
        Self {
            writer: Mutex::new(writer),
            receipts: ReceiptFactory::new("log"),
        }
    }

    /// Creates a log sink with a custom dispatcher name.
    pub fn with_dispatcher(writer: W, dispatcher: impl Into<String>) -> Self {
        Self {
            writer: Mutex::new(writer),
            receipts: ReceiptFactory::new(dispatcher),
        }
    }
}

impl<W: Write + Send> Sink for LogSink<W> {
    fn deliver(
        &self,
        target: &DispatchTarget,
        payload: &Payload,
    ) -> Result<DispatchReceipt, SinkError> {
        let receipt = self.receipts.next(target, payload);
        let record = json!({
            "dispatch_id": receipt.dispatch_id,
            "dispatcher": receipt.dispatcher,
            "target": receipt.target,
            "content_type": payload.envelope.content_type,
            "content_hash": payload.envelope.content_hash,
            "payload_len": payload.len(),
            "dispatched_at": receipt.dispatched_at,
        });
        let mut guard = self
            .writer
            .lock()
            .map_err(|_| SinkError::LogWriteFailed("log writer mutex poisoned".to_string()))?;
        serde_json::to_writer(&mut *guard, &record)
            .map_err(|err| SinkError::LogWriteFailed(err.to_string()))?;
        guard.write_all(b"\n").map_err(|err| SinkError::LogWriteFailed(err.to_string()))?;
        drop(guard);
        Ok(receipt)
    }
}
