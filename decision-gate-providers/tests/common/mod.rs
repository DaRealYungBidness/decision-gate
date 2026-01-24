// decision-gate-providers/tests/common/mod.rs
// ============================================================================
// Module: Common Test Fixtures
// Description: Shared test utilities and fixtures for provider tests.
// Purpose: Provide reusable test infrastructure for deterministic testing.
// Dependencies: decision-gate-core
// ============================================================================

//! ## Overview
//! This module provides shared test fixtures, helper functions, and adversarial
//! input generators for use across all provider test files.
//!
//! Security posture: Test fixtures are designed to exercise trust boundaries
//! and validate fail-closed behavior under adversarial conditions.

#![allow(dead_code, reason = "Shared test helpers may be unused in some cases.")]

// ============================================================================
// SECTION: Imports
// ============================================================================

use decision_gate_core::EvidenceContext;
use decision_gate_core::NamespaceId;
use decision_gate_core::RunId;
use decision_gate_core::ScenarioId;
use decision_gate_core::StageId;
use decision_gate_core::TenantId;
use decision_gate_core::Timestamp;
use decision_gate_core::TriggerId;

// ============================================================================
// SECTION: Test Fixtures
// ============================================================================

/// Creates a deterministic evidence context for testing.
///
/// Uses logical timestamp by default for reproducibility.
#[must_use]
pub fn sample_context() -> EvidenceContext {
    sample_context_with_time(Timestamp::Logical(1))
}

/// Creates a deterministic evidence context with a specific trigger time.
#[must_use]
pub fn sample_context_with_time(trigger_time: Timestamp) -> EvidenceContext {
    EvidenceContext {
        tenant_id: TenantId::new("test-tenant"),
        namespace_id: NamespaceId::new("default"),
        run_id: RunId::new("test-run"),
        scenario_id: ScenarioId::new("test-scenario"),
        stage_id: StageId::new("test-stage"),
        trigger_id: TriggerId::new("test-trigger"),
        trigger_time,
        correlation_id: None,
    }
}

/// Creates a context with Unix milliseconds timestamp.
#[must_use]
pub fn sample_context_unix_millis(millis: i64) -> EvidenceContext {
    sample_context_with_time(Timestamp::UnixMillis(millis))
}

// ============================================================================
// SECTION: Adversarial Input Generators
// ============================================================================

/// Returns a collection of path traversal attack vectors.
///
/// These vectors attempt to escape a configured root directory using various
/// encoding and traversal techniques. Tests should verify all vectors are
/// rejected when a root is configured.
///
/// Threat model: TM-FILE-001 - File system traversal via untrusted input.
#[must_use]
pub fn path_traversal_vectors() -> Vec<&'static str> {
    vec![
        // Basic traversal patterns
        "../etc/passwd",
        "../../etc/passwd",
        "../../../etc/passwd",
        "../../../../etc/passwd",
        // Windows-style traversal
        "..\\etc\\passwd",
        "..\\..\\etc\\passwd",
        // Mixed separators
        "../..\\etc/passwd",
        "..\\../etc\\passwd",
        // Absolute paths that should be rejected or normalized
        "/etc/passwd",
        "C:\\Windows\\System32\\config\\sam",
        // Double-encoded traversal (URL encoding)
        "%2e%2e%2f%2e%2e%2fetc%2fpasswd",
        "%2e%2e/%2e%2e/etc/passwd",
        // Null byte injection attempts
        "../etc/passwd%00.json",
        "../etc/passwd\x00.json",
        // Unicode normalization attacks
        "..%c0%af..%c0%afetc%c0%afpasswd",
        // Dot-dot-slash with extra dots
        "....//....//etc/passwd",
        "..../..../etc/passwd",
        // Trailing slash variations
        "../etc/passwd/",
        "../etc/passwd/.",
        // Hidden directory traversal
        "./../etc/passwd",
        "./../../etc/passwd",
    ]
}

/// Returns a collection of oversized input strings for resource exhaustion tests.
///
/// Tests should verify that inputs exceeding configured limits are rejected.
#[must_use]
pub fn oversized_string(size: usize) -> String {
    "A".repeat(size)
}

/// Returns a collection of sensitive environment variable names that should be
/// blocked by default denylist or security policy.
///
/// Threat model: TM-ENV-001 - Information disclosure via environment variables.
#[must_use]
pub fn sensitive_env_keys() -> Vec<&'static str> {
    vec![
        // AWS credentials
        "AWS_ACCESS_KEY_ID",
        "AWS_SECRET_ACCESS_KEY",
        "AWS_SESSION_TOKEN",
        // GitHub tokens
        "GITHUB_TOKEN",
        "GH_TOKEN",
        "GITHUB_PAT",
        // Generic secrets
        "SECRET_KEY",
        "API_KEY",
        "PRIVATE_KEY",
        "PASSWORD",
        "DB_PASSWORD",
        // Cloud provider credentials
        "AZURE_CLIENT_SECRET",
        "GOOGLE_APPLICATION_CREDENTIALS",
        // SSH keys
        "SSH_PRIVATE_KEY",
        // Database connection strings
        "DATABASE_URL",
        "REDIS_URL",
    ]
}

/// Returns invalid URL schemes for HTTP provider testing.
///
/// Tests should verify that non-HTTPS schemes are rejected by default.
#[must_use]
pub fn invalid_url_schemes() -> Vec<&'static str> {
    vec![
        "ftp://example.com/file",
        "file:///etc/passwd",
        "gopher://example.com/",
        "data:text/plain,hello",
        "javascript:alert(1)",
        "mailto:test@example.com",
    ]
}

/// Returns SSRF attack vectors targeting internal/private networks.
///
/// Threat model: TM-HTTP-001 - Server-side request forgery via HTTP provider.
#[must_use]
pub fn ssrf_vectors() -> Vec<&'static str> {
    vec![
        // Localhost variations
        "http://localhost/",
        "http://127.0.0.1/",
        "http://127.0.0.2/",
        "http://[::1]/",
        "http://0.0.0.0/",
        // Private networks (RFC 1918)
        "http://10.0.0.1/",
        "http://10.255.255.255/",
        "http://172.16.0.1/",
        "http://172.31.255.255/",
        "http://192.168.0.1/",
        "http://192.168.255.255/",
        // Link-local
        "http://169.254.169.254/latest/meta-data/", // AWS metadata
        "http://169.254.0.1/",
        // IPv6 private
        "http://[fe80::1]/",
        "http://[fc00::1]/",
        // Decimal/octal IP encoding
        "http://2130706433/", // 127.0.0.1 in decimal
        "http://0177.0.0.1/", // 127.0.0.1 in octal
        // URL with credentials (should be rejected)
        "http://user:pass@example.com/",
    ]
}

// ============================================================================
// SECTION: Test Helper Macros
// ============================================================================

/// Asserts that an evidence error contains the expected substring.
#[macro_export]
macro_rules! assert_evidence_error_contains {
    ($result:expr, $expected:expr) => {
        match $result {
            Err(decision_gate_core::EvidenceError::Provider(msg)) => {
                assert!(
                    msg.contains($expected),
                    "Error message '{}' does not contain '{}'",
                    msg,
                    $expected
                );
            }
            Err(other) => panic!("Expected Provider error, got: {:?}", other),
            Ok(value) => panic!("Expected error, got Ok: {:?}", value),
        }
    };
}

/// Asserts that an evidence result has no value (missing evidence).
#[macro_export]
macro_rules! assert_evidence_none {
    ($result:expr) => {
        match $result {
            Ok(result) => {
                assert!(result.value.is_none(), "Expected no value, got: {:?}", result.value);
            }
            Err(e) => panic!("Expected Ok with no value, got error: {:?}", e),
        }
    };
}
