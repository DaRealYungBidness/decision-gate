//! Tooltip manifest tests for Decision Gate contract outputs.
// decision-gate-contract/tests/tooltips_manifest.rs
// ============================================================================
// Module: Tooltip Manifest Tests
// Description: Validate tooltip manifest coverage and hygiene.
// Purpose: Ensure tooltip terms stay stable and complete.
// Dependencies: decision-gate-contract
// ============================================================================
use decision_gate_contract::tooltips::tooltips_manifest;
use decision_gate_contract::types::ToolName;

fn is_sorted(items: &[String]) -> bool {
    items.windows(2).all(|pair| pair[0] <= pair[1])
}

#[test]
fn tooltips_manifest_has_unique_sorted_terms() {
    let manifest = tooltips_manifest();
    let terms: Vec<String> = manifest.entries.iter().map(|entry| entry.term.clone()).collect();
    assert!(!terms.is_empty(), "tooltips manifest is empty");
    let mut deduped = terms.clone();
    deduped.sort();
    deduped.dedup();
    assert_eq!(terms.len(), deduped.len(), "tooltip terms must be unique");
    assert!(is_sorted(&deduped), "tooltip terms must be sorted");
}

#[test]
fn tooltips_manifest_includes_tool_names() {
    let manifest = tooltips_manifest();
    let terms: Vec<&str> = manifest.entries.iter().map(|entry| entry.term.as_str()).collect();
    for tool in ToolName::all() {
        let term = tool.as_str();
        assert!(terms.contains(&term), "tooltip terms missing tool name: {}", term);
    }
}

#[test]
fn tooltips_manifest_includes_core_terms() {
    let manifest = tooltips_manifest();
    let terms: Vec<&str> = manifest.entries.iter().map(|entry| entry.term.as_str()).collect();
    let required = [
        "scenario_id",
        "ScenarioSpec",
        "run_id",
        "stage_id",
        "trigger_id",
        "provider_id",
        "predicate",
        "Predicate",
        "PredicateSpec",
        "params",
        "comparator",
        "expected",
        "EvidenceQuery",
        "EvidenceContext",
        "EvidenceResult",
        "requirement",
        "RequireGroup",
        "GateSpec",
    ];
    for term in required {
        assert!(terms.contains(&term), "tooltip terms missing: {}", term);
    }
}

#[test]
fn tooltips_manifest_is_ascii() {
    let manifest = tooltips_manifest();
    for entry in manifest.entries {
        assert!(entry.term.is_ascii(), "tooltip term must be ASCII: {}", entry.term);
        assert!(entry.title.is_ascii(), "tooltip title must be ASCII: {}", entry.title);
        assert!(
            entry.description.is_ascii(),
            "tooltip description must be ASCII: {}",
            entry.description
        );
    }
}
