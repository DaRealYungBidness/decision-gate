// system-tests/tests/security.rs
// ============================================================================
// Module: Security Suite
// Description: Aggregates security and authz system tests.
// Purpose: Reduce binaries while keeping security coverage centralized.
// Dependencies: suites/*, helpers
// ============================================================================

//! ## Overview
//! Aggregates security and authz system tests.
//! Purpose: Reduce binaries while keeping security coverage centralized.
//! Invariants:
//! - System-test execution is deterministic and fail-closed.
//! - Inputs are treated as untrusted unless explicitly mocked.
//!
//! Security posture: system-test inputs are untrusted; see `Docs/security/threat_model.md`.

mod helpers;

#[path = "suites/anchor_fuzz.rs"]
mod anchor_fuzz;
#[path = "suites/audit_registry.rs"]
mod audit_registry;
#[path = "suites/auth_matrix.rs"]
mod auth_matrix;
#[path = "suites/cli_auth.rs"]
mod cli_auth;
#[path = "suites/config_defaults.rs"]
mod config_defaults;
#[path = "suites/config_validation.rs"]
mod config_validation;
#[path = "suites/evidence_fuzz.rs"]
mod evidence_fuzz;
#[path = "suites/log_leak_scan.rs"]
mod log_leak_scan;
#[path = "suites/mcp_auth.rs"]
mod mcp_auth;
#[path = "suites/namespace_defaults.rs"]
mod namespace_defaults;
#[path = "suites/registry_acl.rs"]
mod registry_acl;
#[path = "suites/schema_registry_fuzz.rs"]
mod schema_registry_fuzz;
#[path = "suites/security.rs"]
mod security;
#[path = "suites/tool_visibility.rs"]
mod tool_visibility;
