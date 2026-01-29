// decision-gate-contract/tests/provider_capabilities.rs
// ============================================================================
// Module: Provider Capability Tests
// Description: Validate comparator allow-lists and determinism metadata.
// Purpose: Ensure provider contracts stay canonical and strict.
// Dependencies: decision-gate-contract, decision-gate-core
// ============================================================================

//! ## Overview
//! Validates provider capability metadata, comparator allow-lists, and example
//! schema conformance.
//! Security posture: provider contracts gate untrusted inputs; see
//! `Docs/security/threat_model.md`.

// ============================================================================
// SECTION: Imports
// ============================================================================

use decision_gate_contract::providers::provider_contracts;
use decision_gate_contract::types::DeterminismClass;
use decision_gate_core::Comparator;
use jsonschema::CompilationOptions;
use jsonschema::Draft;
use jsonschema::JSONSchema;
use serde_json::Value;

// ============================================================================
// SECTION: Comparator Ordering Helpers
// ============================================================================

const fn comparator_order() -> [Comparator; 16] {
    [
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
    ]
}

fn comparator_index(comparator: Comparator) -> Option<usize> {
    comparator_order().iter().position(|candidate| *candidate == comparator)
}

fn is_canonical_order(list: &[Comparator]) -> bool {
    let mut iter = list.iter();
    let Some(first) = iter.next() else {
        return true;
    };
    let Some(mut previous) = comparator_index(*first) else {
        return false;
    };
    for comparator in iter {
        let Some(current) = comparator_index(*comparator) else {
            return false;
        };
        if previous > current {
            return false;
        }
        previous = current;
    }
    true
}

// ============================================================================
// SECTION: Schema Helpers
// ============================================================================

fn compile_schema(schema: &Value) -> Result<JSONSchema, String> {
    let mut options = CompilationOptions::default();
    options.with_draft(Draft::Draft202012);
    options.compile(schema).map_err(|err| format!("provider schema compilation failed: {err}"))
}

// ============================================================================
// SECTION: Tests
// ============================================================================

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
fn time_provider_comparators_match_schema_expectations() -> Result<(), String> {
    let contracts = provider_contracts();
    let time = contracts
        .iter()
        .find(|provider| provider.provider_id == "time")
        .ok_or_else(|| "time provider missing".to_string())?;
    let now = time
        .predicates
        .iter()
        .find(|predicate| predicate.name == "now")
        .ok_or_else(|| "time.now predicate missing".to_string())?;
    let expected_now = vec![
        Comparator::Equals,
        Comparator::NotEquals,
        Comparator::GreaterThan,
        Comparator::GreaterThanOrEqual,
        Comparator::LessThan,
        Comparator::LessThanOrEqual,
        Comparator::InSet,
        Comparator::Exists,
        Comparator::NotExists,
    ];
    if now.allowed_comparators != expected_now {
        return Err("time.now comparators mismatch".to_string());
    }

    let after = time
        .predicates
        .iter()
        .find(|predicate| predicate.name == "after")
        .ok_or_else(|| "time.after predicate missing".to_string())?;
    let expected_after = vec![
        Comparator::Equals,
        Comparator::NotEquals,
        Comparator::InSet,
        Comparator::Exists,
        Comparator::NotExists,
    ];
    if after.allowed_comparators != expected_after {
        return Err("time.after comparators mismatch".to_string());
    }
    Ok(())
}

#[test]
fn provider_determinism_metadata_is_set() -> Result<(), String> {
    let contracts = provider_contracts();
    let time = contracts
        .iter()
        .find(|provider| provider.provider_id == "time")
        .ok_or_else(|| "time provider missing".to_string())?;
    for predicate in &time.predicates {
        if predicate.determinism != DeterminismClass::TimeDependent {
            return Err("time predicate determinism mismatch".to_string());
        }
    }

    let env = contracts
        .iter()
        .find(|provider| provider.provider_id == "env")
        .ok_or_else(|| "env provider missing".to_string())?;
    let env_predicate = env
        .predicates
        .iter()
        .find(|predicate| predicate.name == "get")
        .ok_or_else(|| "env.get predicate missing".to_string())?;
    if env_predicate.determinism != DeterminismClass::External {
        return Err("env.get determinism mismatch".to_string());
    }
    Ok(())
}

#[test]
fn provider_predicate_examples_match_schemas() -> Result<(), String> {
    let contracts = provider_contracts();
    for provider in contracts {
        for predicate in provider.predicates {
            if predicate.examples.is_empty() {
                return Err(format!(
                    "{}.{} missing examples",
                    provider.provider_id, predicate.name
                ));
            }
            let params_schema = compile_schema(&predicate.params_schema)?;
            let result_schema = compile_schema(&predicate.result_schema)?;
            for example in predicate.examples {
                if params_schema.validate(&example.params).is_err() {
                    return Err(format!(
                        "{}.{} example params failed",
                        provider.provider_id, predicate.name
                    ));
                }
                if result_schema.validate(&example.result).is_err() {
                    return Err(format!(
                        "{}.{} example result failed",
                        provider.provider_id, predicate.name
                    ));
                }
            }
        }
    }
    Ok(())
}
