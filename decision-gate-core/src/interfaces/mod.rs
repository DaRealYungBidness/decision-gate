// decision-gate-core/src/interfaces/mod.rs
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

// ============================================================================
// SECTION: Imports
// ============================================================================

use serde::Deserialize;
use serde::Serialize;
use thiserror::Error;

use crate::core::ArtifactKind;
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceContext {
    /// Tenant identifier.
    pub tenant_id: TenantId,
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
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtifactRef {
    /// Runpack-relative path or external URI.
    pub uri: String,
}

/// Artifact sink errors.
#[derive(Debug, Error)]
pub enum ArtifactError {
    /// Artifact sink reported an error.
    #[error("artifact error: {0}")]
    Sink(String),
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
    fn read(&self, path: &str) -> Result<Vec<u8>, ArtifactError>;
}

// ============================================================================
// SECTION: Run State Store
// ============================================================================

/// Run state store errors.
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
    /// Loads run state by run identifier.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when loading fails.
    fn load(&self, run_id: &RunId) -> Result<Option<RunState>, StoreError>;

    /// Saves run state.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when saving fails.
    fn save(&self, state: &RunState) -> Result<(), StoreError>;
}

// ============================================================================
// SECTION: Policy Decider
// ============================================================================

/// Dispatch policy decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PolicyDecision {
    /// Permit the dispatch.
    Permit,
    /// Deny the dispatch.
    Deny,
}

/// Policy decision errors.
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
