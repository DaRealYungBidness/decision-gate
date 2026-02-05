// crates/decision-gate-broker/tests/common/mod.rs
// ============================================================================
// Module: Common Test Utilities
// Description: Shared helpers for decision-gate-broker tests.
// Purpose: Provide reusable builders and helpers for broker integration tests.
// Dependencies: decision-gate-core, serde_json
// ============================================================================

//! ## Overview
//! Provides shared helper functions and test utilities for broker sinks and
//! sources.

#![allow(
    clippy::panic,
    clippy::print_stdout,
    clippy::print_stderr,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::use_debug,
    clippy::dbg_macro,
    clippy::panic_in_result_fn,
    clippy::unwrap_in_result,
    reason = "Test-only output and panic-based assertions are permitted."
)]

use std::io::Write;
use std::sync::Arc;
use std::sync::Mutex;

use decision_gate_core::DispatchTarget;
use decision_gate_core::PacketEnvelope;
use decision_gate_core::PacketId;
use decision_gate_core::RunId;
use decision_gate_core::ScenarioId;
use decision_gate_core::SchemaId;
use decision_gate_core::StageId;
use decision_gate_core::Timestamp;
use decision_gate_core::VisibilityPolicy;
use decision_gate_core::hashing::DEFAULT_HASH_ALGORITHM;
use decision_gate_core::hashing::HashDigest;
use decision_gate_core::hashing::hash_bytes;
use decision_gate_core::hashing::hash_canonical_json;
use serde_json::Value;

// ============================================================================
// SECTION: Envelope Helpers
// ============================================================================

/// Creates a sample envelope with the given content type and hash.
pub fn sample_envelope(content_type: &str, content_hash: HashDigest) -> PacketEnvelope {
    PacketEnvelope {
        scenario_id: ScenarioId::new("test-scenario"),
        run_id: RunId::new("test-run"),
        stage_id: StageId::new("test-stage"),
        packet_id: PacketId::new("test-packet"),
        schema_id: SchemaId::new("test-schema"),
        content_type: content_type.to_string(),
        content_hash,
        visibility: VisibilityPolicy::new(vec![], vec![]),
        expiry: None,
        correlation_id: None,
        issued_at: Timestamp::Logical(1),
    }
}

/// Creates a sample envelope for bytes payload.
pub fn sample_bytes_envelope(bytes: &[u8]) -> PacketEnvelope {
    let content_hash = hash_bytes(DEFAULT_HASH_ALGORITHM, bytes);
    sample_envelope("application/octet-stream", content_hash)
}

/// Creates a sample envelope for JSON payload.
pub fn sample_json_envelope(value: &Value) -> PacketEnvelope {
    let content_hash = hash_canonical_json(DEFAULT_HASH_ALGORITHM, value)
        .expect("json hash should succeed for valid json");
    sample_envelope("application/json", content_hash)
}

// ============================================================================
// SECTION: Target Helpers
// ============================================================================

/// Creates a sample dispatch target.
pub fn sample_target() -> DispatchTarget {
    DispatchTarget::Agent {
        agent_id: "test-agent".to_string(),
    }
}

/// Creates a sample dispatch target with a custom agent ID.
pub fn sample_target_with_id(agent_id: &str) -> DispatchTarget {
    DispatchTarget::Agent {
        agent_id: agent_id.to_string(),
    }
}

// ============================================================================
// SECTION: Content Helpers
// ============================================================================

/// Creates a hash for the given bytes using the default algorithm.
pub fn hash_for_bytes(bytes: &[u8]) -> HashDigest {
    hash_bytes(DEFAULT_HASH_ALGORITHM, bytes)
}

/// Creates a hash for the given JSON value using the default algorithm.
pub fn hash_for_json(value: &Value) -> HashDigest {
    hash_canonical_json(DEFAULT_HASH_ALGORITHM, value)
        .expect("json hash should succeed for valid json")
}

// ============================================================================
// SECTION: Shared Buffer for Write Testing
// ============================================================================

/// A thread-safe buffer for testing Write implementations.
#[derive(Clone)]
pub struct SharedBuffer {
    inner: Arc<Mutex<Vec<u8>>>,
}

impl SharedBuffer {
    /// Creates a new empty shared buffer.
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Returns the contents as a string.
    pub fn to_string_lossy(&self) -> String {
        let guard = self.inner.lock().expect("buffer lock");
        String::from_utf8_lossy(&guard).to_string()
    }

    /// Returns the raw bytes.
    pub fn to_bytes(&self) -> Vec<u8> {
        let guard = self.inner.lock().expect("buffer lock");
        guard.clone()
    }

    /// Returns true if the buffer is empty.
    pub fn is_empty(&self) -> bool {
        let guard = self.inner.lock().expect("buffer lock");
        guard.is_empty()
    }
}

impl Default for SharedBuffer {
    fn default() -> Self {
        Self::new()
    }
}

impl Write for SharedBuffer {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.inner.lock().expect("buffer lock").extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

// ============================================================================
// SECTION: Failing Writer for Error Testing
// ============================================================================

/// A writer that always fails, for testing error paths.
pub struct FailingWriter;

impl Write for FailingWriter {
    fn write(&mut self, _buf: &[u8]) -> std::io::Result<usize> {
        Err(std::io::Error::other("simulated write failure"))
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
