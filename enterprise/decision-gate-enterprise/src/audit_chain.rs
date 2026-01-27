// enterprise/decision-gate-enterprise/src/audit_chain.rs
// ============================================================================
// Module: Hash-Chained Audit Sink
// Description: Append-only audit logging with hash chaining.
// Purpose: Provide tamper-evident audit logs for managed deployments.
// ============================================================================

use std::fs::OpenOptions;
use std::io::BufRead;
use std::io::BufReader;
use std::io::Write;
use std::path::Path;
use std::sync::Mutex;

use decision_gate_core::hashing::HashAlgorithm;
use decision_gate_core::hashing::hash_bytes;
use decision_gate_mcp::McpAuditEvent;
use decision_gate_mcp::McpAuditSink;
use decision_gate_mcp::PrecheckAuditEvent;
use decision_gate_mcp::TenantAuthzEvent;
use decision_gate_mcp::UsageAuditEvent;
use decision_gate_mcp::audit::RegistryAuditEvent;
use decision_gate_mcp::audit::SecurityAuditEvent;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;
use thiserror::Error;

/// Hash-chained audit envelope.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct AuditEnvelope {
    /// Embedded audit payload.
    payload: Value,
    /// Previous hash value.
    prev_hash: String,
    /// Current hash value.
    hash: String,
}

/// Errors for hash-chained audit sink.
#[derive(Debug, Error)]
pub enum AuditChainError {
    /// I/O error.
    #[error("audit chain io error: {0}")]
    Io(String),
    /// Parse error.
    #[error("audit chain parse error: {0}")]
    Parse(String),
}

/// Hash-chained audit sink (append-only).
pub struct HashChainedAuditSink {
    /// Open file handle for appending audit events.
    file: Mutex<std::fs::File>,
    /// Last recorded hash for chaining.
    last_hash: Mutex<String>,
}

impl HashChainedAuditSink {
    /// Opens or creates a hash-chained audit log file.
    ///
    /// # Errors
    ///
    /// Returns [`AuditChainError`] when initialization fails.
    pub fn new(path: &Path) -> Result<Self, AuditChainError> {
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .read(true)
            .open(path)
            .map_err(|err| AuditChainError::Io(err.to_string()))?;
        let last_hash = Self::load_last_hash(path)?;
        Ok(Self { file: Mutex::new(file), last_hash: Mutex::new(last_hash) })
    }

    /// Loads the last hash from the audit log file.
    fn load_last_hash(path: &Path) -> Result<String, AuditChainError> {
        let file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(path)
            .map_err(|err| AuditChainError::Io(err.to_string()))?;
        let reader = BufReader::new(file);
        let mut last_hash = String::from("0");
        for line in reader.lines() {
            let line = line.map_err(|err| AuditChainError::Io(err.to_string()))?;
            if line.trim().is_empty() {
                continue;
            }
            let envelope: AuditEnvelope = serde_json::from_str(&line)
                .map_err(|err| AuditChainError::Parse(err.to_string()))?;
            last_hash = envelope.hash;
        }
        Ok(last_hash)
    }

    /// Appends a JSON payload with hash chaining.
    fn append_payload(&self, payload: &Value) {
        let Ok(mut hash_guard) = self.last_hash.lock() else {
            return;
        };
        let prev_hash = hash_guard.clone();
        let payload_bytes = serde_json::to_vec(payload).unwrap_or_default();
        let mut combined = prev_hash.as_bytes().to_vec();
        combined.extend_from_slice(&payload_bytes);
        let digest = hash_bytes(HashAlgorithm::Sha256, &combined);
        let hash = digest.value;
        let envelope = AuditEnvelope { payload: payload.clone(), prev_hash, hash: hash.clone() };
        if let Ok(line) = serde_json::to_string(&envelope)
            && let Ok(mut file) = self.file.lock()
        {
            let _ = writeln!(file, "{line}");
            let _ = file.flush();
        }
        *hash_guard = hash;
    }

    /// Records a typed audit event payload.
    fn record_value<T: Serialize>(&self, event: &T) {
        if let Ok(payload) = serde_json::to_value(event) {
            self.append_payload(&payload);
        }
    }
}

impl McpAuditSink for HashChainedAuditSink {
    fn record(&self, event: &McpAuditEvent) {
        self.record_value(event);
    }

    fn record_precheck(&self, event: &PrecheckAuditEvent) {
        self.record_value(event);
    }

    fn record_registry(&self, event: &RegistryAuditEvent) {
        self.record_value(event);
    }

    fn record_tenant_authz(&self, event: &TenantAuthzEvent) {
        self.record_value(event);
    }

    fn record_usage(&self, event: &UsageAuditEvent) {
        self.record_value(event);
    }

    fn record_security(&self, event: &SecurityAuditEvent) {
        self.record_value(event);
    }
}
