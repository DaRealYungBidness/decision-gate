// system-tests/tests/providers.rs
// ============================================================================
// Module: Providers Suite
// Description: Aggregates provider and AssetCore integration system tests.
// Purpose: Reduce binaries while keeping provider coverage centralized.
// Dependencies: suites/*, helpers
// ============================================================================

//! ## Overview
//! Aggregates provider and `AssetCore` integration system tests.
//! Purpose: Reduce binaries while keeping provider coverage centralized.
//! Invariants:
//! - System-test execution is deterministic and fail-closed.
//! - Inputs are treated as untrusted unless explicitly mocked.
//!
//! Security posture: system-test inputs are untrusted; see `Docs/security/threat_model.md`.

mod helpers;

#[path = "suites/assetcore_integration.rs"]
mod assetcore_integration;
#[path = "suites/provider_templates.rs"]
mod provider_templates;
#[path = "suites/providers.rs"]
mod providers;
