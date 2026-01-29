// system-tests/tests/sdk_examples.rs
// ============================================================================
// Module: SDK Examples Suite
// Description: Aggregates repository example system tests.
// Purpose: Ensure examples execute against live MCP servers.
// Dependencies: suites/*
// ============================================================================

//! SDK examples suite entry point for system-tests.

mod helpers;

#[path = "suites/sdk_examples.rs"]
mod sdk_examples;
