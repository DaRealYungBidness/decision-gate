//! Boundary validation tests for decision-gate-config.
// crates/decision-gate-config/tests/boundary_validation.rs
// =============================================================================
// Module: Boundary Validation Tests
// Description: Comprehensive tests for min/max boundaries and edge cases.
// Purpose: Ensure all numeric and size boundaries are properly tested.
// =============================================================================

use std::path::PathBuf;

use decision_gate_config::ConfigError;
use decision_gate_config::RateLimitConfig;
use decision_gate_config::RunStateStoreConfig;
use decision_gate_config::RunStateStoreType;
use decision_gate_config::ServerAuthConfig;
use decision_gate_config::ServerAuthMode;
use decision_gate_store_sqlite::SqliteStoreMode;
use decision_gate_store_sqlite::SqliteSyncMode;

mod common;

type TestResult = Result<(), String>;

/// Assert that a validation result is an error containing a specific substring.
fn assert_invalid(result: Result<(), ConfigError>, needle: &str) -> TestResult {
    match result {
        Err(error) => {
            let message = error.to_string();
            if message.contains(needle) {
                Ok(())
            } else {
                Err(format!("error '{message}' did not contain '{needle}'"))
            }
        }
        Ok(()) => Err("expected invalid config".to_string()),
    }
}

// ============================================================================
// SECTION: Min/Max Boundary Testing
// ============================================================================

#[test]
fn max_body_bytes_at_minimum_1() -> TestResult {
    let mut config = common::minimal_config().map_err(|err| err.to_string())?;
    config.server.max_body_bytes = 1;
    config.validate().map_err(|err| err.to_string())?;
    Ok(())
}

#[test]
fn max_body_bytes_at_zero_rejected() -> TestResult {
    let mut config = common::minimal_config().map_err(|err| err.to_string())?;
    config.server.max_body_bytes = 0;
    assert_invalid(config.validate(), "max_body_bytes must be greater than zero")?;
    Ok(())
}

#[test]
fn max_inflight_at_minimum_1() -> TestResult {
    let mut config = common::minimal_config().map_err(|err| err.to_string())?;
    config.server.limits.max_inflight = 1;
    config.validate().map_err(|err| err.to_string())?;
    Ok(())
}

#[test]
fn max_inflight_at_zero_rejected() -> TestResult {
    let mut config = common::minimal_config().map_err(|err| err.to_string())?;
    config.server.limits.max_inflight = 0;
    assert_invalid(config.validate(), "max_inflight must be greater than zero")?;
    Ok(())
}

#[test]
fn rate_limit_window_ms_at_min_100() -> TestResult {
    let rate_limit = RateLimitConfig {
        max_requests: 100,
        window_ms: 100,
        max_entries: 1000,
    };
    let mut config = common::config_with_rate_limit(rate_limit).map_err(|err| err.to_string())?;
    config.validate().map_err(|err| err.to_string())?;
    Ok(())
}

#[test]
fn rate_limit_window_ms_at_max_60000() -> TestResult {
    let rate_limit = RateLimitConfig {
        max_requests: 100,
        window_ms: 60_000,
        max_entries: 1000,
    };
    let mut config = common::config_with_rate_limit(rate_limit).map_err(|err| err.to_string())?;
    config.validate().map_err(|err| err.to_string())?;
    Ok(())
}

#[test]
fn rate_limit_max_requests_at_min_1() -> TestResult {
    let rate_limit = RateLimitConfig {
        max_requests: 1,
        window_ms: 1000,
        max_entries: 1000,
    };
    let mut config = common::config_with_rate_limit(rate_limit).map_err(|err| err.to_string())?;
    config.validate().map_err(|err| err.to_string())?;
    Ok(())
}

#[test]
fn rate_limit_max_requests_at_max_100000() -> TestResult {
    let rate_limit = RateLimitConfig {
        max_requests: 100_000,
        window_ms: 1000,
        max_entries: 1000,
    };
    let mut config = common::config_with_rate_limit(rate_limit).map_err(|err| err.to_string())?;
    config.validate().map_err(|err| err.to_string())?;
    Ok(())
}

// ============================================================================
// SECTION: Zero/Negative Values
// ============================================================================

#[test]
fn all_numeric_fields_reject_zero() -> TestResult {
    // max_body_bytes = 0
    let mut config1 = common::minimal_config().map_err(|err| err.to_string())?;
    config1.server.max_body_bytes = 0;
    if config1.validate().is_ok() {
        return Err("max_body_bytes=0 should be rejected".to_string());
    }

    // max_inflight = 0
    let mut config2 = common::minimal_config().map_err(|err| err.to_string())?;
    config2.server.limits.max_inflight = 0;
    if config2.validate().is_ok() {
        return Err("max_inflight=0 should be rejected".to_string());
    }

    // rate_limit max_requests = 0
    let rate_limit = RateLimitConfig {
        max_requests: 0,
        window_ms: 1000,
        max_entries: 1000,
    };
    let mut config = common::config_with_rate_limit(rate_limit).map_err(|err| err.to_string())?;
    if config.validate().is_ok() {
        return Err("rate_limit max_requests=0 should be rejected".to_string());
    }

    Ok(())
}

#[test]
fn max_versions_zero_rejected() -> TestResult {
    let mut config = common::minimal_config().map_err(|err| err.to_string())?;
    config.run_state_store = RunStateStoreConfig {
        store_type: RunStateStoreType::Sqlite,
        path: Some(PathBuf::from("store.db")),
        busy_timeout_ms: 5000,
        journal_mode: SqliteStoreMode::Wal,
        sync_mode: SqliteSyncMode::Full,
        max_versions: Some(0),
        writer_queue_capacity: 1_024,
        batch_max_ops: 64,
        batch_max_bytes: 512 * 1024,
        batch_max_wait_ms: 2,
        read_pool_size: 4,
    };
    assert_invalid(config.validate(), "run_state_store max_versions must be greater than zero")?;
    Ok(())
}

// ============================================================================
// SECTION: Very Long Strings
// ============================================================================

#[test]
fn bearer_token_exactly_256_bytes() -> TestResult {
    let token = "a".repeat(256);
    let auth = ServerAuthConfig {
        mode: ServerAuthMode::BearerToken,
        bearer_tokens: vec![token],
        mtls_subjects: Vec::new(),
        allowed_tools: Vec::new(),
        principals: Vec::new(),
    };
    let mut config = common::config_with_auth(auth).map_err(|err| err.to_string())?;
    config.validate().map_err(|err| err.to_string())?;
    Ok(())
}

#[test]
fn bearer_token_257_bytes_rejected() -> TestResult {
    let token = "a".repeat(257);
    let auth = ServerAuthConfig {
        mode: ServerAuthMode::BearerToken,
        bearer_tokens: vec![token],
        mtls_subjects: Vec::new(),
        allowed_tools: Vec::new(),
        principals: Vec::new(),
    };
    let mut config = common::config_with_auth(auth).map_err(|err| err.to_string())?;
    assert_invalid(config.validate(), "auth token too long")?;
    Ok(())
}

#[test]
fn mtls_subject_exactly_512_bytes() -> TestResult {
    let subject = "a".repeat(512);
    let auth = ServerAuthConfig {
        mode: ServerAuthMode::Mtls,
        bearer_tokens: Vec::new(),
        mtls_subjects: vec![subject],
        allowed_tools: Vec::new(),
        principals: Vec::new(),
    };
    let mut config = common::config_with_auth(auth).map_err(|err| err.to_string())?;
    config.validate().map_err(|err| err.to_string())?;
    Ok(())
}

#[test]
fn mtls_subject_513_bytes_rejected() -> TestResult {
    let subject = "a".repeat(513);
    let auth = ServerAuthConfig {
        mode: ServerAuthMode::Mtls,
        bearer_tokens: Vec::new(),
        mtls_subjects: vec![subject],
        allowed_tools: Vec::new(),
        principals: Vec::new(),
    };
    let mut config = common::config_with_auth(auth).map_err(|err| err.to_string())?;
    assert_invalid(config.validate(), "mTLS subject too long")?;
    Ok(())
}

// ============================================================================
// SECTION: Empty vs Whitespace
// ============================================================================

#[test]
fn field_empty_string_vs_whitespace_only() -> TestResult {
    // Empty string for bearer token
    let auth1 = ServerAuthConfig {
        mode: ServerAuthMode::BearerToken,
        bearer_tokens: vec![String::new()],
        mtls_subjects: Vec::new(),
        allowed_tools: Vec::new(),
        principals: Vec::new(),
    };
    let mut config1 = common::config_with_auth(auth1).map_err(|err| err.to_string())?;
    if config1.validate().is_ok() {
        return Err("empty bearer token should be rejected".to_string());
    }

    // Whitespace-only for bearer token
    let auth2 = ServerAuthConfig {
        mode: ServerAuthMode::BearerToken,
        bearer_tokens: vec!["   ".to_string()],
        mtls_subjects: Vec::new(),
        allowed_tools: Vec::new(),
        principals: Vec::new(),
    };
    let mut config2 = common::config_with_auth(auth2).map_err(|err| err.to_string())?;
    if config2.validate().is_ok() {
        return Err("whitespace-only bearer token should be rejected".to_string());
    }

    Ok(())
}

#[test]
fn field_unicode_whitespace_u00a0() -> TestResult {
    // U+00A0 is non-breaking space
    let auth = ServerAuthConfig {
        mode: ServerAuthMode::BearerToken,
        bearer_tokens: vec!["token\u{00A0}value".to_string()],
        mtls_subjects: Vec::new(),
        allowed_tools: Vec::new(),
        principals: Vec::new(),
    };
    let mut config = common::config_with_auth(auth).map_err(|err| err.to_string())?;
    if config.validate().is_ok() {
        return Err("unicode non-breaking-space bearer token should be rejected".to_string());
    }
    Ok(())
}

#[test]
fn field_unicode_whitespace_u2000() -> TestResult {
    // U+2000 is en quad
    let auth = ServerAuthConfig {
        mode: ServerAuthMode::BearerToken,
        bearer_tokens: vec!["token\u{2000}value".to_string()],
        mtls_subjects: Vec::new(),
        allowed_tools: Vec::new(),
        principals: Vec::new(),
    };
    let mut config = common::config_with_auth(auth).map_err(|err| err.to_string())?;
    if config.validate().is_ok() {
        return Err("unicode en-quad bearer token should be rejected".to_string());
    }
    Ok(())
}

#[test]
fn field_tab_vs_space_vs_newline() -> TestResult {
    // Tab
    let auth1 = ServerAuthConfig {
        mode: ServerAuthMode::BearerToken,
        bearer_tokens: vec!["token\tvalue".to_string()],
        mtls_subjects: Vec::new(),
        allowed_tools: Vec::new(),
        principals: Vec::new(),
    };
    let mut config1 = common::config_with_auth(auth1).map_err(|err| err.to_string())?;
    if config1.validate().is_ok() {
        return Err("tab-bearing bearer token should be rejected".to_string());
    }

    // Space
    let auth2 = ServerAuthConfig {
        mode: ServerAuthMode::BearerToken,
        bearer_tokens: vec!["token value".to_string()],
        mtls_subjects: Vec::new(),
        allowed_tools: Vec::new(),
        principals: Vec::new(),
    };
    let mut config2 = common::config_with_auth(auth2).map_err(|err| err.to_string())?;
    if config2.validate().is_ok() {
        return Err("space-bearing bearer token should be rejected".to_string());
    }

    // Newline
    let auth3 = ServerAuthConfig {
        mode: ServerAuthMode::BearerToken,
        bearer_tokens: vec!["token\nvalue".to_string()],
        mtls_subjects: Vec::new(),
        allowed_tools: Vec::new(),
        principals: Vec::new(),
    };
    let mut config3 = common::config_with_auth(auth3).map_err(|err| err.to_string())?;
    if config3.validate().is_ok() {
        return Err("newline-bearing bearer token should be rejected".to_string());
    }

    Ok(())
}

// ============================================================================
// SECTION: Array Size Limits
// ============================================================================

#[test]
fn all_arrays_tested_at_max_allowed_size() -> TestResult {
    // bearer_tokens at max (64)
    let tokens: Vec<String> = (0 .. 64).map(|i| format!("token{i}")).collect();
    let auth1 = ServerAuthConfig {
        mode: ServerAuthMode::BearerToken,
        bearer_tokens: tokens,
        mtls_subjects: Vec::new(),
        allowed_tools: Vec::new(),
        principals: Vec::new(),
    };
    let mut config1 = common::config_with_auth(auth1).map_err(|err| err.to_string())?;
    config1.validate().map_err(|err| err.to_string())?;

    // mtls_subjects at max (64)
    let subjects: Vec<String> = (0 .. 64).map(|i| format!("CN=subject{i}")).collect();
    let auth2 = ServerAuthConfig {
        mode: ServerAuthMode::Mtls,
        bearer_tokens: Vec::new(),
        mtls_subjects: subjects,
        allowed_tools: Vec::new(),
        principals: Vec::new(),
    };
    let mut config2 = common::config_with_auth(auth2).map_err(|err| err.to_string())?;
    config2.validate().map_err(|err| err.to_string())?;

    Ok(())
}

#[test]
fn all_arrays_tested_at_max_plus_one() -> TestResult {
    // bearer_tokens at max+1 (65)
    let tokens: Vec<String> = (0 .. 65).map(|i| format!("token{i}")).collect();
    let auth1 = ServerAuthConfig {
        mode: ServerAuthMode::BearerToken,
        bearer_tokens: tokens,
        mtls_subjects: Vec::new(),
        allowed_tools: Vec::new(),
        principals: Vec::new(),
    };
    let mut config1 = common::config_with_auth(auth1).map_err(|err| err.to_string())?;
    if config1.validate().is_ok() {
        return Err("bearer_tokens at 65 should be rejected".to_string());
    }

    // mtls_subjects at max+1 (65)
    let subjects: Vec<String> = (0 .. 65).map(|i| format!("CN=subject{i}")).collect();
    let auth2 = ServerAuthConfig {
        mode: ServerAuthMode::Mtls,
        bearer_tokens: Vec::new(),
        mtls_subjects: subjects,
        allowed_tools: Vec::new(),
        principals: Vec::new(),
    };
    let mut config2 = common::config_with_auth(auth2).map_err(|err| err.to_string())?;
    if config2.validate().is_ok() {
        return Err("mtls_subjects at 65 should be rejected".to_string());
    }

    Ok(())
}

#[test]
fn empty_arrays_where_valid() -> TestResult {
    // Empty bearer_tokens is valid for LocalOnly mode
    let auth = ServerAuthConfig {
        mode: ServerAuthMode::LocalOnly,
        bearer_tokens: Vec::new(),
        mtls_subjects: Vec::new(),
        allowed_tools: Vec::new(),
        principals: Vec::new(),
    };
    let mut config = common::config_with_auth(auth).map_err(|err| err.to_string())?;
    config.validate().map_err(|err| err.to_string())?;
    Ok(())
}

#[test]
fn empty_arrays_where_invalid() -> TestResult {
    // Empty bearer_tokens is invalid for BearerToken mode
    let auth1 = ServerAuthConfig {
        mode: ServerAuthMode::BearerToken,
        bearer_tokens: Vec::new(),
        mtls_subjects: Vec::new(),
        allowed_tools: Vec::new(),
        principals: Vec::new(),
    };
    let mut config1 = common::config_with_auth(auth1).map_err(|err| err.to_string())?;
    if config1.validate().is_ok() {
        return Err("empty bearer_tokens with BearerToken mode should be rejected".to_string());
    }

    // Empty mtls_subjects is invalid for Mtls mode
    let auth2 = ServerAuthConfig {
        mode: ServerAuthMode::Mtls,
        bearer_tokens: Vec::new(),
        mtls_subjects: Vec::new(),
        allowed_tools: Vec::new(),
        principals: Vec::new(),
    };
    let mut config2 = common::config_with_auth(auth2).map_err(|err| err.to_string())?;
    if config2.validate().is_ok() {
        return Err("empty mtls_subjects with Mtls mode should be rejected".to_string());
    }

    Ok(())
}

// ============================================================================
// SECTION: Very Large Numbers
// ============================================================================

#[test]
fn max_body_bytes_very_large() -> TestResult {
    let mut config = common::minimal_config().map_err(|err| err.to_string())?;
    config.server.max_body_bytes = 1_000_000_000; // 1GB
    config.validate().map_err(|err| err.to_string())?;
    Ok(())
}

#[test]
fn max_inflight_very_large() -> TestResult {
    let mut config = common::minimal_config().map_err(|err| err.to_string())?;
    config.server.limits.max_inflight = 1_000_000;
    config.validate().map_err(|err| err.to_string())?;
    Ok(())
}

// ============================================================================
// SECTION: Boundary Interactions
// ============================================================================

#[test]
fn rate_limit_max_requests_equals_max_entries() -> TestResult {
    let rate_limit = RateLimitConfig {
        max_requests: 1000,
        window_ms: 1000,
        max_entries: 1000,
    };
    let mut config = common::config_with_rate_limit(rate_limit).map_err(|err| err.to_string())?;
    config.validate().map_err(|err| err.to_string())?;
    Ok(())
}

#[test]
fn rate_limit_max_requests_exceeds_max_entries() -> TestResult {
    let rate_limit = RateLimitConfig {
        max_requests: 10_000,
        window_ms: 1000,
        max_entries: 1000,
    };
    // This is allowed - max_entries limits the tracking, not the request count
    let mut config = common::config_with_rate_limit(rate_limit).map_err(|err| err.to_string())?;
    config.validate().map_err(|err| err.to_string())?;
    Ok(())
}
