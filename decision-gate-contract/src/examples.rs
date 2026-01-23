// decision-gate-contract/src/examples.rs
// ============================================================================
// Module: Contract Examples
// Description: Canonical example payloads for scenarios and configuration.
// Purpose: Provide deterministic, real-world examples for docs and SDKs.
// Dependencies: decision-gate-core, ret-logic, serde_json
// ============================================================================

//! ## Overview
//! This module constructs example payloads from the real core types. The output
//! is serialized using canonical JSON to ensure format accuracy and to prevent
//! divergence between docs and runtime expectations.
//! Security posture: examples are static templates; see
//! `Docs/security/threat_model.md`.

// ============================================================================
// SECTION: Imports
// ============================================================================

use decision_gate_core::AdvanceTo;
use decision_gate_core::Comparator;
use decision_gate_core::EvidenceQuery;
use decision_gate_core::GateSpec;
use decision_gate_core::PacketPayload;
use decision_gate_core::PacketSpec;
use decision_gate_core::PredicateSpec;
use decision_gate_core::RunConfig;
use decision_gate_core::ScenarioSpec;
use decision_gate_core::StageSpec;
use decision_gate_core::TimeoutPolicy;
use decision_gate_core::disclosure::DispatchTarget;
use decision_gate_core::identifiers::GateId;
use decision_gate_core::identifiers::PacketId;
use decision_gate_core::identifiers::PredicateKey;
use decision_gate_core::identifiers::ProviderId;
use decision_gate_core::identifiers::RunId;
use decision_gate_core::identifiers::ScenarioId;
use decision_gate_core::identifiers::SchemaId;
use decision_gate_core::identifiers::SpecVersion;
use decision_gate_core::identifiers::StageId;
use decision_gate_core::identifiers::TenantId;
use ret_logic::Requirement;
use ron::ser::PrettyConfig;
use serde_json::Value;
use serde_json::json;

// ============================================================================
// SECTION: Example Builders
// ============================================================================

/// Returns a canonical example scenario spec.
#[must_use]
pub fn scenario_example() -> ScenarioSpec {
    ScenarioSpec {
        scenario_id: ScenarioId::from("example-scenario"),
        spec_version: SpecVersion::from("v1"),
        stages: vec![example_stage()],
        predicates: vec![env_predicate_example(), time_predicate_example()],
        policies: Vec::new(),
        schemas: Vec::new(),
        default_tenant_id: None,
    }
}

/// Returns a RON authoring example for the canonical scenario spec.
///
/// # Errors
///
/// Returns a RON serialization error when the example cannot be rendered.
#[must_use = "use the rendered RON example or handle the error"]
pub fn scenario_example_ron() -> Result<String, ron::Error> {
    let value = serde_json::to_value(scenario_example())
        .map_err(|err| ron::Error::Message(err.to_string()))?;
    let pretty = PrettyConfig::new().depth_limit(6).separate_tuple_members(true);
    ron::ser::to_string_pretty(&value, pretty)
}

/// Returns a canonical example run configuration.
#[must_use]
pub fn run_config_example() -> RunConfig {
    RunConfig {
        tenant_id: TenantId::from("tenant-001"),
        run_id: RunId::from("run-0001"),
        scenario_id: ScenarioId::from("example-scenario"),
        dispatch_targets: vec![DispatchTarget::Agent {
            agent_id: String::from("agent-alpha"),
        }],
        policy_tags: Vec::new(),
    }
}

/// Returns a canonical example `decision-gate.toml` configuration.
#[must_use]
pub fn config_toml_example() -> String {
    String::from(
        r#"[server]
transport = "stdio"
max_body_bytes = 1048576

[trust]
default_policy = "audit"

[evidence]
allow_raw_values = false
require_provider_opt_in = true

[[providers]]
name = "time"
type = "builtin"

[[providers]]
name = "env"
type = "builtin"

[[providers]]
name = "json"
type = "builtin"
config = { root = "/etc/decision-gate", max_bytes = 1048576, allow_yaml = true }

[[providers]]
name = "http"
type = "builtin"
config = { allow_http = false, timeout_ms = 5000, max_response_bytes = 1048576, allowed_hosts = ["api.example.com"], user_agent = "decision-gate/0.1", hash_algorithm = "sha256" }
"#,
    )
}

// ============================================================================
// SECTION: Example Helpers
// ============================================================================

/// Builds the example stage for the scenario.
#[must_use]
fn example_stage() -> StageSpec {
    StageSpec {
        stage_id: StageId::from("main"),
        entry_packets: vec![example_packet()],
        gates: vec![env_gate_example(), time_gate_example()],
        advance_to: AdvanceTo::Terminal,
        timeout: None,
        on_timeout: TimeoutPolicy::Fail,
    }
}

/// Builds a gate that references the env predicate.
#[must_use]
fn env_gate_example() -> GateSpec {
    GateSpec {
        gate_id: GateId::from("env_gate"),
        requirement: Requirement::Predicate(PredicateKey::from("env_is_prod")),
    }
}

/// Builds a gate that references the time predicate.
#[must_use]
fn time_gate_example() -> GateSpec {
    GateSpec {
        gate_id: GateId::from("time_gate"),
        requirement: Requirement::Predicate(PredicateKey::from("after_freeze")),
    }
}

/// Builds the example packet disclosed on entry.
#[must_use]
fn example_packet() -> PacketSpec {
    PacketSpec {
        packet_id: PacketId::from("packet-hello"),
        schema_id: SchemaId::from("schema-hello"),
        content_type: String::from("application/json"),
        visibility_labels: vec![String::from("public")],
        policy_tags: Vec::new(),
        expiry: None,
        payload: PacketPayload::Json {
            value: json!({
                "message": "hello",
                "purpose": "scenario entry packet"
            }),
        },
    }
}

/// Builds the environment predicate example.
#[must_use]
fn env_predicate_example() -> PredicateSpec {
    PredicateSpec {
        predicate: PredicateKey::from("env_is_prod"),
        query: EvidenceQuery {
            provider_id: ProviderId::from("env"),
            predicate: String::from("get"),
            params: Some(json!({ "key": "DEPLOY_ENV" })),
        },
        comparator: Comparator::Equals,
        expected: Some(Value::String(String::from("production"))),
        policy_tags: Vec::new(),
    }
}

/// Builds the time predicate example.
#[must_use]
fn time_predicate_example() -> PredicateSpec {
    PredicateSpec {
        predicate: PredicateKey::from("after_freeze"),
        query: EvidenceQuery {
            provider_id: ProviderId::from("time"),
            predicate: String::from("after"),
            params: Some(json!({ "timestamp": 1_710_000_000_000_i64 })),
        },
        comparator: Comparator::Equals,
        expected: Some(Value::Bool(true)),
        policy_tags: Vec::new(),
    }
}
