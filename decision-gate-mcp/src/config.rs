// decision-gate-mcp/src/config.rs
// ============================================================================
// Module: MCP Configuration (Re-export)
// Description: Re-export canonical Decision Gate config types.
// Purpose: Preserve MCP public API while centralizing config logic.
// Dependencies: decision-gate-config
// ============================================================================

//! ## Overview
//! This module re-exports the canonical configuration model from
//! `decision-gate-config` to keep MCP callers stable while enforcing a single
//! source of truth.

/// Re-export canonical config types and helpers.
pub use decision_gate_config::*;
