// enterprise/enterprise-system-tests/tests/security.rs
// ============================================================================
// Module: Security Suite
// Description: Aggregates enterprise authz and security system tests.
// Purpose: Reduce binaries while keeping security coverage centralized.
// Dependencies: suites/*, helpers
// ============================================================================

//! Security suite entry point for enterprise system-tests.

mod helpers;

#[path = "suites/tenant_authz.rs"]
mod tenant_authz;
