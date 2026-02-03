//! `ScenarioSpec` fuzz tests for Decision Gate core.
// decision-gate-core/tests/spec_fuzz.rs
// ============================================================================
// Module: ScenarioSpec Fuzz Tests
// Description: Deterministic fuzz-style coverage for ScenarioSpec parsing.
// Purpose: Ensure malformed specs fail closed without panicking.
// ============================================================================

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
    reason = "Test-only assertions and helpers are permitted."
)]

use decision_gate_core::ScenarioSpec;
use serde_json::Value;
use serde_json::json;

#[test]
fn scenario_spec_fuzz_inputs_fail_closed() {
    let cases: Vec<Value> = vec![
        Value::Null,
        json!({}),
        json!({"scenario_id": 1}),
        json!({"scenario_id": "demo", "namespace_id": 0}),
        json!({
            "scenario_id": "demo",
            "namespace_id": 1,
            "spec_version": "1",
            "stages": [],
            "conditions": [],
            "policies": [],
            "schemas": []
        }),
        json!({
            "scenario_id": "demo",
            "namespace_id": 1,
            "spec_version": "1",
            "stages": [{"stage_id": "stage-1", "entry_packets": [], "gates": []}],
            "conditions": [{"condition_id": "c1", "query": {"provider_id": "time"}}],
            "policies": [],
            "schemas": []
        }),
        json!({
            "scenario_id": "demo",
            "namespace_id": 1,
            "spec_version": "1",
            "stages": [{"stage_id": "stage-1", "entry_packets": [], "gates": [{"gate_id": "g1", "requirement": {"kind": "condition", "condition_id": "c1"}}]}],
            "conditions": [{"condition_id": "c1", "query": {"provider_id": "time", "check_id": "after"}, "comparator": "nope"}],
            "policies": [],
            "schemas": []
        }),
        json!({
            "scenario_id": "demo",
            "namespace_id": 1,
            "spec_version": "1",
            "stages": [{"stage_id": "stage-1", "entry_packets": [], "gates": [{"gate_id": "g1", "requirement": {"kind": "condition", "condition_id": "c1"}}]}],
            "conditions": [
                {"condition_id": "c1", "query": {"provider_id": "time", "check_id": "after"}, "comparator": "equals", "expected": true},
                {"condition_id": "c1", "query": {"provider_id": "time", "check_id": "after"}, "comparator": "equals", "expected": true}
            ],
            "policies": [],
            "schemas": []
        }),
    ];

    for (index, case) in cases.into_iter().enumerate() {
        let bytes = serde_json::to_vec(&case).unwrap_or_default();
        if let Ok(spec) = serde_json::from_slice::<ScenarioSpec>(&bytes) {
            assert!(spec.validate().is_err(), "expected validation failure for fuzz case {index}");
        }
    }
}
