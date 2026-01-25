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
use decision_gate_core::DataShapeId;
use decision_gate_core::DataShapeRecord;
use decision_gate_core::DataShapeRef;
use decision_gate_core::DataShapeRegistry;
use decision_gate_core::DataShapeRegistryError;
use decision_gate_core::DataShapeVersion;
use decision_gate_core::DecisionOutcome;
use decision_gate_core::DispatchReceipt;
use decision_gate_core::DispatchTarget;
use decision_gate_core::Dispatcher;
use decision_gate_core::EvidenceContext;
use decision_gate_core::EvidenceProvider;
use decision_gate_core::EvidenceQuery;
use decision_gate_core::EvidenceResult;
use decision_gate_core::EvidenceValue;
use decision_gate_core::GateEvaluation;
use decision_gate_core::HashAlgorithm;
use decision_gate_core::NamespaceId;
use decision_gate_core::PacketEnvelope;
use decision_gate_core::PacketPayload;
use decision_gate_core::PrecheckRequest as CorePrecheckRequest;
use decision_gate_core::PredicateKey;
use decision_gate_core::RunConfig;
use decision_gate_core::RunId;
use decision_gate_core::RunState;
use decision_gate_core::RunStateStore;
use decision_gate_core::ScenarioId;
use decision_gate_core::ScenarioSpec;
use decision_gate_core::SharedDataShapeRegistry;
use decision_gate_core::SharedRunStateStore;
use decision_gate_core::StageId;
use decision_gate_core::TenantId;
use decision_gate_core::Timestamp;
use decision_gate_core::TriggerEvent;
use decision_gate_core::TrustLane;
use decision_gate_core::TrustRequirement;
use decision_gate_core::hashing::DEFAULT_HASH_ALGORITHM;
use decision_gate_core::hashing::hash_canonical_json;
use decision_gate_core::runtime::ControlPlane;
use decision_gate_core::runtime::ControlPlaneConfig;
use decision_gate_core::runtime::ControlPlaneError;
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
use jsonschema::CompilationOptions;
use jsonschema::Draft;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;
use thiserror::Error;

use crate::auth::AuthAction;
use crate::auth::AuthAuditEvent;
use crate::auth::AuthAuditSink;
use crate::auth::AuthError;
use crate::auth::RequestContext;
use crate::auth::ToolAuthz;
use crate::capabilities::CapabilityError;
use crate::capabilities::CapabilityRegistry;
use crate::config::DispatchPolicy;
use crate::config::EvidencePolicyConfig;
use crate::config::ValidationConfig;
use crate::evidence::FederatedEvidenceProvider;
use crate::runpack::FileArtifactReader;
use crate::runpack::FileArtifactSink;
use crate::validation::StrictValidator;

/// Default page size for list-style tools.
const DEFAULT_LIST_LIMIT: usize = 50;
/// Maximum page size for list-style tools.
const MAX_LIST_LIMIT: usize = 1000;

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
    /// Dispatch policy for packet disclosure.
    dispatch_policy: DispatchPolicy,
    /// Run state store for scenario runtimes.
    store: SharedRunStateStore,
    /// Data shape registry for asserted schemas.
    schema_registry: SharedDataShapeRegistry,
    /// Strict comparator validation.
    validation: StrictValidator,
    /// Provider transport metadata for discovery.
    provider_transports: BTreeMap<String, ProviderTransport>,
    /// Limits for schema registration.
    schema_registry_limits: SchemaRegistryLimits,
    /// Minimum trust requirement for evidence evaluation.
    trust_requirement: TrustRequirement,
    /// Capability registry used for preflight validation.
    capabilities: Arc<CapabilityRegistry>,
    /// Authn/authz policy for tool calls.
    authz: Arc<dyn ToolAuthz>,
    /// Audit sink for auth decisions.
    audit: Arc<dyn AuthAuditSink>,
}

/// Configuration inputs for building a tool router.
pub struct ToolRouterConfig {
    /// Evidence provider used for evidence queries.
    pub evidence: FederatedEvidenceProvider,
    /// Evidence disclosure policy configuration.
    pub evidence_policy: EvidencePolicyConfig,
    /// Dispatch policy for packet disclosure.
    pub dispatch_policy: DispatchPolicy,
    /// Run state store for scenario runtimes.
    pub store: SharedRunStateStore,
    /// Data shape registry for asserted schemas.
    pub schema_registry: SharedDataShapeRegistry,
    /// Provider transport metadata for discovery.
    pub provider_transports: BTreeMap<String, ProviderTransport>,
    /// Limits for schema registration.
    pub schema_registry_limits: SchemaRegistryLimits,
    /// Capability registry used for preflight validation.
    pub capabilities: Arc<CapabilityRegistry>,
    /// Validation configuration for strict comparator enforcement.
    pub validation: ValidationConfig,
    /// Authn/authz policy for tool calls.
    pub authz: Arc<dyn ToolAuthz>,
    /// Audit sink for auth decisions.
    pub audit: Arc<dyn AuthAuditSink>,
    /// Minimum trust requirement for evidence evaluation.
    pub trust_requirement: TrustRequirement,
}

impl ToolRouter {
    /// Creates a new tool router.
    #[must_use]
    pub fn new(config: ToolRouterConfig) -> Self {
        Self {
            state: Arc::new(Mutex::new(RouterState::default())),
            evidence: config.evidence,
            evidence_policy: config.evidence_policy,
            dispatch_policy: config.dispatch_policy,
            store: config.store,
            schema_registry: config.schema_registry,
            validation: StrictValidator::new(config.validation),
            provider_transports: config.provider_transports,
            schema_registry_limits: config.schema_registry_limits,
            capabilities: config.capabilities,
            authz: config.authz,
            audit: config.audit,
            trust_requirement: config.trust_requirement,
        }
    }

    /// Lists the MCP tools supported by this server.
    ///
    /// # Errors
    ///
    /// Returns [`ToolError`] when authorization fails.
    pub fn list_tools(&self, context: &RequestContext) -> Result<Vec<ToolDefinition>, ToolError> {
        self.authorize(context, AuthAction::ListTools)?;
        Ok(decision_gate_contract::tooling::tool_definitions())
    }

    /// Handles a tool call by name with JSON payload.
    ///
    /// # Errors
    ///
    /// Returns [`ToolError`] when routing fails.
    pub fn handle_tool_call(
        &self,
        context: &RequestContext,
        name: &str,
        payload: Value,
    ) -> Result<Value, ToolError> {
        let tool = ToolName::parse(name).ok_or(ToolError::UnknownTool)?;
        self.authorize(context, AuthAction::CallTool(&tool))?;
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
            ToolName::ProvidersList => {
                let request = decode::<ProvidersListRequest>(payload)?;
                let response = self.providers_list(&request);
                serde_json::to_value(response).map_err(|_| ToolError::Serialization)
            }
            ToolName::SchemasRegister => {
                let request = decode::<SchemasRegisterRequest>(payload)?;
                let response = self.schemas_register(&request)?;
                serde_json::to_value(response).map_err(|_| ToolError::Serialization)
            }
            ToolName::SchemasList => {
                let request = decode::<SchemasListRequest>(payload)?;
                let response = self.schemas_list(&request)?;
                serde_json::to_value(response).map_err(|_| ToolError::Serialization)
            }
            ToolName::SchemasGet => {
                let request = decode::<SchemasGetRequest>(payload)?;
                let response = self.schemas_get(&request)?;
                serde_json::to_value(response).map_err(|_| ToolError::Serialization)
            }
            ToolName::ScenariosList => {
                let request = decode::<ScenariosListRequest>(payload)?;
                let response = self.scenarios_list(&request)?;
                serde_json::to_value(response).map_err(|_| ToolError::Serialization)
            }
            ToolName::Precheck => {
                let request = decode::<PrecheckToolRequest>(payload)?;
                let response = self.precheck(&request)?;
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
    /// Tenant identifier.
    pub tenant_id: TenantId,
    /// Namespace identifier.
    pub namespace_id: NamespaceId,
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

/// Limits for schema registry operations.
#[derive(Debug, Clone, Copy)]
pub struct SchemaRegistryLimits {
    /// Maximum schema payload size in bytes.
    pub max_schema_bytes: usize,
    /// Optional maximum number of schemas per tenant + namespace.
    pub max_entries: Option<usize>,
}

/// Provider transport type for discovery output.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderTransport {
    /// Built-in provider.
    Builtin,
    /// External MCP provider.
    Mcp,
}

/// Provider summary returned by discovery tools.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderSummary {
    /// Provider identifier.
    pub provider_id: String,
    /// Provider transport type.
    pub transport: ProviderTransport,
    /// Predicate identifiers available on the provider.
    pub predicates: Vec<String>,
}

/// `providers_list` request payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProvidersListRequest {}

/// `providers_list` response payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProvidersListResponse {
    /// Provider summaries.
    pub providers: Vec<ProviderSummary>,
}

/// `schemas_register` request payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemasRegisterRequest {
    /// Data shape record to register.
    pub record: DataShapeRecord,
}

/// `schemas_register` response payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemasRegisterResponse {
    /// Registered data shape record.
    pub record: DataShapeRecord,
}

/// `schemas_list` request payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemasListRequest {
    /// Tenant identifier.
    pub tenant_id: TenantId,
    /// Namespace identifier.
    pub namespace_id: NamespaceId,
    /// Pagination cursor.
    pub cursor: Option<String>,
    /// Maximum number of records to return.
    pub limit: Option<usize>,
}

/// `schemas_list` response payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemasListResponse {
    /// Data shape records.
    pub items: Vec<DataShapeRecord>,
    /// Pagination token for the next page.
    pub next_token: Option<String>,
}

/// `schemas_get` request payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemasGetRequest {
    /// Tenant identifier.
    pub tenant_id: TenantId,
    /// Namespace identifier.
    pub namespace_id: NamespaceId,
    /// Schema identifier.
    pub schema_id: DataShapeId,
    /// Schema version.
    pub version: DataShapeVersion,
}

/// `schemas_get` response payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemasGetResponse {
    /// Data shape record.
    pub record: DataShapeRecord,
}

/// `scenarios_list` request payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScenariosListRequest {
    /// Tenant identifier.
    pub tenant_id: TenantId,
    /// Namespace identifier.
    pub namespace_id: NamespaceId,
    /// Pagination cursor.
    pub cursor: Option<String>,
    /// Maximum number of records to return.
    pub limit: Option<usize>,
}

/// Scenario summary returned by discovery tools.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScenarioSummary {
    /// Scenario identifier.
    pub scenario_id: ScenarioId,
    /// Namespace identifier.
    pub namespace_id: NamespaceId,
    /// Canonical spec hash.
    pub spec_hash: decision_gate_core::HashDigest,
}

/// `scenarios_list` response payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScenariosListResponse {
    /// Scenario summaries.
    pub items: Vec<ScenarioSummary>,
    /// Pagination token for the next page.
    pub next_token: Option<String>,
}

/// precheck request payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrecheckToolRequest {
    /// Tenant identifier.
    pub tenant_id: TenantId,
    /// Namespace identifier.
    pub namespace_id: NamespaceId,
    /// Optional scenario identifier to use.
    pub scenario_id: Option<ScenarioId>,
    /// Optional scenario spec override.
    pub spec: Option<ScenarioSpec>,
    /// Optional stage identifier override.
    pub stage_id: Option<StageId>,
    /// Data shape reference for payload validation.
    pub data_shape: DataShapeRef,
    /// Asserted payload.
    pub payload: Value,
}

/// precheck response payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrecheckToolResponse {
    /// Predicted decision outcome.
    pub decision: DecisionOutcome,
    /// Gate evaluations for the stage.
    pub gate_evaluations: Vec<GateEvaluation>,
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
    store: SharedRunStateStore,
    /// Control plane instance for the scenario.
    control:
        ControlPlane<FederatedEvidenceProvider, McpDispatcher, SharedRunStateStore, DispatchPolicy>,
}

/// Control plane wrapper for owned or borrowed runtimes.
enum ControlPlaneWrapper {
    /// Owned control plane instance for ad-hoc execution.
    Owned(
        Box<
            ControlPlane<
                FederatedEvidenceProvider,
                McpDispatcher,
                SharedRunStateStore,
                DispatchPolicy,
            >,
        >,
    ),
    /// Borrowed runtime reference from the router state.
    Borrowed(Arc<ScenarioRuntime>),
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

        self.capabilities.validate_spec(&request.spec).map_err(ToolError::from)?;
        self.validation
            .validate_spec(&request.spec, &self.capabilities)
            .map_err(|err| ToolError::InvalidParams(err.to_string()))?;

        let store = self.store.clone();
        let dispatcher = McpDispatcher::new(DEFAULT_HASH_ALGORITHM);
        let policy = self.dispatch_policy.clone();
        let control = ControlPlane::new(
            request.spec.clone(),
            self.evidence.clone(),
            dispatcher,
            store.clone(),
            Some(policy),
            ControlPlaneConfig {
                trust_requirement: self.trust_requirement,
                ..ControlPlaneConfig::default()
            },
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
            runtime.control.scenario_submit(&request.request).map_err(|err| match err {
                ControlPlaneError::SubmissionConflict(submission_id) => {
                    ToolError::Conflict(format!("submission_id conflict: {submission_id}"))
                }
                _ => ToolError::ControlPlane(err),
            })?;
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
        self.capabilities.validate_query(&request.query).map_err(ToolError::from)?;
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
            .load(&request.tenant_id, &request.namespace_id, &request.run_id)
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

    /// Lists configured providers and predicates.
    fn providers_list(&self, _request: &ProvidersListRequest) -> ProvidersListResponse {
        let mut providers = Vec::new();
        for (provider_id, predicates) in self.capabilities.list_providers() {
            let transport = self
                .provider_transports
                .get(&provider_id)
                .copied()
                .unwrap_or(ProviderTransport::Builtin);
            providers.push(ProviderSummary {
                provider_id,
                transport,
                predicates,
            });
        }
        ProvidersListResponse {
            providers,
        }
    }

    /// Registers a data shape schema.
    fn schemas_register(
        &self,
        request: &SchemasRegisterRequest,
    ) -> Result<SchemasRegisterResponse, ToolError> {
        self.validate_schema_limits(&request.record)?;
        let _ = compile_json_schema(&request.record.schema)?;
        self.schema_registry.register(request.record.clone())?;
        Ok(SchemasRegisterResponse {
            record: request.record.clone(),
        })
    }

    /// Lists data shape schemas.
    fn schemas_list(&self, request: &SchemasListRequest) -> Result<SchemasListResponse, ToolError> {
        let limit = normalize_limit(request.limit)?;
        let page = self.schema_registry.list(
            &request.tenant_id,
            &request.namespace_id,
            request.cursor.clone(),
            limit,
        )?;
        Ok(SchemasListResponse {
            items: page.items,
            next_token: page.next_token,
        })
    }

    /// Fetches a data shape schema by id and version.
    fn schemas_get(&self, request: &SchemasGetRequest) -> Result<SchemasGetResponse, ToolError> {
        let record = self
            .schema_registry
            .get(&request.tenant_id, &request.namespace_id, &request.schema_id, &request.version)?
            .ok_or_else(|| ToolError::NotFound("schema not found".to_string()))?;
        Ok(SchemasGetResponse {
            record,
        })
    }

    /// Lists registered scenarios for a tenant and namespace.
    fn scenarios_list(
        &self,
        request: &ScenariosListRequest,
    ) -> Result<ScenariosListResponse, ToolError> {
        let limit = normalize_limit(request.limit)?;
        let mut items: Vec<ScenarioSummary> = {
            let guard = self
                .state
                .lock()
                .map_err(|_| ToolError::Internal("router lock poisoned".to_string()))?;
            guard
                .scenarios
                .values()
                .filter(|runtime| runtime.spec.namespace_id == request.namespace_id)
                .filter(|runtime| {
                    runtime
                        .spec
                        .default_tenant_id
                        .as_ref()
                        .is_none_or(|tenant| tenant == &request.tenant_id)
                })
                .map(|runtime| {
                    let spec_hash = runtime
                        .spec
                        .canonical_hash_with(DEFAULT_HASH_ALGORITHM)
                        .map_err(|err| ToolError::Internal(err.to_string()))?;
                    Ok(ScenarioSummary {
                        scenario_id: runtime.spec.scenario_id.clone(),
                        namespace_id: runtime.spec.namespace_id.clone(),
                        spec_hash,
                    })
                })
                .collect::<Result<Vec<_>, ToolError>>()?
        };
        items.sort_by(|a, b| a.scenario_id.as_str().cmp(b.scenario_id.as_str()));
        let start_index = request.cursor.as_ref().map_or(0, |cursor| {
            items
                .iter()
                .position(|item| item.scenario_id.as_str() == cursor)
                .map_or(0, |idx| idx + 1)
        });
        let page: Vec<ScenarioSummary> = items.into_iter().skip(start_index).take(limit).collect();
        let next_token = page.last().map(|item| item.scenario_id.to_string());
        Ok(ScenariosListResponse {
            items: page,
            next_token,
        })
    }

    /// Evaluates a scenario stage using asserted data without mutating state.
    fn precheck(&self, request: &PrecheckToolRequest) -> Result<PrecheckToolResponse, ToolError> {
        let record = self
            .schema_registry
            .get(
                &request.tenant_id,
                &request.namespace_id,
                &request.data_shape.schema_id,
                &request.data_shape.version,
            )?
            .ok_or_else(|| ToolError::NotFound("schema not found".to_string()))?;
        let schema = compile_json_schema(&record.schema)?;
        validate_payload(&schema, &request.payload)?;

        let (spec, control) = match (&request.scenario_id, &request.spec) {
            (Some(scenario_id), Some(spec)) => {
                if &spec.scenario_id != scenario_id {
                    return Err(ToolError::InvalidParams(
                        "scenario_id does not match spec".to_string(),
                    ));
                }
                if spec.namespace_id != request.namespace_id {
                    return Err(ToolError::InvalidParams(
                        "namespace_id does not match spec".to_string(),
                    ));
                }
                self.capabilities.validate_spec(spec)?;
                let dispatcher = McpDispatcher::new(DEFAULT_HASH_ALGORITHM);
                let policy = self.dispatch_policy.clone();
                let control = ControlPlane::new(
                    spec.clone(),
                    self.evidence.clone(),
                    dispatcher,
                    self.store.clone(),
                    Some(policy),
                    ControlPlaneConfig {
                        trust_requirement: self.trust_requirement,
                        ..ControlPlaneConfig::default()
                    },
                )?;
                (spec.clone(), ControlPlaneWrapper::Owned(Box::new(control)))
            }
            (Some(scenario_id), None) => {
                let runtime = self.runtime_for(scenario_id)?;
                if runtime.spec.namespace_id != request.namespace_id {
                    return Err(ToolError::NotFound("scenario not in namespace".to_string()));
                }
                (runtime.spec.clone(), ControlPlaneWrapper::Borrowed(runtime))
            }
            (None, Some(spec)) => {
                if spec.namespace_id != request.namespace_id {
                    return Err(ToolError::InvalidParams(
                        "namespace_id does not match spec".to_string(),
                    ));
                }
                self.capabilities.validate_spec(spec)?;
                let dispatcher = McpDispatcher::new(DEFAULT_HASH_ALGORITHM);
                let policy = self.dispatch_policy.clone();
                let control = ControlPlane::new(
                    spec.clone(),
                    self.evidence.clone(),
                    dispatcher,
                    self.store.clone(),
                    Some(policy),
                    ControlPlaneConfig {
                        trust_requirement: self.trust_requirement,
                        ..ControlPlaneConfig::default()
                    },
                )?;
                (spec.clone(), ControlPlaneWrapper::Owned(Box::new(control)))
            }
            (None, None) => {
                return Err(ToolError::InvalidParams(
                    "scenario_id or spec is required".to_string(),
                ));
            }
        };
        if let Some(default_tenant) = &spec.default_tenant_id
            && default_tenant != &request.tenant_id
        {
            return Err(ToolError::Unauthorized(
                "tenant_id does not match scenario tenant".to_string(),
            ));
        }

        self.validation
            .validate_precheck(&spec, &record.schema)
            .map_err(|err| ToolError::InvalidParams(err.to_string()))?;

        let evidence = build_asserted_evidence(&spec, &request.payload)?;
        let core_request = CorePrecheckRequest {
            stage_id: request.stage_id.clone(),
            evidence,
        };
        let result = match control {
            ControlPlaneWrapper::Owned(control) => control.precheck(&core_request)?,
            ControlPlaneWrapper::Borrowed(runtime) => runtime.control.precheck(&core_request)?,
        };
        Ok(PrecheckToolResponse {
            decision: result.decision,
            gate_evaluations: result.gate_evaluations,
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

    /// Validates schema registration limits for a data shape record.
    fn validate_schema_limits(&self, record: &DataShapeRecord) -> Result<(), ToolError> {
        let schema_bytes = serde_json::to_vec(&record.schema)
            .map_err(|err| ToolError::InvalidParams(err.to_string()))?;
        if schema_bytes.len() > self.schema_registry_limits.max_schema_bytes {
            return Err(ToolError::InvalidParams(format!(
                "schema exceeds size limit: {} bytes (max {})",
                schema_bytes.len(),
                self.schema_registry_limits.max_schema_bytes
            )));
        }
        if let Some(max_entries) = self.schema_registry_limits.max_entries {
            let page = self.schema_registry.list(
                &record.tenant_id,
                &record.namespace_id,
                None,
                max_entries,
            )?;
            if page.items.len() >= max_entries {
                return Err(ToolError::Conflict(
                    "schema registry max entries exceeded".to_string(),
                ));
            }
        }
        Ok(())
    }

    /// Authorizes a tool action and emits an auth audit record.
    fn authorize(&self, context: &RequestContext, action: AuthAction<'_>) -> Result<(), ToolError> {
        match self.authz.authorize(context, action) {
            Ok(auth_ctx) => {
                self.audit.record(&AuthAuditEvent::allowed(context, action, &auth_ctx));
                Ok(())
            }
            Err(err) => {
                self.audit.record(&AuthAuditEvent::denied(context, action, &err));
                Err(err.into())
            }
        }
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

impl decision_gate_core::PolicyDecider for DispatchPolicy {
    fn authorize(
        &self,
        _target: &DispatchTarget,
        _envelope: &PacketEnvelope,
        _payload: &PacketPayload,
    ) -> Result<decision_gate_core::PolicyDecision, decision_gate_core::PolicyError> {
        match self {
            Self::PermitAll => Ok(decision_gate_core::PolicyDecision::Permit),
            Self::DenyAll => Ok(decision_gate_core::PolicyDecision::Deny),
        }
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
    /// Missing or invalid authentication.
    #[error("unauthenticated: {0}")]
    Unauthenticated(String),
    /// Authenticated caller not authorized to access tool.
    #[error("unauthorized: {0}")]
    Unauthorized(String),
    /// Tool payload serialization failed.
    #[error("serialization failure")]
    Serialization,
    /// Tool payload deserialization failed.
    #[error("invalid parameters: {0}")]
    InvalidParams(String),
    /// Capability registry validation error.
    #[error("capability violation: {code}: {message}")]
    CapabilityViolation {
        /// Stable error code.
        code: String,
        /// Human-readable message.
        message: String,
    },
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

impl From<CapabilityError> for ToolError {
    fn from(error: CapabilityError) -> Self {
        Self::CapabilityViolation {
            code: error.code().to_string(),
            message: error.to_string(),
        }
    }
}

impl From<DataShapeRegistryError> for ToolError {
    fn from(error: DataShapeRegistryError) -> Self {
        match error {
            DataShapeRegistryError::Io(message) => Self::Internal(message),
            DataShapeRegistryError::Invalid(message) => Self::InvalidParams(message),
            DataShapeRegistryError::Conflict(message) => Self::Conflict(message),
            DataShapeRegistryError::Access(message) => Self::Unauthorized(message),
        }
    }
}

impl From<AuthError> for ToolError {
    fn from(error: AuthError) -> Self {
        match error {
            AuthError::Unauthenticated(message) => Self::Unauthenticated(message),
            AuthError::Unauthorized(message) => Self::Unauthorized(message),
        }
    }
}

/// Decodes a JSON value into a typed request payload.
fn decode<T: for<'de> Deserialize<'de>>(payload: Value) -> Result<T, ToolError> {
    serde_json::from_value(payload).map_err(|err| ToolError::InvalidParams(err.to_string()))
}

/// Normalizes list limits against configured defaults and bounds.
fn normalize_limit(limit: Option<usize>) -> Result<usize, ToolError> {
    let limit = limit.unwrap_or(DEFAULT_LIST_LIMIT);
    if limit == 0 || limit > MAX_LIST_LIMIT {
        return Err(ToolError::InvalidParams(format!(
            "limit must be between 1 and {MAX_LIST_LIMIT}"
        )));
    }
    Ok(limit)
}

/// Compiles a JSON schema and maps errors to tool input failures.
fn compile_json_schema(schema: &Value) -> Result<jsonschema::JSONSchema, ToolError> {
    let mut options = CompilationOptions::default();
    options.with_draft(Draft::Draft202012);
    options
        .compile(schema)
        .map_err(|err| ToolError::InvalidParams(format!("invalid schema: {err}")))
}

/// Validates a JSON payload against a compiled schema.
fn validate_payload(schema: &jsonschema::JSONSchema, payload: &Value) -> Result<(), ToolError> {
    if let Err(errors) = schema.validate(payload) {
        let messages = errors.map(|error| error.to_string()).collect::<Vec<_>>();
        return Err(ToolError::InvalidParams(format!(
            "payload does not match schema: {}",
            messages.join("; ")
        )));
    }
    Ok(())
}

/// Builds asserted evidence results from a request payload.
fn build_asserted_evidence(
    spec: &ScenarioSpec,
    payload: &Value,
) -> Result<BTreeMap<PredicateKey, EvidenceResult>, ToolError> {
    let mut evidence = BTreeMap::new();
    match payload {
        Value::Object(map) => {
            for predicate in &spec.predicates {
                if let Some(value) = map.get(predicate.predicate.as_str()) {
                    evidence.insert(predicate.predicate.clone(), asserted_evidence(value.clone()));
                }
            }
        }
        _ => {
            if spec.predicates.len() == 1 {
                let predicate =
                    spec.predicates.first().map(|spec| spec.predicate.clone()).ok_or_else(
                        || ToolError::InvalidParams("scenario has no predicates".to_string()),
                    )?;
                evidence.insert(predicate, asserted_evidence(payload.clone()));
            } else {
                return Err(ToolError::InvalidParams(
                    "payload must be an object keyed by predicate ids".to_string(),
                ));
            }
        }
    }
    Ok(evidence)
}

/// Wraps a JSON value in an asserted evidence result.
const fn asserted_evidence(value: Value) -> EvidenceResult {
    EvidenceResult {
        value: Some(EvidenceValue::Json(value)),
        lane: TrustLane::Asserted,
        evidence_hash: None,
        evidence_ref: None,
        evidence_anchor: None,
        signature: None,
        content_type: None,
    }
}
