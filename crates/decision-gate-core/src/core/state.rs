// crates/decision-gate-core/src/core/state.rs
// ============================================================================
// Module: Decision Gate Run State
// Description: Run state, trigger logs, decisions, and evaluations.
// Purpose: Capture deterministic run evolution for replay and verification.
// Dependencies: crate::core::{disclosure, evidence, hashing, identifiers, summary, time},
// ret-logic, serde
// ============================================================================

//! ## Overview
//! Run state captures the full control-plane history needed for offline
//! verification. All state changes are append-only and deterministic.
//!
//! Security posture: run state must be treated as untrusted on load; see
//! `Docs/security/threat_model.md`.

// ============================================================================
// SECTION: Imports
// ============================================================================

use ret_logic::TriState;
use serde::Deserialize;
use serde::Serialize;

use crate::core::disclosure::DispatchTarget;
use crate::core::disclosure::PacketPayload;
use crate::core::disclosure::PacketRecord;
use crate::core::evidence::EvidenceResult;
use crate::core::evidence::ProviderMissingError;
use crate::core::hashing::HashDigest;
use crate::core::identifiers::ConditionId;
use crate::core::identifiers::CorrelationId;
use crate::core::identifiers::DecisionId;
use crate::core::identifiers::GateId;
use crate::core::identifiers::NamespaceId;
use crate::core::identifiers::RunId;
use crate::core::identifiers::ScenarioId;
use crate::core::identifiers::StageId;
use crate::core::identifiers::TenantId;
use crate::core::identifiers::TriggerId;
use crate::core::summary::SafeSummary;
use crate::core::time::Timestamp;

// ============================================================================
// SECTION: Run Configuration
// ============================================================================

/// Configuration required to start a run.
///
/// # Invariants
/// - Identifiers must refer to the same tenant/namespace/scenario scope.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunConfig {
    /// Tenant identifier for the run.
    pub tenant_id: TenantId,
    /// Namespace identifier for the run.
    pub namespace_id: NamespaceId,
    /// Run identifier.
    pub run_id: RunId,
    /// Scenario identifier.
    pub scenario_id: ScenarioId,
    /// Dispatch targets for disclosures.
    pub dispatch_targets: Vec<DispatchTarget>,
    /// Optional policy tags for run-level disclosure.
    pub policy_tags: Vec<String>,
}

// ============================================================================
// SECTION: Run Status
// ============================================================================

/// Run lifecycle status.
///
/// # Invariants
/// - Variants are stable for serialization and contract matching.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunStatus {
    /// Run is active and awaiting gate advancement.
    Active,
    /// Run has completed successfully.
    Completed,
    /// Run has failed.
    Failed,
}

// ============================================================================
// SECTION: Trigger Events
// ============================================================================

/// Trigger event kinds supported by Decision Gate.
///
/// # Invariants
/// - Variants are stable for serialization and contract matching.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TriggerKind {
    /// Agent requested the next stage.
    AgentRequestNext,
    /// Scheduled tick or timeout trigger.
    Tick,
    /// External event trigger.
    ExternalEvent,
    /// Backend event trigger.
    BackendEvent,
}

/// Canonical trigger event.
///
/// # Invariants
/// - Identifiers must refer to the same run scope.
/// - `payload` is optional and not interpreted by the core.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TriggerEvent {
    /// Trigger identifier for idempotency.
    pub trigger_id: TriggerId,
    /// Tenant identifier for the run.
    pub tenant_id: TenantId,
    /// Namespace identifier for the run.
    pub namespace_id: NamespaceId,
    /// Run identifier.
    pub run_id: RunId,
    /// Trigger kind.
    pub kind: TriggerKind,
    /// Trigger timestamp.
    pub time: Timestamp,
    /// Source identifier (agent, scheduler, or external system).
    pub source_id: String,
    /// Optional trigger payload.
    pub payload: Option<PacketPayload>,
    /// Optional correlation identifier.
    pub correlation_id: Option<CorrelationId>,
}

/// Trigger record logged in the run state.
///
/// # Invariants
/// - `seq` is monotonic within a run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TriggerRecord {
    /// Monotonic sequence number assigned by Decision Gate.
    pub seq: u64,
    /// Trigger event.
    pub event: TriggerEvent,
}

// ============================================================================
// SECTION: Evidence Records
// ============================================================================

/// Evidence record logged for condition evaluation.
///
/// # Invariants
/// - `status` reflects the comparator outcome for `result`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceRecord {
    /// Condition identifier that was evaluated.
    pub condition_id: ConditionId,
    /// Condition status derived by comparator.
    pub status: TriState,
    /// Evidence result metadata.
    pub result: EvidenceResult,
}

// ============================================================================
// SECTION: Gate Evaluation Records
// ============================================================================

/// Trace entry for condition evaluation.
///
/// # Invariants
/// - `status` corresponds to the evaluated condition outcome.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GateTraceEntry {
    /// Condition identifier that was evaluated.
    pub condition_id: ConditionId,
    /// Result of the condition evaluation.
    pub status: TriState,
}

/// Gate evaluation result with trace entries.
///
/// # Invariants
/// - `trace` contains condition evaluations referenced by the gate requirement.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GateEvaluation {
    /// Gate identifier.
    pub gate_id: GateId,
    /// Final gate status.
    pub status: TriState,
    /// Condition evaluation trace.
    pub trace: Vec<GateTraceEntry>,
}

/// Gate evaluation record logged in the run state.
///
/// # Invariants
/// - `evidence` corresponds to conditions used for this gate evaluation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GateEvalRecord {
    /// Trigger identifier associated with this evaluation.
    pub trigger_id: TriggerId,
    /// Stage identifier evaluated.
    pub stage_id: StageId,
    /// Gate evaluation output.
    pub evaluation: GateEvaluation,
    /// Evidence records for conditions used by this gate.
    pub evidence: Vec<EvidenceRecord>,
}

// ============================================================================
// SECTION: Decisions
// ============================================================================

/// Decision outcome for a trigger evaluation.
///
/// # Invariants
/// - Variants are stable for serialization and contract matching.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DecisionOutcome {
    /// Start the run at the initial stage.
    Start {
        /// Initial stage identifier.
        stage_id: StageId,
    },
    /// Complete the run at the terminal stage.
    Complete {
        /// Terminal stage identifier.
        stage_id: StageId,
    },
    /// Advance to the next stage.
    Advance {
        /// Previous stage identifier.
        from_stage: StageId,
        /// Next stage identifier.
        to_stage: StageId,
        /// Indicates a timeout-driven advance.
        timeout: bool,
    },
    /// Hold the run at the current stage.
    Hold {
        /// Safe summary describing unmet gates.
        summary: SafeSummary,
    },
    /// Fail the run.
    Fail {
        /// Failure reason description.
        reason: String,
    },
}

/// Decision record logged in the run state.
///
/// # Invariants
/// - `seq` is monotonic within a run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecisionRecord {
    /// Decision identifier.
    pub decision_id: DecisionId,
    /// Monotonic decision sequence.
    pub seq: u64,
    /// Trigger identifier associated with the decision.
    pub trigger_id: TriggerId,
    /// Stage identifier when decision was made.
    pub stage_id: StageId,
    /// Decision timestamp.
    pub decided_at: Timestamp,
    /// Decision outcome.
    pub outcome: DecisionOutcome,
    /// Optional correlation identifier.
    pub correlation_id: Option<CorrelationId>,
}

// ============================================================================
// SECTION: Submissions and Tool Calls
// ============================================================================

/// Submission record for model-provided artifacts.
///
/// # Invariants
/// - `content_hash` must match the submitted payload bytes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SubmissionRecord {
    /// Submission identifier.
    pub submission_id: String,
    /// Run identifier.
    pub run_id: RunId,
    /// Submitted payload.
    pub payload: PacketPayload,
    /// Content type for the submission payload.
    pub content_type: String,
    /// Content hash of the submission payload.
    pub content_hash: HashDigest,
    /// Submission timestamp.
    pub submitted_at: Timestamp,
    /// Optional correlation identifier.
    pub correlation_id: Option<CorrelationId>,
}

/// Tool-call record for `scenario.next/status/submit/trigger` APIs.
///
/// # Invariants
/// - `request_hash` and `response_hash` are canonical hashes of the payloads.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolCallRecord {
    /// Tool-call identifier.
    pub call_id: String,
    /// Tool name or method invoked.
    pub method: String,
    /// Request hash for the tool call.
    pub request_hash: HashDigest,
    /// Response hash for the tool call.
    pub response_hash: HashDigest,
    /// Tool-call timestamp.
    pub called_at: Timestamp,
    /// Optional correlation identifier.
    pub correlation_id: Option<CorrelationId>,
    /// Optional error details when the tool call failed.
    pub error: Option<ToolCallError>,
}

/// Tool-call error metadata recorded for failed calls.
///
/// # Invariants
/// - `code` is a stable error identifier for the tool surface.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolCallError {
    /// Stable error code string.
    pub code: String,
    /// Human-readable error message.
    pub message: String,
    /// Optional structured error details.
    pub details: Option<ToolCallErrorDetails>,
}

/// Structured details for tool-call errors.
///
/// # Invariants
/// - Variants are stable for serialization and contract matching.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ToolCallErrorDetails {
    /// Missing or blocked provider details.
    ProviderMissing(ProviderMissingError),
    /// Generic error details.
    Message {
        /// Additional error context.
        info: String,
    },
}

// ============================================================================
// SECTION: Run State
// ============================================================================

/// Decision Gate run state with append-only logs.
///
/// # Invariants
/// - Logs are append-only and ordered by `seq` within each stream.
/// - `spec_hash` matches the canonical hash of the scenario spec.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunState {
    /// Tenant identifier.
    pub tenant_id: TenantId,
    /// Namespace identifier.
    pub namespace_id: NamespaceId,
    /// Run identifier.
    pub run_id: RunId,
    /// Scenario identifier.
    pub scenario_id: ScenarioId,
    /// Canonical hash of the scenario specification.
    pub spec_hash: HashDigest,
    /// Current stage identifier.
    pub current_stage_id: StageId,
    /// Timestamp when the current stage was entered.
    pub stage_entered_at: Timestamp,
    /// Run lifecycle status.
    pub status: RunStatus,
    /// Dispatch targets for disclosures.
    pub dispatch_targets: Vec<DispatchTarget>,
    /// Trigger log.
    pub triggers: Vec<TriggerRecord>,
    /// Gate evaluation log.
    pub gate_evals: Vec<GateEvalRecord>,
    /// Decision log.
    pub decisions: Vec<DecisionRecord>,
    /// Issued packet log.
    pub packets: Vec<PacketRecord>,
    /// Submissions log.
    pub submissions: Vec<SubmissionRecord>,
    /// Tool-call transcript.
    pub tool_calls: Vec<ToolCallRecord>,
}
