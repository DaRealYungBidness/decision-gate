// system-tests/tests/sdk_client.rs
// ============================================================================
// Module: SDK Client Suite
// Description: Aggregates Decision Gate SDK system tests.
// Purpose: Reduce binaries while keeping SDK coverage centralized.
// Dependencies: suites/*
// ============================================================================

//! ## Overview
//! Aggregates Decision Gate SDK system tests.
//! Purpose: Reduce binaries while keeping SDK coverage centralized.
//! Invariants:
//! - System-test execution is deterministic and fail-closed.
//! - Inputs are treated as untrusted unless explicitly mocked.
//!
//! Security posture: system-test inputs are untrusted; see `Docs/security/threat_model.md`.

mod helpers;

#[path = "suites/sdk_client.rs"]
mod sdk_client;
