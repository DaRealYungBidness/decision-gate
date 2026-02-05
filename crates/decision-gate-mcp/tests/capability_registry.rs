// crates/decision-gate-mcp/tests/capability_registry.rs
// ============================================================================
// Module: Capability Registry Tests
// Description: Validate capability contract loading and enforcement.
// Purpose: Ensure provider contracts are strict, deterministic, and safe.
// Dependencies: decision-gate-contract, decision-gate-mcp
// ============================================================================

//! ## Overview
//! Exercises capability registry validation for provider contracts, comparator
//! allowlists, and schema constraints.
//!
//! Security posture: tests enforce strict contract validation and fail-closed
//! behavior for malformed inputs; see `Docs/security/threat_model.md`.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::err_expect,
    reason = "Test-only fixtures use unwraps for clarity."
)]

mod common;

use std::path::Path;
use std::path::PathBuf;

use decision_gate_contract::types::CheckContract;
use decision_gate_contract::types::CheckExample;
use decision_gate_contract::types::DeterminismClass;
use decision_gate_contract::types::ProviderContract;
use decision_gate_core::Comparator;
use decision_gate_core::EvidenceQuery;
use decision_gate_core::ProviderId;
use decision_gate_core::TenantId;
use decision_gate_mcp::capabilities::CapabilityError;
use decision_gate_mcp::capabilities::CapabilityRegistry;
use decision_gate_mcp::config::AnchorPolicyConfig;
use decision_gate_mcp::config::DecisionGateConfig;
use decision_gate_mcp::config::DocsConfig;
use decision_gate_mcp::config::EvidencePolicyConfig;
use decision_gate_mcp::config::PolicyConfig;
use decision_gate_mcp::config::ProviderConfig;
use decision_gate_mcp::config::ProviderTimeoutConfig;
use decision_gate_mcp::config::ProviderType;
use decision_gate_mcp::config::RunStateStoreConfig;
use decision_gate_mcp::config::SchemaRegistryConfig;
use decision_gate_mcp::config::ServerConfig;
use decision_gate_mcp::config::TrustConfig;
use decision_gate_mcp::config::ValidationConfig;
use serde_json::json;
use tempfile::TempDir;

fn base_config() -> DecisionGateConfig {
    DecisionGateConfig {
        server: ServerConfig::default(),
        namespace: decision_gate_mcp::config::NamespaceConfig {
            allow_default: true,
            default_tenants: vec![TenantId::from_raw(100).expect("nonzero tenantid")],
            ..decision_gate_mcp::config::NamespaceConfig::default()
        },
        trust: TrustConfig::default(),
        evidence: EvidencePolicyConfig::default(),
        anchors: AnchorPolicyConfig::default(),
        provider_discovery: decision_gate_mcp::config::ProviderDiscoveryConfig::default(),
        validation: ValidationConfig::default(),
        policy: PolicyConfig::default(),
        run_state_store: RunStateStoreConfig::default(),
        schema_registry: SchemaRegistryConfig::default(),
        providers: Vec::new(),
        dev: decision_gate_mcp::config::DevConfig::default(),
        docs: DocsConfig::default(),
        runpack_storage: None,

        source_modified_at: None,
    }
}

fn mcp_provider(name: &str, path: &Path) -> ProviderConfig {
    ProviderConfig {
        name: name.to_string(),
        provider_type: ProviderType::Mcp,
        command: vec!["provider".to_string()],
        url: None,
        allow_insecure_http: false,
        capabilities_path: Some(PathBuf::from(path)),
        auth: None,
        trust: None,
        allow_raw: false,
        timeouts: ProviderTimeoutConfig::default(),
        config: None,
    }
}

fn builtin_provider(name: &str) -> ProviderConfig {
    ProviderConfig {
        name: name.to_string(),
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
    }
}

fn write_contract(path: &Path, contract: &ProviderContract) -> Result<(), String> {
    let bytes = serde_json::to_vec(contract).map_err(|err| err.to_string())?;
    std::fs::write(path, bytes).map_err(|err| err.to_string())
}

fn base_contract(provider_id: &str) -> ProviderContract {
    ProviderContract {
        provider_id: provider_id.to_string(),
        name: "Echo Provider".to_string(),
        description: "Echo check used for registry validation.".to_string(),
        transport: "mcp".to_string(),
        config_schema: json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {}
        }),
        checks: vec![CheckContract {
            check_id: "echo".to_string(),
            description: "Return the provided boolean value.".to_string(),
            determinism: DeterminismClass::External,
            params_required: true,
            params_schema: json!({
                "type": "object",
                "required": ["value"],
                "properties": {
                    "value": { "type": "boolean" }
                },
                "additionalProperties": false
            }),
            result_schema: json!({ "type": "boolean" }),
            allowed_comparators: vec![
                Comparator::Equals,
                Comparator::NotEquals,
                Comparator::Exists,
                Comparator::NotExists,
            ],
            anchor_types: vec![String::from("stub")],
            content_types: vec![String::from("application/json")],
            examples: vec![CheckExample {
                description: "Echo true.".to_string(),
                params: json!({ "value": true }),
                result: json!(true),
            }],
        }],
        notes: Vec::new(),
    }
}

#[test]
fn registry_rejects_duplicate_provider_ids() {
    let mut config = base_config();
    config.providers = vec![builtin_provider("time"), builtin_provider("time")];
    let result = CapabilityRegistry::from_config(&config);
    let err = result.err().expect("expected duplicate provider rejection");
    assert!(matches!(err, CapabilityError::DuplicateProvider { .. }));
}

#[test]
fn registry_rejects_provider_id_mismatch() {
    let temp = TempDir::new().unwrap();
    let contract_path = temp.path().join("provider.json");
    let contract = base_contract("other");
    write_contract(&contract_path, &contract).unwrap();

    let mut config = base_config();
    config.providers = vec![mcp_provider("echo", &contract_path)];
    let result = CapabilityRegistry::from_config(&config);
    let err = result.err().expect("expected provider id mismatch");
    assert!(matches!(err, CapabilityError::ContractInvalid { .. }));
    assert!(err.to_string().contains("provider id mismatch"));
}

#[test]
fn registry_rejects_transport_mismatch() {
    let temp = TempDir::new().unwrap();
    let contract_path = temp.path().join("provider.json");
    let mut contract = base_contract("echo");
    contract.transport = "builtin".to_string();
    write_contract(&contract_path, &contract).unwrap();

    let mut config = base_config();
    config.providers = vec![mcp_provider("echo", &contract_path)];
    let result = CapabilityRegistry::from_config(&config);
    let err = result.err().expect("expected transport mismatch");
    assert!(matches!(err, CapabilityError::ContractInvalid { .. }));
    assert!(err.to_string().contains("transport=mcp"));
}

#[test]
fn registry_rejects_empty_comparator_allowlist() {
    let temp = TempDir::new().unwrap();
    let contract_path = temp.path().join("provider.json");
    let mut contract = base_contract("echo");
    contract.checks[0].allowed_comparators = Vec::new();
    write_contract(&contract_path, &contract).unwrap();

    let mut config = base_config();
    config.providers = vec![mcp_provider("echo", &contract_path)];
    let result = CapabilityRegistry::from_config(&config);
    let err = result.err().expect("expected empty allow-list rejection");
    assert!(matches!(err, CapabilityError::ContractInvalid { .. }));
}

#[test]
fn registry_rejects_non_canonical_comparator_order() {
    let temp = TempDir::new().unwrap();
    let contract_path = temp.path().join("provider.json");
    let mut contract = base_contract("echo");
    contract.checks[0].allowed_comparators = vec![Comparator::NotEquals, Comparator::Equals];
    write_contract(&contract_path, &contract).unwrap();

    let mut config = base_config();
    config.providers = vec![mcp_provider("echo", &contract_path)];
    let result = CapabilityRegistry::from_config(&config);
    let err = result.err().expect("expected non-canonical comparator order rejection");
    assert!(matches!(err, CapabilityError::ContractInvalid { .. }));
}

#[test]
fn registry_rejects_invalid_schema() {
    let temp = TempDir::new().unwrap();
    let contract_path = temp.path().join("provider.json");
    let mut contract = base_contract("echo");
    contract.checks[0].params_schema = json!({ "type": 5 });
    write_contract(&contract_path, &contract).unwrap();

    let mut config = base_config();
    config.providers = vec![mcp_provider("echo", &contract_path)];
    let result = CapabilityRegistry::from_config(&config);
    let err = result.err().expect("expected schema compile rejection");
    assert!(matches!(err, CapabilityError::SchemaCompile { .. }));
}

#[test]
fn registry_rejects_overlong_path_component() {
    let mut config = base_config();
    let too_long = "a".repeat(256);
    config.providers = vec![mcp_provider("echo", Path::new(&too_long))];
    let result = CapabilityRegistry::from_config(&config);
    let err = result.err().expect("expected path component rejection");
    assert!(matches!(err, CapabilityError::ContractPathInvalid { .. }));
}

#[test]
fn registry_rejects_overlong_total_path() {
    let mut config = base_config();
    let too_long = "a".repeat(4097);
    config.providers = vec![mcp_provider("echo", Path::new(&too_long))];
    let result = CapabilityRegistry::from_config(&config);
    let err = result.err().expect("expected path length rejection");
    assert!(matches!(err, CapabilityError::ContractPathInvalid { .. }));
}

#[test]
fn registry_rejects_oversized_contract() {
    let temp = TempDir::new().unwrap();
    let contract_path = temp.path().join("provider.json");
    let bytes = vec![0u8; 1_048_577];
    std::fs::write(&contract_path, bytes).unwrap();

    let mut config = base_config();
    config.providers = vec![mcp_provider("echo", &contract_path)];
    let result = CapabilityRegistry::from_config(&config);
    let err = result.err().expect("expected oversized contract rejection");
    assert!(matches!(err, CapabilityError::ContractInvalid { .. }));
}

#[test]
fn validate_spec_rejects_missing_provider() {
    let config = base_config();
    let registry = CapabilityRegistry::from_config(&config).unwrap();
    let spec = common::sample_spec();
    let result = registry.validate_spec(&spec);
    let err = result.err().expect("expected missing provider rejection");
    assert!(matches!(err, CapabilityError::ProviderMissing { .. }));
}

#[test]
fn validate_query_rejects_optional_params_when_invalid() {
    let config = common::sample_config();
    let registry = CapabilityRegistry::from_config(&config).unwrap();
    let query = EvidenceQuery {
        provider_id: ProviderId::new("time"),
        check_id: "now".to_string(),
        params: Some(json!({ "unexpected": true })),
    };
    let result = registry.validate_query(&query);
    let err = result.err().expect("expected params invalid");
    assert!(matches!(err, CapabilityError::ParamsInvalid { .. }));
}

#[test]
fn validate_spec_rejects_expected_schema_mismatch_non_boolean() {
    let config = common::sample_config();
    let registry = CapabilityRegistry::from_config(&config).unwrap();
    let mut spec = common::sample_spec();
    spec.conditions[0].query.provider_id = ProviderId::new("env");
    spec.conditions[0].query.check_id = "get".to_string();
    spec.conditions[0].query.params = Some(json!({ "key": "PATH" }));
    spec.conditions[0].expected = Some(json!(123));

    let result = registry.validate_spec(&spec);
    let err = result.err().expect("expected value schema mismatch");
    assert!(matches!(err, CapabilityError::ExpectedInvalid { .. }));
}
