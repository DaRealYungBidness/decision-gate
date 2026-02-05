// crates/decision-gate-core/src/interfaces/mod.rs
// ============================================================================
// Module: Decision Gate Interfaces
// Description: Backend-agnostic interfaces for evidence, dispatch, and storage.
// Purpose: Define the contract surfaces used by Decision Gate runtime.
// Dependencies: crate::core
// ============================================================================

//! ## Overview
//! Interfaces define how Decision Gate integrates with external systems without embedding
//! backend-specific details. Implementations must be deterministic and fail
//! closed on missing or invalid data.
//!
//! Security posture: interface implementations consume untrusted inputs; see
//! `Docs/security/threat_model.md`.

// ============================================================================
// SECTION: Imports
// ============================================================================

use serde::Deserialize;
use serde::Serialize;
use thiserror::Error;

use crate::core::ArtifactKind;
use crate::core::DataShapeId;
use crate::core::DataShapePage;
use crate::core::DataShapeRecord;
use crate::core::DataShapeVersion;
use crate::core::RunState;
use crate::core::ScenarioSpec;
use crate::core::TriggerEvent;
use crate::core::disclosure::DispatchReceipt;
use crate::core::disclosure::DispatchTarget;
use crate::core::disclosure::PacketEnvelope;
use crate::core::disclosure::PacketPayload;
use crate::core::evidence::EvidenceQuery;
use crate::core::evidence::EvidenceResult;
use crate::core::evidence::ProviderMissingError;
use crate::core::identifiers::CorrelationId;
use crate::core::identifiers::NamespaceId;
use crate::core::identifiers::RunId;
use crate::core::identifiers::ScenarioId;
use crate::core::identifiers::StageId;
use crate::core::identifiers::TenantId;
use crate::core::identifiers::TriggerId;
use crate::core::runpack::RunpackManifest;
use crate::core::time::Timestamp;

// ============================================================================
// SECTION: Evidence Provider
// ============================================================================

/// Context provided to evidence providers for query evaluation.
///
/// # Invariants
/// - Identifiers refer to the same run and scenario scope.
/// - Values are snapshots; providers must not mutate them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceContext {
    /// Tenant identifier.
    pub tenant_id: TenantId,
    /// Namespace identifier.
    pub namespace_id: NamespaceId,
    /// Run identifier.
    pub run_id: RunId,
    /// Scenario identifier.
    pub scenario_id: ScenarioId,
    /// Stage identifier.
    pub stage_id: StageId,
    /// Trigger identifier.
    pub trigger_id: TriggerId,
    /// Trigger timestamp.
    pub trigger_time: Timestamp,
    /// Optional correlation identifier.
    pub correlation_id: Option<CorrelationId>,
}

/// Evidence provider errors.
///
/// # Invariants
/// - Variants are stable for programmatic handling.
#[derive(Debug, Error)]
pub enum EvidenceError {
    /// Evidence provider reported an error.
    #[error("evidence provider error: {0}")]
    Provider(String),
}

/// Backend-agnostic evidence provider.
pub trait EvidenceProvider {
    /// Resolves an evidence query into a result.
    ///
    /// # Errors
    ///
    /// Returns [`EvidenceError`] when evidence cannot be fetched or verified.
    fn query(
        &self,
        query: &EvidenceQuery,
        ctx: &EvidenceContext,
    ) -> Result<EvidenceResult, EvidenceError>;

    /// Validates that all providers referenced by the scenario are available.
    ///
    /// # Errors
    ///
    /// Returns [`ProviderMissingError`] when required providers are missing or blocked.
    fn validate_providers(&self, spec: &ScenarioSpec) -> Result<(), ProviderMissingError>;
}

// ============================================================================
// SECTION: Dispatcher
// ============================================================================

/// Dispatch errors for packet delivery.
///
/// # Invariants
/// - Variants are stable for programmatic handling.
#[derive(Debug, Error)]
pub enum DispatchError {
    /// Dispatcher reported an error.
    #[error("dispatch error: {0}")]
    DispatchFailed(String),
}

/// Packet dispatcher responsible for delivering disclosures.
pub trait Dispatcher {
    /// Dispatches a packet to a target.
    ///
    /// # Errors
    ///
    /// Returns [`DispatchError`] when dispatch fails.
    fn dispatch(
        &self,
        target: &DispatchTarget,
        envelope: &PacketEnvelope,
        payload: &PacketPayload,
    ) -> Result<DispatchReceipt, DispatchError>;
}

// ============================================================================
// SECTION: Artifact Sink / Reader
// ============================================================================

/// Artifact data payload written into runpacks.
///
/// # Invariants
/// - `path` is runpack-relative and stable for verification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Artifact {
    /// Artifact kind.
    pub kind: ArtifactKind,
    /// Runpack-relative path.
    pub path: String,
    /// Content type for the artifact.
    pub content_type: Option<String>,
    /// Artifact bytes.
    pub bytes: Vec<u8>,
    /// Indicates whether the artifact is required.
    pub required: bool,
}

/// Artifact reference returned by sinks.
///
/// # Invariants
/// - `uri` is opaque and may be runpack-relative or external.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtifactRef {
    /// Runpack-relative path or external URI.
    pub uri: String,
}

/// Artifact sink errors.
///
/// # Invariants
/// - Variants are stable for programmatic handling.
#[derive(Debug, Error)]
pub enum ArtifactError {
    /// Artifact sink reported an error.
    #[error("artifact error: {0}")]
    Sink(String),
    /// Artifact exceeds size limit.
    #[error("artifact too large: {path} ({actual_bytes} > {max_bytes})")]
    TooLarge {
        /// Artifact path.
        path: String,
        /// Maximum allowed bytes.
        max_bytes: usize,
        /// Actual artifact size in bytes.
        actual_bytes: usize,
    },
}

/// Artifact sink for runpack generation.
pub trait ArtifactSink {
    /// Writes an artifact into the runpack.
    ///
    /// # Errors
    ///
    /// Returns [`ArtifactError`] when writing fails.
    fn write(&mut self, artifact: &Artifact) -> Result<ArtifactRef, ArtifactError>;

    /// Finalizes the runpack manifest.
    ///
    /// # Errors
    ///
    /// Returns [`ArtifactError`] when writing the manifest fails.
    fn finalize(&mut self, manifest: &RunpackManifest) -> Result<ArtifactRef, ArtifactError>;
}

/// Artifact reader for runpack verification.
pub trait ArtifactReader {
    /// Reads artifact bytes from a runpack.
    ///
    /// # Errors
    ///
    /// Returns [`ArtifactError`] when reading fails.
    fn read(&self, path: &str) -> Result<Vec<u8>, ArtifactError> {
        self.read_with_limit(path, usize::MAX)
    }

    /// Reads artifact bytes from a runpack with a size limit.
    ///
    /// # Errors
    ///
    /// Returns [`ArtifactError`] when reading fails or the artifact exceeds `max_bytes`.
    fn read_with_limit(&self, path: &str, max_bytes: usize) -> Result<Vec<u8>, ArtifactError>;
}

// ============================================================================
// SECTION: Run State Store
// ============================================================================

/// Run state store errors.
///
/// # Invariants
/// - Variants are stable for programmatic handling.
#[derive(Debug, Error)]
pub enum StoreError {
    /// Store I/O error.
    #[error("run state store io error: {0}")]
    Io(String),
    /// Store data is corrupted or fails integrity checks.
    #[error("run state store corruption: {0}")]
    Corrupt(String),
    /// Store data version is incompatible.
    #[error("run state store version mismatch: {0}")]
    VersionMismatch(String),
    /// Store data is invalid.
    #[error("run state store invalid data: {0}")]
    Invalid(String),
    /// Store reported an error.
    #[error("run state store error: {0}")]
    Store(String),
}

/// Run state store for persistence.
pub trait RunStateStore {
    /// Loads run state by tenant, namespace, and run identifier.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when loading fails.
    fn load(
        &self,
        tenant_id: &TenantId,
        namespace_id: &NamespaceId,
        run_id: &RunId,
    ) -> Result<Option<RunState>, StoreError>;

    /// Saves run state.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when saving fails.
    fn save(&self, state: &RunState) -> Result<(), StoreError>;

    /// Reports store readiness for liveness/readiness probes.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the store is unavailable.
    fn readiness(&self) -> Result<(), StoreError> {
        Ok(())
    }
}

// ============================================================================
// SECTION: Data Shape Registry
// ============================================================================

/// Registry errors for data shape operations.
///
/// # Invariants
/// - Variants are stable for programmatic handling.
#[derive(Debug, Error)]
pub enum DataShapeRegistryError {
    /// Registry I/O error.
    #[error("data shape registry io error: {0}")]
    Io(String),
    /// Registry invalid data error.
    #[error("data shape registry invalid data: {0}")]
    Invalid(String),
    /// Registry conflict (duplicate schema).
    #[error("data shape registry conflict: {0}")]
    Conflict(String),
    /// Registry access error.
    #[error("data shape registry access error: {0}")]
    Access(String),
}

/// Registry interface for data shapes.
pub trait DataShapeRegistry {
    /// Registers a new data shape record.
    ///
    /// # Errors
    ///
    /// Returns [`DataShapeRegistryError`] when registration fails.
    fn register(&self, record: DataShapeRecord) -> Result<(), DataShapeRegistryError>;

    /// Loads a data shape by identifier and version.
    ///
    /// # Errors
    ///
    /// Returns [`DataShapeRegistryError`] when lookup fails.
    fn get(
        &self,
        tenant_id: &TenantId,
        namespace_id: &NamespaceId,
        schema_id: &DataShapeId,
        version: &DataShapeVersion,
    ) -> Result<Option<DataShapeRecord>, DataShapeRegistryError>;

    /// Lists data shapes with pagination.
    ///
    /// # Errors
    ///
    /// Returns [`DataShapeRegistryError`] when listing fails.
    fn list(
        &self,
        tenant_id: &TenantId,
        namespace_id: &NamespaceId,
        cursor: Option<String>,
        limit: usize,
    ) -> Result<DataShapePage, DataShapeRegistryError>;

    /// Reports registry readiness for liveness/readiness probes.
    ///
    /// # Errors
    ///
    /// Returns [`DataShapeRegistryError`] when the registry is unavailable.
    fn readiness(&self) -> Result<(), DataShapeRegistryError> {
        Ok(())
    }
}

// ============================================================================
// SECTION: Policy Decider
// ============================================================================

/// Dispatch policy decision.
///
/// # Invariants
/// - Variants are stable and exhaustive for authorization outcomes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PolicyDecision {
    /// Permit the dispatch.
    Permit,
    /// Deny the dispatch.
    Deny,
}

/// Policy decision errors.
///
/// # Invariants
/// - Variants are stable for programmatic handling.
#[derive(Debug, Error)]
pub enum PolicyError {
    /// Policy engine reported an error.
    #[error("policy decision error: {0}")]
    DecisionFailed(String),
}

/// Policy decider for disclosure authorization.
pub trait PolicyDecider {
    /// Evaluates whether a packet may be dispatched to the target.
    ///
    /// # Errors
    ///
    /// Returns [`PolicyError`] when policy evaluation fails.
    fn authorize(
        &self,
        target: &DispatchTarget,
        envelope: &PacketEnvelope,
        payload: &PacketPayload,
    ) -> Result<PolicyDecision, PolicyError>;
}

// ============================================================================
// SECTION: Trigger Sources
// ============================================================================

/// Trigger source for push-mode ingestion.
pub trait TriggerSource {
    /// Returns the next available trigger event, if any.
    fn next_trigger(&mut self) -> Option<TriggerEvent>;
}
