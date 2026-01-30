// decision-gate-core/tests/identifiers.rs
// ============================================================================
// Module: Identifier Tests
// Description: Tests for Decision Gate identifier wrappers.
// Purpose: Ensure IDs round-trip through serde and display correctly.
// Dependencies: decision-gate-core, serde_json
// ============================================================================
//! ## Overview
//! Validates that identifier wrappers preserve their underlying string values.
//!
//! Security posture: Identifiers are opaque but must serialize deterministically.
//! Threat model: TM-ID-001 - Identifier confusion or serialization drift.

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

use decision_gate_core::ConditionId;
use decision_gate_core::CorrelationId;
use decision_gate_core::DecisionId;
use decision_gate_core::GateId;
use decision_gate_core::NamespaceId;
use decision_gate_core::PacketId;
use decision_gate_core::PolicyId;
use decision_gate_core::ProviderId;
use decision_gate_core::RunId;
use decision_gate_core::ScenarioId;
use decision_gate_core::SchemaId;
use decision_gate_core::SpecVersion;
use decision_gate_core::StageId;
use decision_gate_core::TenantId;
use decision_gate_core::TriggerId;

macro_rules! assert_id_roundtrip {
    ($ty:ty, $value:expr) => {{
        let id = <$ty>::new($value);
        assert_eq!(id.as_str(), $value);
        assert_eq!(id.to_string(), $value);

        let json = serde_json::to_string(&id).expect("serialize");
        assert_eq!(json, format!("\"{}\"", $value));

        let decoded: $ty = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(decoded.as_str(), $value);
    }};
}

/// Verifies identifier wrappers expose stable string values and serde.
#[test]
fn identifiers_roundtrip_with_serde_and_display() {
    let tenant_id = TenantId::from_raw(1).expect("nonzero tenant id");
    assert_eq!(tenant_id.get(), 1);
    assert_eq!(tenant_id.to_string(), "1");

    let json = serde_json::to_string(&tenant_id).expect("serialize tenant");
    assert_eq!(json, "1");

    let decoded: TenantId = serde_json::from_str(&json).expect("deserialize tenant");
    assert_eq!(decoded.get(), 1);

    let namespace_id = NamespaceId::from_raw(1).expect("nonzero namespace id");
    assert_eq!(namespace_id.get(), 1);
    assert_eq!(namespace_id.to_string(), "1");

    let json = serde_json::to_string(&namespace_id).expect("serialize namespace");
    assert_eq!(json, "1");

    let decoded: NamespaceId = serde_json::from_str(&json).expect("deserialize namespace");
    assert_eq!(decoded.get(), 1);

    assert_id_roundtrip!(ScenarioId, "scenario-1");
    assert_id_roundtrip!(SpecVersion, "v1");
    assert_id_roundtrip!(RunId, "run-1");
    assert_id_roundtrip!(StageId, "stage-1");
    assert_id_roundtrip!(PacketId, "packet-1");
    assert_id_roundtrip!(GateId, "gate-1");
    assert_id_roundtrip!(ConditionId, "condition-1");
    assert_id_roundtrip!(ProviderId, "provider-1");
    assert_id_roundtrip!(TriggerId, "trigger-1");
    assert_id_roundtrip!(DecisionId, "decision-1");
    assert_id_roundtrip!(CorrelationId, "corr-1");
    assert_id_roundtrip!(SchemaId, "schema-1");
    assert_id_roundtrip!(PolicyId, "policy-1");
}
