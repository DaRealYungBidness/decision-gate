// decision-gate-mcp/tests/policy_engine.rs
// ============================================================================
// Module: Policy Engine Tests
// Description: Tests for static policy rule evaluation.
// Purpose: Verify static policy matches and fails closed.
// Dependencies: decision-gate-mcp, decision-gate-core
// ============================================================================

//! Policy engine tests for deterministic static rules.

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
    reason = "Test-only output and panic-based assertions are permitted."
)]

use decision_gate_core::DispatchTarget;
use decision_gate_core::PacketEnvelope;
use decision_gate_core::PacketId;
use decision_gate_core::PacketPayload;
use decision_gate_core::PolicyDecider;
use decision_gate_core::PolicyDecision;
use decision_gate_core::RunId;
use decision_gate_core::ScenarioId;
use decision_gate_core::SchemaId;
use decision_gate_core::StageId;
use decision_gate_core::Timestamp;
use decision_gate_core::hashing::DEFAULT_HASH_ALGORITHM;
use decision_gate_core::hashing::hash_bytes;
use decision_gate_mcp::policy::DispatchPolicy;
use decision_gate_mcp::policy::DispatchTargetKind;
use decision_gate_mcp::policy::PolicyEffect;
use decision_gate_mcp::policy::PolicyRule;
use decision_gate_mcp::policy::PolicyTargetSelector;
use decision_gate_mcp::policy::StaticPolicyConfig;

fn sample_envelope(labels: Vec<&str>, tags: Vec<&str>) -> PacketEnvelope {
    sample_envelope_with_content_type(labels, tags, "application/json")
}

fn sample_envelope_with_content_type(
    labels: Vec<&str>,
    tags: Vec<&str>,
    content_type: &str,
) -> PacketEnvelope {
    PacketEnvelope {
        scenario_id: ScenarioId::new("scenario"),
        run_id: RunId::new("run-1"),
        stage_id: StageId::new("stage-1"),
        packet_id: PacketId::new("packet-1"),
        schema_id: SchemaId::new("schema-1"),
        content_type: content_type.to_string(),
        content_hash: hash_bytes(DEFAULT_HASH_ALGORITHM, b"payload"),
        visibility: decision_gate_core::VisibilityPolicy::new(
            labels.into_iter().map(ToString::to_string).collect(),
            tags.into_iter().map(ToString::to_string).collect(),
        ),
        expiry: None,
        correlation_id: None,
        issued_at: Timestamp::Logical(1),
    }
}

#[test]
fn static_policy_permits_matching_rule() {
    let policy = DispatchPolicy::Static(StaticPolicyConfig {
        default: PolicyEffect::Deny,
        rules: vec![PolicyRule {
            effect: PolicyEffect::Permit,
            error_message: None,
            target_kinds: Vec::new(),
            targets: Vec::new(),
            require_labels: vec!["public".to_string()],
            forbid_labels: Vec::new(),
            require_policy_tags: Vec::new(),
            forbid_policy_tags: Vec::new(),
            content_types: Vec::new(),
            schema_ids: Vec::new(),
            packet_ids: Vec::new(),
            stage_ids: Vec::new(),
            scenario_ids: Vec::new(),
        }],
    });

    let target = DispatchTarget::Agent {
        agent_id: "agent-1".to_string(),
    };
    let envelope = sample_envelope(vec!["public"], vec![]);
    let payload = PacketPayload::Json {
        value: serde_json::json!({"hello": "world"}),
    };

    let decision = policy.authorize(&target, &envelope, &payload).expect("policy decision");
    assert_eq!(decision, PolicyDecision::Permit);
}

#[test]
fn static_policy_denies_matching_rule() {
    let policy = DispatchPolicy::Static(StaticPolicyConfig {
        default: PolicyEffect::Permit,
        rules: vec![PolicyRule {
            effect: PolicyEffect::Deny,
            error_message: None,
            target_kinds: Vec::new(),
            targets: Vec::new(),
            require_labels: vec!["internal".to_string()],
            forbid_labels: Vec::new(),
            require_policy_tags: Vec::new(),
            forbid_policy_tags: Vec::new(),
            content_types: Vec::new(),
            schema_ids: Vec::new(),
            packet_ids: Vec::new(),
            stage_ids: Vec::new(),
            scenario_ids: Vec::new(),
        }],
    });

    let target = DispatchTarget::Agent {
        agent_id: "agent-1".to_string(),
    };
    let envelope = sample_envelope(vec!["internal"], vec![]);
    let payload = PacketPayload::Json {
        value: serde_json::json!({"hello": "world"}),
    };

    let decision = policy.authorize(&target, &envelope, &payload).expect("policy decision");
    assert_eq!(decision, PolicyDecision::Deny);
}

#[test]
fn static_policy_errors_when_rule_matches() {
    let policy = DispatchPolicy::Static(StaticPolicyConfig {
        default: PolicyEffect::Permit,
        rules: vec![PolicyRule {
            effect: PolicyEffect::Error,
            error_message: Some("policy error".to_string()),
            target_kinds: Vec::new(),
            targets: Vec::new(),
            require_labels: vec!["restricted".to_string()],
            forbid_labels: Vec::new(),
            require_policy_tags: Vec::new(),
            forbid_policy_tags: Vec::new(),
            content_types: Vec::new(),
            schema_ids: Vec::new(),
            packet_ids: Vec::new(),
            stage_ids: Vec::new(),
            scenario_ids: Vec::new(),
        }],
    });

    let target = DispatchTarget::Agent {
        agent_id: "agent-1".to_string(),
    };
    let envelope = sample_envelope(vec!["restricted"], vec![]);
    let payload = PacketPayload::Json {
        value: serde_json::json!({"hello": "world"}),
    };

    let error = policy.authorize(&target, &envelope, &payload).expect_err("policy error expected");
    assert!(error.to_string().contains("policy error"));
}

#[test]
fn static_policy_first_match_wins() {
    let policy = DispatchPolicy::Static(StaticPolicyConfig {
        default: PolicyEffect::Permit,
        rules: vec![
            PolicyRule {
                effect: PolicyEffect::Deny,
                error_message: None,
                target_kinds: Vec::new(),
                targets: Vec::new(),
                require_labels: vec!["public".to_string()],
                forbid_labels: Vec::new(),
                require_policy_tags: Vec::new(),
                forbid_policy_tags: Vec::new(),
                content_types: Vec::new(),
                schema_ids: Vec::new(),
                packet_ids: Vec::new(),
                stage_ids: Vec::new(),
                scenario_ids: Vec::new(),
            },
            PolicyRule {
                effect: PolicyEffect::Permit,
                error_message: None,
                target_kinds: Vec::new(),
                targets: Vec::new(),
                require_labels: vec!["public".to_string()],
                forbid_labels: Vec::new(),
                require_policy_tags: Vec::new(),
                forbid_policy_tags: Vec::new(),
                content_types: Vec::new(),
                schema_ids: Vec::new(),
                packet_ids: Vec::new(),
                stage_ids: Vec::new(),
                scenario_ids: Vec::new(),
            },
        ],
    });

    let target = DispatchTarget::Agent {
        agent_id: "agent-1".to_string(),
    };
    let envelope = sample_envelope(vec!["public"], vec![]);
    let payload = PacketPayload::Json {
        value: serde_json::json!({"hello": "world"}),
    };

    let decision = policy.authorize(&target, &envelope, &payload).expect("policy decision");
    assert_eq!(decision, PolicyDecision::Deny);
}

#[test]
fn static_policy_matches_target_selector() {
    let policy = DispatchPolicy::Static(StaticPolicyConfig {
        default: PolicyEffect::Deny,
        rules: vec![PolicyRule {
            effect: PolicyEffect::Permit,
            error_message: None,
            target_kinds: Vec::new(),
            targets: vec![PolicyTargetSelector {
                target_kind: DispatchTargetKind::Agent,
                target_id: Some("agent-2".to_string()),
                system: None,
                target: None,
            }],
            require_labels: Vec::new(),
            forbid_labels: Vec::new(),
            require_policy_tags: Vec::new(),
            forbid_policy_tags: Vec::new(),
            content_types: Vec::new(),
            schema_ids: Vec::new(),
            packet_ids: Vec::new(),
            stage_ids: Vec::new(),
            scenario_ids: Vec::new(),
        }],
    });

    let target = DispatchTarget::Agent {
        agent_id: "agent-2".to_string(),
    };
    let envelope = sample_envelope(vec!["public"], vec![]);
    let payload = PacketPayload::Json {
        value: serde_json::json!({"hello": "world"}),
    };

    let decision = policy.authorize(&target, &envelope, &payload).expect("policy decision");
    assert_eq!(decision, PolicyDecision::Permit);
}

#[test]
fn static_policy_forbid_labels_blocks_match() {
    let policy = DispatchPolicy::Static(StaticPolicyConfig {
        default: PolicyEffect::Deny,
        rules: vec![PolicyRule {
            effect: PolicyEffect::Permit,
            error_message: None,
            target_kinds: Vec::new(),
            targets: Vec::new(),
            require_labels: vec!["public".to_string()],
            forbid_labels: vec!["secret".to_string()],
            require_policy_tags: Vec::new(),
            forbid_policy_tags: Vec::new(),
            content_types: Vec::new(),
            schema_ids: Vec::new(),
            packet_ids: Vec::new(),
            stage_ids: Vec::new(),
            scenario_ids: Vec::new(),
        }],
    });

    let target = DispatchTarget::Agent {
        agent_id: "agent-1".to_string(),
    };
    let envelope = sample_envelope(vec!["public", "secret"], vec![]);
    let payload = PacketPayload::Json {
        value: serde_json::json!({"hello": "world"}),
    };

    let decision = policy.authorize(&target, &envelope, &payload).expect("policy decision");
    assert_eq!(decision, PolicyDecision::Deny);
}

#[test]
fn static_policy_content_type_matches() {
    let policy = DispatchPolicy::Static(StaticPolicyConfig {
        default: PolicyEffect::Deny,
        rules: vec![PolicyRule {
            effect: PolicyEffect::Permit,
            error_message: None,
            target_kinds: Vec::new(),
            targets: Vec::new(),
            require_labels: Vec::new(),
            forbid_labels: Vec::new(),
            require_policy_tags: Vec::new(),
            forbid_policy_tags: Vec::new(),
            content_types: vec!["text/plain".to_string()],
            schema_ids: Vec::new(),
            packet_ids: Vec::new(),
            stage_ids: Vec::new(),
            scenario_ids: Vec::new(),
        }],
    });

    let target = DispatchTarget::Agent {
        agent_id: "agent-1".to_string(),
    };
    let envelope = sample_envelope_with_content_type(vec!["public"], vec![], "text/plain");
    let payload = PacketPayload::Json {
        value: serde_json::json!({"hello": "world"}),
    };

    let decision = policy.authorize(&target, &envelope, &payload).expect("policy decision");
    assert_eq!(decision, PolicyDecision::Permit);
}
