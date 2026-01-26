// decision-gate-core/src/runtime/engine.rs
// ============================================================================
// Module: Decision Gate Control Plane Engine
// Description: Deterministic evaluation, decision logging, and disclosure.
// Purpose: Execute Decision Gate scenarios with evidence-backed gates and idempotency.
// Dependencies: crate::{core, interfaces, runtime}
// ============================================================================

//! ## Overview
//! The control plane engine is the single canonical execution path for Decision Gate.
//! All API surfaces (HTTP, MCP, SDKs) must call into these methods to preserve
//! invariance and auditability.

// ============================================================================
// SECTION: Imports
// ============================================================================

use std::collections::BTreeMap;

use ret_logic::LogicMode;
use ret_logic::TriState;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;
use thiserror::Error;

use crate::core::AdvanceTo;
use crate::core::AnchorRequirement;
use crate::core::DecisionId;
use crate::core::DecisionOutcome;
use crate::core::DecisionRecord;
use crate::core::EvidenceAnchorPolicy;
use crate::core::EvidenceRecord;
use crate::core::EvidenceResult;
use crate::core::EvidenceValue;
use crate::core::GateEvalRecord;
use crate::core::GateEvaluation;
use crate::core::GateOutcome;
use crate::core::GateSpec;
use crate::core::NamespaceId;
use crate::core::PacketEnvelope;
use crate::core::PacketPayload;
use crate::core::PacketRecord;
use crate::core::PacketSpec;
use crate::core::PredicateKey;
use crate::core::PredicateSpec;
use crate::core::ProviderMissingError;
use crate::core::RunConfig;
use crate::core::RunId;
use crate::core::RunState;
use crate::core::RunStatus;
use crate::core::ScenarioId;
use crate::core::ScenarioSpec;
use crate::core::SpecError;
use crate::core::StageId;
use crate::core::StageSpec;
use crate::core::SubmissionRecord;
use crate::core::TenantId;
use crate::core::TimeoutPolicy;
use crate::core::Timestamp;
use crate::core::ToolCallError;
use crate::core::ToolCallErrorDetails;
use crate::core::ToolCallRecord;
use crate::core::TriggerEvent;
use crate::core::TriggerId;
use crate::core::TriggerKind;
use crate::core::TriggerRecord;
use crate::core::TrustLane;
use crate::core::TrustRequirement;
use crate::core::hashing::DEFAULT_HASH_ALGORITHM;
use crate::core::hashing::HashAlgorithm;
use crate::core::hashing::HashDigest;
use crate::core::hashing::HashError;
use crate::core::hashing::hash_bytes;
use crate::core::hashing::hash_canonical_json;
use crate::core::hashing::hash_canonical_json_with_limit;
use crate::core::summary::SafeSummary;
use crate::interfaces::ArtifactError;
use crate::interfaces::DispatchError;
use crate::interfaces::Dispatcher;
use crate::interfaces::EvidenceContext;
use crate::interfaces::EvidenceError;
use crate::interfaces::EvidenceProvider;
use crate::interfaces::PolicyDecider;
use crate::interfaces::PolicyDecision;
use crate::interfaces::PolicyError;
use crate::interfaces::RunStateStore;
use crate::interfaces::StoreError;
use crate::runtime::GateEvaluator;
use crate::runtime::comparator::evaluate_comparator;
use crate::runtime::gate::EvidenceSnapshot;
use crate::runtime::gate::collect_predicates;

// ============================================================================
// SECTION: Constants
// ============================================================================

/// Maximum bytes allowed for evidence payloads before hashing.
pub const MAX_EVIDENCE_VALUE_BYTES: usize = 1024 * 1024;
/// Maximum bytes allowed for packet and submission payloads before hashing.
pub const MAX_PAYLOAD_BYTES: usize = 2 * 1024 * 1024;

// ============================================================================
// SECTION: Control Plane Configuration
// ============================================================================

/// Configuration for the Decision Gate control plane engine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ControlPlaneConfig {
    /// Tri-state logic mode used for gate evaluation.
    pub logic_mode: LogicMode,
    /// Hash algorithm used for canonical hashing.
    pub hash_algorithm: HashAlgorithm,
    /// Minimum trust requirement for evidence.
    pub trust_requirement: TrustRequirement,
    /// Anchor policy requirements for evidence providers.
    pub anchor_policy: EvidenceAnchorPolicy,
}

impl Default for ControlPlaneConfig {
    fn default() -> Self {
        Self {
            logic_mode: LogicMode::Kleene,
            hash_algorithm: DEFAULT_HASH_ALGORITHM,
            trust_requirement: TrustRequirement::default(),
            anchor_policy: EvidenceAnchorPolicy::default(),
        }
    }
}

// ============================================================================
// SECTION: Control Plane Engine
// ============================================================================

/// Control plane engine implementing deterministic Decision Gate evaluation.
pub struct ControlPlane<P, D, S, Pol> {
    /// Scenario specification used for evaluation.
    spec: ScenarioSpec,
    /// Evidence provider implementation.
    evidence: P,
    /// Packet dispatcher implementation.
    dispatcher: D,
    /// Run state store implementation.
    store: S,
    /// Optional policy decider.
    policy: Option<Pol>,
    /// Control plane configuration.
    config: ControlPlaneConfig,
}

impl<P, D, S, Pol> ControlPlane<P, D, S, Pol>
where
    P: EvidenceProvider,
    D: Dispatcher,
    S: RunStateStore,
    Pol: PolicyDecider,
{
    /// Creates a new control plane engine.
    ///
    /// # Errors
    ///
    /// Returns [`ControlPlaneError::InvalidSpec`] when the scenario spec fails validation.
    pub fn new(
        spec: ScenarioSpec,
        evidence: P,
        dispatcher: D,
        store: S,
        policy: Option<Pol>,
        config: ControlPlaneConfig,
    ) -> Result<Self, ControlPlaneError> {
        spec.validate().map_err(ControlPlaneError::InvalidSpec)?;
        Ok(Self {
            spec,
            evidence,
            dispatcher,
            store,
            policy,
            config,
        })
    }

    /// Starts a new run and optionally issues initial stage packets.
    ///
    /// # Errors
    ///
    /// Returns [`ControlPlaneError`] when initialization fails.
    pub fn start_run(
        &self,
        config: RunConfig,
        started_at: Timestamp,
        dispatch_initial: bool,
    ) -> Result<RunState, ControlPlaneError> {
        if config.scenario_id != self.spec.scenario_id {
            return Err(ControlPlaneError::ScenarioMismatch(config.scenario_id.to_string()));
        }
        if config.namespace_id != self.spec.namespace_id {
            return Err(ControlPlaneError::NamespaceMismatch(config.namespace_id.to_string()));
        }

        if self.store.load(&config.tenant_id, &config.namespace_id, &config.run_id)?.is_some() {
            return Err(ControlPlaneError::RunAlreadyExists(config.run_id.to_string()));
        }

        let initial_stage =
            self.spec.stages.first().ok_or(ControlPlaneError::MissingStages)?.stage_id.clone();

        let spec_hash = self.spec.canonical_hash_with(self.config.hash_algorithm)?;

        let mut state = RunState {
            tenant_id: config.tenant_id,
            namespace_id: config.namespace_id,
            run_id: config.run_id,
            scenario_id: config.scenario_id,
            spec_hash,
            current_stage_id: initial_stage.clone(),
            stage_entered_at: started_at,
            status: RunStatus::Active,
            dispatch_targets: config.dispatch_targets,
            triggers: Vec::new(),
            gate_evals: Vec::new(),
            decisions: Vec::new(),
            packets: Vec::new(),
            submissions: Vec::new(),
            tool_calls: Vec::new(),
        };

        if dispatch_initial {
            let trigger_id = TriggerId::new("init");
            let init_trigger = TriggerEvent {
                trigger_id: trigger_id.clone(),
                tenant_id: state.tenant_id.clone(),
                namespace_id: state.namespace_id.clone(),
                run_id: state.run_id.clone(),
                kind: TriggerKind::ExternalEvent,
                time: started_at,
                source_id: "system".to_string(),
                payload: None,
                correlation_id: None,
            };
            let trigger_seq = next_seq(&state.triggers)?;
            state.triggers.push(TriggerRecord {
                seq: trigger_seq,
                event: init_trigger,
            });
            let decision_seq = next_seq(&state.decisions)?;
            let decision_id = DecisionId::new(format!("decision-{decision_seq}"));
            let decision = DecisionRecord {
                decision_id,
                seq: decision_seq,
                trigger_id,
                stage_id: initial_stage.clone(),
                decided_at: started_at,
                outcome: DecisionOutcome::Start {
                    stage_id: initial_stage.clone(),
                },
                correlation_id: None,
            };

            let packets = self.issue_stage_packets(&state, &decision, &initial_stage)?;
            state.packets.extend(packets);
            state.decisions.push(decision);
        }

        self.store.save(&state)?;
        Ok(state)
    }

    /// Returns the current status for a run.
    ///
    /// # Errors
    ///
    /// Returns [`ControlPlaneError::RunNotFound`] when the run does not exist.
    pub fn scenario_status(
        &self,
        request: &StatusRequest,
    ) -> Result<ScenarioStatus, ControlPlaneError> {
        let mut state =
            self.load_run(&request.tenant_id, &request.namespace_id, &request.run_id)?;
        let status = ScenarioStatus::from_state(&state);
        let call_id = format!("call-{}", state.tool_calls.len() + 1);
        let tool_record = build_tool_call_record(
            "scenario.status",
            request,
            &status,
            request.requested_at,
            self.config.hash_algorithm,
            call_id,
            request.correlation_id.clone(),
        )?;
        state.tool_calls.push(tool_record);
        self.store.save(&state)?;
        Ok(status)
    }

    /// Processes a pull-mode `scenario.next` request.
    ///
    /// # Errors
    ///
    /// Returns [`ControlPlaneError`] when trigger evaluation fails.
    pub fn scenario_next(&self, request: &NextRequest) -> Result<NextResult, ControlPlaneError> {
        let trigger = TriggerEvent {
            trigger_id: request.trigger_id.clone(),
            tenant_id: request.tenant_id.clone(),
            namespace_id: request.namespace_id.clone(),
            run_id: request.run_id.clone(),
            kind: TriggerKind::AgentRequestNext,
            time: request.time,
            source_id: request.agent_id.clone(),
            payload: None,
            correlation_id: request.correlation_id.clone(),
        };

        let mut state =
            self.load_run(&request.tenant_id, &request.namespace_id, &request.run_id)?;
        if let Err(err) = self.evidence.validate_providers(&self.spec) {
            let tool_error = provider_missing_tool_error(&err);
            let call_id = format!("call-{}", state.tool_calls.len() + 1);
            let tool_record = build_tool_call_record_error(
                "scenario.next",
                request,
                &tool_error,
                request.time,
                self.config.hash_algorithm,
                call_id,
                request.correlation_id.clone(),
            )?;
            state.tool_calls.push(tool_record);
            self.store.save(&state)?;
            return Err(ControlPlaneError::ProviderMissing(err));
        }

        let (mut state, result) = self.handle_trigger_internal(state, &trigger)?;
        let next_result = NextResult::from_eval(result);
        let call_id = format!("call-{}", state.tool_calls.len() + 1);
        let tool_record = build_tool_call_record(
            "scenario.next",
            request,
            &next_result,
            request.time,
            self.config.hash_algorithm,
            call_id,
            request.correlation_id.clone(),
        )?;
        state.tool_calls.push(tool_record);
        self.store.save(&state)?;

        Ok(next_result)
    }

    /// Records a model submission.
    ///
    /// # Errors
    ///
    /// Returns [`ControlPlaneError`] when submission recording fails.
    pub fn scenario_submit(
        &self,
        request: &SubmitRequest,
    ) -> Result<SubmitResult, ControlPlaneError> {
        let mut state =
            self.load_run(&request.tenant_id, &request.namespace_id, &request.run_id)?;
        if let Some(existing) = state
            .submissions
            .iter()
            .find(|record| record.submission_id == request.submission_id)
            .cloned()
        {
            if existing.content_type == request.content_type && existing.payload == request.payload
            {
                let submit_result = SubmitResult {
                    record: existing,
                };
                let call_id = format!("call-{}", state.tool_calls.len() + 1);
                let tool_record = build_tool_call_record(
                    "scenario.submit",
                    request,
                    &submit_result,
                    request.submitted_at,
                    self.config.hash_algorithm,
                    call_id,
                    request.correlation_id.clone(),
                )?;
                state.tool_calls.push(tool_record);
                self.store.save(&state)?;
                return Ok(submit_result);
            }

            let tool_error = ToolCallError {
                code: "submission_conflict".to_string(),
                message: "submission_id conflicts with existing record".to_string(),
                details: Some(ToolCallErrorDetails::Message {
                    info: format!("submission_id={}", request.submission_id),
                }),
            };
            let call_id = format!("call-{}", state.tool_calls.len() + 1);
            let tool_record = build_tool_call_record_error(
                "scenario.submit",
                request,
                &tool_error,
                request.submitted_at,
                self.config.hash_algorithm,
                call_id,
                request.correlation_id.clone(),
            )?;
            state.tool_calls.push(tool_record);
            self.store.save(&state)?;
            return Err(ControlPlaneError::SubmissionConflict(request.submission_id.clone()));
        }

        let content_hash = payload_hash(&request.payload, self.config.hash_algorithm)?;
        let record = SubmissionRecord {
            submission_id: request.submission_id.clone(),
            run_id: request.run_id.clone(),
            payload: request.payload.clone(),
            content_type: request.content_type.clone(),
            content_hash,
            submitted_at: request.submitted_at,
            correlation_id: request.correlation_id.clone(),
        };
        state.submissions.push(record.clone());
        let submit_result = SubmitResult {
            record,
        };
        let call_id = format!("call-{}", state.tool_calls.len() + 1);
        let tool_record = build_tool_call_record(
            "scenario.submit",
            request,
            &submit_result,
            request.submitted_at,
            self.config.hash_algorithm,
            call_id,
            request.correlation_id.clone(),
        )?;
        state.tool_calls.push(tool_record);
        self.store.save(&state)?;

        Ok(submit_result)
    }

    /// Processes an external trigger event.
    ///
    /// # Errors
    ///
    /// Returns [`ControlPlaneError`] when trigger evaluation fails.
    pub fn trigger(&self, trigger: &TriggerEvent) -> Result<TriggerResult, ControlPlaneError> {
        let mut state =
            self.load_run(&trigger.tenant_id, &trigger.namespace_id, &trigger.run_id)?;
        if let Err(err) = self.evidence.validate_providers(&self.spec) {
            let tool_error = provider_missing_tool_error(&err);
            let call_id = format!("call-{}", state.tool_calls.len() + 1);
            let tool_record = build_tool_call_record_error(
                "scenario.trigger",
                trigger,
                &tool_error,
                trigger.time,
                self.config.hash_algorithm,
                call_id,
                trigger.correlation_id.clone(),
            )?;
            state.tool_calls.push(tool_record);
            self.store.save(&state)?;
            return Err(ControlPlaneError::ProviderMissing(err));
        }

        let (mut state, result) = self.handle_trigger_internal(state, trigger)?;
        let trigger_result = TriggerResult::from_eval(result);
        let call_id = format!("call-{}", state.tool_calls.len() + 1);
        let tool_record = build_tool_call_record(
            "scenario.trigger",
            trigger,
            &trigger_result,
            trigger.time,
            self.config.hash_algorithm,
            call_id,
            trigger.correlation_id.clone(),
        )?;
        state.tool_calls.push(tool_record);
        self.store.save(&state)?;
        Ok(trigger_result)
    }

    /// Evaluates a stage using asserted evidence without mutating run state.
    ///
    /// # Errors
    ///
    /// Returns [`ControlPlaneError`] when precheck evaluation fails.
    pub fn precheck(&self, request: &PrecheckRequest) -> Result<PrecheckResult, ControlPlaneError> {
        let stage_id = if let Some(stage_id) = &request.stage_id {
            stage_id.clone()
        } else {
            self.spec.stages.first().ok_or(ControlPlaneError::MissingStages)?.stage_id.clone()
        };
        let stage_def = stage_spec(&self.spec, &stage_id)?;
        let predicate_specs = predicate_specs(&self.spec, stage_def)?;
        let default_requirement = self.config.trust_requirement;
        let mut predicate_requirements = BTreeMap::new();
        for spec in &predicate_specs {
            let requirement = spec.trust.unwrap_or(default_requirement);
            let requirement = default_requirement.stricter(requirement);
            predicate_requirements.insert(spec.predicate.clone(), requirement);
        }

        let mut evidence_records = Vec::with_capacity(predicate_specs.len());
        for spec in &predicate_specs {
            let result =
                request.evidence.get(&spec.predicate).cloned().unwrap_or_else(|| EvidenceResult {
                    value: None,
                    lane: TrustLane::Asserted,
                    evidence_hash: None,
                    evidence_ref: None,
                    evidence_anchor: None,
                    signature: None,
                    content_type: None,
                });
            let normalized = normalize_evidence_result(&result, self.config.hash_algorithm)?;
            let status = evaluate_comparator(spec.comparator, spec.expected.as_ref(), &normalized);
            evidence_records.push(EvidenceRecord {
                predicate: spec.predicate.clone(),
                status,
                result: normalized,
                error: None,
            });
        }

        let evaluator = GateEvaluator::new(self.config.logic_mode);
        let mut gate_evaluations = Vec::new();
        let mut gate_outcomes = Vec::new();
        for gate in &stage_def.gates {
            let gate_requirement =
                default_requirement.stricter(gate.trust.unwrap_or(default_requirement));
            let gate_evidence = evidence_for_gate(&evidence_records, gate);
            let adjusted_evidence: Vec<EvidenceRecord> = gate_evidence
                .into_iter()
                .map(|record| {
                    let predicate_requirement = predicate_requirements
                        .get(&record.predicate)
                        .copied()
                        .unwrap_or(default_requirement);
                    let effective_requirement = predicate_requirement.stricter(gate_requirement);
                    apply_trust_requirement(record, effective_requirement)
                })
                .collect();
            let snapshot = EvidenceSnapshot::new(adjusted_evidence);
            let evaluation = evaluator.evaluate_gate(gate, &snapshot);
            gate_outcomes.push((gate.gate_id.clone(), evaluation.status));
            gate_evaluations.push(evaluation);
        }

        let decision =
            if gate_evaluations.iter().all(|evaluation| evaluation.status == TriState::True) {
                if let Some(next_stage) =
                    resolve_next_stage_precheck(&self.spec, stage_def, &gate_outcomes)?
                {
                    DecisionOutcome::Advance {
                        from_stage: stage_id,
                        to_stage: next_stage,
                        timeout: false,
                    }
                } else {
                    DecisionOutcome::Complete {
                        stage_id,
                    }
                }
            } else {
                let unmet = gate_evaluations
                    .iter()
                    .filter(|evaluation| evaluation.status != TriState::True)
                    .map(|evaluation| evaluation.gate_id.clone())
                    .collect();
                DecisionOutcome::Hold {
                    summary: SafeSummary {
                        status: "hold".to_string(),
                        unmet_gates: unmet,
                        retry_hint: Some("await_evidence".to_string()),
                        policy_tags: Vec::new(),
                    },
                }
            };

        Ok(PrecheckResult {
            decision,
            gate_evaluations,
        })
    }

    /// Evaluates a trigger and returns the updated state plus result.
    fn handle_trigger_internal(
        &self,
        mut state: RunState,
        trigger: &TriggerEvent,
    ) -> Result<(RunState, EvaluationResult), ControlPlaneError> {
        if state.status != RunStatus::Active {
            let result = inactive_result(&state)?;
            return Ok((state, result));
        }

        if trigger.run_id != state.run_id {
            return Err(ControlPlaneError::RunMismatch(trigger.run_id.to_string()));
        }

        if let Some(result) = existing_trigger_result(&state, trigger) {
            return Ok((state, result));
        }

        record_trigger(&mut state, trigger)?;

        let stage_def = stage_spec(&self.spec, &state.current_stage_id)?;
        if let Some(result) = self.handle_timeout(&mut state, trigger, stage_def)? {
            return Ok((state, result));
        }
        let evidence_context = build_evidence_context(&state, trigger);
        let gate_results =
            self.evaluate_stage_gates(&state, trigger, stage_def, &evidence_context)?;
        state.gate_evals.extend(gate_results.records.clone());
        let (decision, packets) = self.decide_from_gate_evals(
            &mut state,
            trigger,
            stage_def,
            &gate_results.records,
            &gate_results.outcomes,
        )?;

        state.decisions.push(decision.clone());
        state.packets.extend(packets.clone());

        let result = EvaluationResult {
            decision,
            packets,
            status: state.status,
        };

        Ok((state, result))
    }

    /// Evaluates all gates for the stage using the provided evidence context.
    fn evaluate_stage_gates(
        &self,
        state: &RunState,
        trigger: &TriggerEvent,
        stage_def: &StageSpec,
        evidence_context: &EvidenceContext,
    ) -> Result<GateEvaluationOutcome, ControlPlaneError> {
        let predicate_specs = predicate_specs(&self.spec, stage_def)?;
        let evidence_records = self.evaluate_predicates(&predicate_specs, evidence_context)?;
        let default_requirement = self.config.trust_requirement;
        let mut predicate_requirements = BTreeMap::new();
        for spec in &predicate_specs {
            let requirement = spec.trust.unwrap_or(default_requirement);
            let requirement = default_requirement.stricter(requirement);
            predicate_requirements.insert(spec.predicate.clone(), requirement);
        }
        let evaluator = GateEvaluator::new(self.config.logic_mode);
        let mut gate_eval_records = Vec::new();
        let mut gate_outcomes = Vec::new();

        for gate in &stage_def.gates {
            let gate_requirement =
                default_requirement.stricter(gate.trust.unwrap_or(default_requirement));
            let gate_evidence = evidence_for_gate(&evidence_records, gate);
            let adjusted_evidence: Vec<EvidenceRecord> = gate_evidence
                .into_iter()
                .map(|record| {
                    let predicate_requirement = predicate_requirements
                        .get(&record.predicate)
                        .copied()
                        .unwrap_or(default_requirement);
                    let effective_requirement = predicate_requirement.stricter(gate_requirement);
                    apply_trust_requirement(record, effective_requirement)
                })
                .collect();
            let snapshot = EvidenceSnapshot::new(adjusted_evidence.clone());
            let evaluation = evaluator.evaluate_gate(gate, &snapshot);
            gate_outcomes.push((gate.gate_id.clone(), evaluation.status));
            gate_eval_records.push(GateEvalRecord {
                trigger_id: trigger.trigger_id.clone(),
                stage_id: state.current_stage_id.clone(),
                evaluation,
                evidence: adjusted_evidence,
            });
        }

        Ok(GateEvaluationOutcome {
            records: gate_eval_records,
            outcomes: gate_outcomes,
        })
    }

    /// Builds a decision and packets from evaluated gate records.
    fn decide_from_gate_evals(
        &self,
        state: &mut RunState,
        trigger: &TriggerEvent,
        stage_def: &StageSpec,
        gate_eval_records: &[GateEvalRecord],
        gate_outcomes: &[(crate::core::GateId, TriState)],
    ) -> Result<(DecisionRecord, Vec<PacketRecord>), ControlPlaneError> {
        let decision_seq = next_seq(&state.decisions)?;
        let decision_id = DecisionId::new(format!("decision-{decision_seq}"));
        if gates_passed(gate_eval_records) {
            if let Some(next_stage) =
                resolve_next_stage(&self.spec, stage_def, gate_outcomes, state)?
            {
                let decision = DecisionRecord {
                    decision_id: decision_id.clone(),
                    seq: decision_seq,
                    trigger_id: trigger.trigger_id.clone(),
                    stage_id: state.current_stage_id.clone(),
                    decided_at: trigger.time,
                    outcome: DecisionOutcome::Advance {
                        from_stage: state.current_stage_id.clone(),
                        to_stage: next_stage.clone(),
                        timeout: false,
                    },
                    correlation_id: trigger.correlation_id.clone(),
                };

                match self.issue_stage_packets(state, &decision, &next_stage) {
                    Ok(packets) => {
                        state.current_stage_id = next_stage.clone();
                        state.stage_entered_at = trigger.time;
                        Ok((decision, packets))
                    }
                    Err(err) => {
                        state.status = RunStatus::Failed;
                        let fail_decision = DecisionRecord {
                            decision_id,
                            seq: decision_seq,
                            trigger_id: trigger.trigger_id.clone(),
                            stage_id: state.current_stage_id.clone(),
                            decided_at: trigger.time,
                            outcome: DecisionOutcome::Fail {
                                reason: err.to_string(),
                            },
                            correlation_id: trigger.correlation_id.clone(),
                        };
                        Ok((fail_decision, Vec::new()))
                    }
                }
            } else {
                let decision = DecisionRecord {
                    decision_id,
                    seq: decision_seq,
                    trigger_id: trigger.trigger_id.clone(),
                    stage_id: state.current_stage_id.clone(),
                    decided_at: trigger.time,
                    outcome: DecisionOutcome::Complete {
                        stage_id: state.current_stage_id.clone(),
                    },
                    correlation_id: trigger.correlation_id.clone(),
                };
                state.status = RunStatus::Completed;
                Ok((decision, Vec::new()))
            }
        } else {
            let summary = build_safe_summary(gate_eval_records);
            let decision = DecisionRecord {
                decision_id,
                seq: decision_seq,
                trigger_id: trigger.trigger_id.clone(),
                stage_id: state.current_stage_id.clone(),
                decided_at: trigger.time,
                outcome: DecisionOutcome::Hold {
                    summary,
                },
                correlation_id: trigger.correlation_id.clone(),
            };
            Ok((decision, Vec::new()))
        }
    }

    /// Applies timeout policy for tick triggers when the stage timeout has elapsed.
    fn handle_timeout(
        &self,
        state: &mut RunState,
        trigger: &TriggerEvent,
        stage_def: &StageSpec,
    ) -> Result<Option<EvaluationResult>, ControlPlaneError> {
        if trigger.kind != TriggerKind::Tick {
            return Ok(None);
        }

        let Some(timeout_spec) = &stage_def.timeout else {
            return Ok(None);
        };

        if !timeout_expired(state.stage_entered_at, trigger.time, timeout_spec.timeout_ms)? {
            return Ok(None);
        }

        let decision_seq = next_seq(&state.decisions)?;
        let decision_id = DecisionId::new(format!("decision-{decision_seq}"));

        let (decision, packets) = match stage_def.on_timeout {
            TimeoutPolicy::Fail => {
                state.status = RunStatus::Failed;
                (
                    DecisionRecord {
                        decision_id,
                        seq: decision_seq,
                        trigger_id: trigger.trigger_id.clone(),
                        stage_id: state.current_stage_id.clone(),
                        decided_at: trigger.time,
                        outcome: DecisionOutcome::Fail {
                            reason: "timeout".to_string(),
                        },
                        correlation_id: trigger.correlation_id.clone(),
                    },
                    Vec::new(),
                )
            }
            TimeoutPolicy::AdvanceWithFlag => self.timeout_advance(
                state,
                trigger,
                stage_def,
                decision_id,
                decision_seq,
                TriState::True,
            )?,
            TimeoutPolicy::AlternateBranch => self.timeout_advance(
                state,
                trigger,
                stage_def,
                decision_id,
                decision_seq,
                TriState::Unknown,
            )?,
        };

        state.decisions.push(decision.clone());
        state.packets.extend(packets.clone());

        Ok(Some(EvaluationResult {
            decision,
            packets,
            status: state.status,
        }))
    }

    /// Advances the run on timeout using a synthetic outcome.
    fn timeout_advance(
        &self,
        state: &mut RunState,
        trigger: &TriggerEvent,
        stage_def: &StageSpec,
        decision_id: DecisionId,
        decision_seq: u64,
        synthetic_outcome: TriState,
    ) -> Result<(DecisionRecord, Vec<PacketRecord>), ControlPlaneError> {
        let gate_outcomes = synthetic_gate_outcomes(stage_def, synthetic_outcome);
        if let Some(next_stage) = resolve_next_stage(&self.spec, stage_def, &gate_outcomes, state)?
        {
            let decision = DecisionRecord {
                decision_id: decision_id.clone(),
                seq: decision_seq,
                trigger_id: trigger.trigger_id.clone(),
                stage_id: state.current_stage_id.clone(),
                decided_at: trigger.time,
                outcome: DecisionOutcome::Advance {
                    from_stage: state.current_stage_id.clone(),
                    to_stage: next_stage.clone(),
                    timeout: true,
                },
                correlation_id: trigger.correlation_id.clone(),
            };

            match self.issue_stage_packets(state, &decision, &next_stage) {
                Ok(packets) => {
                    state.current_stage_id = next_stage.clone();
                    state.stage_entered_at = trigger.time;
                    Ok((decision, packets))
                }
                Err(err) => {
                    state.status = RunStatus::Failed;
                    let fail_decision = DecisionRecord {
                        decision_id,
                        seq: decision_seq,
                        trigger_id: trigger.trigger_id.clone(),
                        stage_id: state.current_stage_id.clone(),
                        decided_at: trigger.time,
                        outcome: DecisionOutcome::Fail {
                            reason: err.to_string(),
                        },
                        correlation_id: trigger.correlation_id.clone(),
                    };
                    Ok((fail_decision, Vec::new()))
                }
            }
        } else {
            state.status = RunStatus::Completed;
            Ok((
                DecisionRecord {
                    decision_id,
                    seq: decision_seq,
                    trigger_id: trigger.trigger_id.clone(),
                    stage_id: state.current_stage_id.clone(),
                    decided_at: trigger.time,
                    outcome: DecisionOutcome::Complete {
                        stage_id: state.current_stage_id.clone(),
                    },
                    correlation_id: trigger.correlation_id.clone(),
                },
                Vec::new(),
            ))
        }
    }

    /// Evaluates predicate specs against evidence providers.
    fn evaluate_predicates(
        &self,
        predicate_specs: &[PredicateSpec],
        context: &EvidenceContext,
    ) -> Result<Vec<EvidenceRecord>, ControlPlaneError> {
        let mut records = Vec::with_capacity(predicate_specs.len());
        for spec in predicate_specs {
            let (mut result, mut error) = match self.evidence.query(&spec.query, context) {
                Ok(result) => (result, None),
                Err(err) => (
                    Self::empty_verified_result(),
                    Some(crate::core::EvidenceProviderError {
                        code: "provider_error".to_string(),
                        message: err.to_string(),
                    }),
                ),
            };
            if error.is_none()
                && let Some(requirement) =
                    self.config.anchor_policy.requirement_for(&spec.query.provider_id)
                && let Err(message) = Self::validate_anchor_requirement(requirement, &result)
            {
                result = Self::empty_verified_result();
                error = Some(crate::core::EvidenceProviderError {
                    code: "anchor_invalid".to_string(),
                    message,
                });
            }
            let normalized = normalize_evidence_result(&result, self.config.hash_algorithm)?;
            let status = if error.is_some() {
                TriState::Unknown
            } else {
                evaluate_comparator(spec.comparator, spec.expected.as_ref(), &normalized)
            };
            records.push(EvidenceRecord {
                predicate: spec.predicate.clone(),
                status,
                result: normalized,
                error,
            });
        }
        Ok(records)
    }

    /// Returns an empty verified evidence result for error paths.
    const fn empty_verified_result() -> EvidenceResult {
        EvidenceResult {
            value: None,
            lane: TrustLane::Verified,
            evidence_hash: None,
            evidence_ref: None,
            evidence_anchor: None,
            signature: None,
            content_type: None,
        }
    }

    /// Validates anchor metadata against the configured requirement.
    fn validate_anchor_requirement(
        requirement: &AnchorRequirement,
        result: &EvidenceResult,
    ) -> Result<(), String> {
        let anchor =
            result.evidence_anchor.as_ref().ok_or_else(|| "missing evidence_anchor".to_string())?;
        if anchor.anchor_type != requirement.anchor_type {
            return Err(format!(
                "anchor_type mismatch: expected {} got {}",
                requirement.anchor_type, anchor.anchor_type
            ));
        }
        let value: Value = serde_json::from_str(&anchor.anchor_value)
            .map_err(|_| "anchor_value must be canonical JSON".to_string())?;
        let object =
            value.as_object().ok_or_else(|| "anchor_value must be a JSON object".to_string())?;
        for field in &requirement.required_fields {
            match object.get(field) {
                Some(Value::String(_) | Value::Number(_)) => {}
                Some(Value::Bool(_)) => {
                    return Err(format!("anchor field {field} must be string or number"));
                }
                Some(Value::Null) => {
                    return Err(format!("anchor field {field} must be set"));
                }
                Some(Value::Array(_) | Value::Object(_)) => {
                    return Err(format!("anchor field {field} must be scalar"));
                }
                None => return Err(format!("anchor field {field} missing")),
            }
        }
        Ok(())
    }

    /// Issues disclosure packets for a stage decision.
    fn issue_stage_packets(
        &self,
        state: &RunState,
        decision: &DecisionRecord,
        stage_id: &StageId,
    ) -> Result<Vec<PacketRecord>, ControlPlaneError> {
        let stage_def = stage_spec(&self.spec, stage_id)?;
        let mut packets = Vec::new();
        for spec in &stage_def.entry_packets {
            let envelope = build_packet_envelope(
                &self.spec,
                state,
                stage_id,
                spec,
                decision,
                self.config.hash_algorithm,
                decision.decided_at,
            )?;
            let payload = spec.payload.clone();
            let receipts = self.dispatch_packet(state, &envelope, &payload)?;
            packets.push(PacketRecord {
                envelope,
                payload,
                receipts,
                decision_id: decision.decision_id.clone(),
            });
        }
        Ok(packets)
    }

    /// Dispatches a packet to all configured targets.
    fn dispatch_packet(
        &self,
        state: &RunState,
        envelope: &PacketEnvelope,
        payload: &PacketPayload,
    ) -> Result<Vec<crate::core::DispatchReceipt>, ControlPlaneError> {
        let mut receipts = Vec::new();
        for target in &state.dispatch_targets {
            if let Some(policy) = &self.policy {
                let decision = policy.authorize(target, envelope, payload)?;
                if decision == PolicyDecision::Deny {
                    return Err(ControlPlaneError::PolicyDenied);
                }
            }
            let receipt = self.dispatcher.dispatch(target, envelope, payload)?;
            receipts.push(receipt);
        }
        Ok(receipts)
    }

    /// Loads the run state or returns an error if missing.
    fn load_run(
        &self,
        tenant_id: &TenantId,
        namespace_id: &NamespaceId,
        run_id: &RunId,
    ) -> Result<RunState, ControlPlaneError> {
        self.store
            .load(tenant_id, namespace_id, run_id)?
            .ok_or_else(|| ControlPlaneError::RunNotFound(run_id.to_string()))
    }
}

// ============================================================================
// SECTION: Requests and Results
// ============================================================================

/// Request payload for `scenario.status`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StatusRequest {
    /// Run identifier.
    pub run_id: RunId,
    /// Tenant identifier.
    pub tenant_id: TenantId,
    /// Namespace identifier.
    pub namespace_id: NamespaceId,
    /// Request timestamp.
    pub requested_at: Timestamp,
    /// Optional correlation identifier.
    pub correlation_id: Option<crate::core::CorrelationId>,
}

/// Pull-mode request for `scenario.next`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NextRequest {
    /// Run identifier.
    pub run_id: RunId,
    /// Tenant identifier.
    pub tenant_id: TenantId,
    /// Namespace identifier.
    pub namespace_id: NamespaceId,
    /// Trigger identifier.
    pub trigger_id: TriggerId,
    /// Agent identifier.
    pub agent_id: String,
    /// Trigger timestamp.
    pub time: Timestamp,
    /// Optional correlation identifier.
    pub correlation_id: Option<crate::core::CorrelationId>,
}

/// Request payload for `scenario.submit`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SubmitRequest {
    /// Run identifier.
    pub run_id: RunId,
    /// Tenant identifier.
    pub tenant_id: TenantId,
    /// Namespace identifier.
    pub namespace_id: NamespaceId,
    /// Submission identifier.
    pub submission_id: String,
    /// Submission payload.
    pub payload: PacketPayload,
    /// Submission content type.
    pub content_type: String,
    /// Submission timestamp.
    pub submitted_at: Timestamp,
    /// Optional correlation identifier.
    pub correlation_id: Option<crate::core::CorrelationId>,
}

/// Request payload for precheck evaluation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrecheckRequest {
    /// Optional stage identifier override.
    pub stage_id: Option<StageId>,
    /// Asserted evidence keyed by predicate identifier.
    pub evidence: BTreeMap<PredicateKey, EvidenceResult>,
}

/// Result returned by `scenario.next`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NextResult {
    /// Decision record produced by Decision Gate.
    pub decision: DecisionRecord,
    /// Packet records dispatched for the decision.
    pub packets: Vec<PacketRecord>,
    /// Run status after evaluation.
    pub status: RunStatus,
}

impl NextResult {
    /// Builds a `NextResult` from an evaluation result.
    fn from_eval(result: EvaluationResult) -> Self {
        Self {
            decision: result.decision,
            packets: result.packets,
            status: result.status,
        }
    }
}

/// Result returned by `scenario.submit`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SubmitResult {
    /// Submission record appended to the run state.
    pub record: SubmissionRecord,
}

/// Result returned by `scenario.trigger`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TriggerResult {
    /// Decision record produced by Decision Gate.
    pub decision: DecisionRecord,
    /// Packet records dispatched for the decision.
    pub packets: Vec<PacketRecord>,
    /// Run status after evaluation.
    pub status: RunStatus,
}

/// Result returned by precheck evaluation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrecheckResult {
    /// Predicted decision outcome.
    pub decision: DecisionOutcome,
    /// Gate evaluations for the stage.
    pub gate_evaluations: Vec<GateEvaluation>,
}

impl TriggerResult {
    /// Builds a `TriggerResult` from an evaluation result.
    fn from_eval(result: EvaluationResult) -> Self {
        Self {
            decision: result.decision,
            packets: result.packets,
            status: result.status,
        }
    }
}

/// Evaluation result produced by the control plane engine.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvaluationResult {
    /// Decision record.
    pub decision: DecisionRecord,
    /// Packets issued by the decision.
    pub packets: Vec<PacketRecord>,
    /// Run status after evaluation.
    pub status: RunStatus,
}

/// Scenario status response.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScenarioStatus {
    /// Run identifier.
    pub run_id: RunId,
    /// Scenario identifier.
    pub scenario_id: ScenarioId,
    /// Current stage identifier.
    pub current_stage_id: StageId,
    /// Run status.
    pub status: RunStatus,
    /// Last decision record, if any.
    pub last_decision: Option<DecisionRecord>,
    /// Issued packet identifiers.
    pub issued_packet_ids: Vec<crate::core::PacketId>,
    /// Safe summary for unmet gates, if applicable.
    pub safe_summary: Option<SafeSummary>,
}

impl ScenarioStatus {
    /// Builds a status response from the current run state.
    fn from_state(state: &RunState) -> Self {
        let last_decision = state.decisions.last().cloned();
        let safe_summary = last_decision.as_ref().and_then(|decision| match &decision.outcome {
            DecisionOutcome::Hold {
                summary,
            } => Some(summary.clone()),
            _ => None,
        });

        let issued_packet_ids =
            state.packets.iter().map(|packet| packet.envelope.packet_id.clone()).collect();

        Self {
            run_id: state.run_id.clone(),
            scenario_id: state.scenario_id.clone(),
            current_stage_id: state.current_stage_id.clone(),
            status: state.status,
            last_decision,
            issued_packet_ids,
            safe_summary,
        }
    }
}

// ============================================================================
// SECTION: Errors
// ============================================================================

/// Control plane execution errors.
#[derive(Debug, Error)]
pub enum ControlPlaneError {
    /// Scenario spec failed validation.
    #[error("invalid scenario spec: {0}")]
    InvalidSpec(#[from] SpecError),
    /// Scenario spec contains no stages.
    #[error("scenario spec contains no stages")]
    MissingStages,
    /// Scenario identifier mismatch.
    #[error("scenario mismatch for run: {0}")]
    ScenarioMismatch(String),
    /// Namespace identifier mismatch.
    #[error("namespace mismatch for run: {0}")]
    NamespaceMismatch(String),
    /// Run already exists.
    #[error("run already exists: {0}")]
    RunAlreadyExists(String),
    /// Run not found.
    #[error("run not found: {0}")]
    RunNotFound(String),
    /// Run mismatch for trigger.
    #[error("trigger run mismatch: {0}")]
    RunMismatch(String),
    /// Run is inactive.
    #[error("run is not active: {0:?}")]
    RunInactive(RunStatus),
    /// Submission id conflicts with existing record.
    #[error("submission_id conflict: {0}")]
    SubmissionConflict(String),
    /// Run state sequence counters exceeded addressable range.
    #[error("run state sequence overflow")]
    SequenceOverflow,
    /// Evidence payload exceeds size limits.
    #[error("evidence payload exceeds size limit ({actual} > {max})")]
    EvidenceTooLarge {
        /// Maximum allowed bytes.
        max: usize,
        /// Actual payload size in bytes.
        actual: usize,
    },
    /// Packet payload exceeds size limits.
    #[error("payload exceeds size limit ({actual} > {max})")]
    PayloadTooLarge {
        /// Maximum allowed bytes.
        max: usize,
        /// Actual payload size in bytes.
        actual: usize,
    },
    /// Stage identifier not found.
    #[error("unknown stage identifier: {0}")]
    StageNotFound(String),
    /// Gate resolution failed.
    #[error("gate resolution failed: {0}")]
    GateResolutionFailed(String),
    /// Trigger timestamp kind mismatch.
    #[error("trigger time kind mismatch: {0}")]
    TriggerTimeMismatch(String),
    /// Timeout calculation error.
    #[error("timeout calculation error: {0}")]
    TimeoutCalculation(String),
    /// Policy denied disclosure.
    #[error("policy denied disclosure")]
    PolicyDenied,
    /// Required evidence providers are missing or blocked.
    #[error("provider validation failed: {0:?}")]
    ProviderMissing(ProviderMissingError),
    /// Evidence provider error.
    #[error(transparent)]
    Evidence(#[from] EvidenceError),
    /// Dispatcher error.
    #[error(transparent)]
    Dispatch(#[from] DispatchError),
    /// Policy decision error.
    #[error(transparent)]
    Policy(#[from] PolicyError),
    /// Run state store error.
    #[error(transparent)]
    Store(#[from] StoreError),
    /// Hashing error.
    #[error(transparent)]
    Hash(#[from] HashError),
    /// Tool-call hashing error.
    #[error("tool-call hashing error: {0}")]
    ToolCallHash(String),
    /// Payload hashing error.
    #[error("payload hashing error: {0}")]
    PayloadHash(String),
    /// Artifact error.
    #[error(transparent)]
    Artifact(#[from] ArtifactError),
}

// ============================================================================
// SECTION: Helper Functions
// ============================================================================

/// Returns the stage specification for the provided stage id.
fn stage_spec<'a>(
    spec: &'a ScenarioSpec,
    stage_id: &StageId,
) -> Result<&'a crate::core::StageSpec, ControlPlaneError> {
    spec.stages
        .iter()
        .find(|stage| &stage.stage_id == stage_id)
        .ok_or_else(|| ControlPlaneError::StageNotFound(stage_id.to_string()))
}

/// Collects predicate specs referenced by stage gates.
fn predicate_specs(
    spec: &ScenarioSpec,
    stage: &crate::core::StageSpec,
) -> Result<Vec<PredicateSpec>, ControlPlaneError> {
    let mut keys = Vec::new();
    for gate in &stage.gates {
        for key in collect_predicates(&gate.requirement) {
            if !keys.contains(&key) {
                keys.push(key);
            }
        }
    }

    let mut specs = Vec::new();
    for key in keys {
        let predicate = spec
            .predicates
            .iter()
            .find(|spec| spec.predicate == key)
            .ok_or_else(|| ControlPlaneError::GateResolutionFailed(key.to_string()))?;
        specs.push(predicate.clone());
    }
    Ok(specs)
}

/// Filters evidence records relevant to a gate.
/// Filters evidence records relevant to a gate.
fn evidence_for_gate(records: &[EvidenceRecord], gate: &GateSpec) -> Vec<EvidenceRecord> {
    let predicates = collect_predicates(&gate.requirement);
    records.iter().filter(|record| predicates.contains(&record.predicate)).cloned().collect()
}

/// Container for gate evaluation records and summarized outcomes.
struct GateEvaluationOutcome {
    /// Full gate evaluation records.
    records: Vec<GateEvalRecord>,
    /// Gate outcomes keyed by gate identifier.
    outcomes: Vec<(crate::core::GateId, TriState)>,
}

/// Builds an evaluation result for inactive runs using the latest decision.
fn inactive_result(state: &RunState) -> Result<EvaluationResult, ControlPlaneError> {
    let decision =
        state.decisions.last().cloned().ok_or(ControlPlaneError::RunInactive(state.status))?;
    let packets = packets_for_decision(state, &decision.decision_id);
    Ok(EvaluationResult {
        decision,
        packets,
        status: state.status,
    })
}

/// Returns an existing decision result for an already-processed trigger.
fn existing_trigger_result(state: &RunState, trigger: &TriggerEvent) -> Option<EvaluationResult> {
    state.decisions.iter().find(|decision| decision.trigger_id == trigger.trigger_id).cloned().map(
        |decision| EvaluationResult {
            packets: packets_for_decision(state, &decision.decision_id),
            status: state.status,
            decision,
        },
    )
}

/// Appends a trigger record to the run state.
fn record_trigger(state: &mut RunState, trigger: &TriggerEvent) -> Result<(), ControlPlaneError> {
    let trigger_seq = next_seq(&state.triggers)?;
    state.triggers.push(TriggerRecord {
        seq: trigger_seq,
        event: trigger.clone(),
    });
    Ok(())
}

/// Builds evidence context from the run state and trigger metadata.
fn build_evidence_context(state: &RunState, trigger: &TriggerEvent) -> EvidenceContext {
    EvidenceContext {
        tenant_id: state.tenant_id.clone(),
        namespace_id: state.namespace_id.clone(),
        run_id: state.run_id.clone(),
        scenario_id: state.scenario_id.clone(),
        stage_id: state.current_stage_id.clone(),
        trigger_id: trigger.trigger_id.clone(),
        trigger_time: trigger.time,
        correlation_id: trigger.correlation_id.clone(),
    }
}

/// Applies a trust requirement to an evidence record.
fn apply_trust_requirement(
    mut record: EvidenceRecord,
    requirement: TrustRequirement,
) -> EvidenceRecord {
    if record.result.lane.satisfies(requirement) {
        return record;
    }
    record.status = TriState::Unknown;
    if record.error.is_none() {
        record.error = Some(crate::core::EvidenceProviderError {
            code: "trust_lane".to_string(),
            message: format!(
                "evidence lane {:?} does not satisfy {:?}",
                record.result.lane, requirement.min_lane
            ),
        });
    }
    record
}

/// Returns true if every gate evaluation passed.
fn gates_passed(records: &[GateEvalRecord]) -> bool {
    records.iter().all(|record| record.evaluation.status == TriState::True)
}

/// Builds synthetic gate outcomes for timeout routing.
fn synthetic_gate_outcomes(
    stage: &StageSpec,
    outcome: TriState,
) -> Vec<(crate::core::GateId, TriState)> {
    stage.gates.iter().map(|gate| (gate.gate_id.clone(), outcome)).collect()
}

/// Returns true if the timeout deadline has elapsed.
fn timeout_expired(
    entered_at: Timestamp,
    now: Timestamp,
    timeout_ms: u64,
) -> Result<bool, ControlPlaneError> {
    match (entered_at, now) {
        (Timestamp::UnixMillis(start), Timestamp::UnixMillis(now)) => {
            let timeout_i64 = i64::try_from(timeout_ms).map_err(|_| {
                ControlPlaneError::TimeoutCalculation("timeout_ms exceeds i64 range".to_string())
            })?;
            let deadline = start.checked_add(timeout_i64).ok_or_else(|| {
                ControlPlaneError::TimeoutCalculation("timeout deadline overflow".to_string())
            })?;
            Ok(now >= deadline)
        }
        (Timestamp::Logical(start), Timestamp::Logical(now)) => {
            let deadline = start.checked_add(timeout_ms).ok_or_else(|| {
                ControlPlaneError::TimeoutCalculation("timeout deadline overflow".to_string())
            })?;
            Ok(now >= deadline)
        }
        _ => Err(ControlPlaneError::TriggerTimeMismatch(
            "timeout evaluation requires matching timestamp kinds".to_string(),
        )),
    }
}

/// Resolves the next stage based on gate outcomes.
fn resolve_next_stage(
    spec: &ScenarioSpec,
    stage: &crate::core::StageSpec,
    gate_outcomes: &[(crate::core::GateId, TriState)],
    _state: &RunState,
) -> Result<Option<StageId>, ControlPlaneError> {
    match &stage.advance_to {
        AdvanceTo::Linear => {
            let index = spec
                .stages
                .iter()
                .position(|spec| spec.stage_id == stage.stage_id)
                .ok_or_else(|| ControlPlaneError::StageNotFound(stage.stage_id.to_string()))?;
            let next = spec.stages.get(index + 1);
            Ok(next.map(|stage| stage.stage_id.clone()))
        }
        AdvanceTo::Fixed {
            stage_id,
        } => Ok(Some(stage_id.clone())),
        AdvanceTo::Branch {
            branches,
            default,
        } => {
            for branch in branches {
                let outcome = gate_outcomes
                    .iter()
                    .find(|(gate_id, _)| *gate_id == branch.gate_id)
                    .map_or(TriState::Unknown, |(_, status)| *status);
                if gate_outcome_matches(outcome, branch.outcome) {
                    return Ok(Some(branch.next_stage_id.clone()));
                }
            }
            default.clone().map(Some).ok_or_else(|| {
                ControlPlaneError::GateResolutionFailed("no branch matched".to_string())
            })
        }
        AdvanceTo::Terminal => Ok(None),
    }
}

/// Resolves the next stage without run state (used for precheck).
fn resolve_next_stage_precheck(
    spec: &ScenarioSpec,
    stage: &crate::core::StageSpec,
    gate_outcomes: &[(crate::core::GateId, TriState)],
) -> Result<Option<StageId>, ControlPlaneError> {
    match &stage.advance_to {
        AdvanceTo::Linear => {
            let index = spec
                .stages
                .iter()
                .position(|spec| spec.stage_id == stage.stage_id)
                .ok_or_else(|| ControlPlaneError::StageNotFound(stage.stage_id.to_string()))?;
            let next = spec.stages.get(index + 1);
            Ok(next.map(|stage| stage.stage_id.clone()))
        }
        AdvanceTo::Fixed {
            stage_id,
        } => Ok(Some(stage_id.clone())),
        AdvanceTo::Branch {
            branches,
            default,
        } => {
            for branch in branches {
                let outcome = gate_outcomes
                    .iter()
                    .find(|(gate_id, _)| *gate_id == branch.gate_id)
                    .map_or(TriState::Unknown, |(_, status)| *status);
                if gate_outcome_matches(outcome, branch.outcome) {
                    return Ok(Some(branch.next_stage_id.clone()));
                }
            }
            default.clone().map(Some).ok_or_else(|| {
                ControlPlaneError::GateResolutionFailed("no branch matched".to_string())
            })
        }
        AdvanceTo::Terminal => Ok(None),
    }
}

/// Returns true if a gate status satisfies the expected outcome.
fn gate_outcome_matches(status: TriState, outcome: GateOutcome) -> bool {
    match outcome {
        GateOutcome::True => status == TriState::True,
        GateOutcome::False => status == TriState::False,
        GateOutcome::Unknown => status == TriState::Unknown,
    }
}

/// Builds a packet envelope with hashed payload metadata.
fn build_packet_envelope(
    spec: &ScenarioSpec,
    state: &RunState,
    stage_id: &StageId,
    packet: &PacketSpec,
    decision: &DecisionRecord,
    algorithm: HashAlgorithm,
    issued_at: Timestamp,
) -> Result<PacketEnvelope, ControlPlaneError> {
    let content_hash = payload_hash(&packet.payload, algorithm)?;

    Ok(PacketEnvelope {
        scenario_id: spec.scenario_id.clone(),
        run_id: state.run_id.clone(),
        stage_id: stage_id.clone(),
        packet_id: packet.packet_id.clone(),
        schema_id: packet.schema_id.clone(),
        content_type: packet.content_type.clone(),
        content_hash,
        visibility: crate::core::VisibilityPolicy::new(
            packet.visibility_labels.clone(),
            packet.policy_tags.clone(),
        ),
        expiry: packet.expiry,
        correlation_id: decision.correlation_id.clone(),
        issued_at,
    })
}

/// Computes the payload hash for a packet payload.
fn payload_hash(
    payload: &PacketPayload,
    algorithm: HashAlgorithm,
) -> Result<HashDigest, ControlPlaneError> {
    match payload {
        PacketPayload::Json {
            value,
        } => match hash_canonical_json_with_limit(algorithm, value, MAX_PAYLOAD_BYTES) {
            Ok(hash) => Ok(hash),
            Err(HashError::SizeLimitExceeded {
                limit,
                actual,
            }) => Err(ControlPlaneError::PayloadTooLarge {
                max: limit,
                actual,
            }),
            Err(err) => Err(ControlPlaneError::Hash(err)),
        },
        PacketPayload::Bytes {
            bytes,
        } => {
            if bytes.len() > MAX_PAYLOAD_BYTES {
                return Err(ControlPlaneError::PayloadTooLarge {
                    max: MAX_PAYLOAD_BYTES,
                    actual: bytes.len(),
                });
            }
            Ok(hash_bytes(algorithm, bytes))
        }
        PacketPayload::External {
            content_ref,
        } => {
            if content_ref.content_hash.algorithm != algorithm {
                return Err(ControlPlaneError::PayloadHash(
                    "external payload hash algorithm mismatch".to_string(),
                ));
            }
            Ok(content_ref.content_hash.clone())
        }
    }
}

/// Normalizes evidence results by computing payload hashes.
fn normalize_evidence_result(
    result: &EvidenceResult,
    algorithm: HashAlgorithm,
) -> Result<EvidenceResult, ControlPlaneError> {
    let mut normalized = result.clone();
    if let Some(value) = &result.value {
        let hash = match value {
            EvidenceValue::Json(json) => {
                match hash_canonical_json_with_limit(algorithm, json, MAX_EVIDENCE_VALUE_BYTES) {
                    Ok(hash) => hash,
                    Err(HashError::SizeLimitExceeded {
                        limit,
                        actual,
                    }) => {
                        return Err(ControlPlaneError::EvidenceTooLarge {
                            max: limit,
                            actual,
                        });
                    }
                    Err(err) => return Err(ControlPlaneError::Hash(err)),
                }
            }
            EvidenceValue::Bytes(bytes) => {
                if bytes.len() > MAX_EVIDENCE_VALUE_BYTES {
                    return Err(ControlPlaneError::EvidenceTooLarge {
                        max: MAX_EVIDENCE_VALUE_BYTES,
                        actual: bytes.len(),
                    });
                }
                hash_bytes(algorithm, bytes)
            }
        };
        normalized.evidence_hash = Some(hash);
    }
    Ok(normalized)
}

/// Builds a safe summary for unmet gates.
fn build_safe_summary(records: &[GateEvalRecord]) -> SafeSummary {
    let unmet = records
        .iter()
        .filter(|record| record.evaluation.status != TriState::True)
        .map(|record| record.evaluation.gate_id.clone())
        .collect();
    SafeSummary {
        status: "hold".to_string(),
        unmet_gates: unmet,
        retry_hint: Some("await_evidence".to_string()),
        policy_tags: Vec::new(),
    }
}

/// Returns packet records associated with a decision id.
fn packets_for_decision(state: &RunState, decision_id: &DecisionId) -> Vec<PacketRecord> {
    state.packets.iter().filter(|packet| &packet.decision_id == decision_id).cloned().collect()
}

/// Computes the next sequence number for an append-only list.
fn next_seq<T>(items: &[T]) -> Result<u64, ControlPlaneError> {
    let base = u64::try_from(items.len()).map_err(|_| ControlPlaneError::SequenceOverflow)?;
    base.checked_add(1).ok_or(ControlPlaneError::SequenceOverflow)
}

/// Builds a tool call record with hashed request/response payloads.
fn build_tool_call_record<T: Serialize, R: Serialize>(
    method: &str,
    request: &T,
    response: &R,
    called_at: Timestamp,
    algorithm: HashAlgorithm,
    call_id: String,
    correlation_id: Option<crate::core::CorrelationId>,
) -> Result<ToolCallRecord, ControlPlaneError> {
    let request_hash = hash_canonical_json(algorithm, request)
        .map_err(|err| ControlPlaneError::ToolCallHash(err.to_string()))?;
    let response_hash = hash_canonical_json(algorithm, response)
        .map_err(|err| ControlPlaneError::ToolCallHash(err.to_string()))?;
    Ok(ToolCallRecord {
        call_id,
        method: method.to_string(),
        request_hash,
        response_hash,
        called_at,
        correlation_id,
        error: None,
    })
}

/// Builds a tool call record for a failed tool invocation.
fn build_tool_call_record_error<T: Serialize>(
    method: &str,
    request: &T,
    error: &ToolCallError,
    called_at: Timestamp,
    algorithm: HashAlgorithm,
    call_id: String,
    correlation_id: Option<crate::core::CorrelationId>,
) -> Result<ToolCallRecord, ControlPlaneError> {
    let request_hash = hash_canonical_json(algorithm, request)
        .map_err(|err| ControlPlaneError::ToolCallHash(err.to_string()))?;
    let response_hash = hash_canonical_json(algorithm, error)
        .map_err(|err| ControlPlaneError::ToolCallHash(err.to_string()))?;
    Ok(ToolCallRecord {
        call_id,
        method: method.to_string(),
        request_hash,
        response_hash,
        called_at,
        correlation_id,
        error: Some(error.clone()),
    })
}

/// Maps provider validation failures into tool-call error metadata.
fn provider_missing_tool_error(error: &ProviderMissingError) -> ToolCallError {
    ToolCallError {
        code: "provider_missing".to_string(),
        message: "required evidence providers are missing or blocked".to_string(),
        details: Some(ToolCallErrorDetails::ProviderMissing(error.clone())),
    }
}
