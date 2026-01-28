// enterprise-system-tests/tests/helpers/env.rs
// ============================================================================
// Module: Test Environment Helpers
// Description: Safe wrappers for test-only environment mutation.
// Purpose: Centralize env var changes with explicit safety notes.
// ============================================================================

#![allow(unsafe_code, reason = "Test harness mutates process env for configuration.")]

/// Sets an environment variable for the current process.
pub fn set_var(key: &str, value: &str) {
    // SAFETY: Tests control process lifecycle and set env vars before server start.
    unsafe {
        std::env::set_var(key, value);
    }
}

/// Removes an environment variable from the current process.
pub fn remove_var(key: &str) {
    // SAFETY: Tests cleanup env vars after use in a controlled process.
    unsafe {
        std::env::remove_var(key);
    }
}
