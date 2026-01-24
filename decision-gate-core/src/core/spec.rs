// decision-gate-core/src/core/spec.rs
// ============================================================================
// Module: Decision Gate Scenario Specification
// Description: Scenario, stage, gate, and predicate specifications.
// Purpose: Define canonical Decision Gate specs with validation helpers.
// Dependencies: crate::core::{disclosure, evidence, identifiers, hashing, time}, ret-logic, serde
// ============================================================================

//! ## Overview
//! Scenario specifications define the staged disclosure workflow, including
//! gate logic, packet disclosures, and branching rules. Specs are validated at
//! load time to enforce invariants such as unique identifiers and resolvable
//! predicate definitions.

// ============================================================================
// SECTION: Imports
// ============================================================================

use ret_logic::Requirement;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;
use thiserror::Error;

use crate::core::TrustRequirement;
use crate::core::disclosure::PacketPayload;
use crate::core::evidence::Comparator;
use crate::core::evidence::EvidenceQuery;
use crate::core::hashing::DEFAULT_HASH_ALGORITHM;
use crate::core::hashing::HashAlgorithm;
use crate::core::hashing::HashDigest;
use crate::core::hashing::HashError;
use crate::core::identifiers::GateId;
use crate::core::identifiers::NamespaceId;
use crate::core::identifiers::PacketId;
use crate::core::identifiers::PolicyId;
use crate::core::identifiers::PredicateKey;
use crate::core::identifiers::ScenarioId;
use crate::core::identifiers::SchemaId;
use crate::core::identifiers::SpecVersion;
use crate::core::identifiers::StageId;
use crate::core::identifiers::TenantId;
use crate::core::time::Timestamp;

// ============================================================================
// SECTION: Scenario Specification
// ============================================================================

/// Canonical scenario specification.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScenarioSpec {
    /// Scenario identifier.
    pub scenario_id: ScenarioId,
    /// Namespace identifier.
    pub namespace_id: NamespaceId,
    /// Specification version identifier.
    pub spec_version: SpecVersion,
    /// Scenario stages in deterministic order.
    pub stages: Vec<StageSpec>,
    /// Predicate definitions referenced by gates.
    pub predicates: Vec<PredicateSpec>,
    /// Optional policy references for disclosure.
    pub policies: Vec<PolicyRef>,
    /// Optional schema registry references.
    pub schemas: Vec<SchemaRef>,
    /// Optional default tenant identifier for single-tenant specs.
    pub default_tenant_id: Option<TenantId>,
}

impl ScenarioSpec {
    /// Computes the canonical hash of the scenario specification.
    ///
    /// # Errors
    ///
    /// Returns [`HashError::Canonicalization`] when serialization fails.
    pub fn canonical_hash(&self) -> Result<HashDigest, HashError> {
        crate::core::hashing::hash_canonical_json(DEFAULT_HASH_ALGORITHM, self)
    }

    /// Computes the canonical hash using a specific algorithm.
    ///
    /// # Errors
    ///
    /// Returns [`HashError::Canonicalization`] when serialization fails.
    pub fn canonical_hash_with(&self, algorithm: HashAlgorithm) -> Result<HashDigest, HashError> {
        crate::core::hashing::hash_canonical_json(algorithm, self)
    }

    /// Validates the scenario specification invariants.
    ///
    /// # Errors
    ///
    /// Returns [`SpecError`] when validation fails.
    pub fn validate(&self) -> Result<(), SpecError> {
        if self.stages.is_empty() {
            return Err(SpecError::MissingStages);
        }

        ensure_unique_stage_ids(&self.stages)?;
        ensure_unique_gate_ids(&self.stages)?;
        ensure_unique_packet_ids(&self.stages)?;
        ensure_unique_predicates(&self.predicates)?;
        ensure_predicate_queries_well_formed(&self.predicates)?;
        ensure_gate_predicates_resolve(&self.stages, &self.predicates)?;
        ensure_branch_targets_exist(&self.stages)?;

        Ok(())
    }
}

// ============================================================================
// SECTION: Stage Specifications
// ============================================================================

/// Stage specification defining gates and disclosures.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StageSpec {
    /// Stage identifier.
    pub stage_id: StageId,
    /// Packets disclosed on entry.
    pub entry_packets: Vec<PacketSpec>,
    /// Gates that must pass to advance.
    pub gates: Vec<GateSpec>,
    /// Stage advancement behavior.
    pub advance_to: AdvanceTo,
    /// Optional timeout specification.
    pub timeout: Option<TimeoutSpec>,
    /// Timeout handling policy.
    pub on_timeout: TimeoutPolicy,
}

/// Stage advancement policy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AdvanceTo {
    /// Advance to the next stage in specification order.
    Linear,
    /// Advance to a fixed stage identifier.
    Fixed {
        /// Next stage identifier.
        stage_id: StageId,
    },
    /// Advance based on gate outcomes.
    Branch {
        /// Branch rules applied in order.
        branches: Vec<BranchRule>,
        /// Optional default branch when no rules match.
        default: Option<StageId>,
    },
    /// Terminal stage (no further advancement).
    Terminal,
}

/// Branch rule mapping a gate outcome to the next stage.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BranchRule {
    /// Gate identifier referenced for the branch.
    pub gate_id: GateId,
    /// Required outcome for the branch.
    pub outcome: GateOutcome,
    /// Stage identifier to advance to.
    pub next_stage_id: StageId,
}

/// Gate outcome for branch selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GateOutcome {
    /// Gate evaluated to true.
    True,
    /// Gate evaluated to false.
    False,
    /// Gate evaluated to unknown.
    Unknown,
}

/// Stage timeout specification.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TimeoutSpec {
    /// Timeout duration in milliseconds.
    pub timeout_ms: u64,
    /// Optional policy tags for timeout handling.
    pub policy_tags: Vec<String>,
}

/// Timeout handling policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TimeoutPolicy {
    /// Fail the run when timeout triggers.
    Fail,
    /// Advance the run with a timeout flag.
    AdvanceWithFlag,
    /// Move to an alternate branch stage when timeout triggers.
    AlternateBranch,
}

// ============================================================================
// SECTION: Gate Specifications
// ============================================================================

/// Gate specification defined by a requirement algebra tree.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GateSpec {
    /// Stable identifier for the gate.
    pub gate_id: GateId,
    /// Requirement tree defining the gate logic.
    pub requirement: Requirement<PredicateKey>,
    /// Optional trust requirement override for this gate.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trust: Option<TrustRequirement>,
}

/// Predicate specification mapping a predicate key to evidence query rules.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PredicateSpec {
    /// Predicate identifier referenced by requirements.
    pub predicate: PredicateKey,
    /// Evidence query definition.
    pub query: EvidenceQuery,
    /// Comparator applied to evidence values.
    pub comparator: Comparator,
    /// Expected value for comparison.
    pub expected: Option<Value>,
    /// Optional policy tags for safe summaries.
    pub policy_tags: Vec<String>,
    /// Optional trust requirement override for this predicate.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trust: Option<TrustRequirement>,
}

// ============================================================================
// SECTION: Packet Specifications
// ============================================================================

/// Packet specification defined in the scenario spec.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PacketSpec {
    /// Packet identifier.
    pub packet_id: PacketId,
    /// Packet schema identifier.
    pub schema_id: SchemaId,
    /// Content type for the packet payload.
    pub content_type: String,
    /// Visibility labels controlling disclosure.
    pub visibility_labels: Vec<String>,
    /// Optional policy tags applied during dispatch.
    pub policy_tags: Vec<String>,
    /// Optional expiry timestamp.
    pub expiry: Option<Timestamp>,
    /// Packet payload definition.
    pub payload: PacketPayload,
}

// ============================================================================
// SECTION: Policy and Schema References
// ============================================================================

/// Policy reference used by scenario specifications.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PolicyRef {
    /// Policy identifier.
    pub policy_id: PolicyId,
    /// Optional policy description.
    pub description: Option<String>,
}

/// Schema registry reference for packet schemas.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SchemaRef {
    /// Schema identifier.
    pub schema_id: SchemaId,
    /// Optional schema version string.
    pub version: Option<String>,
    /// Optional schema URI.
    pub uri: Option<String>,
}

// ============================================================================
// SECTION: Errors
// ============================================================================

/// Scenario specification validation errors.
#[derive(Debug, Error)]
pub enum SpecError {
    /// Specification contains no stages.
    #[error("scenario spec must define at least one stage")]
    MissingStages,
    /// Duplicate stage identifiers detected.
    #[error("duplicate stage identifier: {0}")]
    DuplicateStageId(String),
    /// Duplicate gate identifiers detected.
    #[error("duplicate gate identifier: {0}")]
    DuplicateGateId(String),
    /// Duplicate packet identifiers detected.
    #[error("duplicate packet identifier: {0}")]
    DuplicatePacketId(String),
    /// Duplicate predicate identifiers detected.
    #[error("duplicate predicate key: {0}")]
    DuplicatePredicate(String),
    /// Gate references a predicate that is not defined.
    #[error("gate references undefined predicate: {0}")]
    MissingPredicate(String),
    /// Evidence query is missing required fields.
    #[error("invalid evidence query for predicate {0}: {1}")]
    InvalidEvidenceQuery(String, String),
    /// Branch target refers to a missing stage.
    #[error("branch target refers to unknown stage: {0}")]
    MissingBranchTarget(String),
}

// ============================================================================
// SECTION: Validation Helpers
// ============================================================================

/// Ensures stage identifiers are unique within the spec.
fn ensure_unique_stage_ids(stages: &[StageSpec]) -> Result<(), SpecError> {
    for (index, stage) in stages.iter().enumerate() {
        if stages.iter().skip(index + 1).any(|other| other.stage_id == stage.stage_id) {
            return Err(SpecError::DuplicateStageId(stage.stage_id.to_string()));
        }
    }
    Ok(())
}

/// Ensures gate identifiers are unique across all stages.
fn ensure_unique_gate_ids(stages: &[StageSpec]) -> Result<(), SpecError> {
    let mut seen: Vec<&GateId> = Vec::new();
    for stage in stages {
        for gate in &stage.gates {
            if seen.contains(&&gate.gate_id) {
                return Err(SpecError::DuplicateGateId(gate.gate_id.to_string()));
            }
            seen.push(&gate.gate_id);
        }
    }
    Ok(())
}

/// Ensures packet identifiers are unique across all stages.
fn ensure_unique_packet_ids(stages: &[StageSpec]) -> Result<(), SpecError> {
    let mut seen: Vec<&PacketId> = Vec::new();
    for stage in stages {
        for packet in &stage.entry_packets {
            if seen.contains(&&packet.packet_id) {
                return Err(SpecError::DuplicatePacketId(packet.packet_id.to_string()));
            }
            seen.push(&packet.packet_id);
        }
    }
    Ok(())
}

/// Ensures predicate keys are unique.
fn ensure_unique_predicates(predicates: &[PredicateSpec]) -> Result<(), SpecError> {
    for (index, predicate) in predicates.iter().enumerate() {
        if predicates.iter().skip(index + 1).any(|other| other.predicate == predicate.predicate) {
            return Err(SpecError::DuplicatePredicate(predicate.predicate.to_string()));
        }
    }
    Ok(())
}

/// Ensures evidence queries have required fields populated.
fn ensure_predicate_queries_well_formed(predicates: &[PredicateSpec]) -> Result<(), SpecError> {
    for predicate in predicates {
        let provider_id = predicate.query.provider_id.as_str();
        if provider_id.trim().is_empty() {
            return Err(SpecError::InvalidEvidenceQuery(
                predicate.predicate.to_string(),
                "provider_id is empty".to_string(),
            ));
        }
        if predicate.query.predicate.trim().is_empty() {
            return Err(SpecError::InvalidEvidenceQuery(
                predicate.predicate.to_string(),
                "predicate name is empty".to_string(),
            ));
        }
    }
    Ok(())
}

/// Ensures gate requirements reference defined predicates.
fn ensure_gate_predicates_resolve(
    stages: &[StageSpec],
    predicates: &[PredicateSpec],
) -> Result<(), SpecError> {
    for stage in stages {
        for gate in &stage.gates {
            let required = collect_predicates(&gate.requirement);
            for predicate in required {
                if !predicates.iter().any(|spec| spec.predicate == predicate) {
                    return Err(SpecError::MissingPredicate(predicate.to_string()));
                }
            }
        }
    }
    Ok(())
}

/// Ensures branch targets reference existing stages.
fn ensure_branch_targets_exist(stages: &[StageSpec]) -> Result<(), SpecError> {
    for stage in stages {
        match &stage.advance_to {
            AdvanceTo::Fixed {
                stage_id,
            } => {
                if !stages.iter().any(|spec| &spec.stage_id == stage_id) {
                    return Err(SpecError::MissingBranchTarget(stage_id.to_string()));
                }
            }
            AdvanceTo::Branch {
                branches,
                default,
            } => {
                for branch in branches {
                    if !stages.iter().any(|spec| spec.stage_id == branch.next_stage_id) {
                        return Err(SpecError::MissingBranchTarget(
                            branch.next_stage_id.to_string(),
                        ));
                    }
                }
                if let Some(stage_id) = default
                    && !stages.iter().any(|spec| &spec.stage_id == stage_id)
                {
                    return Err(SpecError::MissingBranchTarget(stage_id.to_string()));
                }
            }
            AdvanceTo::Linear | AdvanceTo::Terminal => {}
        }
    }
    Ok(())
}

/// Collects predicate keys referenced by a requirement tree.
fn collect_predicates(requirement: &Requirement<PredicateKey>) -> Vec<PredicateKey> {
    let mut out = Vec::new();
    collect_predicates_inner(requirement, &mut out);
    out
}

/// Walks a requirement tree and appends predicate keys.
fn collect_predicates_inner(requirement: &Requirement<PredicateKey>, out: &mut Vec<PredicateKey>) {
    match requirement {
        Requirement::Predicate(predicate) => {
            if !out.contains(predicate) {
                out.push(predicate.clone());
            }
        }
        Requirement::Not(inner) => collect_predicates_inner(inner, out),
        Requirement::And(reqs) | Requirement::Or(reqs) => {
            for req in reqs {
                collect_predicates_inner(req, out);
            }
        }
        Requirement::RequireGroup {
            reqs, ..
        } => {
            for req in reqs {
                collect_predicates_inner(req, out);
            }
        }
    }
}
