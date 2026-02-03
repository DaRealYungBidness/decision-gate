// decision-gate-core/src/core/mod.rs
// ============================================================================
// Module: Decision Gate Core Types
// Description: Canonical Decision Gate schema and run-state structures.
// Purpose: Provide stable, serializable types for Decision Gate specifications and logs.
// Dependencies: ret-logic, serde
// ============================================================================

//! ## Overview
//! Decision Gate core types define scenario specifications, run state, evidence schemas,
//! and runpack manifests. These types are the canonical source of truth for
//! any derived API surfaces (HTTP, MCP, or SDKs).

// ============================================================================
// SECTION: Submodules
// ============================================================================

pub mod data_shape;
pub mod disclosure;
pub mod evidence;
pub mod hashing;
pub mod identifiers;
pub mod providers;
pub mod runpack;
pub mod spec;
pub mod state;
pub mod summary;
pub mod time;

// ============================================================================
// SECTION: Re-Exports
// ============================================================================

pub use data_shape::DataShapePage;
pub use data_shape::DataShapeRecord;
pub use data_shape::DataShapeRef;
pub use data_shape::DataShapeSignature;
pub use disclosure::ContentRef;
pub use disclosure::DispatchReceipt;
pub use disclosure::DispatchTarget;
pub use disclosure::PacketEnvelope;
pub use disclosure::PacketPayload;
pub use disclosure::PacketRecord;
pub use disclosure::VisibilityPolicy;
pub use evidence::AnchorRequirement;
pub use evidence::Comparator;
pub use evidence::EvidenceAnchor;
pub use evidence::EvidenceAnchorPolicy;
pub use evidence::EvidenceProviderError;
pub use evidence::EvidenceQuery;
pub use evidence::EvidenceRef;
pub use evidence::EvidenceResult;
pub use evidence::EvidenceSignature;
pub use evidence::EvidenceValue;
pub use evidence::ProviderAnchorPolicy;
pub use evidence::ProviderMissingError;
pub use evidence::TrustLane;
pub use evidence::TrustRequirement;
pub use hashing::DEFAULT_HASH_ALGORITHM;
pub use hashing::HashAlgorithm;
pub use hashing::HashDigest;
pub use identifiers::ConditionId;
pub use identifiers::CorrelationId;
pub use identifiers::DataShapeId;
pub use identifiers::DataShapeVersion;
pub use identifiers::DecisionId;
pub use identifiers::GateId;
pub use identifiers::NamespaceId;
pub use identifiers::PacketId;
pub use identifiers::PolicyId;
pub use identifiers::ProviderId;
pub use identifiers::RunId;
pub use identifiers::ScenarioId;
pub use identifiers::SchemaId;
pub use identifiers::SpecVersion;
pub use identifiers::StageId;
pub use identifiers::TenantId;
pub use identifiers::TriggerId;
pub use providers::BUILTIN_PROVIDER_IDS;
pub use providers::is_builtin_provider_id;
pub use runpack::ArtifactKind;
pub use runpack::ArtifactRecord;
pub use runpack::FileHashEntry;
pub use runpack::RunpackIntegrity;
pub use runpack::RunpackManifest;
pub use runpack::RunpackSecurityContext;
pub use runpack::RunpackVersion;
pub use runpack::VerifierMode;
pub use spec::AdvanceTo;
pub use spec::BranchRule;
pub use spec::ConditionSpec;
pub use spec::GateOutcome;
pub use spec::GateSpec;
pub use spec::PacketSpec;
pub use spec::PolicyRef;
pub use spec::ScenarioSpec;
pub use spec::SchemaRef;
pub use spec::SpecError;
pub use spec::StageSpec;
pub use spec::TimeoutPolicy;
pub use spec::TimeoutSpec;
pub use state::DecisionOutcome;
pub use state::DecisionRecord;
pub use state::EvidenceRecord;
pub use state::GateEvalRecord;
pub use state::GateEvaluation;
pub use state::GateTraceEntry;
pub use state::RunConfig;
pub use state::RunState;
pub use state::RunStatus;
pub use state::SubmissionRecord;
pub use state::ToolCallError;
pub use state::ToolCallErrorDetails;
pub use state::ToolCallRecord;
pub use state::TriggerEvent;
pub use state::TriggerKind;
pub use state::TriggerRecord;
pub use summary::SafeSummary;
pub use time::Timestamp;
