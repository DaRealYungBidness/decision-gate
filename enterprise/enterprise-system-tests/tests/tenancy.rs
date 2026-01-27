// enterprise/enterprise-system-tests/tests/tenancy.rs
// ============================================================================
// Module: Tenancy Suite
// Description: Aggregates enterprise tenant isolation system tests.
// Purpose: Reduce binaries while keeping tenancy coverage centralized.
// Dependencies: suites/*, helpers
// ============================================================================

//! Tenancy suite entry point for enterprise system-tests.

mod helpers;

#[path = "suites/tenant_isolation.rs"]
mod tenant_isolation;
