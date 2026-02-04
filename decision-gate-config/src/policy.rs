// decision-gate-config/src/policy.rs
// ============================================================================
// Module: Policy Engine Adapters
// Description: Deterministic policy engines for dispatch authorization.
// Purpose: Provide swappable, fail-closed policy evaluation for dispatch.
// Dependencies: decision-gate-core, serde
// ============================================================================

//! ## Overview
//! Policy engine adapters used to authorize packet dispatch with deterministic,
//! fail-closed decisions.
//!
//! Security posture: policy evaluation is a trust boundary; see
//! `Docs/security/threat_model.md`.

// ============================================================================
// SECTION: Imports
// ============================================================================

use decision_gate_core::DispatchTarget;
use decision_gate_core::PacketEnvelope;
use decision_gate_core::PacketPayload;
use decision_gate_core::PolicyDecider;
use decision_gate_core::PolicyDecision;
use decision_gate_core::PolicyError;
use serde::Deserialize;
use serde::Serialize;

// ============================================================================
// SECTION: Policy Model
// ============================================================================

/// Policy engine selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum PolicyEngine {
    /// Permit all dispatches.
    #[default]
    PermitAll,
    /// Deny all dispatches.
    DenyAll,
    /// Evaluate deterministic static rules.
    Static,
}

/// Static policy configuration.
#[derive(Debug, Clone, Deserialize)]
pub struct StaticPolicyConfig {
    /// Default decision when no rules match.
    #[serde(default = "default_static_effect")]
    pub default: PolicyEffect,
    /// Ordered list of policy rules.
    #[serde(default)]
    pub rules: Vec<PolicyRule>,
}

impl Default for StaticPolicyConfig {
    fn default() -> Self {
        Self {
            default: default_static_effect(),
            rules: Vec::new(),
        }
    }
}

impl StaticPolicyConfig {
    /// Validates static policy configuration.
    ///
    /// # Errors
    ///
    /// Returns an error when defaults or rules are invalid.
    pub fn validate(&self) -> Result<(), String> {
        if self.default == PolicyEffect::Error {
            return Err("static policy default must be permit or deny".to_string());
        }
        for (idx, rule) in self.rules.iter().enumerate() {
            rule.validate().map_err(|err| format!("policy.rules[{idx}]: {err}"))?;
        }
        Ok(())
    }

    /// Evaluates the policy rules for a dispatch request.
    fn evaluate(
        &self,
        target: &DispatchTarget,
        envelope: &PacketEnvelope,
    ) -> Result<PolicyDecision, PolicyError> {
        for rule in &self.rules {
            if rule.matches(target, envelope) {
                return rule.effect.to_decision(rule.error_message.as_deref());
            }
        }
        self.default.to_decision(None)
    }
}

/// Policy rule effect.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PolicyEffect {
    /// Permit the dispatch.
    Permit,
    /// Deny the dispatch.
    Deny,
    /// Raise a policy evaluation error.
    Error,
}

impl PolicyEffect {
    /// Converts the policy effect into a concrete decision.
    ///
    /// # Errors
    ///
    /// Returns [`PolicyError`] when the effect is `error`.
    fn to_decision(self, error_message: Option<&str>) -> Result<PolicyDecision, PolicyError> {
        match self {
            Self::Permit => Ok(PolicyDecision::Permit),
            Self::Deny => Ok(PolicyDecision::Deny),
            Self::Error => Err(PolicyError::DecisionFailed(
                error_message.unwrap_or("policy rule error").to_string(),
            )),
        }
    }
}

/// Policy rule for static evaluation.
#[derive(Debug, Clone, Deserialize)]
pub struct PolicyRule {
    /// Effect to apply when the rule matches.
    pub effect: PolicyEffect,
    /// Optional error message when effect is `error`.
    #[serde(default)]
    pub error_message: Option<String>,
    /// Target kinds that may receive the packet.
    #[serde(default)]
    pub target_kinds: Vec<DispatchTargetKind>,
    /// Specific target selectors.
    #[serde(default)]
    pub targets: Vec<PolicyTargetSelector>,
    /// Labels that must be present on the packet visibility policy.
    #[serde(default)]
    pub require_labels: Vec<String>,
    /// Labels that must not be present on the packet visibility policy.
    #[serde(default)]
    pub forbid_labels: Vec<String>,
    /// Policy tags that must be present on the packet visibility policy.
    #[serde(default)]
    pub require_policy_tags: Vec<String>,
    /// Policy tags that must not be present on the packet visibility policy.
    #[serde(default)]
    pub forbid_policy_tags: Vec<String>,
    /// Allowed content types for the packet.
    #[serde(default)]
    pub content_types: Vec<String>,
    /// Allowed schema identifiers for the packet.
    #[serde(default)]
    pub schema_ids: Vec<String>,
    /// Allowed packet identifiers.
    #[serde(default)]
    pub packet_ids: Vec<String>,
    /// Allowed stage identifiers.
    #[serde(default)]
    pub stage_ids: Vec<String>,
    /// Allowed scenario identifiers.
    #[serde(default)]
    pub scenario_ids: Vec<String>,
}

impl PolicyRule {
    /// Validates rule configuration for internal consistency.
    fn validate(&self) -> Result<(), String> {
        let has_selector = !self.target_kinds.is_empty()
            || !self.targets.is_empty()
            || !self.require_labels.is_empty()
            || !self.forbid_labels.is_empty()
            || !self.require_policy_tags.is_empty()
            || !self.forbid_policy_tags.is_empty()
            || !self.content_types.is_empty()
            || !self.schema_ids.is_empty()
            || !self.packet_ids.is_empty()
            || !self.stage_ids.is_empty()
            || !self.scenario_ids.is_empty();
        if !has_selector {
            return Err("rule must include at least one match criterion".to_string());
        }
        if self.effect == PolicyEffect::Error
            && self.error_message.as_deref().unwrap_or("").is_empty()
        {
            return Err("error effect requires error_message".to_string());
        }
        for (idx, target) in self.targets.iter().enumerate() {
            target.validate().map_err(|err| format!("targets[{idx}]: {err}"))?;
        }
        Ok(())
    }

    /// Returns true when the rule matches the dispatch target and envelope.
    fn matches(&self, target: &DispatchTarget, envelope: &PacketEnvelope) -> bool {
        if !self.target_kinds.is_empty()
            && !self.target_kinds.iter().any(|kind| kind.matches(target))
        {
            return false;
        }
        if !self.targets.is_empty() && !self.targets.iter().any(|rule| rule.matches(target)) {
            return false;
        }
        if !self
            .require_labels
            .iter()
            .all(|label| envelope.visibility.labels.iter().any(|item| item == label))
        {
            return false;
        }
        if self
            .forbid_labels
            .iter()
            .any(|label| envelope.visibility.labels.iter().any(|item| item == label))
        {
            return false;
        }
        if !self
            .require_policy_tags
            .iter()
            .all(|tag| envelope.visibility.policy_tags.iter().any(|item| item == tag))
        {
            return false;
        }
        if self
            .forbid_policy_tags
            .iter()
            .any(|tag| envelope.visibility.policy_tags.iter().any(|item| item == tag))
        {
            return false;
        }
        if !self.content_types.is_empty()
            && !self.content_types.iter().any(|content_type| content_type == &envelope.content_type)
        {
            return false;
        }
        if !self.schema_ids.is_empty()
            && !self.schema_ids.iter().any(|schema_id| schema_id == envelope.schema_id.as_str())
        {
            return false;
        }
        if !self.packet_ids.is_empty()
            && !self.packet_ids.iter().any(|packet_id| packet_id == envelope.packet_id.as_str())
        {
            return false;
        }
        if !self.stage_ids.is_empty()
            && !self.stage_ids.iter().any(|stage_id| stage_id == envelope.stage_id.as_str())
        {
            return false;
        }
        if !self.scenario_ids.is_empty()
            && !self
                .scenario_ids
                .iter()
                .any(|scenario_id| scenario_id == envelope.scenario_id.as_str())
        {
            return false;
        }
        true
    }
}

/// Target kinds supported by dispatch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DispatchTargetKind {
    /// Agent-specific target.
    Agent,
    /// Session-specific target.
    Session,
    /// External system target.
    External,
    /// Broadcast channel target.
    Channel,
}

impl DispatchTargetKind {
    /// Returns true when the target matches this kind.
    const fn matches(self, target: &DispatchTarget) -> bool {
        matches!(
            (self, target),
            (Self::Agent, DispatchTarget::Agent { .. })
                | (Self::Session, DispatchTarget::Session { .. })
                | (Self::External, DispatchTarget::External { .. })
                | (Self::Channel, DispatchTarget::Channel { .. })
        )
    }
}

/// Target selector for policy rules.
#[derive(Debug, Clone, Deserialize)]
pub struct PolicyTargetSelector {
    /// Target kind.
    #[serde(rename = "target_kind")]
    pub target_kind: DispatchTargetKind,
    /// Target identifier for agent/session/channel.
    #[serde(rename = "target_id", default)]
    pub target_id: Option<String>,
    /// External system name (external targets only).
    #[serde(default)]
    pub system: Option<String>,
    /// External target identifier (external targets only).
    #[serde(default)]
    pub target: Option<String>,
}

impl PolicyTargetSelector {
    /// Validates target selector fields for the target kind.
    fn validate(&self) -> Result<(), String> {
        match self.target_kind {
            DispatchTargetKind::External => {
                if self.target_id.is_some() {
                    return Err("external target must not set target_id".to_string());
                }
                if self.system.is_none() && self.target.is_none() {
                    return Err("external target requires system or target".to_string());
                }
            }
            DispatchTargetKind::Agent
            | DispatchTargetKind::Session
            | DispatchTargetKind::Channel => {
                if self.system.is_some() || self.target.is_some() {
                    return Err("non-external targets must not set system/target".to_string());
                }
            }
        }
        Ok(())
    }

    /// Returns true when the selector matches the target.
    fn matches(&self, target: &DispatchTarget) -> bool {
        match target {
            DispatchTarget::Agent {
                agent_id,
            } => {
                self.target_kind == DispatchTargetKind::Agent
                    && self.target_id.as_deref().is_none_or(|id| id == agent_id)
            }
            DispatchTarget::Session {
                session_id,
            } => {
                self.target_kind == DispatchTargetKind::Session
                    && self.target_id.as_deref().is_none_or(|id| id == session_id)
            }
            DispatchTarget::Channel {
                channel,
            } => {
                self.target_kind == DispatchTargetKind::Channel
                    && self.target_id.as_deref().is_none_or(|id| id == channel)
            }
            DispatchTarget::External {
                system,
                target: target_id,
            } => {
                if self.target_kind != DispatchTargetKind::External {
                    return false;
                }
                if let Some(required) = self.system.as_deref()
                    && required != system
                {
                    return false;
                }
                if let Some(required) = self.target.as_deref()
                    && required != target_id
                {
                    return false;
                }
                true
            }
        }
    }
}

/// Returns the default static policy effect.
const fn default_static_effect() -> PolicyEffect {
    PolicyEffect::Deny
}

// ============================================================================
// SECTION: Dispatch Adapter
// ============================================================================

/// Runtime policy decider for dispatch authorization.
#[derive(Debug, Clone)]
pub enum DispatchPolicy {
    /// Permit all disclosures.
    PermitAll,
    /// Deny all disclosures.
    DenyAll,
    /// Static rule evaluation.
    Static(StaticPolicyConfig),
}

impl PolicyDecider for DispatchPolicy {
    fn authorize(
        &self,
        target: &DispatchTarget,
        envelope: &PacketEnvelope,
        _payload: &PacketPayload,
    ) -> Result<PolicyDecision, PolicyError> {
        match self {
            Self::PermitAll => Ok(PolicyDecision::Permit),
            Self::DenyAll => Ok(PolicyDecision::Deny),
            Self::Static(policy) => policy.evaluate(target, envelope),
        }
    }
}
