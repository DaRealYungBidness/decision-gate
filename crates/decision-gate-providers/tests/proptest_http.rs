//! HTTP provider property-based tests.
//!
//! ## Purpose
//! These tests exercise URL validation and policy enforcement using randomized inputs.
//! They are designed to prove fail-closed behavior and panic safety under adversarial
//! URL strings without relying on network access.
//!
//! ## Threat model
//! - TM-HTTP-001 (SSRF): untrusted URLs must be blocked by scheme/allowlist rules.
//! - TM-HTTP-003 (resource exhaustion): malformed URLs must not trigger panics.
//!
//! ## What is covered
//! - Invalid URL strings are rejected.
//! - Unlisted hosts are rejected when an allowlist is configured.
//!
//! ## What is intentionally out of scope
//! - TLS handshake behavior (covered by dedicated TLS guardrail tests).
//! - Network error classification (requires integration fixtures).
// crates/decision-gate-providers/tests/proptest_http.rs
// ============================================================================
// Module: HTTP Provider Property-Based Tests
// Description: Fuzz-like checks for URL parsing and policy enforcement.
// Purpose: Ensure invalid or disallowed URLs fail closed without panics.
// ============================================================================

#![allow(
    clippy::panic,
    clippy::print_stdout,
    clippy::print_stderr,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::use_debug,
    clippy::dbg_macro,
    clippy::panic_in_result_fn,
    clippy::unwrap_in_result,
    reason = "Test-only assertions and helpers are permitted."
)]

use std::collections::BTreeSet;

use decision_gate_core::EvidenceProvider;
use decision_gate_core::EvidenceQuery;
use decision_gate_core::ProviderId;
use decision_gate_providers::HttpProvider;
use decision_gate_providers::HttpProviderConfig;
use proptest::prelude::*;
use serde_json::json;

mod common;
use crate::common::sample_context;

fn blocked_provider() -> HttpProvider {
    let allowlist: BTreeSet<String> = BTreeSet::new();
    HttpProvider::new(HttpProviderConfig {
        allow_http: true,
        allowed_hosts: Some(allowlist),
        timeout_ms: 50,
        ..HttpProviderConfig::default()
    })
    .unwrap()
}

proptest! {
    #[test]
    fn http_provider_rejects_unlisted_hosts(host in "[a-z0-9.-]{1,32}") {
        let provider = blocked_provider();
        let url = format!("http://{host}/path");
        let query = EvidenceQuery {
            provider_id: ProviderId::new("http"),
            check_id: "status".to_string(),
            params: Some(json!({"url": url})),
        };
        let result = provider.query(&query, &sample_context());
        prop_assert!(result.is_err());
    }

    #[test]
    fn http_provider_rejects_invalid_urls(raw in ".{1,64}") {
        let provider = blocked_provider();
        let query = EvidenceQuery {
            provider_id: ProviderId::new("http"),
            check_id: "status".to_string(),
            params: Some(json!({"url": raw})),
        };
        let result = provider.query(&query, &sample_context());
        prop_assert!(result.is_err());
    }
}
