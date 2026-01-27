// enterprise/enterprise-system-tests/tests/usage.rs
// ============================================================================
// Module: Usage Suite
// Description: Aggregates enterprise usage metering and quota tests.
// Purpose: Reduce binaries while keeping usage coverage centralized.
// Dependencies: suites/*, helpers
// ============================================================================

//! Usage suite entry point for enterprise system-tests.

mod helpers;

#[path = "suites/usage.rs"]
mod usage;
