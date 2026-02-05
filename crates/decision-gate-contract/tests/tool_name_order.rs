// crates/decision-gate-contract/tests/tool_name_order.rs
// ============================================================================
// Module: Tool Name Ordering Tests
// Description: Ensure canonical tool ordering stays consistent.
// Purpose: Prevent drift between ToolName::all and tool contract ordering.
// Dependencies: decision-gate-contract
// ============================================================================

//! ## Overview
//! Confirms the canonical tool ordering used in docs and SDKs is stable.
//! Security posture: tool contracts define external interfaces; see
//! `Docs/security/threat_model.md`.

// ============================================================================
// SECTION: Imports
// ============================================================================

use decision_gate_contract::ToolName;
use decision_gate_contract::tooling::tool_contracts;

// ============================================================================
// SECTION: Tests
// ============================================================================

#[test]
fn tool_name_order_matches_tool_contracts() {
    let contract_names: Vec<ToolName> =
        tool_contracts().into_iter().map(|contract| contract.name).collect();
    assert_eq!(
        ToolName::all(),
        contract_names.as_slice(),
        "ToolName::all order drifted from tool_contracts()",
    );
}
