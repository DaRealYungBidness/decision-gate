// system-tests/tests/helpers/timeouts.rs
// ============================================================================
// Module: System Test Timeouts
// Description: Centralized timeout configuration with env overrides.
// Purpose: Keep system-test timeouts consistent and configurable across suites.
// Dependencies: system-tests
// ============================================================================

//! ## Overview
//! Centralized timeout configuration with env overrides.
//! Purpose: Keep system-test timeouts consistent and configurable across suites.
//! Invariants:
//! - System-test execution is deterministic and fail-closed.
//! - Inputs are treated as untrusted unless explicitly mocked.
//!
//! Security posture: system-test inputs are untrusted; see `Docs/security/threat_model.md`.

// ============================================================================
// SECTION: Lint Configuration
// ============================================================================

#![allow(clippy::panic, reason = "System-test helpers fail fast on invalid timeout config.")]

use std::time::Duration;

use system_tests::config::SystemTestConfig;

/// Returns the effective timeout, honoring `DECISION_GATE_SYSTEM_TEST_TIMEOUT_SEC` when set.
/// The override acts as a minimum to avoid shortening explicitly longer test timeouts.
#[must_use]
pub fn resolve_timeout(requested: Duration) -> Duration {
    match SystemTestConfig::load() {
        Ok(config) => config
            .timeout
            .map_or(requested, |override_timeout| std::cmp::max(requested, override_timeout)),
        Err(err) => panic!("system-test timeout configuration error: {err}"),
    }
}
