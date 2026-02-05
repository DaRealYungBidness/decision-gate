// crates/decision-gate-core/tests/proptest_comparator.rs
// ============================================================================
// Module: Comparator Property-Based Tests
// Description: Property tests for comparator correctness and stability.
// Purpose: Detect panics and invariants across wide input ranges.
// ============================================================================

//! Property-based tests for comparator invariants.

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
use decision_gate_core::TrustLane;
use decision_gate_core::runtime::comparator::evaluate_comparator;
use proptest::prelude::*;
use ret_logic::TriState;
use serde_json::Value;
use serde_json::json;

fn eval_json(comparator: Comparator, expected: &Value, evidence: &Value) -> TriState {
    let evidence_result = EvidenceResult {
        value: Some(EvidenceValue::Json(evidence.clone())),
        lane: TrustLane::Verified,
        error: None,
        evidence_hash: None,
        evidence_ref: None,
        evidence_anchor: None,
        signature: None,
        content_type: None,
    };
    evaluate_comparator(comparator, Some(expected), &evidence_result)
}

fn json_value_strategy(max_depth: u32) -> impl Strategy<Value = Value> {
    let leaf = prop_oneof![
        Just(Value::Null),
        any::<bool>().prop_map(Value::Bool),
        any::<i64>().prop_map(|v| Value::Number(v.into())),
        any::<f64>()
            .prop_filter("finite", |v| v.is_finite())
            .prop_map(|v| { serde_json::Number::from_f64(v).map_or(Value::Null, Value::Number) }),
        ".*".prop_map(Value::String),
    ];

    leaf.prop_recursive(max_depth, 64, 8, |inner| {
        prop_oneof![
            prop::collection::vec(inner.clone(), 0 .. 4).prop_map(Value::Array),
            prop::collection::btree_map("[a-z]{1,4}", inner, 0 .. 4).prop_map(|map| {
                let mut object = serde_json::Map::new();
                for (key, value) in map {
                    object.insert(key, value);
                }
                Value::Object(object)
            }),
        ]
    })
}

proptest! {
    #[test]
    fn comparator_numeric_equality_is_correct(a in any::<i64>(), b in any::<i64>()) {
        let expected = json!(b);
        let evidence = json!(a);
        let result = eval_json(Comparator::Equals, &expected, &evidence);
        if a == b {
            prop_assert_eq!(result, TriState::True);
        } else {
            prop_assert_eq!(result, TriState::False);
        }
    }

    #[test]
    fn comparator_numeric_ordering_is_correct(a in any::<i64>(), b in any::<i64>()) {
        let expected = json!(b);
        let evidence = json!(a);
        let gt = eval_json(Comparator::GreaterThan, &expected, &evidence);
        let lt = eval_json(Comparator::LessThan, &expected, &evidence);
        match a.cmp(&b) {
            std::cmp::Ordering::Greater => {
                prop_assert_eq!(gt, TriState::True);
                prop_assert_eq!(lt, TriState::False);
            }
            std::cmp::Ordering::Less => {
                prop_assert_eq!(gt, TriState::False);
                prop_assert_eq!(lt, TriState::True);
            }
            std::cmp::Ordering::Equal => {
                prop_assert_eq!(gt, TriState::False);
                prop_assert_eq!(lt, TriState::False);
            }
        }
    }

    #[test]
    fn comparator_never_panics_on_random_json(expected in json_value_strategy(2), evidence in json_value_strategy(2)) {
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

        let evidence_result = EvidenceResult {
            value: Some(EvidenceValue::Json(evidence)),
            lane: TrustLane::Verified,
            error: None,
            evidence_hash: None,
            evidence_ref: None,
            evidence_anchor: None,
            signature: None,
            content_type: None,
        };

        for comparator in comparators {
            let _ = evaluate_comparator(comparator, Some(&expected), &evidence_result);
        }
    }
}
