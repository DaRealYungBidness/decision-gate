// system-tests/tests/security.rs
// ============================================================================
// Module: Security Suite
// Description: Aggregates security and authz system tests.
// Purpose: Reduce binaries while keeping security coverage centralized.
// Dependencies: suites/*, helpers
// ============================================================================

//! Security suite entry point for system-tests.

mod helpers;

#[path = "suites/anchor_fuzz.rs"]
mod anchor_fuzz;
#[path = "suites/audit_registry.rs"]
mod audit_registry;
#[path = "suites/auth_matrix.rs"]
mod auth_matrix;
#[path = "suites/config_validation.rs"]
mod config_validation;
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
