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
//!
//! ## Layer Responsibilities
//! - Route MCP tool calls to deterministic control-plane operations.
//! - Enforce authn/authz, tenant isolation, namespace authority, and quotas.
//! - Emit audit + telemetry events for tool invocations.
//!
//! ## Invariants
//! - Tool handlers never mutate state without passing validation gates.
//! - Authorization and namespace checks fail closed on missing context.
//! - Responses remain deterministic for identical inputs.

// ============================================================================
// SECTION: Imports
// ============================================================================

use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;

use decision_gate_contract::ToolName;
pub use decision_gate_contract::tooling::ToolDefinition;
use decision_gate_contract::types::DeterminismClass;
use decision_gate_contract::types::PredicateExample;
use decision_gate_core::ArtifactReader;
use decision_gate_core::Comparator;
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
use decision_gate_core::EvidenceAnchorPolicy;
use decision_gate_core::EvidenceContext;
use decision_gate_core::EvidenceProvider;
use decision_gate_core::EvidenceProviderError;
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
use decision_gate_core::hashing::HashError;
use decision_gate_core::hashing::canonical_json_bytes_with_limit;
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

use crate::audit::McpAuditSink;
use crate::audit::PrecheckAuditEvent;
use crate::audit::PrecheckAuditEventParams;
use crate::audit::RegistryAuditEvent;
use crate::audit::RegistryAuditEventParams;
use crate::audit::TenantAuthzEvent;
use crate::audit::TenantAuthzEventParams;
use crate::audit::UsageAuditEvent;
use crate::audit::UsageAuditEventParams;
use crate::auth::AuthAction;
use crate::auth::AuthAuditEvent;
use crate::auth::AuthAuditSink;
use crate::auth::AuthContext;
use crate::auth::AuthError;
use crate::auth::RequestContext;
use crate::auth::ToolAuthz;
use crate::capabilities::CapabilityError;
use crate::capabilities::CapabilityRegistry;
use crate::capabilities::ProviderContractSource;
use crate::config::EvidencePolicyConfig;
use crate::config::ProviderDiscoveryConfig;
use crate::config::RegistryAclAction;
use crate::config::ValidationConfig;
use crate::evidence::FederatedEvidenceProvider;
use crate::namespace_authority::NamespaceAuthority;
use crate::namespace_authority::NamespaceAuthorityError;
use crate::policy::DispatchPolicy;
use crate::registry_acl::PrincipalResolver;
use crate::registry_acl::RegistryAcl;
use crate::registry_acl::RegistryAclDecision;
use crate::runpack::FileArtifactReader;
use crate::runpack::FileArtifactSink;
use crate::runpack_object_store::ObjectStoreRunpackBackend;
use crate::runpack_object_store::RunpackObjectKey;
use crate::runpack_storage::RunpackStorage;
use crate::runpack_storage::RunpackStorageError;
use crate::runpack_storage::RunpackStorageKey;
use crate::tenant_authz::TenantAccessRequest;
use crate::tenant_authz::TenantAuthorizer;
use crate::tenant_authz::TenantAuthzAction;
use crate::tenant_authz::TenantAuthzDecision;
use crate::usage::UsageCheckRequest;
use crate::usage::UsageDecision;
use crate::usage::UsageMeter;
use crate::usage::UsageMetric;
use crate::usage::UsageRecord;
use crate::validation::StrictValidator;

/// Default page size for list-style tools.
const DEFAULT_LIST_LIMIT: usize = 50;
/// Maximum page size for list-style tools.
const MAX_LIST_LIMIT: usize = 1000;
/// Reserved default namespace identifier.
const DEFAULT_NAMESPACE_ID: u64 = 1;

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
    /// Anchor policy requirements for evidence providers.
    anchor_policy: EvidenceAnchorPolicy,
    /// Per-provider trust requirement overrides.
    provider_trust_overrides: BTreeMap<String, TrustRequirement>,
    /// Runpack security context metadata.
    runpack_security_context: Option<decision_gate_core::RunpackSecurityContext>,
    /// Capability registry used for preflight validation.
    capabilities: Arc<CapabilityRegistry>,
    /// Provider discovery configuration.
    provider_discovery: ProviderDiscoveryConfig,
    /// Authn/authz policy for tool calls.
    authz: Arc<dyn ToolAuthz>,
    /// Tenant authorization policy for tool calls.
    tenant_authorizer: Arc<dyn TenantAuthorizer>,
    /// Usage metering and quota enforcement.
    usage_meter: Arc<dyn UsageMeter>,
    /// Optional runpack storage backend for managed deployments.
    runpack_storage: Option<Arc<dyn RunpackStorage>>,
    /// Optional object-store backend for runpack export/verify.
    runpack_object_store: Option<Arc<ObjectStoreRunpackBackend>>,
    /// Audit sink for auth decisions.
    audit: Arc<dyn AuthAuditSink>,
    /// Audit sink for precheck events.
    precheck_audit: Arc<dyn McpAuditSink>,
    /// Registry ACL evaluator.
    registry_acl: RegistryAcl,
    /// Principal resolver for registry ACL.
    principal_resolver: PrincipalResolver,
    /// Whether to log raw precheck request/response payloads.
    precheck_audit_payloads: bool,
    /// Allow default namespace usage.
    allow_default_namespace: bool,
    /// Tenant allowlist for default namespace usage.
    default_namespace_tenants: BTreeSet<TenantId>,
    /// Namespace authority for integrated deployments.
    namespace_authority: Arc<dyn NamespaceAuthority>,
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
    /// Provider discovery configuration.
    pub provider_discovery: ProviderDiscoveryConfig,
    /// Validation configuration for strict comparator enforcement.
    pub validation: ValidationConfig,
    /// Authn/authz policy for tool calls.
    pub authz: Arc<dyn ToolAuthz>,
    /// Tenant authorization policy for tool calls.
    pub tenant_authorizer: Arc<dyn TenantAuthorizer>,
    /// Usage metering and quota enforcement.
    pub usage_meter: Arc<dyn UsageMeter>,
    /// Optional runpack storage backend for managed deployments.
    pub runpack_storage: Option<Arc<dyn RunpackStorage>>,
    /// Optional object-store backend for runpack export/verify.
    pub runpack_object_store: Option<Arc<ObjectStoreRunpackBackend>>,
    /// Audit sink for auth decisions.
    pub audit: Arc<dyn AuthAuditSink>,
    /// Minimum trust requirement for evidence evaluation.
    pub trust_requirement: TrustRequirement,
    /// Anchor policy requirements for evidence providers.
    pub anchor_policy: EvidenceAnchorPolicy,
    /// Per-provider trust requirement overrides.
    pub provider_trust_overrides: BTreeMap<String, TrustRequirement>,
    /// Runpack security context metadata.
    pub runpack_security_context: Option<decision_gate_core::RunpackSecurityContext>,
    /// Audit sink for precheck events.
    pub precheck_audit: Arc<dyn McpAuditSink>,
    /// Registry ACL evaluator.
    pub registry_acl: RegistryAcl,
    /// Principal resolver for registry ACL.
    pub principal_resolver: PrincipalResolver,
    /// Whether to log raw precheck request/response payloads.
    pub precheck_audit_payloads: bool,
    /// Allow default namespace usage.
    pub allow_default_namespace: bool,
    /// Tenant allowlist for default namespace usage.
    pub default_namespace_tenants: BTreeSet<TenantId>,
    /// Namespace authority for integrated deployments.
    pub namespace_authority: Arc<dyn NamespaceAuthority>,
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
            provider_discovery: config.provider_discovery,
            authz: config.authz,
            tenant_authorizer: config.tenant_authorizer,
            usage_meter: config.usage_meter,
            runpack_storage: config.runpack_storage,
            runpack_object_store: config.runpack_object_store,
            audit: config.audit,
            trust_requirement: config.trust_requirement,
            anchor_policy: config.anchor_policy,
            provider_trust_overrides: config.provider_trust_overrides,
            runpack_security_context: config.runpack_security_context,
            precheck_audit: config.precheck_audit,
            precheck_audit_payloads: config.precheck_audit_payloads,
            registry_acl: config.registry_acl,
            principal_resolver: config.principal_resolver,
            allow_default_namespace: config.allow_default_namespace,
            default_namespace_tenants: config.default_namespace_tenants,
            namespace_authority: config.namespace_authority,
        }
    }

    /// Lists the MCP tools supported by this server.
    ///
    /// # Errors
    ///
    /// Returns [`ToolError`] when authorization fails.
    pub async fn list_tools(
        &self,
        context: &RequestContext,
    ) -> Result<Vec<ToolDefinition>, ToolError> {
        let _ = self.authorize(context, AuthAction::ListTools).await?;
        Ok(decision_gate_contract::tooling::tool_definitions())
    }

    /// Handles a tool call by name with JSON payload.
    ///
    /// # Errors
    ///
    /// Returns [`ToolError`] when routing fails.
    pub async fn handle_tool_call(
        &self,
        context: &RequestContext,
        name: &str,
        payload: Value,
    ) -> Result<Value, ToolError> {
        let tool = ToolName::parse(name).ok_or(ToolError::UnknownTool)?;
        let auth_ctx = self.authorize(context, AuthAction::CallTool(&tool)).await?;
        match tool {
            ToolName::ScenarioDefine => {
                self.handle_scenario_define(context, &auth_ctx, payload).await
            }
            ToolName::ScenarioStart => {
                self.handle_scenario_start(context, &auth_ctx, payload).await
            }
            ToolName::ScenarioStatus => {
                self.handle_scenario_status(context, &auth_ctx, payload).await
            }
            ToolName::ScenarioNext => self.handle_scenario_next(context, &auth_ctx, payload).await,
            ToolName::ScenarioSubmit => {
                self.handle_scenario_submit(context, &auth_ctx, payload).await
            }
            ToolName::ScenarioTrigger => {
                self.handle_scenario_trigger(context, &auth_ctx, payload).await
            }
            ToolName::EvidenceQuery => {
                self.handle_evidence_query(context, &auth_ctx, payload).await
            }
            ToolName::RunpackExport => {
                self.handle_runpack_export(context, &auth_ctx, payload).await
            }
            ToolName::RunpackVerify => Self::handle_runpack_verify(payload),
            ToolName::ProvidersList => self.handle_providers_list(payload),
            ToolName::ProviderContractGet => {
                self.handle_provider_contract_get(context, &auth_ctx, payload)
            }
            ToolName::ProviderSchemaGet => {
                self.handle_provider_schema_get(context, &auth_ctx, payload)
            }
            ToolName::SchemasRegister => {
                self.handle_schemas_register(context, &auth_ctx, payload).await
            }
            ToolName::SchemasList => self.handle_schemas_list(context, &auth_ctx, payload).await,
            ToolName::SchemasGet => self.handle_schemas_get(context, &auth_ctx, payload).await,
            ToolName::ScenariosList => {
                self.handle_scenarios_list(context, &auth_ctx, payload).await
            }
            ToolName::Precheck => self.handle_precheck(context, &auth_ctx, payload).await,
        }
    }

    /// Handles scenario definition tool requests.
    async fn handle_scenario_define(
        &self,
        context: &RequestContext,
        auth_ctx: &AuthContext,
        payload: Value,
    ) -> Result<Value, ToolError> {
        let tool = ToolName::ScenarioDefine;
        let request = decode::<ScenarioDefineRequest>(payload)?;
        let tenant_id = request.spec.default_tenant_id;
        let namespace_id = request.spec.namespace_id;
        self.ensure_tool_call_allowed(
            context,
            auth_ctx,
            tool,
            tenant_id.as_ref(),
            Some(&namespace_id),
        )
        .await?;
        self.ensure_namespace_allowed(context, tenant_id.as_ref(), &namespace_id).await?;
        let router = self.clone();
        let context = context.clone();
        let context_for_define = context.clone();
        let response = tokio::task::spawn_blocking(move || {
            router.define_scenario(&context_for_define, request)
        })
        .await
        .map_err(|err| ToolError::Internal(format!("scenario define join failed: {err}")))??;
        self.record_tool_call_usage(
            &context,
            auth_ctx,
            tool,
            tenant_id.as_ref(),
            Some(&namespace_id),
        );
        serde_json::to_value(response).map_err(|_| ToolError::Serialization)
    }

    /// Handles scenario start tool requests.
    async fn handle_scenario_start(
        &self,
        context: &RequestContext,
        auth_ctx: &AuthContext,
        payload: Value,
    ) -> Result<Value, ToolError> {
        let tool = ToolName::ScenarioStart;
        let request = decode::<ScenarioStartRequest>(payload)?;
        let tenant_id = request.run_config.tenant_id;
        let namespace_id = request.run_config.namespace_id;
        self.ensure_tool_call_allowed(
            context,
            auth_ctx,
            tool,
            Some(&tenant_id),
            Some(&namespace_id),
        )
        .await?;
        self.ensure_namespace_allowed(context, Some(&tenant_id), &namespace_id).await?;
        self.ensure_usage_allowed(
            context,
            auth_ctx,
            tool,
            Some(&tenant_id),
            Some(&namespace_id),
            UsageMetric::RunsStarted,
            1,
        )?;
        let router = self.clone();
        let context = context.clone();
        let context_for_start = context.clone();
        let response =
            tokio::task::spawn_blocking(move || router.start_run(&context_for_start, request))
                .await
                .map_err(|err| {
                    ToolError::Internal(format!("scenario start join failed: {err}"))
                })??;
        self.record_tool_call_usage(
            &context,
            auth_ctx,
            tool,
            Some(&tenant_id),
            Some(&namespace_id),
        );
        self.record_usage(
            &context,
            auth_ctx,
            tool,
            Some(&tenant_id),
            Some(&namespace_id),
            UsageMetric::RunsStarted,
            1,
        );
        serde_json::to_value(response).map_err(|_| ToolError::Serialization)
    }

    /// Handles scenario status tool requests.
    async fn handle_scenario_status(
        &self,
        context: &RequestContext,
        auth_ctx: &AuthContext,
        payload: Value,
    ) -> Result<Value, ToolError> {
        let tool = ToolName::ScenarioStatus;
        let request = decode::<ScenarioStatusRequest>(payload)?;
        let tenant_id = request.request.tenant_id;
        let namespace_id = request.request.namespace_id;
        self.ensure_tool_call_allowed(
            context,
            auth_ctx,
            tool,
            Some(&tenant_id),
            Some(&namespace_id),
        )
        .await?;
        self.ensure_namespace_allowed(context, Some(&tenant_id), &namespace_id).await?;
        let router = self.clone();
        let context = context.clone();
        let context_for_status = context.clone();
        let response =
            tokio::task::spawn_blocking(move || router.status(&context_for_status, &request))
                .await
                .map_err(|err| {
                    ToolError::Internal(format!("scenario status join failed: {err}"))
                })??;
        self.record_tool_call_usage(
            &context,
            auth_ctx,
            tool,
            Some(&tenant_id),
            Some(&namespace_id),
        );
        serde_json::to_value(response).map_err(|_| ToolError::Serialization)
    }

    /// Handles scenario next tool requests.
    async fn handle_scenario_next(
        &self,
        context: &RequestContext,
        auth_ctx: &AuthContext,
        payload: Value,
    ) -> Result<Value, ToolError> {
        let tool = ToolName::ScenarioNext;
        let request = decode::<ScenarioNextRequest>(payload)?;
        self.ensure_tool_call_allowed(
            context,
            auth_ctx,
            tool,
            Some(&request.request.tenant_id),
            Some(&request.request.namespace_id),
        )
        .await?;
        self.ensure_namespace_allowed(
            context,
            Some(&request.request.tenant_id),
            &request.request.namespace_id,
        )
        .await?;
        let router = self.clone();
        let context = context.clone();
        let context_for_next = context.clone();
        let request_for_next = request.clone();
        let response =
            tokio::task::spawn_blocking(move || router.next(&context_for_next, &request_for_next))
                .await
                .map_err(|err| {
                    ToolError::Internal(format!("scenario next join failed: {err}"))
                })??;
        self.record_tool_call_usage(
            &context,
            auth_ctx,
            tool,
            Some(&request.request.tenant_id),
            Some(&request.request.namespace_id),
        );
        serde_json::to_value(response).map_err(|_| ToolError::Serialization)
    }

    /// Handles scenario submit tool requests.
    async fn handle_scenario_submit(
        &self,
        context: &RequestContext,
        auth_ctx: &AuthContext,
        payload: Value,
    ) -> Result<Value, ToolError> {
        let tool = ToolName::ScenarioSubmit;
        let request = decode::<ScenarioSubmitRequest>(payload)?;
        let tenant_id = request.request.tenant_id;
        let namespace_id = request.request.namespace_id;
        self.ensure_tool_call_allowed(
            context,
            auth_ctx,
            tool,
            Some(&tenant_id),
            Some(&namespace_id),
        )
        .await?;
        self.ensure_namespace_allowed(context, Some(&tenant_id), &namespace_id).await?;
        let router = self.clone();
        let context = context.clone();
        let context_for_submit = context.clone();
        let response =
            tokio::task::spawn_blocking(move || router.submit(&context_for_submit, &request))
                .await
                .map_err(|err| {
                    ToolError::Internal(format!("scenario submit join failed: {err}"))
                })??;
        self.record_tool_call_usage(
            &context,
            auth_ctx,
            tool,
            Some(&tenant_id),
            Some(&namespace_id),
        );
        serde_json::to_value(response).map_err(|_| ToolError::Serialization)
    }

    /// Handles scenario trigger tool requests.
    async fn handle_scenario_trigger(
        &self,
        context: &RequestContext,
        auth_ctx: &AuthContext,
        payload: Value,
    ) -> Result<Value, ToolError> {
        let tool = ToolName::ScenarioTrigger;
        let request = decode::<ScenarioTriggerRequest>(payload)?;
        self.ensure_tool_call_allowed(
            context,
            auth_ctx,
            tool,
            Some(&request.trigger.tenant_id),
            Some(&request.trigger.namespace_id),
        )
        .await?;
        self.ensure_namespace_allowed(
            context,
            Some(&request.trigger.tenant_id),
            &request.trigger.namespace_id,
        )
        .await?;
        let router = self.clone();
        let context = context.clone();
        let context_for_trigger = context.clone();
        let request_for_trigger = request.clone();
        let response = tokio::task::spawn_blocking(move || {
            router.trigger(&context_for_trigger, &request_for_trigger)
        })
        .await
        .map_err(|err| ToolError::Internal(format!("scenario trigger join failed: {err}")))??;
        self.record_tool_call_usage(
            &context,
            auth_ctx,
            tool,
            Some(&request.trigger.tenant_id),
            Some(&request.trigger.namespace_id),
        );
        serde_json::to_value(response).map_err(|_| ToolError::Serialization)
    }

    /// Handles evidence query tool requests.
    async fn handle_evidence_query(
        &self,
        context: &RequestContext,
        auth_ctx: &AuthContext,
        payload: Value,
    ) -> Result<Value, ToolError> {
        let tool = ToolName::EvidenceQuery;
        let request = decode::<EvidenceQueryRequest>(payload)?;
        self.ensure_tool_call_allowed(
            context,
            auth_ctx,
            tool,
            Some(&request.context.tenant_id),
            Some(&request.context.namespace_id),
        )
        .await?;
        self.ensure_namespace_allowed(
            context,
            Some(&request.context.tenant_id),
            &request.context.namespace_id,
        )
        .await?;
        self.ensure_usage_allowed(
            context,
            auth_ctx,
            tool,
            Some(&request.context.tenant_id),
            Some(&request.context.namespace_id),
            UsageMetric::EvidenceQueries,
            1,
        )?;
        let router = self.clone();
        let context = context.clone();
        let context_for_query = context.clone();
        let request_for_query = request.clone();
        let response = tokio::task::spawn_blocking(move || {
            router.query_evidence(&context_for_query, &request_for_query)
        })
        .await
        .map_err(|err| ToolError::Internal(format!("evidence query join failed: {err}")))??;
        self.record_tool_call_usage(
            &context,
            auth_ctx,
            tool,
            Some(&request.context.tenant_id),
            Some(&request.context.namespace_id),
        );
        self.record_usage(
            &context,
            auth_ctx,
            tool,
            Some(&request.context.tenant_id),
            Some(&request.context.namespace_id),
            UsageMetric::EvidenceQueries,
            1,
        );
        serde_json::to_value(response).map_err(|_| ToolError::Serialization)
    }

    /// Handles runpack export tool requests.
    async fn handle_runpack_export(
        &self,
        context: &RequestContext,
        auth_ctx: &AuthContext,
        payload: Value,
    ) -> Result<Value, ToolError> {
        let tool = ToolName::RunpackExport;
        let request = decode::<RunpackExportRequest>(payload)?;
        let tenant_id = request.tenant_id;
        let namespace_id = request.namespace_id;
        self.ensure_tool_call_allowed(
            context,
            auth_ctx,
            tool,
            Some(&tenant_id),
            Some(&namespace_id),
        )
        .await?;
        self.ensure_namespace_allowed(context, Some(&tenant_id), &namespace_id).await?;
        self.ensure_usage_allowed(
            context,
            auth_ctx,
            tool,
            Some(&tenant_id),
            Some(&namespace_id),
            UsageMetric::RunpackExports,
            1,
        )?;
        let router = self.clone();
        let context = context.clone();
        let context_for_export = context.clone();
        let response = tokio::task::spawn_blocking(move || {
            router.export_runpack(&context_for_export, &request)
        })
        .await
        .map_err(|err| ToolError::Internal(format!("runpack export join failed: {err}")))??;
        self.record_tool_call_usage(
            &context,
            auth_ctx,
            tool,
            Some(&tenant_id),
            Some(&namespace_id),
        );
        self.record_usage(
            &context,
            auth_ctx,
            tool,
            Some(&tenant_id),
            Some(&namespace_id),
            UsageMetric::RunpackExports,
            1,
        );
        serde_json::to_value(response).map_err(|_| ToolError::Serialization)
    }

    /// Handles runpack verification tool requests.
    fn handle_runpack_verify(payload: Value) -> Result<Value, ToolError> {
        let request = decode::<RunpackVerifyRequest>(payload)?;
        let response = Self::verify_runpack(&request)?;
        serde_json::to_value(response).map_err(|_| ToolError::Serialization)
    }

    /// Handles provider discovery tool requests.
    fn handle_providers_list(&self, payload: Value) -> Result<Value, ToolError> {
        let request = decode::<ProvidersListRequest>(payload)?;
        let response = self.providers_list(&request);
        serde_json::to_value(response).map_err(|_| ToolError::Serialization)
    }

    /// Handles provider contract discovery requests.
    fn handle_provider_contract_get(
        &self,
        _context: &RequestContext,
        _auth_ctx: &AuthContext,
        payload: Value,
    ) -> Result<Value, ToolError> {
        let request = decode::<ProviderContractGetRequest>(payload)?;
        if request.provider_id.trim().is_empty() {
            return Err(ToolError::InvalidParams("provider_id must be non-empty".to_string()));
        }
        self.ensure_provider_disclosure_allowed(&request.provider_id)?;
        let view = self.capabilities.provider_contract_view(&request.provider_id)?;
        let response = ProviderContractGetResponse {
            provider_id: view.provider_id,
            contract: view.contract,
            contract_hash: view.contract_hash,
            source: view.source,
            version: view.version,
        };
        self.ensure_discovery_response_size(&response)?;
        serde_json::to_value(response).map_err(|_| ToolError::Serialization)
    }

    /// Handles provider predicate schema discovery requests.
    fn handle_provider_schema_get(
        &self,
        _context: &RequestContext,
        _auth_ctx: &AuthContext,
        payload: Value,
    ) -> Result<Value, ToolError> {
        let request = decode::<ProviderSchemaGetRequest>(payload)?;
        if request.provider_id.trim().is_empty() {
            return Err(ToolError::InvalidParams("provider_id must be non-empty".to_string()));
        }
        if request.predicate.trim().is_empty() {
            return Err(ToolError::InvalidParams("predicate must be non-empty".to_string()));
        }
        self.ensure_provider_disclosure_allowed(&request.provider_id)?;
        let view =
            self.capabilities.predicate_schema_view(&request.provider_id, &request.predicate)?;
        let response = ProviderSchemaGetResponse {
            provider_id: view.provider_id,
            predicate: view.predicate,
            params_required: view.params_required,
            params_schema: view.params_schema,
            result_schema: view.result_schema,
            allowed_comparators: view.allowed_comparators,
            determinism: view.determinism,
            anchor_types: view.anchor_types,
            content_types: view.content_types,
            examples: view.examples,
            contract_hash: view.contract_hash,
        };
        self.ensure_discovery_response_size(&response)?;
        serde_json::to_value(response).map_err(|_| ToolError::Serialization)
    }

    /// Handles schema registration tool requests.
    async fn handle_schemas_register(
        &self,
        context: &RequestContext,
        auth_ctx: &AuthContext,
        payload: Value,
    ) -> Result<Value, ToolError> {
        let tool = ToolName::SchemasRegister;
        let request = decode::<SchemasRegisterRequest>(payload)?;
        let tenant_id = request.record.tenant_id;
        let namespace_id = request.record.namespace_id;
        self.ensure_tenant_access(context, auth_ctx, tool, Some(&tenant_id), Some(&namespace_id))
            .await?;
        self.ensure_namespace_allowed(context, Some(&tenant_id), &namespace_id).await?;
        let schema_bytes = serde_json::to_vec(&request.record.schema)
            .map_err(|err| ToolError::InvalidParams(err.to_string()))?;
        self.ensure_usage_allowed(
            context,
            auth_ctx,
            tool,
            Some(&tenant_id),
            Some(&namespace_id),
            UsageMetric::ToolCall,
            1,
        )?;
        self.ensure_usage_allowed(
            context,
            auth_ctx,
            tool,
            Some(&tenant_id),
            Some(&namespace_id),
            UsageMetric::SchemasWritten,
            1,
        )?;
        self.ensure_usage_allowed(
            context,
            auth_ctx,
            tool,
            Some(&tenant_id),
            Some(&namespace_id),
            UsageMetric::RegistryEntries,
            1,
        )?;
        self.ensure_usage_allowed(
            context,
            auth_ctx,
            tool,
            Some(&tenant_id),
            Some(&namespace_id),
            UsageMetric::StorageBytes,
            u64::try_from(schema_bytes.len()).unwrap_or(u64::MAX),
        )?;
        let router = self.clone();
        let context = context.clone();
        let context_for_register = context.clone();
        let auth_ctx = auth_ctx.clone();
        let auth_ctx_for_register = auth_ctx.clone();
        let response = tokio::task::spawn_blocking(move || {
            router.schemas_register(&context_for_register, &auth_ctx_for_register, &request)
        })
        .await
        .map_err(|err| ToolError::Internal(format!("schemas register join failed: {err}")))??;
        self.record_usage(
            &context,
            &auth_ctx,
            tool,
            Some(&tenant_id),
            Some(&namespace_id),
            UsageMetric::ToolCall,
            1,
        );
        self.record_usage(
            &context,
            &auth_ctx,
            tool,
            Some(&tenant_id),
            Some(&namespace_id),
            UsageMetric::SchemasWritten,
            1,
        );
        self.record_usage(
            &context,
            &auth_ctx,
            tool,
            Some(&tenant_id),
            Some(&namespace_id),
            UsageMetric::RegistryEntries,
            1,
        );
        self.record_usage(
            &context,
            &auth_ctx,
            tool,
            Some(&tenant_id),
            Some(&namespace_id),
            UsageMetric::StorageBytes,
            u64::try_from(schema_bytes.len()).unwrap_or(u64::MAX),
        );
        serde_json::to_value(response).map_err(|_| ToolError::Serialization)
    }

    /// Handles schema list tool requests.
    async fn handle_schemas_list(
        &self,
        context: &RequestContext,
        auth_ctx: &AuthContext,
        payload: Value,
    ) -> Result<Value, ToolError> {
        let tool = ToolName::SchemasList;
        let request = decode::<SchemasListRequest>(payload)?;
        let tenant_id = request.tenant_id;
        let namespace_id = request.namespace_id;
        self.ensure_tool_call_allowed(
            context,
            auth_ctx,
            tool,
            Some(&tenant_id),
            Some(&namespace_id),
        )
        .await?;
        self.ensure_namespace_allowed(context, Some(&tenant_id), &namespace_id).await?;
        let router = self.clone();
        let context = context.clone();
        let context_for_list = context.clone();
        let auth_ctx = auth_ctx.clone();
        let auth_ctx_for_list = auth_ctx.clone();
        let response = tokio::task::spawn_blocking(move || {
            router.schemas_list(&context_for_list, &auth_ctx_for_list, &request)
        })
        .await
        .map_err(|err| ToolError::Internal(format!("schemas list join failed: {err}")))??;
        self.record_tool_call_usage(
            &context,
            &auth_ctx,
            tool,
            Some(&tenant_id),
            Some(&namespace_id),
        );
        serde_json::to_value(response).map_err(|_| ToolError::Serialization)
    }

    /// Handles schema get tool requests.
    async fn handle_schemas_get(
        &self,
        context: &RequestContext,
        auth_ctx: &AuthContext,
        payload: Value,
    ) -> Result<Value, ToolError> {
        let tool = ToolName::SchemasGet;
        let request = decode::<SchemasGetRequest>(payload)?;
        let tenant_id = request.tenant_id;
        let namespace_id = request.namespace_id;
        self.ensure_tool_call_allowed(
            context,
            auth_ctx,
            tool,
            Some(&tenant_id),
            Some(&namespace_id),
        )
        .await?;
        self.ensure_namespace_allowed(context, Some(&tenant_id), &namespace_id).await?;
        let router = self.clone();
        let context = context.clone();
        let context_for_get = context.clone();
        let auth_ctx = auth_ctx.clone();
        let auth_ctx_for_get = auth_ctx.clone();
        let response = tokio::task::spawn_blocking(move || {
            router.schemas_get(&context_for_get, &auth_ctx_for_get, &request)
        })
        .await
        .map_err(|err| ToolError::Internal(format!("schemas get join failed: {err}")))??;
        self.record_tool_call_usage(
            &context,
            &auth_ctx,
            tool,
            Some(&tenant_id),
            Some(&namespace_id),
        );
        serde_json::to_value(response).map_err(|_| ToolError::Serialization)
    }

    /// Handles scenario list tool requests.
    async fn handle_scenarios_list(
        &self,
        context: &RequestContext,
        auth_ctx: &AuthContext,
        payload: Value,
    ) -> Result<Value, ToolError> {
        let tool = ToolName::ScenariosList;
        let request = decode::<ScenariosListRequest>(payload)?;
        let tenant_id = request.tenant_id;
        let namespace_id = request.namespace_id;
        self.ensure_tool_call_allowed(
            context,
            auth_ctx,
            tool,
            Some(&tenant_id),
            Some(&namespace_id),
        )
        .await?;
        self.ensure_namespace_allowed(context, Some(&tenant_id), &namespace_id).await?;
        let router = self.clone();
        let context = context.clone();
        let context_for_list = context.clone();
        let response =
            tokio::task::spawn_blocking(move || router.scenarios_list(&context_for_list, &request))
                .await
                .map_err(|err| {
                    ToolError::Internal(format!("scenarios list join failed: {err}"))
                })??;
        self.record_tool_call_usage(
            &context,
            auth_ctx,
            tool,
            Some(&tenant_id),
            Some(&namespace_id),
        );
        serde_json::to_value(response).map_err(|_| ToolError::Serialization)
    }

    /// Handles precheck tool requests.
    async fn handle_precheck(
        &self,
        context: &RequestContext,
        auth_ctx: &AuthContext,
        payload: Value,
    ) -> Result<Value, ToolError> {
        let tool = ToolName::Precheck;
        let request = decode::<PrecheckToolRequest>(payload)?;
        let tenant_id = request.tenant_id;
        let namespace_id = request.namespace_id;
        self.ensure_tool_call_allowed(
            context,
            auth_ctx,
            tool,
            Some(&tenant_id),
            Some(&namespace_id),
        )
        .await?;
        self.ensure_namespace_allowed(context, Some(&tenant_id), &namespace_id).await?;
        let router = self.clone();
        let context = context.clone();
        let context_for_precheck = context.clone();
        let response =
            tokio::task::spawn_blocking(move || router.precheck(&context_for_precheck, &request))
                .await
                .map_err(|err| ToolError::Internal(format!("precheck join failed: {err}")))??;
        self.record_tool_call_usage(
            &context,
            auth_ctx,
            tool,
            Some(&tenant_id),
            Some(&namespace_id),
        );
        serde_json::to_value(response).map_err(|_| ToolError::Serialization)
    }

    /// Enforces tenant access and tool call usage limits.
    async fn ensure_tool_call_allowed(
        &self,
        context: &RequestContext,
        auth_ctx: &AuthContext,
        tool: ToolName,
        tenant_id: Option<&TenantId>,
        namespace_id: Option<&NamespaceId>,
    ) -> Result<(), ToolError> {
        self.ensure_tenant_access(context, auth_ctx, tool, tenant_id, namespace_id).await?;
        self.ensure_usage_allowed(
            context,
            auth_ctx,
            tool,
            tenant_id,
            namespace_id,
            UsageMetric::ToolCall,
            1,
        )
    }

    /// Records tool call usage after successful actions.
    fn record_tool_call_usage(
        &self,
        context: &RequestContext,
        auth_ctx: &AuthContext,
        tool: ToolName,
        tenant_id: Option<&TenantId>,
        namespace_id: Option<&NamespaceId>,
    ) {
        self.record_usage(
            context,
            auth_ctx,
            tool,
            tenant_id,
            namespace_id,
            UsageMetric::ToolCall,
            1,
        );
    }

    /// Ensures provider contract/schema disclosure is permitted.
    fn ensure_provider_disclosure_allowed(&self, provider_id: &str) -> Result<(), ToolError> {
        if self.provider_discovery.is_allowed(provider_id) {
            return Ok(());
        }
        Err(ToolError::Unauthorized("provider contract disclosure denied".to_string()))
    }

    /// Ensures provider discovery responses stay within configured size limits.
    fn ensure_discovery_response_size<T: Serialize>(&self, payload: &T) -> Result<(), ToolError> {
        match canonical_json_bytes_with_limit(payload, self.provider_discovery.max_response_bytes) {
            Ok(_) => Ok(()),
            Err(HashError::SizeLimitExceeded {
                limit,
                actual,
            }) => Err(ToolError::ResponseTooLarge(format!(
                "provider discovery response exceeds size limit ({actual} > {limit})"
            ))),
            Err(HashError::Canonicalization(err)) => Err(ToolError::Internal(format!(
                "failed to canonicalize discovery response: {err}"
            ))),
        }
    }
}

#[cfg(test)]
mod tests;

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
    /// Output directory for runpack artifacts (filesystem storage only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_dir: Option<String>,
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
    /// Optional storage URI for managed runpack storage.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub storage_uri: Option<String>,
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

/// `provider_contract_get` request payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderContractGetRequest {
    /// Provider identifier.
    pub provider_id: String,
}

/// `provider_contract_get` response payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderContractGetResponse {
    /// Provider identifier.
    pub provider_id: String,
    /// Provider contract payload.
    pub contract: decision_gate_contract::types::ProviderContract,
    /// Canonical contract hash.
    pub contract_hash: decision_gate_core::hashing::HashDigest,
    /// Contract source origin.
    pub source: ProviderContractSource,
    /// Optional contract version label.
    pub version: Option<String>,
}

/// `provider_schema_get` request payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderSchemaGetRequest {
    /// Provider identifier.
    pub provider_id: String,
    /// Predicate name.
    pub predicate: String,
}

/// `provider_schema_get` response payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderSchemaGetResponse {
    /// Provider identifier.
    pub provider_id: String,
    /// Predicate name.
    pub predicate: String,
    /// Whether params are required for this predicate.
    pub params_required: bool,
    /// JSON schema for predicate params.
    pub params_schema: Value,
    /// JSON schema for predicate result values.
    pub result_schema: Value,
    /// Comparator allow-list.
    pub allowed_comparators: Vec<Comparator>,
    /// Determinism classification.
    pub determinism: DeterminismClass,
    /// Anchor types emitted by this predicate.
    pub anchor_types: Vec<String>,
    /// Content types for predicate output.
    pub content_types: Vec<String>,
    /// Predicate examples.
    pub examples: Vec<PredicateExample>,
    /// Canonical contract hash.
    pub contract_hash: decision_gate_core::hashing::HashDigest,
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
        _context: &RequestContext,
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
                anchor_policy: self.anchor_policy.clone(),
                provider_trust_overrides: self.provider_trust_overrides.clone(),
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
    fn start_run(
        &self,
        _context: &RequestContext,
        request: ScenarioStartRequest,
    ) -> Result<RunState, ToolError> {
        let runtime = self.runtime_for(&request.scenario_id)?;
        let state = runtime
            .control
            .start_run(request.run_config, request.started_at, request.issue_entry_packets)
            .map_err(ToolError::ControlPlane)?;
        Ok(state)
    }

    /// Returns the current status for a scenario run.
    fn status(
        &self,
        _context: &RequestContext,
        request: &ScenarioStatusRequest,
    ) -> Result<ScenarioStatus, ToolError> {
        let runtime = self.runtime_for(&request.scenario_id)?;
        let status =
            runtime.control.scenario_status(&request.request).map_err(ToolError::ControlPlane)?;
        Ok(status)
    }

    /// Advances a scenario evaluation.
    fn next(
        &self,
        _context: &RequestContext,
        request: &ScenarioNextRequest,
    ) -> Result<NextResult, ToolError> {
        let runtime = self.runtime_for(&request.scenario_id)?;
        let result =
            runtime.control.scenario_next(&request.request).map_err(ToolError::ControlPlane)?;
        Ok(result)
    }

    /// Submits external artifacts to a scenario run.
    fn submit(
        &self,
        _context: &RequestContext,
        request: &ScenarioSubmitRequest,
    ) -> Result<SubmitResult, ToolError> {
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
    fn trigger(
        &self,
        _context: &RequestContext,
        request: &ScenarioTriggerRequest,
    ) -> Result<TriggerResult, ToolError> {
        let runtime = self.runtime_for(&request.scenario_id)?;
        let result = runtime.control.trigger(&request.trigger).map_err(ToolError::ControlPlane)?;
        Ok(result)
    }

    /// Queries evidence providers with disclosure policy enforcement.
    fn query_evidence(
        &self,
        _context: &RequestContext,
        request: &EvidenceQueryRequest,
    ) -> Result<EvidenceQueryResponse, ToolError> {
        self.capabilities.validate_query(&request.query).map_err(ToolError::from)?;
        let mut result = match self.evidence.query(&request.query, &request.context) {
            Ok(result) => result,
            Err(err) => EvidenceResult {
                value: None,
                lane: TrustLane::Verified,
                error: Some(EvidenceProviderError {
                    code: "provider_error".to_string(),
                    message: err.to_string(),
                    details: None,
                }),
                evidence_hash: None,
                evidence_ref: None,
                evidence_anchor: None,
                signature: None,
                content_type: None,
            },
        };
        if result.error.is_some() {
            result.value = None;
            result.content_type = None;
            result.evidence_hash = None;
        }
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
    #[allow(
        clippy::too_many_lines,
        reason = "Runpack export handles all backend modes and audit paths."
    )]
    fn export_runpack(
        &self,
        _context: &RequestContext,
        request: &RunpackExportRequest,
    ) -> Result<RunpackExportResponse, ToolError> {
        let runtime = self.runtime_for(&request.scenario_id)?;
        let manifest_name = request.manifest_name.as_deref().unwrap_or("manifest.json");
        let state = runtime
            .store
            .load(&request.tenant_id, &request.namespace_id, &request.run_id)
            .map_err(|err| ToolError::Runpack(err.to_string()))?
            .ok_or_else(|| ToolError::NotFound("run not found".to_string()))?;
        let mut builder = RunpackBuilder::new(self.anchor_policy.clone());
        if let Some(context) = self.runpack_security_context.clone() {
            builder = builder.with_security_context(context);
        }
        if let Some(storage) = &self.runpack_storage {
            let temp_dir = tempfile::Builder::new()
                .prefix("decision-gate-runpack-")
                .tempdir()
                .map_err(|err| ToolError::Runpack(err.to_string()))?;
            let output_dir = temp_dir.path().to_path_buf();
            let mut sink = FileArtifactSink::new(output_dir.clone(), manifest_name)
                .map_err(|err| ToolError::Runpack(err.to_string()))?;
            if request.include_verification {
                let reader = FileArtifactReader::new(output_dir.clone())
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
                let storage_uri = Self::store_runpack(storage, request, &output_dir)
                    .map_err(|err| ToolError::Runpack(err.to_string()))?;
                return Ok(RunpackExportResponse {
                    manifest,
                    report: Some(report),
                    storage_uri,
                });
            }
            let manifest = builder
                .build(&mut sink, &runtime.spec, &state, request.generated_at)
                .map_err(|err| ToolError::Runpack(err.to_string()))?;
            let storage_uri = Self::store_runpack(storage, request, &output_dir)
                .map_err(|err| ToolError::Runpack(err.to_string()))?;
            return Ok(RunpackExportResponse {
                manifest,
                report: None,
                storage_uri,
            });
        }

        if let Some(backend) = &self.runpack_object_store {
            let spec_hash = runtime
                .spec
                .canonical_hash_with(DEFAULT_HASH_ALGORITHM)
                .map_err(|err| ToolError::Runpack(err.to_string()))?;
            let key = RunpackObjectKey {
                tenant_id: request.tenant_id,
                namespace_id: request.namespace_id,
                scenario_id: request.scenario_id.clone(),
                run_id: request.run_id.clone(),
                spec_hash,
            };
            let mut sink = backend
                .sink(&key, manifest_name)
                .map_err(|err| ToolError::Runpack(err.to_string()))?;
            if request.include_verification {
                let reader =
                    backend.reader(&key).map_err(|err| ToolError::Runpack(err.to_string()))?;
                let (manifest, report) = builder
                    .build_with_verification(
                        &mut sink,
                        &reader,
                        &runtime.spec,
                        &state,
                        request.generated_at,
                    )
                    .map_err(|err| ToolError::Runpack(err.to_string()))?;
                let storage_uri = Some(
                    backend.storage_uri(&key).map_err(|err| ToolError::Runpack(err.to_string()))?,
                );
                return Ok(RunpackExportResponse {
                    manifest,
                    report: Some(report),
                    storage_uri,
                });
            }
            let manifest = builder
                .build(&mut sink, &runtime.spec, &state, request.generated_at)
                .map_err(|err| ToolError::Runpack(err.to_string()))?;
            let storage_uri =
                Some(backend.storage_uri(&key).map_err(|err| ToolError::Runpack(err.to_string()))?);
            return Ok(RunpackExportResponse {
                manifest,
                report: None,
                storage_uri,
            });
        }

        let output_dir = request.output_dir.as_ref().ok_or_else(|| {
            ToolError::Runpack(
                "output_dir is required when runpack storage is not configured".to_string(),
            )
        })?;
        let output_dir = PathBuf::from(output_dir);
        let mut sink = FileArtifactSink::new(output_dir.clone(), manifest_name)
            .map_err(|err| ToolError::Runpack(err.to_string()))?;
        if request.include_verification {
            let reader = FileArtifactReader::new(output_dir)
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
                storage_uri: None,
            });
        }
        let manifest = builder
            .build(&mut sink, &runtime.spec, &state, request.generated_at)
            .map_err(|err| ToolError::Runpack(err.to_string()))?;
        Ok(RunpackExportResponse {
            manifest,
            report: None,
            storage_uri: None,
        })
    }

    /// Stores the runpack directory using the configured storage backend.
    fn store_runpack(
        storage: &Arc<dyn RunpackStorage>,
        request: &RunpackExportRequest,
        output_dir: &Path,
    ) -> Result<Option<String>, RunpackStorageError> {
        let key = RunpackStorageKey {
            tenant_id: request.tenant_id,
            namespace_id: request.namespace_id,
            run_id: request.run_id.clone(),
        };
        storage.store_runpack(&key, output_dir)
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
        context: &RequestContext,
        auth_ctx: &AuthContext,
        request: &SchemasRegisterRequest,
    ) -> Result<SchemasRegisterResponse, ToolError> {
        self.ensure_registry_access(
            context,
            auth_ctx,
            RegistryAclAction::Register,
            request.record.tenant_id,
            request.record.namespace_id,
            Some((&request.record.schema_id, &request.record.version)),
        )?;
        self.validate_schema_signing(&request.record)?;
        self.validate_schema_limits(&request.record)?;
        let _ = compile_json_schema(&request.record.schema)?;
        self.schema_registry.register(request.record.clone())?;
        Ok(SchemasRegisterResponse {
            record: request.record.clone(),
        })
    }

    /// Lists data shape schemas.
    fn schemas_list(
        &self,
        context: &RequestContext,
        auth_ctx: &AuthContext,
        request: &SchemasListRequest,
    ) -> Result<SchemasListResponse, ToolError> {
        self.ensure_registry_access(
            context,
            auth_ctx,
            RegistryAclAction::List,
            request.tenant_id,
            request.namespace_id,
            None,
        )?;
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
    fn schemas_get(
        &self,
        context: &RequestContext,
        auth_ctx: &AuthContext,
        request: &SchemasGetRequest,
    ) -> Result<SchemasGetResponse, ToolError> {
        self.ensure_registry_access(
            context,
            auth_ctx,
            RegistryAclAction::Get,
            request.tenant_id,
            request.namespace_id,
            Some((&request.schema_id, &request.version)),
        )?;
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
        _context: &RequestContext,
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
                        namespace_id: runtime.spec.namespace_id,
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
    fn precheck(
        &self,
        context: &RequestContext,
        request: &PrecheckToolRequest,
    ) -> Result<PrecheckToolResponse, ToolError> {
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

        let (spec, control) = self.resolve_precheck_control(request)?;
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
        let response = PrecheckToolResponse {
            decision: result.decision,
            gate_evaluations: result.gate_evaluations,
        };
        self.record_precheck_audit(context, request, &response)?;
        Ok(response)
    }

    /// Resolves the scenario spec and control plane used for precheck requests.
    fn resolve_precheck_control(
        &self,
        request: &PrecheckToolRequest,
    ) -> Result<(ScenarioSpec, ControlPlaneWrapper), ToolError> {
        match (&request.scenario_id, &request.spec) {
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
                        anchor_policy: self.anchor_policy.clone(),
                        ..ControlPlaneConfig::default()
                    },
                )?;
                Ok((spec.clone(), ControlPlaneWrapper::Owned(Box::new(control))))
            }
            (Some(scenario_id), None) => {
                let runtime = self.runtime_for(scenario_id)?;
                if runtime.spec.namespace_id != request.namespace_id {
                    return Err(ToolError::NotFound("scenario not in namespace".to_string()));
                }
                Ok((runtime.spec.clone(), ControlPlaneWrapper::Borrowed(runtime)))
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
                        anchor_policy: self.anchor_policy.clone(),
                        ..ControlPlaneConfig::default()
                    },
                )?;
                Ok((spec.clone(), ControlPlaneWrapper::Owned(Box::new(control))))
            }
            (None, None) => {
                Err(ToolError::InvalidParams("scenario_id or spec is required".to_string()))
            }
        }
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

    /// Enforces the default namespace policy.
    async fn ensure_namespace_allowed(
        &self,
        context: &RequestContext,
        tenant_id: Option<&TenantId>,
        namespace_id: &NamespaceId,
    ) -> Result<(), ToolError> {
        if namespace_id.get() == DEFAULT_NAMESPACE_ID {
            if !self.allow_default_namespace {
                return Err(ToolError::Unauthorized(
                    "default namespace is not allowed".to_string(),
                ));
            }
            let tenant = tenant_id.ok_or_else(|| {
                ToolError::Unauthorized("default namespace requires tenant_id".to_string())
            })?;
            if !self.default_namespace_tenants.contains(tenant) {
                return Err(ToolError::Unauthorized(
                    "default namespace not allowed for tenant".to_string(),
                ));
            }
        }
        self.namespace_authority
            .ensure_namespace(
                tenant_id,
                namespace_id,
                context
                    .unsafe_client_correlation_id
                    .as_deref()
                    .or(context.server_correlation_id.as_deref())
                    .or(context.request_id.as_deref()),
            )
            .await
            .map_err(map_namespace_error)
    }

    /// Enforces tenant authorization for a tool call.
    async fn ensure_tenant_access(
        &self,
        context: &RequestContext,
        auth_ctx: &AuthContext,
        tool: ToolName,
        tenant_id: Option<&TenantId>,
        namespace_id: Option<&NamespaceId>,
    ) -> Result<(), ToolError> {
        let decision = self
            .tenant_authorizer
            .authorize(
                auth_ctx,
                TenantAccessRequest {
                    action: TenantAuthzAction::ToolCall(&tool),
                    tenant_id,
                    namespace_id,
                },
            )
            .await;
        self.record_tenant_authz(context, auth_ctx, tool, tenant_id, namespace_id, &decision);
        if decision.allowed { Ok(()) } else { Err(ToolError::Unauthorized(decision.reason)) }
    }

    /// Enforces registry ACL decisions and emits registry audit events.
    fn ensure_registry_access(
        &self,
        context: &RequestContext,
        auth_ctx: &AuthContext,
        action: RegistryAclAction,
        tenant_id: TenantId,
        namespace_id: NamespaceId,
        schema: Option<(&DataShapeId, &DataShapeVersion)>,
    ) -> Result<(), ToolError> {
        let principal = self.principal_resolver.resolve(auth_ctx);
        let decision = self.registry_acl.authorize(&principal, action, &tenant_id, &namespace_id);
        self.record_registry_audit(
            context,
            &principal,
            action,
            tenant_id,
            namespace_id,
            schema,
            &decision,
        );
        if decision.allowed {
            Ok(())
        } else {
            Err(ToolError::Unauthorized("schema registry access denied".to_string()))
        }
    }

    /// Records a tenant authorization audit event.
    fn record_tenant_authz(
        &self,
        context: &RequestContext,
        auth_ctx: &AuthContext,
        tool: ToolName,
        tenant_id: Option<&TenantId>,
        namespace_id: Option<&NamespaceId>,
        decision: &TenantAuthzDecision,
    ) {
        let event = TenantAuthzEvent::new(TenantAuthzEventParams {
            request_id: context.request_id.clone(),
            unsafe_client_correlation_id: context.unsafe_client_correlation_id.clone(),
            server_correlation_id: server_correlation_id_for(context),
            tool: Some(tool),
            allowed: decision.allowed,
            reason: decision.reason.clone(),
            principal_id: auth_ctx.principal_id(),
            tenant_id: tenant_id.map(ToString::to_string),
            namespace_id: namespace_id.map(ToString::to_string),
        });
        self.precheck_audit.record_tenant_authz(&event);
    }

    /// Enforces usage quotas for a tool call.
    #[allow(clippy::too_many_arguments, reason = "Usage checks require full request context.")]
    fn ensure_usage_allowed(
        &self,
        context: &RequestContext,
        auth_ctx: &AuthContext,
        tool: ToolName,
        tenant_id: Option<&TenantId>,
        namespace_id: Option<&NamespaceId>,
        metric: UsageMetric,
        units: u64,
    ) -> Result<(), ToolError> {
        let decision = self.usage_meter.check(
            auth_ctx,
            UsageCheckRequest {
                tool: &tool,
                tenant_id,
                namespace_id,
                correlation_id: context.unsafe_client_correlation_id.as_deref(),
                server_correlation_id: context.server_correlation_id.as_deref(),
                request_id: context.request_id.as_deref(),
                metric,
                units,
            },
        );
        self.record_usage_audit(
            context,
            auth_ctx,
            tool,
            tenant_id,
            namespace_id,
            metric,
            units,
            &decision,
        );
        if decision.allowed { Ok(()) } else { Err(ToolError::Unauthorized(decision.reason)) }
    }

    /// Records usage after a successful action.
    #[allow(clippy::too_many_arguments, reason = "Usage records mirror check context.")]
    fn record_usage(
        &self,
        context: &RequestContext,
        auth_ctx: &AuthContext,
        tool: ToolName,
        tenant_id: Option<&TenantId>,
        namespace_id: Option<&NamespaceId>,
        metric: UsageMetric,
        units: u64,
    ) {
        self.usage_meter.record(
            auth_ctx,
            UsageRecord {
                tool: &tool,
                tenant_id,
                namespace_id,
                correlation_id: context.unsafe_client_correlation_id.as_deref(),
                server_correlation_id: context.server_correlation_id.as_deref(),
                request_id: context.request_id.as_deref(),
                metric,
                units,
            },
        );
    }

    /// Records a usage audit event.
    #[allow(clippy::too_many_arguments, reason = "Audit records capture full usage context.")]
    fn record_usage_audit(
        &self,
        context: &RequestContext,
        auth_ctx: &AuthContext,
        tool: ToolName,
        tenant_id: Option<&TenantId>,
        namespace_id: Option<&NamespaceId>,
        metric: UsageMetric,
        units: u64,
        decision: &UsageDecision,
    ) {
        let event = UsageAuditEvent::new(UsageAuditEventParams {
            request_id: context.request_id.clone(),
            unsafe_client_correlation_id: context.unsafe_client_correlation_id.clone(),
            server_correlation_id: server_correlation_id_for(context),
            tool: Some(tool),
            tenant_id: tenant_id.map(ToString::to_string),
            namespace_id: namespace_id.map(ToString::to_string),
            principal_id: auth_ctx.principal_id(),
            metric: usage_metric_label(metric).to_string(),
            units,
            allowed: decision.allowed,
            reason: decision.reason.clone(),
        });
        self.precheck_audit.record_usage(&event);
    }

    /// Validates schema signing metadata when required by registry ACL.
    fn validate_schema_signing(&self, record: &DataShapeRecord) -> Result<(), ToolError> {
        if !self.registry_acl.require_signing() {
            return Ok(());
        }
        let Some(signing) = &record.signing else {
            return Err(ToolError::Unauthorized("schema signing metadata required".to_string()));
        };
        if signing.key_id.trim().is_empty() || signing.signature.trim().is_empty() {
            return Err(ToolError::Unauthorized("schema signing metadata invalid".to_string()));
        }
        Ok(())
    }

    /// Records a registry audit event.
    #[allow(
        clippy::too_many_arguments,
        reason = "Audit call collects all fields for a single log entry."
    )]
    fn record_registry_audit(
        &self,
        context: &RequestContext,
        principal: &crate::registry_acl::RegistryPrincipal,
        action: RegistryAclAction,
        tenant_id: TenantId,
        namespace_id: NamespaceId,
        schema: Option<(&DataShapeId, &DataShapeVersion)>,
        decision: &RegistryAclDecision,
    ) {
        let (schema_id, schema_version) = schema.map_or((None, None), |(id, version)| {
            (Some(id.to_string()), Some(version.to_string()))
        });
        let event = RegistryAuditEvent::new(RegistryAuditEventParams {
            request_id: context.request_id.clone(),
            unsafe_client_correlation_id: context.unsafe_client_correlation_id.clone(),
            server_correlation_id: server_correlation_id_for(context),
            tenant_id: tenant_id.to_string(),
            namespace_id: namespace_id.to_string(),
            action,
            allowed: decision.allowed,
            reason: decision.reason.clone(),
            principal_id: principal.principal_id.clone(),
            policy_class: principal.policy_class.clone(),
            roles: principal.roles.iter().map(|role| role.name.clone()).collect(),
            schema_id,
            schema_version,
        });
        self.precheck_audit.record_registry(&event);
    }

    /// Emits a hash-only precheck audit event.
    fn record_precheck_audit(
        &self,
        context: &RequestContext,
        request: &PrecheckToolRequest,
        response: &PrecheckToolResponse,
    ) -> Result<(), ToolError> {
        let request_hash = hash_canonical_json(DEFAULT_HASH_ALGORITHM, request)
            .map_err(|err| ToolError::Internal(err.to_string()))?;
        let response_hash = hash_canonical_json(DEFAULT_HASH_ALGORITHM, response)
            .map_err(|err| ToolError::Internal(err.to_string()))?;
        let (request_payload, response_payload, redaction) = if self.precheck_audit_payloads {
            (
                Some(
                    serde_json::to_value(request)
                        .map_err(|err| ToolError::Internal(err.to_string()))?,
                ),
                Some(
                    serde_json::to_value(response)
                        .map_err(|err| ToolError::Internal(err.to_string()))?,
                ),
                "payload",
            )
        } else {
            (None, None, "hash_only")
        };
        let scenario_id = request
            .scenario_id
            .clone()
            .or_else(|| request.spec.as_ref().map(|spec| spec.scenario_id.clone()))
            .map(|id| id.to_string());
        let event = PrecheckAuditEvent::new(PrecheckAuditEventParams {
            tenant_id: request.tenant_id.to_string(),
            namespace_id: request.namespace_id.to_string(),
            unsafe_client_correlation_id: context.unsafe_client_correlation_id.clone(),
            server_correlation_id: server_correlation_id_for(context),
            scenario_id,
            stage_id: request.stage_id.as_ref().map(ToString::to_string),
            schema_id: request.data_shape.schema_id.to_string(),
            schema_version: request.data_shape.version.to_string(),
            request_hash,
            response_hash,
            request: request_payload,
            response: response_payload,
            redaction,
        });
        self.precheck_audit.record_precheck(&event);
        Ok(())
    }

    /// Authorizes a tool action and emits an auth audit record.
    async fn authorize(
        &self,
        context: &RequestContext,
        action: AuthAction<'_>,
    ) -> Result<AuthContext, ToolError> {
        match self.authz.authorize(context, action).await {
            Ok(auth_ctx) => {
                self.audit.record(&AuthAuditEvent::allowed(context, action, &auth_ctx));
                Ok(auth_ctx)
            }
            Err(err) => {
                self.audit.record(&AuthAuditEvent::denied(context, action, &err));
                Err(err.into())
            }
        }
    }
}

/// Returns the server correlation identifier or a sentinel label if missing.
fn server_correlation_id_for(context: &RequestContext) -> String {
    context.server_correlation_id.clone().unwrap_or_else(|| "missing".to_string())
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

/// Maps namespace authority errors into tool errors.
fn map_namespace_error(error: NamespaceAuthorityError) -> ToolError {
    match error {
        NamespaceAuthorityError::InvalidNamespace(message) => ToolError::InvalidParams(message),
        NamespaceAuthorityError::Denied(message)
        | NamespaceAuthorityError::Unavailable(message) => ToolError::Unauthorized(message),
    }
}

/// Returns the canonical label for a usage metric.
const fn usage_metric_label(metric: UsageMetric) -> &'static str {
    match metric {
        UsageMetric::ToolCall => "tool_calls",
        UsageMetric::RunsStarted => "runs_started",
        UsageMetric::EvidenceQueries => "evidence_queries",
        UsageMetric::RunpackExports => "runpack_exports",
        UsageMetric::SchemasWritten => "schemas_written",
        UsageMetric::RegistryEntries => "registry_entries",
        UsageMetric::StorageBytes => "storage_bytes",
    }
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
    /// Tool response exceeds size limits.
    #[error("response too large: {0}")]
    ResponseTooLarge(String),
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
        error: None,
        evidence_hash: None,
        evidence_ref: None,
        evidence_anchor: None,
        signature: None,
        content_type: None,
    }
}
