// decision-gate-contract/tests/provider_capabilities.rs
// ============================================================================
// Module: Provider Capability Tests
// Description: Validate comparator allow-lists and determinism metadata.
// Purpose: Ensure provider contracts stay canonical and strict.
// Dependencies: decision-gate-contract, decision-gate-core
// ============================================================================

//! Provider capability metadata tests for the contract bundle.

use decision_gate_contract::providers::provider_contracts;
use decision_gate_contract::types::DeterminismClass;
use decision_gate_core::Comparator;
use jsonschema::CompilationOptions;
use jsonschema::Draft;
use jsonschema::JSONSchema;
use serde_json::Value;

fn comparator_order() -> [Comparator; 10] {
    [
        Comparator::Equals,
        Comparator::NotEquals,
        Comparator::GreaterThan,
        Comparator::GreaterThanOrEqual,
        Comparator::LessThan,
        Comparator::LessThanOrEqual,
        Comparator::Contains,
        Comparator::InSet,
        Comparator::Exists,
        Comparator::NotExists,
    ]
}

fn comparator_index(comparator: Comparator) -> usize {
    comparator_order().iter().position(|candidate| *candidate == comparator).unwrap_or(usize::MAX)
}

fn is_canonical_order(list: &[Comparator]) -> bool {
    list.windows(2).all(|pair| comparator_index(pair[0]) <= comparator_index(pair[1]))
}

fn compile_schema(schema: &Value) -> JSONSchema {
    let mut options = CompilationOptions::default();
    options.with_draft(Draft::Draft202012);
    options.compile(schema).expect("provider schema compilation failed")
}

#[test]
fn provider_predicates_have_canonical_allowlists() {
    let contracts = provider_contracts();
    for provider in contracts {
        for predicate in provider.predicates {
            assert!(
                !predicate.allowed_comparators.is_empty(),
                "{}.{} missing allowed_comparators",
                provider.provider_id,
                predicate.name
            );
            let mut seen: Vec<Comparator> = Vec::new();
            for comparator in &predicate.allowed_comparators {
                assert!(
                    !seen.contains(comparator),
                    "{}.{} has duplicate comparator {:?}",
                    provider.provider_id,
                    predicate.name,
                    comparator
                );
                seen.push(*comparator);
            }
            assert!(
                is_canonical_order(&predicate.allowed_comparators),
                "{}.{} comparators out of order",
                provider.provider_id,
                predicate.name
            );
        }
    }
}

#[test]
fn time_provider_comparators_match_schema_expectations() {
    let contracts = provider_contracts();
    let time = contracts
        .iter()
        .find(|provider| provider.provider_id == "time")
        .expect("time provider missing");
    let now = time
        .predicates
        .iter()
        .find(|predicate| predicate.name == "now")
        .expect("time.now predicate missing");
    assert_eq!(
        now.allowed_comparators,
        vec![
            Comparator::Equals,
            Comparator::NotEquals,
            Comparator::GreaterThan,
            Comparator::GreaterThanOrEqual,
            Comparator::LessThan,
            Comparator::LessThanOrEqual,
            Comparator::InSet,
            Comparator::Exists,
            Comparator::NotExists,
        ]
    );

    let after = time
        .predicates
        .iter()
        .find(|predicate| predicate.name == "after")
        .expect("time.after predicate missing");
    assert_eq!(
        after.allowed_comparators,
        vec![Comparator::Equals, Comparator::NotEquals, Comparator::Exists, Comparator::NotExists,]
    );
}

#[test]
fn provider_determinism_metadata_is_set() {
    let contracts = provider_contracts();
    let time = contracts
        .iter()
        .find(|provider| provider.provider_id == "time")
        .expect("time provider missing");
    for predicate in &time.predicates {
        assert_eq!(predicate.determinism, DeterminismClass::TimeDependent);
    }

    let env = contracts
        .iter()
        .find(|provider| provider.provider_id == "env")
        .expect("env provider missing");
    let env_predicate = env
        .predicates
        .iter()
        .find(|predicate| predicate.name == "get")
        .expect("env.get predicate missing");
    assert_eq!(env_predicate.determinism, DeterminismClass::External);
}

#[test]
fn provider_predicate_examples_match_schemas() {
    let contracts = provider_contracts();
    for provider in contracts {
        for predicate in provider.predicates {
            assert!(
                !predicate.examples.is_empty(),
                "{}.{} missing examples",
                provider.provider_id,
                predicate.name
            );
            let params_schema = compile_schema(&predicate.params_schema);
            let result_schema = compile_schema(&predicate.result_schema);
            for example in predicate.examples {
                let result = params_schema.validate(&example.params);
                assert!(
                    result.is_ok(),
                    "{}.{} example params failed",
                    provider.provider_id,
                    predicate.name
                );
                let result = result_schema.validate(&example.result);
                assert!(
                    result.is_ok(),
                    "{}.{} example result failed",
                    provider.provider_id,
                    predicate.name
                );
            }
        }
    }
}
