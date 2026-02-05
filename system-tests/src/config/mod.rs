// system-tests/src/config/mod.rs
// ============================================================================
// Module: System Test Configuration
// Description: Centralized configuration for Decision Gate system tests.
// Purpose: Provide typed access to test environment settings and defaults.
// Dependencies: std
// ============================================================================

//! ## Overview
//! System-test configuration is read from environment variables and mapped into
//! a small typed structure for reuse across test helpers.
//! Security posture: environment inputs are untrusted; see `Docs/security/threat_model.md`.

// ============================================================================
// SECTION: Modules
// ============================================================================

mod env;

// ============================================================================
// SECTION: Tests
// ============================================================================

#[cfg(test)]
mod env_tests;

// ============================================================================
// SECTION: Re-exports
// ============================================================================

pub use env::SystemTestConfig;
pub use env::SystemTestEnv;
pub use env::read_env_strict;
