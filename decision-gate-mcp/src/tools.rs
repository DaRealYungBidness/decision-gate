// decision-gate-mcp/src/tools.rs
// ============================================================================
// Module: MCP Tool Router
// Description: Tool routing for Decision Gate MCP server.
// Purpose: Expose thin wrappers over the Decision Gate control plane.
// Dependencies: decision-gate-core, decision-gate-providers
// ============================================================================

//! ## Overview
//! The tool router dispatches MCP tool calls to the control plane and related
//! subsystems. All tool handlers are thin wrappers over
//! [`decision_gate_core::ControlPlane`].
//! Security posture: tool inputs are untrusted; see `Docs/security/threat_model.md`.

// ============================================================================
// SECTION: Imports
// ============================================================================

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;

use decision_gate_contract::ToolName;
pub use decision_gate_contract::tooling::ToolDefinition;
use decision_gate_core::ArtifactReader;
use decision_gate_core::DispatchReceipt;
use decision_gate_core::DispatchTarget;
use decision_gate_core::Dispatcher;
use decision_gate_core::EvidenceContext;
use decision_gate_core::EvidenceProvider;
use decision_gate_core::EvidenceQuery;
use decision_gate_core::EvidenceResult;
use decision_gate_core::EvidenceValue;
use decision_gate_core::HashAlgorithm;
use decision_gate_core::PacketEnvelope;
use decision_gate_core::PacketPayload;
use decision_gate_core::RunConfig;
use decision_gate_core::RunId;
use decision_gate_core::RunState;
use decision_gate_core::RunStateStore;
use decision_gate_core::ScenarioId;
use decision_gate_core::ScenarioSpec;
use decision_gate_core::Timestamp;
use decision_gate_core::TriggerEvent;
use decision_gate_core::hashing::DEFAULT_HASH_ALGORITHM;
use decision_gate_core::hashing::hash_canonical_json;
use decision_gate_core::runtime::ControlPlane;
use decision_gate_core::runtime::ControlPlaneConfig;
use decision_gate_core::runtime::ControlPlaneError;
use decision_gate_core::runtime::InMemoryRunStateStore;
use decision_gate_core::runtime::NextRequest;
use decision_gate_core::runtime::NextResult;
use decision_gate_core::runtime::RunpackBuilder;
use decision_gate_core::runtime::RunpackVerifier;
use decision_gate_core::runtime::ScenarioStatus;
use decision_gate_core::runtime::StatusRequest;
use decision_gate_core::runtime::SubmitRequest;
use decision_gate_core::runtime::SubmitResult;
use decision_gate_core::runtime::TriggerResult;
use decision_gate_core::runtime::VerificationReport;
use decision_gate_core::runtime::VerificationStatus;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;
use thiserror::Error;

use crate::config::EvidencePolicyConfig;
use crate::evidence::FederatedEvidenceProvider;
use crate::runpack::FileArtifactReader;
use crate::runpack::FileArtifactSink;

// ============================================================================
// SECTION: Tool Router
// ============================================================================

/// Tool router for MCP requests.
#[derive(Clone)]
pub struct ToolRouter {
    /// Shared router state for scenario runtimes.
    state: Arc<Mutex<RouterState>>,
    /// Evidence provider used for evidence queries.
    evidence: FederatedEvidenceProvider,
    /// Evidence disclosure policy configuration.
    evidence_policy: EvidencePolicyConfig,
}

impl ToolRouter {
    /// Creates a new tool router.
    #[must_use]
    pub fn new(evidence: FederatedEvidenceProvider, evidence_policy: EvidencePolicyConfig) -> Self {
        Self {
            state: Arc::new(Mutex::new(RouterState::default())),
            evidence,
            evidence_policy,
        }
    }

    /// Lists the MCP tools supported by this server.
    #[must_use]
    pub fn list_tools(&self) -> Vec<ToolDefinition> {
        decision_gate_contract::tooling::tool_definitions()
    }

    /// Handles a tool call by name with JSON payload.
    ///
    /// # Errors
    ///
    /// Returns [`ToolError`] when routing fails.
    pub fn handle_tool_call(&self, name: &str, payload: Value) -> Result<Value, ToolError> {
        let tool = ToolName::parse(name).ok_or(ToolError::UnknownTool)?;
        match tool {
            ToolName::ScenarioDefine => {
                let request = decode::<ScenarioDefineRequest>(payload)?;
                let response = self.define_scenario(request)?;
                serde_json::to_value(response).map_err(|_| ToolError::Serialization)
            }
            ToolName::ScenarioStart => {
                let request = decode::<ScenarioStartRequest>(payload)?;
                let response = self.start_run(request)?;
                serde_json::to_value(response).map_err(|_| ToolError::Serialization)
            }
            ToolName::ScenarioStatus => {
                let request = decode::<ScenarioStatusRequest>(payload)?;
                let response = self.status(&request)?;
                serde_json::to_value(response).map_err(|_| ToolError::Serialization)
            }
            ToolName::ScenarioNext => {
                let request = decode::<ScenarioNextRequest>(payload)?;
                let response = self.next(&request)?;
                serde_json::to_value(response).map_err(|_| ToolError::Serialization)
            }
            ToolName::ScenarioSubmit => {
                let request = decode::<ScenarioSubmitRequest>(payload)?;
                let response = self.submit(&request)?;
                serde_json::to_value(response).map_err(|_| ToolError::Serialization)
            }
            ToolName::ScenarioTrigger => {
                let request = decode::<ScenarioTriggerRequest>(payload)?;
                let response = self.trigger(&request)?;
                serde_json::to_value(response).map_err(|_| ToolError::Serialization)
            }
            ToolName::EvidenceQuery => {
                let request = decode::<EvidenceQueryRequest>(payload)?;
                let response = self.query_evidence(&request)?;
                serde_json::to_value(response).map_err(|_| ToolError::Serialization)
            }
            ToolName::RunpackExport => {
                let request = decode::<RunpackExportRequest>(payload)?;
                let response = self.export_runpack(&request)?;
                serde_json::to_value(response).map_err(|_| ToolError::Serialization)
            }
            ToolName::RunpackVerify => {
                let request = decode::<RunpackVerifyRequest>(payload)?;
                let response = Self::verify_runpack(&request)?;
                serde_json::to_value(response).map_err(|_| ToolError::Serialization)
            }
        }
    }
}

// ============================================================================
// SECTION: Tool Requests and Responses
// ============================================================================

/// Scenario definition request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScenarioDefineRequest {
    /// Scenario specification payload.
    pub spec: ScenarioSpec,
}

/// Scenario definition response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScenarioDefineResponse {
    /// Scenario identifier.
    pub scenario_id: ScenarioId,
    /// Spec hash computed at registration time.
    pub spec_hash: decision_gate_core::HashDigest,
}

/// Scenario start request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScenarioStartRequest {
    /// Scenario identifier.
    pub scenario_id: ScenarioId,
    /// Run configuration.
    pub run_config: RunConfig,
    /// Timestamp for run start.
    pub started_at: Timestamp,
    /// Whether to issue entry packets immediately.
    pub issue_entry_packets: bool,
}

/// Scenario status request wrapper.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScenarioStatusRequest {
    /// Scenario identifier.
    pub scenario_id: ScenarioId,
    /// Core status request.
    pub request: StatusRequest,
}

/// Scenario next request wrapper.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScenarioNextRequest {
    /// Scenario identifier.
    pub scenario_id: ScenarioId,
    /// Core next request.
    pub request: NextRequest,
}

/// Scenario submit request wrapper.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScenarioSubmitRequest {
    /// Scenario identifier.
    pub scenario_id: ScenarioId,
    /// Core submit request.
    pub request: SubmitRequest,
}

/// Scenario trigger request wrapper.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScenarioTriggerRequest {
    /// Scenario identifier.
    pub scenario_id: ScenarioId,
    /// Trigger event payload.
    pub trigger: TriggerEvent,
}

/// Evidence query request wrapper.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvidenceQueryRequest {
    /// Evidence query payload.
    pub query: EvidenceQuery,
    /// Evidence context payload.
    pub context: EvidenceContext,
}

/// Evidence query response payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvidenceQueryResponse {
    /// Evidence result payload (possibly redacted).
    pub result: EvidenceResult,
}

/// Runpack export request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunpackExportRequest {
    /// Scenario identifier.
    pub scenario_id: ScenarioId,
    /// Run identifier.
    pub run_id: RunId,
    /// Output directory for runpack artifacts.
    pub output_dir: String,
    /// Manifest file name.
    pub manifest_name: Option<String>,
    /// Timestamp recorded in the runpack manifest.
    pub generated_at: Timestamp,
    /// Include verification report.
    pub include_verification: bool,
}

/// Runpack export response payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunpackExportResponse {
    /// Runpack manifest.
    pub manifest: decision_gate_core::RunpackManifest,
    /// Optional verification report.
    pub report: Option<VerificationReport>,
}

/// Runpack verification request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunpackVerifyRequest {
    /// Runpack root directory.
    pub runpack_dir: String,
    /// Manifest path relative to the runpack root.
    pub manifest_path: String,
}

/// Runpack verification response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunpackVerifyResponse {
    /// Verification report.
    pub report: VerificationReport,
    /// Verification status.
    pub status: VerificationStatus,
}

// ============================================================================
// SECTION: Router State
// ============================================================================

#[derive(Default)]
/// Internal state for scenario runtimes.
struct RouterState {
    /// Scenario runtimes keyed by scenario ID.
    scenarios: BTreeMap<String, Arc<ScenarioRuntime>>,
}

/// Scenario runtime bundle for tool routing.
struct ScenarioRuntime {
    /// Scenario specification.
    spec: ScenarioSpec,
    /// Run state store for the scenario.
    store: InMemoryRunStateStore,
    /// Control plane instance for the scenario.
    control:
        ControlPlane<FederatedEvidenceProvider, McpDispatcher, InMemoryRunStateStore, PermitAll>,
}

// ============================================================================
// SECTION: Tool Implementations
// ============================================================================

impl ToolRouter {
    /// Defines and registers a scenario specification.
    fn define_scenario(
        &self,
        request: ScenarioDefineRequest,
    ) -> Result<ScenarioDefineResponse, ToolError> {
        let scenario_id = request.spec.scenario_id.to_string();
        {
            let guard = self
                .state
                .lock()
                .map_err(|_| ToolError::Internal("router lock poisoned".to_string()))?;
            if guard.scenarios.contains_key(&scenario_id) {
                return Err(ToolError::Conflict("scenario already defined".to_string()));
            }
        }

        let store = InMemoryRunStateStore::new();
        let dispatcher = McpDispatcher::new(DEFAULT_HASH_ALGORITHM);
        let policy = PermitAll;
        let control = ControlPlane::new(
            request.spec.clone(),
            self.evidence.clone(),
            dispatcher,
            store.clone(),
            Some(policy),
            ControlPlaneConfig::default(),
        )
        .map_err(ToolError::ControlPlane)?;

        let spec_hash = request
            .spec
            .canonical_hash_with(DEFAULT_HASH_ALGORITHM)
            .map_err(|err| ToolError::Internal(err.to_string()))?;

        let runtime = Arc::new(ScenarioRuntime {
            spec: request.spec.clone(),
            store,
            control,
        });
        {
            let mut guard = self
                .state
                .lock()
                .map_err(|_| ToolError::Internal("router lock poisoned".to_string()))?;
            if guard.scenarios.contains_key(&scenario_id) {
                return Err(ToolError::Conflict("scenario already defined".to_string()));
            }
            guard.scenarios.insert(scenario_id, runtime);
        }

        Ok(ScenarioDefineResponse {
            scenario_id: request.spec.scenario_id,
            spec_hash,
        })
    }

    /// Starts a new run for a scenario.
    fn start_run(&self, request: ScenarioStartRequest) -> Result<RunState, ToolError> {
        let runtime = self.runtime_for(&request.scenario_id)?;
        let state = runtime
            .control
            .start_run(request.run_config, request.started_at, request.issue_entry_packets)
            .map_err(ToolError::ControlPlane)?;
        Ok(state)
    }

    /// Returns the current status for a scenario run.
    fn status(&self, request: &ScenarioStatusRequest) -> Result<ScenarioStatus, ToolError> {
        let runtime = self.runtime_for(&request.scenario_id)?;
        let status =
            runtime.control.scenario_status(&request.request).map_err(ToolError::ControlPlane)?;
        Ok(status)
    }

    /// Advances a scenario evaluation.
    fn next(&self, request: &ScenarioNextRequest) -> Result<NextResult, ToolError> {
        let runtime = self.runtime_for(&request.scenario_id)?;
        let result =
            runtime.control.scenario_next(&request.request).map_err(ToolError::ControlPlane)?;
        Ok(result)
    }

    /// Submits external artifacts to a scenario run.
    fn submit(&self, request: &ScenarioSubmitRequest) -> Result<SubmitResult, ToolError> {
        let runtime = self.runtime_for(&request.scenario_id)?;
        let result =
            runtime.control.scenario_submit(&request.request).map_err(ToolError::ControlPlane)?;
        Ok(result)
    }

    /// Submits a trigger event to a scenario run.
    fn trigger(&self, request: &ScenarioTriggerRequest) -> Result<TriggerResult, ToolError> {
        let runtime = self.runtime_for(&request.scenario_id)?;
        let result = runtime.control.trigger(&request.trigger).map_err(ToolError::ControlPlane)?;
        Ok(result)
    }

    /// Queries evidence providers with disclosure policy enforcement.
    fn query_evidence(
        &self,
        request: &EvidenceQueryRequest,
    ) -> Result<EvidenceQueryResponse, ToolError> {
        let mut result = self
            .evidence
            .query(&request.query, &request.context)
            .map_err(|err| ToolError::Evidence(err.to_string()))?;
        ensure_evidence_hash(&mut result)?;
        let provider_id = request.query.provider_id.as_str();
        if !self.evidence_policy.allow_raw_values
            || (self.evidence_policy.require_provider_opt_in
                && !self.evidence.provider_allows_raw(provider_id))
        {
            result.value = None;
            result.content_type = None;
        }
        Ok(EvidenceQueryResponse {
            result,
        })
    }

    /// Exports a runpack for the specified run.
    fn export_runpack(
        &self,
        request: &RunpackExportRequest,
    ) -> Result<RunpackExportResponse, ToolError> {
        let runtime = self.runtime_for(&request.scenario_id)?;
        let manifest_name = request.manifest_name.as_deref().unwrap_or("manifest.json");
        let output_dir = PathBuf::from(&request.output_dir);
        let mut sink = FileArtifactSink::new(output_dir, manifest_name)
            .map_err(|err| ToolError::Runpack(err.to_string()))?;
        let state = runtime
            .store
            .load(&request.run_id)
            .map_err(|err| ToolError::Runpack(err.to_string()))?
            .ok_or_else(|| ToolError::NotFound("run not found".to_string()))?;
        let builder = RunpackBuilder::default();
        if request.include_verification {
            let reader = FileArtifactReader::new(PathBuf::from(&request.output_dir))
                .map_err(|err| ToolError::Runpack(err.to_string()))?;
            let (manifest, report) = builder
                .build_with_verification(
                    &mut sink,
                    &reader,
                    &runtime.spec,
                    &state,
                    request.generated_at,
                )
                .map_err(|err| ToolError::Runpack(err.to_string()))?;
            return Ok(RunpackExportResponse {
                manifest,
                report: Some(report),
            });
        }
        let manifest = builder
            .build(&mut sink, &runtime.spec, &state, request.generated_at)
            .map_err(|err| ToolError::Runpack(err.to_string()))?;
        Ok(RunpackExportResponse {
            manifest,
            report: None,
        })
    }

    /// Verifies a runpack manifest and artifacts.
    fn verify_runpack(request: &RunpackVerifyRequest) -> Result<RunpackVerifyResponse, ToolError> {
        let root = PathBuf::from(&request.runpack_dir);
        let reader =
            FileArtifactReader::new(root).map_err(|err| ToolError::Runpack(err.to_string()))?;
        let manifest_bytes = reader
            .read(&request.manifest_path)
            .map_err(|err| ToolError::Runpack(err.to_string()))?;
        let manifest: decision_gate_core::RunpackManifest = serde_json::from_slice(&manifest_bytes)
            .map_err(|_| ToolError::Runpack("invalid manifest".to_string()))?;
        let verifier = RunpackVerifier::new(DEFAULT_HASH_ALGORITHM);
        let report = verifier
            .verify_manifest(&reader, &manifest)
            .map_err(|err| ToolError::Runpack(err.to_string()))?;
        Ok(RunpackVerifyResponse {
            status: report.status,
            report,
        })
    }

    /// Returns the runtime for a scenario ID.
    fn runtime_for(&self, scenario_id: &ScenarioId) -> Result<Arc<ScenarioRuntime>, ToolError> {
        let key = scenario_id.to_string();
        let runtime = {
            let guard = self
                .state
                .lock()
                .map_err(|_| ToolError::Internal("router lock poisoned".to_string()))?;
            guard.scenarios.get(&key).cloned()
        };
        runtime.ok_or_else(|| ToolError::NotFound("scenario not defined".to_string()))
    }
}

// ============================================================================
// SECTION: Dispatch and Policy
// ============================================================================

/// Dispatcher that stamps MCP receipts with envelope hashes.
struct McpDispatcher {
    /// Hash algorithm used for receipt hashes.
    algorithm: HashAlgorithm,
}

impl McpDispatcher {
    /// Creates a dispatcher with the selected hash algorithm.
    const fn new(algorithm: HashAlgorithm) -> Self {
        Self {
            algorithm,
        }
    }
}

impl Dispatcher for McpDispatcher {
    fn dispatch(
        &self,
        target: &DispatchTarget,
        envelope: &PacketEnvelope,
        _payload: &PacketPayload,
    ) -> Result<DispatchReceipt, decision_gate_core::DispatchError> {
        let hash = hash_canonical_json(self.algorithm, envelope)
            .map_err(|err| decision_gate_core::DispatchError::DispatchFailed(err.to_string()))?;
        Ok(DispatchReceipt {
            dispatch_id: format!("mcp-{}", envelope.packet_id.as_str()),
            target: target.clone(),
            receipt_hash: hash,
            dispatched_at: envelope.issued_at,
            dispatcher: "mcp".to_string(),
        })
    }
}

/// Policy decider that permits all disclosures.
struct PermitAll;

impl decision_gate_core::PolicyDecider for PermitAll {
    fn authorize(
        &self,
        _target: &DispatchTarget,
        _envelope: &PacketEnvelope,
        _payload: &PacketPayload,
    ) -> Result<decision_gate_core::PolicyDecision, decision_gate_core::PolicyError> {
        Ok(decision_gate_core::PolicyDecision::Permit)
    }
}

// ============================================================================
// SECTION: Evidence Helpers
// ============================================================================

/// Ensures evidence results include a hash, computing one if absent.
fn ensure_evidence_hash(result: &mut EvidenceResult) -> Result<(), ToolError> {
    if result.evidence_hash.is_some() {
        return Ok(());
    }
    let Some(value) = &result.value else {
        return Ok(());
    };
    let hash = match value {
        EvidenceValue::Json(json) => hash_canonical_json(HashAlgorithm::Sha256, json)
            .map_err(|err| ToolError::Internal(err.to_string()))?,
        EvidenceValue::Bytes(bytes) => {
            decision_gate_core::hashing::hash_bytes(HashAlgorithm::Sha256, bytes)
        }
    };
    result.evidence_hash = Some(hash);
    Ok(())
}

// ============================================================================
// SECTION: Errors
// ============================================================================

/// Tool routing errors.
#[derive(Debug, Error)]
pub enum ToolError {
    /// Tool name not recognized.
    #[error("unknown tool")]
    UnknownTool,
    /// Tool payload serialization failed.
    #[error("serialization failure")]
    Serialization,
    /// Tool payload deserialization failed.
    #[error("invalid parameters: {0}")]
    InvalidParams(String),
    /// Scenario not found.
    #[error("not found: {0}")]
    NotFound(String),
    /// Scenario conflict.
    #[error("conflict: {0}")]
    Conflict(String),
    /// Evidence provider error.
    #[error("evidence error: {0}")]
    Evidence(String),
    /// Control plane error.
    #[error(transparent)]
    ControlPlane(#[from] ControlPlaneError),
    /// Runpack error.
    #[error("runpack error: {0}")]
    Runpack(String),
    /// Internal error.
    #[error("internal error: {0}")]
    Internal(String),
}

/// Decodes a JSON value into a typed request payload.
fn decode<T: for<'de> Deserialize<'de>>(payload: Value) -> Result<T, ToolError> {
    serde_json::from_value(payload).map_err(|err| ToolError::InvalidParams(err.to_string()))
}
