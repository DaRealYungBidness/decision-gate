// decision-gate-core/tests/store.rs
// ============================================================================
// Module: Run State Store Tests
// Description: Tests for the in-memory run state store implementation.
// Purpose: Validate deterministic save/load behavior in the in-memory store.
// Dependencies: decision-gate-core
// ============================================================================
//! ## Overview
//! Ensures the in-memory store returns saved run states and fails closed on
//! missing entries.
//!
//! Security posture: Store operations are deterministic and isolated.
//! Threat model: TM-STORE-001 - Store corruption or load confusion.

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

use decision_gate_core::InMemoryRunStateStore;
use decision_gate_core::NamespaceId;
use decision_gate_core::RunId;
use decision_gate_core::RunState;
use decision_gate_core::RunStateStore;
use decision_gate_core::RunStatus;
use decision_gate_core::ScenarioId;
use decision_gate_core::StageId;
use decision_gate_core::TenantId;
use decision_gate_core::Timestamp;
use decision_gate_core::hashing::DEFAULT_HASH_ALGORITHM;

fn sample_state(run_id: &str) -> RunState {
    let spec = decision_gate_core::ScenarioSpec {
        scenario_id: ScenarioId::new("scenario"),
        namespace_id: NamespaceId::from_raw(1).expect("nonzero namespaceid"),
        spec_version: decision_gate_core::SpecVersion::new("1"),
        stages: vec![decision_gate_core::StageSpec {
            stage_id: StageId::new("stage-1"),
            entry_packets: Vec::new(),
            gates: Vec::new(),
            advance_to: decision_gate_core::AdvanceTo::Terminal,
            timeout: None,
            on_timeout: decision_gate_core::TimeoutPolicy::Fail,
        }],
        conditions: Vec::new(),
        policies: Vec::new(),
        schemas: Vec::new(),
        default_tenant_id: None,
    };
    let spec_hash = spec.canonical_hash_with(DEFAULT_HASH_ALGORITHM).expect("spec hash");
    RunState {
        tenant_id: TenantId::from_raw(1).expect("nonzero tenantid"),
        namespace_id: NamespaceId::from_raw(1).expect("nonzero namespaceid"),
        run_id: RunId::new(run_id),
        scenario_id: ScenarioId::new("scenario"),
        spec_hash,
        current_stage_id: StageId::new("stage-1"),
        stage_entered_at: Timestamp::Logical(0),
        status: RunStatus::Active,
        dispatch_targets: Vec::new(),
        triggers: Vec::new(),
        gate_evals: Vec::new(),
        decisions: Vec::new(),
        packets: Vec::new(),
        submissions: Vec::new(),
        tool_calls: Vec::new(),
    }
}

/// Verifies saving then loading a run state succeeds.
#[test]
fn store_save_and_load_roundtrip() {
    let store = InMemoryRunStateStore::new();
    let state = sample_state("run-1");

    store.save(&state).unwrap();
    let loaded = store
        .load(
            &TenantId::from_raw(1).expect("nonzero tenantid"),
            &NamespaceId::from_raw(1).expect("nonzero namespaceid"),
            &RunId::new("run-1"),
        )
        .unwrap();
    assert_eq!(loaded, Some(state));
}

/// Verifies loading a missing run state returns None.
#[test]
fn store_returns_none_for_missing_run() {
    let store = InMemoryRunStateStore::new();
    let loaded = store
        .load(
            &TenantId::from_raw(1).expect("nonzero tenantid"),
            &NamespaceId::from_raw(1).expect("nonzero namespaceid"),
            &RunId::new("missing"),
        )
        .unwrap();
    assert!(loaded.is_none());
}
