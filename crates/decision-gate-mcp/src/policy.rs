// crates/decision-gate-mcp/src/policy.rs
// ============================================================================
// Module: Policy Engine Adapters (Re-export)
// Description: Re-export canonical policy config and adapters.
// Purpose: Preserve MCP public API while centralizing policy logic.
// Dependencies: decision-gate-config
// ============================================================================

//! ## Overview
//! This module re-exports policy config and dispatch adapters from
//! `decision-gate-config`.
//! Security posture: dispatch policy affects disclosure and must fail closed;
//! see `Docs/security/threat_model.md`.

/// Re-export canonical policy config and adapters.
pub use decision_gate_config::policy::*;
