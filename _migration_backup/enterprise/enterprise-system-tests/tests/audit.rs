// enterprise/enterprise-system-tests/tests/audit.rs
// ============================================================================
// Module: Audit Suite
// Description: Aggregates enterprise audit chain system tests.
// Purpose: Reduce binaries while keeping audit coverage centralized.
// Dependencies: suites/*, helpers
// ============================================================================

//! Audit suite entry point for enterprise system-tests.

mod helpers;

#[path = "suites/audit.rs"]
mod audit;
