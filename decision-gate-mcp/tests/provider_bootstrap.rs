//! Provider bootstrap tests for decision-gate-mcp.
// decision-gate-mcp/tests/provider_bootstrap.rs
// =============================================================================
// Module: Provider Bootstrap Tests
// Description: Validate provider registry hardening during evidence bootstrap.
// Purpose: Ensure duplicate provider registration fails closed.
// =============================================================================

#![allow(
    clippy::panic,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::unwrap_in_result,
    reason = "Test-only setup assertions."
)]

use decision_gate_core::EvidenceError;
use decision_gate_mcp::DecisionGateConfig;
use decision_gate_mcp::FederatedEvidenceProvider;
use decision_gate_mcp::config::ProviderConfig;
use decision_gate_mcp::config::ProviderTimeoutConfig;
use decision_gate_mcp::config::ProviderType;

#[test]
fn federated_provider_rejects_duplicate_registration() {
    let mut config: DecisionGateConfig = toml::from_str("").expect("default config");
    let provider = ProviderConfig {
        name: "time".to_string(),
        provider_type: ProviderType::Builtin,
        command: Vec::new(),
        url: None,
        allow_insecure_http: false,
        capabilities_path: None,
        auth: None,
        trust: None,
        allow_raw: false,
        timeouts: ProviderTimeoutConfig::default(),
        config: None,
    };
    config.providers = vec![provider.clone(), provider];

    let Err(err) = FederatedEvidenceProvider::from_config(&config) else {
        panic!("expected duplicate provider registration failure");
    };
    let EvidenceError::Provider(message) = err;
    assert!(
        message.contains("already registered"),
        "error should mention duplicate registration: {message}"
    );
}
