//! Policy config validation tests for decision-gate-config.
// decision-gate-config/tests/policy_validation.rs
// =============================================================================
// Module: Policy Config Validation Tests
// Description: Comprehensive tests for policy.rs validation and matching logic.
// Purpose: Ensure dispatch policy engine is fail-closed and deterministic.
// =============================================================================

use decision_gate_config::policy::DispatchPolicy;
use decision_gate_config::policy::DispatchTargetKind;
use decision_gate_config::policy::PolicyEffect;
use decision_gate_config::policy::PolicyRule;
use decision_gate_config::policy::PolicyTargetSelector;
use decision_gate_config::policy::StaticPolicyConfig;
use decision_gate_core::DispatchTarget;
use decision_gate_core::HashAlgorithm;
use decision_gate_core::HashDigest;
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
use decision_gate_core::VisibilityPolicy;
use serde_json::json;

type TestResult = Result<(), String>;

// ============================================================================
// SECTION: Test Helpers
// ============================================================================

/// Assert that a validation result is an error containing a specific substring.
fn assert_invalid(result: Result<(), String>, needle: &str) -> TestResult {
    match result {
        Err(error) => {
            if error.contains(needle) {
                Ok(())
            } else {
                Err(format!("error '{error}' did not contain '{needle}'"))
            }
        }
        Ok(()) => Err("expected invalid config".to_string()),
    }
}

/// Creates a test `PacketEnvelope` with specified visibility policy.
fn test_envelope(labels: Vec<String>, policy_tags: Vec<String>) -> PacketEnvelope {
    PacketEnvelope {
        scenario_id: ScenarioId::new("scenario-1"),
        run_id: RunId::new("run-1"),
        stage_id: StageId::new("stage-1"),
        packet_id: PacketId::new("packet-1"),
        schema_id: SchemaId::new("test.schema.v1"),
        content_type: "application/json".to_string(),
        content_hash: HashDigest::new(HashAlgorithm::Sha256, &[1, 2, 3]),
        visibility: VisibilityPolicy::new(labels, policy_tags),
        expiry: None,
        correlation_id: None,
        issued_at: Timestamp::UnixMillis(0),
    }
}

/// Creates a test `PacketPayload` (JSON variant).
fn test_payload() -> PacketPayload {
    PacketPayload::Json {
        value: json!({"test": "data"}),
    }
}

fn assert_rule_matches(
    mut rule: PolicyRule,
    target: &DispatchTarget,
    envelope: &PacketEnvelope,
) -> TestResult {
    rule.effect = PolicyEffect::Permit;
    let policy = DispatchPolicy::Static(StaticPolicyConfig {
        default: PolicyEffect::Deny,
        rules: vec![rule],
    });
    let payload = test_payload();
    match policy.authorize(target, envelope, &payload) {
        Ok(PolicyDecision::Permit) => Ok(()),
        Ok(PolicyDecision::Deny) => Err("expected rule to match (Permit)".to_string()),
        Err(err) => Err(format!("unexpected error: {err}")),
    }
}

fn assert_rule_not_match(
    mut rule: PolicyRule,
    target: &DispatchTarget,
    envelope: &PacketEnvelope,
) -> TestResult {
    rule.effect = PolicyEffect::Deny;
    let policy = DispatchPolicy::Static(StaticPolicyConfig {
        default: PolicyEffect::Permit,
        rules: vec![rule],
    });
    let payload = test_payload();
    match policy.authorize(target, envelope, &payload) {
        Ok(PolicyDecision::Permit) => Ok(()),
        Ok(PolicyDecision::Deny) => Err("expected rule to not match (Permit)".to_string()),
        Err(err) => Err(format!("unexpected error: {err}")),
    }
}

// ============================================================================
// SECTION: StaticPolicyConfig Validation Tests
// ============================================================================

#[test]
fn static_policy_default_permit_empty_rules() -> TestResult {
    let policy = StaticPolicyConfig {
        default: PolicyEffect::Permit,
        rules: Vec::new(),
    };
    policy.validate()?;
    Ok(())
}

#[test]
fn static_policy_default_deny_empty_rules() -> TestResult {
    let policy = StaticPolicyConfig {
        default: PolicyEffect::Deny,
        rules: Vec::new(),
    };
    policy.validate()?;
    Ok(())
}

#[test]
fn static_policy_default_error_rejected() -> TestResult {
    let policy = StaticPolicyConfig {
        default: PolicyEffect::Error,
        rules: Vec::new(),
    };
    assert_invalid(policy.validate(), "default must be permit or deny")?;
    Ok(())
}

#[test]
fn static_policy_with_valid_rules() -> TestResult {
    let policy = StaticPolicyConfig {
        default: PolicyEffect::Deny,
        rules: vec![PolicyRule {
            effect: PolicyEffect::Permit,
            error_message: None,
            target_kinds: vec![DispatchTargetKind::Agent],
            targets: Vec::new(),
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
    };
    policy.validate()?;
    Ok(())
}

#[test]
fn static_policy_with_invalid_rule_propagates_error() -> TestResult {
    let policy = StaticPolicyConfig {
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
            content_types: Vec::new(),
            schema_ids: Vec::new(),
            packet_ids: Vec::new(),
            stage_ids: Vec::new(),
            scenario_ids: Vec::new(),
        }],
    };
    assert_invalid(policy.validate(), "rule must include at least one match criterion")?;
    Ok(())
}

// ============================================================================
// SECTION: PolicyRule Validation Tests
// ============================================================================

#[test]
fn policy_rule_no_selectors_rejected() -> TestResult {
    let rule = PolicyRule {
        effect: PolicyEffect::Permit,
        error_message: None,
        target_kinds: Vec::new(),
        targets: Vec::new(),
        require_labels: Vec::new(),
        forbid_labels: Vec::new(),
        require_policy_tags: Vec::new(),
        forbid_policy_tags: Vec::new(),
        content_types: Vec::new(),
        schema_ids: Vec::new(),
        packet_ids: Vec::new(),
        stage_ids: Vec::new(),
        scenario_ids: Vec::new(),
    };
    let policy = StaticPolicyConfig {
        default: PolicyEffect::Deny,
        rules: vec![rule],
    };
    assert_invalid(policy.validate(), "rule must include at least one match criterion")?;
    Ok(())
}

#[test]
fn policy_rule_error_effect_without_message_rejected() -> TestResult {
    let rule = PolicyRule {
        effect: PolicyEffect::Error,
        error_message: None,
        target_kinds: vec![DispatchTargetKind::Agent],
        targets: Vec::new(),
        require_labels: Vec::new(),
        forbid_labels: Vec::new(),
        require_policy_tags: Vec::new(),
        forbid_policy_tags: Vec::new(),
        content_types: Vec::new(),
        schema_ids: Vec::new(),
        packet_ids: Vec::new(),
        stage_ids: Vec::new(),
        scenario_ids: Vec::new(),
    };
    let policy = StaticPolicyConfig {
        default: PolicyEffect::Deny,
        rules: vec![rule],
    };
    assert_invalid(policy.validate(), "error effect requires error_message")?;
    Ok(())
}

#[test]
fn policy_rule_error_effect_with_empty_message_rejected() -> TestResult {
    let rule = PolicyRule {
        effect: PolicyEffect::Error,
        error_message: Some(String::new()),
        target_kinds: vec![DispatchTargetKind::Agent],
        targets: Vec::new(),
        require_labels: Vec::new(),
        forbid_labels: Vec::new(),
        require_policy_tags: Vec::new(),
        forbid_policy_tags: Vec::new(),
        content_types: Vec::new(),
        schema_ids: Vec::new(),
        packet_ids: Vec::new(),
        stage_ids: Vec::new(),
        scenario_ids: Vec::new(),
    };
    let policy = StaticPolicyConfig {
        default: PolicyEffect::Deny,
        rules: vec![rule],
    };
    assert_invalid(policy.validate(), "error effect requires error_message")?;
    Ok(())
}

#[test]
fn policy_rule_error_effect_with_message_accepted() -> TestResult {
    let rule = PolicyRule {
        effect: PolicyEffect::Error,
        error_message: Some("access denied".to_string()),
        target_kinds: vec![DispatchTargetKind::Agent],
        targets: Vec::new(),
        require_labels: Vec::new(),
        forbid_labels: Vec::new(),
        require_policy_tags: Vec::new(),
        forbid_policy_tags: Vec::new(),
        content_types: Vec::new(),
        schema_ids: Vec::new(),
        packet_ids: Vec::new(),
        stage_ids: Vec::new(),
        scenario_ids: Vec::new(),
    };
    let policy = StaticPolicyConfig {
        default: PolicyEffect::Deny,
        rules: vec![rule],
    };
    policy.validate()?;
    Ok(())
}

#[test]
fn policy_rule_permit_with_message_accepted() -> TestResult {
    let rule = PolicyRule {
        effect: PolicyEffect::Permit,
        error_message: Some("ignored message".to_string()),
        target_kinds: vec![DispatchTargetKind::Agent],
        targets: Vec::new(),
        require_labels: Vec::new(),
        forbid_labels: Vec::new(),
        require_policy_tags: Vec::new(),
        forbid_policy_tags: Vec::new(),
        content_types: Vec::new(),
        schema_ids: Vec::new(),
        packet_ids: Vec::new(),
        stage_ids: Vec::new(),
        scenario_ids: Vec::new(),
    };
    let policy = StaticPolicyConfig {
        default: PolicyEffect::Deny,
        rules: vec![rule],
    };
    policy.validate()?;
    Ok(())
}

#[test]
fn policy_rule_with_target_kinds_only() -> TestResult {
    let rule = PolicyRule {
        effect: PolicyEffect::Permit,
        error_message: None,
        target_kinds: vec![DispatchTargetKind::Agent],
        targets: Vec::new(),
        require_labels: Vec::new(),
        forbid_labels: Vec::new(),
        require_policy_tags: Vec::new(),
        forbid_policy_tags: Vec::new(),
        content_types: Vec::new(),
        schema_ids: Vec::new(),
        packet_ids: Vec::new(),
        stage_ids: Vec::new(),
        scenario_ids: Vec::new(),
    };
    let policy = StaticPolicyConfig {
        default: PolicyEffect::Deny,
        rules: vec![rule],
    };
    policy.validate()?;
    Ok(())
}

#[test]
fn policy_rule_with_require_labels_only() -> TestResult {
    let rule = PolicyRule {
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
    };
    let policy = StaticPolicyConfig {
        default: PolicyEffect::Deny,
        rules: vec![rule],
    };
    policy.validate()?;
    Ok(())
}

#[test]
fn policy_rule_with_content_types_only() -> TestResult {
    let rule = PolicyRule {
        effect: PolicyEffect::Permit,
        error_message: None,
        target_kinds: Vec::new(),
        targets: Vec::new(),
        require_labels: Vec::new(),
        forbid_labels: Vec::new(),
        require_policy_tags: Vec::new(),
        forbid_policy_tags: Vec::new(),
        content_types: vec!["application/json".to_string()],
        schema_ids: Vec::new(),
        packet_ids: Vec::new(),
        stage_ids: Vec::new(),
        scenario_ids: Vec::new(),
    };
    let policy = StaticPolicyConfig {
        default: PolicyEffect::Deny,
        rules: vec![rule],
    };
    policy.validate()?;
    Ok(())
}

// ============================================================================
// SECTION: PolicyTargetSelector Validation Tests
// ============================================================================

#[test]
fn target_selector_external_with_target_id_rejected() -> TestResult {
    let selector = PolicyTargetSelector {
        target_kind: DispatchTargetKind::External,
        target_id: Some("agent1".to_string()),
        system: Some("external_system".to_string()),
        target: None,
    };
    let policy = StaticPolicyConfig {
        default: PolicyEffect::Deny,
        rules: vec![PolicyRule {
            effect: PolicyEffect::Permit,
            error_message: None,
            target_kinds: Vec::new(),
            targets: vec![selector],
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
    };
    assert_invalid(policy.validate(), "external target must not set target_id")?;
    Ok(())
}

#[test]
fn target_selector_external_without_system_or_target_rejected() -> TestResult {
    let selector = PolicyTargetSelector {
        target_kind: DispatchTargetKind::External,
        target_id: None,
        system: None,
        target: None,
    };
    let policy = StaticPolicyConfig {
        default: PolicyEffect::Deny,
        rules: vec![PolicyRule {
            effect: PolicyEffect::Permit,
            error_message: None,
            target_kinds: Vec::new(),
            targets: vec![selector],
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
    };
    assert_invalid(policy.validate(), "external target requires system or target")?;
    Ok(())
}

#[test]
fn target_selector_external_with_system_only() -> TestResult {
    let selector = PolicyTargetSelector {
        target_kind: DispatchTargetKind::External,
        target_id: None,
        system: Some("external_system".to_string()),
        target: None,
    };
    let policy = StaticPolicyConfig {
        default: PolicyEffect::Deny,
        rules: vec![PolicyRule {
            effect: PolicyEffect::Permit,
            error_message: None,
            target_kinds: Vec::new(),
            targets: vec![selector],
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
    };
    policy.validate()?;
    Ok(())
}

#[test]
fn target_selector_external_with_target_only() -> TestResult {
    let selector = PolicyTargetSelector {
        target_kind: DispatchTargetKind::External,
        target_id: None,
        system: None,
        target: Some("target1".to_string()),
    };
    let policy = StaticPolicyConfig {
        default: PolicyEffect::Deny,
        rules: vec![PolicyRule {
            effect: PolicyEffect::Permit,
            error_message: None,
            target_kinds: Vec::new(),
            targets: vec![selector],
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
    };
    policy.validate()?;
    Ok(())
}

#[test]
fn target_selector_external_with_both() -> TestResult {
    let selector = PolicyTargetSelector {
        target_kind: DispatchTargetKind::External,
        target_id: None,
        system: Some("external_system".to_string()),
        target: Some("target1".to_string()),
    };
    let policy = StaticPolicyConfig {
        default: PolicyEffect::Deny,
        rules: vec![PolicyRule {
            effect: PolicyEffect::Permit,
            error_message: None,
            target_kinds: Vec::new(),
            targets: vec![selector],
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
    };
    policy.validate()?;
    Ok(())
}

#[test]
fn target_selector_agent_with_system_rejected() -> TestResult {
    let selector = PolicyTargetSelector {
        target_kind: DispatchTargetKind::Agent,
        target_id: Some("agent1".to_string()),
        system: Some("external_system".to_string()),
        target: None,
    };
    let policy = StaticPolicyConfig {
        default: PolicyEffect::Deny,
        rules: vec![PolicyRule {
            effect: PolicyEffect::Permit,
            error_message: None,
            target_kinds: Vec::new(),
            targets: vec![selector],
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
    };
    assert_invalid(policy.validate(), "non-external targets must not set system/target")?;
    Ok(())
}

#[test]
fn target_selector_session_with_target_rejected() -> TestResult {
    let selector = PolicyTargetSelector {
        target_kind: DispatchTargetKind::Session,
        target_id: Some("session1".to_string()),
        system: None,
        target: Some("target1".to_string()),
    };
    let policy = StaticPolicyConfig {
        default: PolicyEffect::Deny,
        rules: vec![PolicyRule {
            effect: PolicyEffect::Permit,
            error_message: None,
            target_kinds: Vec::new(),
            targets: vec![selector],
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
    };
    assert_invalid(policy.validate(), "non-external targets must not set system/target")?;
    Ok(())
}

#[test]
fn target_selector_channel_with_system_rejected() -> TestResult {
    let selector = PolicyTargetSelector {
        target_kind: DispatchTargetKind::Channel,
        target_id: Some("channel1".to_string()),
        system: Some("external_system".to_string()),
        target: None,
    };
    let policy = StaticPolicyConfig {
        default: PolicyEffect::Deny,
        rules: vec![PolicyRule {
            effect: PolicyEffect::Permit,
            error_message: None,
            target_kinds: Vec::new(),
            targets: vec![selector],
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
    };
    assert_invalid(policy.validate(), "non-external targets must not set system/target")?;
    Ok(())
}

// ============================================================================
// SECTION: Policy Matching Logic - Target Kinds
// ============================================================================

#[test]
fn rule_matches_target_kind_agent() -> TestResult {
    let rule = PolicyRule {
        effect: PolicyEffect::Permit,
        error_message: None,
        target_kinds: vec![DispatchTargetKind::Agent],
        targets: Vec::new(),
        require_labels: Vec::new(),
        forbid_labels: Vec::new(),
        require_policy_tags: Vec::new(),
        forbid_policy_tags: Vec::new(),
        content_types: Vec::new(),
        schema_ids: Vec::new(),
        packet_ids: Vec::new(),
        stage_ids: Vec::new(),
        scenario_ids: Vec::new(),
    };
    let target = DispatchTarget::Agent {
        agent_id: "agent1".to_string(),
    };
    let envelope = test_envelope(Vec::new(), Vec::new());
    assert_rule_matches(rule, &target, &envelope)
}

#[test]
fn rule_matches_target_kind_session() -> TestResult {
    let rule = PolicyRule {
        effect: PolicyEffect::Permit,
        error_message: None,
        target_kinds: vec![DispatchTargetKind::Session],
        targets: Vec::new(),
        require_labels: Vec::new(),
        forbid_labels: Vec::new(),
        require_policy_tags: Vec::new(),
        forbid_policy_tags: Vec::new(),
        content_types: Vec::new(),
        schema_ids: Vec::new(),
        packet_ids: Vec::new(),
        stage_ids: Vec::new(),
        scenario_ids: Vec::new(),
    };
    let target = DispatchTarget::Session {
        session_id: "session1".to_string(),
    };
    let envelope = test_envelope(Vec::new(), Vec::new());
    assert_rule_matches(rule, &target, &envelope)
}

#[test]
fn rule_matches_target_kind_external() -> TestResult {
    let rule = PolicyRule {
        effect: PolicyEffect::Permit,
        error_message: None,
        target_kinds: vec![DispatchTargetKind::External],
        targets: Vec::new(),
        require_labels: Vec::new(),
        forbid_labels: Vec::new(),
        require_policy_tags: Vec::new(),
        forbid_policy_tags: Vec::new(),
        content_types: Vec::new(),
        schema_ids: Vec::new(),
        packet_ids: Vec::new(),
        stage_ids: Vec::new(),
        scenario_ids: Vec::new(),
    };
    let target = DispatchTarget::External {
        system: "external_system".to_string(),
        target: "target1".to_string(),
    };
    let envelope = test_envelope(Vec::new(), Vec::new());
    assert_rule_matches(rule, &target, &envelope)
}

#[test]
fn rule_matches_target_kind_channel() -> TestResult {
    let rule = PolicyRule {
        effect: PolicyEffect::Permit,
        error_message: None,
        target_kinds: vec![DispatchTargetKind::Channel],
        targets: Vec::new(),
        require_labels: Vec::new(),
        forbid_labels: Vec::new(),
        require_policy_tags: Vec::new(),
        forbid_policy_tags: Vec::new(),
        content_types: Vec::new(),
        schema_ids: Vec::new(),
        packet_ids: Vec::new(),
        stage_ids: Vec::new(),
        scenario_ids: Vec::new(),
    };
    let target = DispatchTarget::Channel {
        channel: "channel1".to_string(),
    };
    let envelope = test_envelope(Vec::new(), Vec::new());
    assert_rule_matches(rule, &target, &envelope)
}

#[test]
fn rule_target_kind_filter_mismatch() -> TestResult {
    let rule = PolicyRule {
        effect: PolicyEffect::Permit,
        error_message: None,
        target_kinds: vec![DispatchTargetKind::Agent],
        targets: Vec::new(),
        require_labels: Vec::new(),
        forbid_labels: Vec::new(),
        require_policy_tags: Vec::new(),
        forbid_policy_tags: Vec::new(),
        content_types: Vec::new(),
        schema_ids: Vec::new(),
        packet_ids: Vec::new(),
        stage_ids: Vec::new(),
        scenario_ids: Vec::new(),
    };
    let target = DispatchTarget::Session {
        session_id: "session1".to_string(),
    };
    let envelope = test_envelope(Vec::new(), Vec::new());
    assert_rule_not_match(rule, &target, &envelope)
}

// ============================================================================
// SECTION: Policy Matching Logic - Target Selectors
// ============================================================================

#[test]
fn rule_matches_target_selector_agent_with_id() -> TestResult {
    let rule = PolicyRule {
        effect: PolicyEffect::Permit,
        error_message: None,
        target_kinds: Vec::new(),
        targets: vec![PolicyTargetSelector {
            target_kind: DispatchTargetKind::Agent,
            target_id: Some("agent1".to_string()),
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
    };
    let target = DispatchTarget::Agent {
        agent_id: "agent1".to_string(),
    };
    let envelope = test_envelope(Vec::new(), Vec::new());
    assert_rule_matches(rule, &target, &envelope)
}

#[test]
fn rule_matches_target_selector_agent_without_id() -> TestResult {
    let rule = PolicyRule {
        effect: PolicyEffect::Permit,
        error_message: None,
        target_kinds: Vec::new(),
        targets: vec![PolicyTargetSelector {
            target_kind: DispatchTargetKind::Agent,
            target_id: None,
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
    };
    let target = DispatchTarget::Agent {
        agent_id: "any_agent".to_string(),
    };
    let envelope = test_envelope(Vec::new(), Vec::new());
    assert_rule_matches(rule, &target, &envelope)
}

#[test]
fn rule_target_selector_agent_id_mismatch() -> TestResult {
    let rule = PolicyRule {
        effect: PolicyEffect::Permit,
        error_message: None,
        target_kinds: Vec::new(),
        targets: vec![PolicyTargetSelector {
            target_kind: DispatchTargetKind::Agent,
            target_id: Some("agent1".to_string()),
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
    };
    let target = DispatchTarget::Agent {
        agent_id: "agent2".to_string(),
    };
    let envelope = test_envelope(Vec::new(), Vec::new());
    assert_rule_not_match(rule, &target, &envelope)
}

#[test]
fn rule_matches_target_selector_external_system_match() -> TestResult {
    let rule = PolicyRule {
        effect: PolicyEffect::Permit,
        error_message: None,
        target_kinds: Vec::new(),
        targets: vec![PolicyTargetSelector {
            target_kind: DispatchTargetKind::External,
            target_id: None,
            system: Some("external_system".to_string()),
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
    };
    let target = DispatchTarget::External {
        system: "external_system".to_string(),
        target: "any_target".to_string(),
    };
    let envelope = test_envelope(Vec::new(), Vec::new());
    assert_rule_matches(rule, &target, &envelope)
}

#[test]
fn rule_target_selector_external_system_mismatch() -> TestResult {
    let rule = PolicyRule {
        effect: PolicyEffect::Permit,
        error_message: None,
        target_kinds: Vec::new(),
        targets: vec![PolicyTargetSelector {
            target_kind: DispatchTargetKind::External,
            target_id: None,
            system: Some("external_system".to_string()),
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
    };
    let target = DispatchTarget::External {
        system: "other_system".to_string(),
        target: "target1".to_string(),
    };
    let envelope = test_envelope(Vec::new(), Vec::new());
    assert_rule_not_match(rule, &target, &envelope)
}

// ============================================================================
// SECTION: Policy Matching Logic - Labels and Tags
// ============================================================================

#[test]
fn rule_matches_require_labels_all_present() -> TestResult {
    let rule = PolicyRule {
        effect: PolicyEffect::Permit,
        error_message: None,
        target_kinds: Vec::new(),
        targets: Vec::new(),
        require_labels: vec!["public".to_string(), "read".to_string()],
        forbid_labels: Vec::new(),
        require_policy_tags: Vec::new(),
        forbid_policy_tags: Vec::new(),
        content_types: Vec::new(),
        schema_ids: Vec::new(),
        packet_ids: Vec::new(),
        stage_ids: Vec::new(),
        scenario_ids: Vec::new(),
    };
    let target = DispatchTarget::Agent {
        agent_id: "agent1".to_string(),
    };
    let envelope = test_envelope(
        vec!["public".to_string(), "read".to_string(), "extra".to_string()],
        Vec::new(),
    );
    assert_rule_matches(rule, &target, &envelope)
}

#[test]
fn rule_require_labels_missing_one() -> TestResult {
    let rule = PolicyRule {
        effect: PolicyEffect::Permit,
        error_message: None,
        target_kinds: Vec::new(),
        targets: Vec::new(),
        require_labels: vec!["public".to_string(), "read".to_string()],
        forbid_labels: Vec::new(),
        require_policy_tags: Vec::new(),
        forbid_policy_tags: Vec::new(),
        content_types: Vec::new(),
        schema_ids: Vec::new(),
        packet_ids: Vec::new(),
        stage_ids: Vec::new(),
        scenario_ids: Vec::new(),
    };
    let target = DispatchTarget::Agent {
        agent_id: "agent1".to_string(),
    };
    let envelope = test_envelope(vec!["public".to_string()], Vec::new());
    assert_rule_not_match(rule, &target, &envelope)
}

#[test]
fn rule_matches_forbid_labels_none_present() -> TestResult {
    let rule = PolicyRule {
        effect: PolicyEffect::Deny,
        error_message: None,
        target_kinds: Vec::new(),
        targets: Vec::new(),
        require_labels: Vec::new(),
        forbid_labels: vec!["sensitive".to_string()],
        require_policy_tags: Vec::new(),
        forbid_policy_tags: Vec::new(),
        content_types: Vec::new(),
        schema_ids: Vec::new(),
        packet_ids: Vec::new(),
        stage_ids: Vec::new(),
        scenario_ids: Vec::new(),
    };
    let target = DispatchTarget::Agent {
        agent_id: "agent1".to_string(),
    };
    let envelope = test_envelope(vec!["public".to_string()], Vec::new());
    assert_rule_matches(rule, &target, &envelope)
}

#[test]
fn rule_forbid_labels_one_present() -> TestResult {
    let rule = PolicyRule {
        effect: PolicyEffect::Deny,
        error_message: None,
        target_kinds: Vec::new(),
        targets: Vec::new(),
        require_labels: Vec::new(),
        forbid_labels: vec!["sensitive".to_string()],
        require_policy_tags: Vec::new(),
        forbid_policy_tags: Vec::new(),
        content_types: Vec::new(),
        schema_ids: Vec::new(),
        packet_ids: Vec::new(),
        stage_ids: Vec::new(),
        scenario_ids: Vec::new(),
    };
    let target = DispatchTarget::Agent {
        agent_id: "agent1".to_string(),
    };
    let envelope = test_envelope(vec!["public".to_string(), "sensitive".to_string()], Vec::new());
    assert_rule_not_match(rule, &target, &envelope)
}

#[test]
fn rule_matches_require_policy_tags() -> TestResult {
    let rule = PolicyRule {
        effect: PolicyEffect::Permit,
        error_message: None,
        target_kinds: Vec::new(),
        targets: Vec::new(),
        require_labels: Vec::new(),
        forbid_labels: Vec::new(),
        require_policy_tags: vec!["audit".to_string()],
        forbid_policy_tags: Vec::new(),
        content_types: Vec::new(),
        schema_ids: Vec::new(),
        packet_ids: Vec::new(),
        stage_ids: Vec::new(),
        scenario_ids: Vec::new(),
    };
    let target = DispatchTarget::Agent {
        agent_id: "agent1".to_string(),
    };
    let envelope = test_envelope(Vec::new(), vec!["audit".to_string()]);
    assert_rule_matches(rule, &target, &envelope)
}

#[test]
fn rule_forbid_policy_tags() -> TestResult {
    let rule = PolicyRule {
        effect: PolicyEffect::Deny,
        error_message: None,
        target_kinds: Vec::new(),
        targets: Vec::new(),
        require_labels: Vec::new(),
        forbid_labels: Vec::new(),
        require_policy_tags: Vec::new(),
        forbid_policy_tags: vec!["debug".to_string()],
        content_types: Vec::new(),
        schema_ids: Vec::new(),
        packet_ids: Vec::new(),
        stage_ids: Vec::new(),
        scenario_ids: Vec::new(),
    };
    let target = DispatchTarget::Agent {
        agent_id: "agent1".to_string(),
    };
    let envelope = test_envelope(Vec::new(), vec!["debug".to_string()]);
    assert_rule_not_match(rule, &target, &envelope)
}

// ============================================================================
// SECTION: Policy Matching Logic - Content/Schema/IDs
// ============================================================================

#[test]
fn rule_matches_content_type() -> TestResult {
    let rule = PolicyRule {
        effect: PolicyEffect::Permit,
        error_message: None,
        target_kinds: Vec::new(),
        targets: Vec::new(),
        require_labels: Vec::new(),
        forbid_labels: Vec::new(),
        require_policy_tags: Vec::new(),
        forbid_policy_tags: Vec::new(),
        content_types: vec!["application/json".to_string()],
        schema_ids: Vec::new(),
        packet_ids: Vec::new(),
        stage_ids: Vec::new(),
        scenario_ids: Vec::new(),
    };
    let target = DispatchTarget::Agent {
        agent_id: "agent1".to_string(),
    };
    let envelope = test_envelope(Vec::new(), Vec::new());
    assert_rule_matches(rule, &target, &envelope)
}

#[test]
fn rule_content_type_mismatch() -> TestResult {
    let rule = PolicyRule {
        effect: PolicyEffect::Permit,
        error_message: None,
        target_kinds: Vec::new(),
        targets: Vec::new(),
        require_labels: Vec::new(),
        forbid_labels: Vec::new(),
        require_policy_tags: Vec::new(),
        forbid_policy_tags: Vec::new(),
        content_types: vec!["application/xml".to_string()],
        schema_ids: Vec::new(),
        packet_ids: Vec::new(),
        stage_ids: Vec::new(),
        scenario_ids: Vec::new(),
    };
    let target = DispatchTarget::Agent {
        agent_id: "agent1".to_string(),
    };
    let envelope = test_envelope(Vec::new(), Vec::new());
    assert_rule_not_match(rule, &target, &envelope)
}

#[test]
fn rule_matches_schema_id() -> TestResult {
    let rule = PolicyRule {
        effect: PolicyEffect::Permit,
        error_message: None,
        target_kinds: Vec::new(),
        targets: Vec::new(),
        require_labels: Vec::new(),
        forbid_labels: Vec::new(),
        require_policy_tags: Vec::new(),
        forbid_policy_tags: Vec::new(),
        content_types: Vec::new(),
        schema_ids: vec!["test.schema.v1".to_string()],
        packet_ids: Vec::new(),
        stage_ids: Vec::new(),
        scenario_ids: Vec::new(),
    };
    let target = DispatchTarget::Agent {
        agent_id: "agent1".to_string(),
    };
    let envelope = test_envelope(Vec::new(), Vec::new());
    assert_rule_matches(rule, &target, &envelope)
}

// ============================================================================
// SECTION: Policy Matching Logic - Multiple Selectors (AND semantics)
// ============================================================================

#[test]
fn rule_matches_multiple_selectors_all_match() -> TestResult {
    let rule = PolicyRule {
        effect: PolicyEffect::Permit,
        error_message: None,
        target_kinds: vec![DispatchTargetKind::Agent],
        targets: Vec::new(),
        require_labels: vec!["public".to_string()],
        forbid_labels: Vec::new(),
        require_policy_tags: Vec::new(),
        forbid_policy_tags: Vec::new(),
        content_types: vec!["application/json".to_string()],
        schema_ids: Vec::new(),
        packet_ids: Vec::new(),
        stage_ids: Vec::new(),
        scenario_ids: Vec::new(),
    };
    let target = DispatchTarget::Agent {
        agent_id: "agent1".to_string(),
    };
    let envelope = test_envelope(vec!["public".to_string()], Vec::new());
    assert_rule_matches(rule, &target, &envelope)
}

#[test]
fn rule_multiple_selectors_one_fails() -> TestResult {
    let rule = PolicyRule {
        effect: PolicyEffect::Permit,
        error_message: None,
        target_kinds: vec![DispatchTargetKind::Agent],
        targets: Vec::new(),
        require_labels: vec!["public".to_string(), "admin".to_string()],
        forbid_labels: Vec::new(),
        require_policy_tags: Vec::new(),
        forbid_policy_tags: Vec::new(),
        content_types: vec!["application/json".to_string()],
        schema_ids: Vec::new(),
        packet_ids: Vec::new(),
        stage_ids: Vec::new(),
        scenario_ids: Vec::new(),
    };
    let target = DispatchTarget::Agent {
        agent_id: "agent1".to_string(),
    };
    let envelope = test_envelope(vec!["public".to_string()], Vec::new()); // missing "admin"
    assert_rule_not_match(rule, &target, &envelope)
}

#[test]
fn rule_empty_selector_array_matches_any() -> TestResult {
    let rule = PolicyRule {
        effect: PolicyEffect::Permit,
        error_message: None,
        target_kinds: vec![DispatchTargetKind::Agent],
        targets: Vec::new(),
        require_labels: Vec::new(), // empty = matches any
        forbid_labels: Vec::new(),
        require_policy_tags: Vec::new(),
        forbid_policy_tags: Vec::new(),
        content_types: Vec::new(), // empty = matches any
        schema_ids: Vec::new(),
        packet_ids: Vec::new(),
        stage_ids: Vec::new(),
        scenario_ids: Vec::new(),
    };
    let target = DispatchTarget::Agent {
        agent_id: "agent1".to_string(),
    };
    let envelope = test_envelope(Vec::new(), Vec::new());
    assert_rule_matches(rule, &target, &envelope)
}

// ============================================================================
// SECTION: DispatchPolicy Authorization Tests
// ============================================================================

#[test]
fn dispatch_policy_permit_all() -> TestResult {
    let policy = DispatchPolicy::PermitAll;
    let target = DispatchTarget::Agent {
        agent_id: "agent1".to_string(),
    };
    let envelope = test_envelope(Vec::new(), Vec::new());
    let payload = test_payload();
    let result = policy.authorize(&target, &envelope, &payload);
    match result {
        Ok(PolicyDecision::Permit) => Ok(()),
        Ok(PolicyDecision::Deny) => Err("expected Permit, got Deny".to_string()),
        Err(err) => Err(format!("unexpected error: {err}")),
    }
}

#[test]
fn dispatch_policy_deny_all() -> TestResult {
    let policy = DispatchPolicy::DenyAll;
    let target = DispatchTarget::Agent {
        agent_id: "agent1".to_string(),
    };
    let envelope = test_envelope(Vec::new(), Vec::new());
    let payload = test_payload();
    let result = policy.authorize(&target, &envelope, &payload);
    match result {
        Ok(PolicyDecision::Deny) => Ok(()),
        Ok(PolicyDecision::Permit) => Err("expected Deny, got Permit".to_string()),
        Err(err) => Err(format!("unexpected error: {err}")),
    }
}

#[test]
fn dispatch_policy_static_first_rule_matches() -> TestResult {
    let policy = DispatchPolicy::Static(StaticPolicyConfig {
        default: PolicyEffect::Deny,
        rules: vec![
            PolicyRule {
                effect: PolicyEffect::Permit,
                error_message: None,
                target_kinds: vec![DispatchTargetKind::Agent],
                targets: Vec::new(),
                require_labels: Vec::new(),
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
                effect: PolicyEffect::Deny,
                error_message: None,
                target_kinds: vec![DispatchTargetKind::Agent],
                targets: Vec::new(),
                require_labels: Vec::new(),
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
        agent_id: "agent1".to_string(),
    };
    let envelope = test_envelope(Vec::new(), Vec::new());
    let payload = test_payload();
    let result = policy.authorize(&target, &envelope, &payload);
    match result {
        Ok(PolicyDecision::Permit) => Ok(()),
        Ok(PolicyDecision::Deny) => Err("expected first rule to match (Permit)".to_string()),
        Err(err) => Err(format!("unexpected error: {err}")),
    }
}

#[test]
fn dispatch_policy_static_no_match_uses_default() -> TestResult {
    let policy = DispatchPolicy::Static(StaticPolicyConfig {
        default: PolicyEffect::Permit,
        rules: vec![PolicyRule {
            effect: PolicyEffect::Deny,
            error_message: None,
            target_kinds: vec![DispatchTargetKind::Session],
            targets: Vec::new(),
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
        agent_id: "agent1".to_string(),
    };
    let envelope = test_envelope(Vec::new(), Vec::new());
    let payload = test_payload();
    let result = policy.authorize(&target, &envelope, &payload);
    match result {
        Ok(PolicyDecision::Permit) => Ok(()),
        Ok(PolicyDecision::Deny) => Err("expected default effect (Permit)".to_string()),
        Err(err) => Err(format!("unexpected error: {err}")),
    }
}

#[test]
fn dispatch_policy_static_error_effect() -> TestResult {
    let policy = DispatchPolicy::Static(StaticPolicyConfig {
        default: PolicyEffect::Deny,
        rules: vec![PolicyRule {
            effect: PolicyEffect::Error,
            error_message: Some("access denied by policy".to_string()),
            target_kinds: vec![DispatchTargetKind::Agent],
            targets: Vec::new(),
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
        agent_id: "agent1".to_string(),
    };
    let envelope = test_envelope(Vec::new(), Vec::new());
    let payload = test_payload();
    let result = policy.authorize(&target, &envelope, &payload);
    match result {
        Err(err) => {
            let msg = err.to_string();
            if msg.contains("access denied by policy") {
                Ok(())
            } else {
                Err(format!("error message did not contain expected text: {msg}"))
            }
        }
        Ok(_) => Err("expected error effect to propagate".to_string()),
    }
}
