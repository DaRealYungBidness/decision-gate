// system-tests/tests/sdk_examples.rs
// ============================================================================
// Module: SDK Examples Suite
// Description: Aggregates repository example system tests.
// Purpose: Ensure examples execute against live MCP servers.
// Dependencies: suites/*
// ============================================================================

//! ## Overview
//! Aggregates repository example system tests.
//! Purpose: Ensure examples execute against live MCP servers.
//! Invariants:
//! - System-test execution is deterministic and fail-closed.
//! - Inputs are treated as untrusted unless explicitly mocked.
//! Security posture: system-test inputs are untrusted; see `Docs/security/threat_model.md`.

mod helpers;

#[path = "suites/sdk_examples.rs"]
mod sdk_examples;
