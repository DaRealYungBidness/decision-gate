//! Comparator fuzz tests for Decision Gate core.
// decision-gate-core/tests/comparator_fuzz.rs
// ============================================================================
// Module: Comparator Fuzz Tests
// Description: Deterministic fuzz-style coverage for comparator evaluation.
// Purpose: Ensure comparator evaluation handles edge cases without panics.
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

use decision_gate_core::Comparator;
use decision_gate_core::EvidenceResult;
use decision_gate_core::EvidenceValue;
use decision_gate_core::runtime::comparator::evaluate_comparator;
use ret_logic::TriState;
use serde_json::Value;
use serde_json::json;

#[test]
fn comparator_fuzz_inputs_do_not_panic() {
    let values = vec![
        Value::Null,
        json!(true),
        json!(false),
        json!(0),
        json!(1),
        json!(-1),
        json!(1.234),
        json!(""),
        json!("text"),
        json!([]),
        json!([1, 2, 3]),
        json!({}),
        json!({"nested": "value"}),
    ];
    let comparators = vec![
        Comparator::Equals,
        Comparator::NotEquals,
        Comparator::GreaterThan,
        Comparator::GreaterThanOrEqual,
        Comparator::LessThan,
        Comparator::LessThanOrEqual,
        Comparator::LexGreaterThan,
        Comparator::LexGreaterThanOrEqual,
        Comparator::LexLessThan,
        Comparator::LexLessThanOrEqual,
        Comparator::Contains,
        Comparator::InSet,
        Comparator::DeepEquals,
        Comparator::DeepNotEquals,
        Comparator::Exists,
        Comparator::NotExists,
    ];

    for expected in &values {
        for evidence in &values {
            for comparator in &comparators {
                let result = EvidenceResult {
                    value: Some(EvidenceValue::Json(evidence.clone())),
                    lane: decision_gate_core::TrustLane::Verified,
                    error: None,
                    evidence_hash: None,
                    evidence_ref: None,
                    evidence_anchor: None,
                    signature: None,
                    content_type: Some("application/json".to_string()),
                };
                let outcome = evaluate_comparator(*comparator, Some(expected), &result);
                match outcome {
                    TriState::True | TriState::False | TriState::Unknown => {}
                }
            }
        }
    }
}
