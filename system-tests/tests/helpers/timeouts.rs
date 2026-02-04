// system-tests/tests/helpers/timeouts.rs
// ============================================================================
// Module: System Test Timeouts
// Description: Centralized timeout configuration with env overrides.
// Purpose: Keep system-test timeouts consistent and configurable across suites.
// ============================================================================

use std::env;
use std::time::Duration;

const ENV_TIMEOUT_SECS: &str = "DECISION_GATE_SYSTEM_TEST_TIMEOUT_SEC";

/// Returns the effective timeout, honoring `DECISION_GATE_SYSTEM_TEST_TIMEOUT_SEC` when set.
/// The override acts as a minimum to avoid shortening explicitly longer test timeouts.
#[must_use]
pub fn resolve_timeout(requested: Duration) -> Duration {
    match env::var(ENV_TIMEOUT_SECS) {
        Ok(raw) => {
            let override_timeout = parse_timeout_secs(&raw).unwrap_or_else(|err| {
                panic!("{ENV_TIMEOUT_SECS} {err}");
            });
            std::cmp::max(requested, override_timeout)
        }
        Err(_) => requested,
    }
}

fn parse_timeout_secs(raw: &str) -> Result<Duration, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err("must be a positive integer number of seconds".to_string());
    }
    let secs: u64 =
        trimmed.parse().map_err(|_| "must be a positive integer number of seconds".to_string())?;
    if secs == 0 {
        return Err("must be greater than zero".to_string());
    }
    Ok(Duration::from_secs(secs))
}
