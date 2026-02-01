// decision-gate-broker/src/sink/callback.rs
// ============================================================================
// Module: Decision Gate Callback Sink
// Description: Callback-based sink for synchronous delivery.
// Purpose: Invoke a user-provided function with resolved payloads.
// Dependencies: decision-gate-core, std
// ============================================================================

//! ## Overview
//! [`CallbackSink`] delivers payloads by invoking a user-supplied function and
//! returning the provided dispatch receipt.
//! Security posture: callback handlers are external sinks; treat payloads as
//! sensitive per `Docs/security/threat_model.md`.

// ============================================================================
// SECTION: Imports
// ============================================================================

use std::sync::Arc;

use decision_gate_core::DispatchReceipt;
use decision_gate_core::DispatchTarget;

use crate::payload::Payload;
use crate::sink::Sink;
use crate::sink::SinkError;

// ============================================================================
// SECTION: Callback Sink
// ============================================================================

/// Callback-based payload sink.
#[derive(Clone)]
pub struct CallbackSink {
    /// Handler invoked with the target and payload.
    handler: Arc<CallbackHandler>,
}

/// Callback handler signature used by the sink.
type CallbackHandler =
    dyn Fn(&DispatchTarget, &Payload) -> Result<DispatchReceipt, SinkError> + Send + Sync;

impl CallbackSink {
    /// Creates a callback sink from a handler function.
    pub fn new<F>(handler: F) -> Self
    where
        F: Fn(&DispatchTarget, &Payload) -> Result<DispatchReceipt, SinkError>
            + Send
            + Sync
            + 'static,
    {
        Self {
            handler: Arc::new(handler),
        }
    }
}

impl Sink for CallbackSink {
    fn deliver(
        &self,
        target: &DispatchTarget,
        payload: &Payload,
    ) -> Result<DispatchReceipt, SinkError> {
        (self.handler)(target, payload)
    }
}
