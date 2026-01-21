// decision-gate-core/src/lib.rs
// ============================================================================
// Module: Decision Gate Core Library
// Description: Public API surface for the Decision Gate core.
// Purpose: Expose core types, interfaces, and runtime helpers.
// Dependencies: crate::{core, interfaces, runtime}
// ============================================================================

//! ## Overview
//! Decision Gate core provides deterministic gate evaluation, disclosure control, and
//! runpack generation for staged scenarios. It is backend-agnostic and
//! integrates through explicit interfaces rather than embedding into agent
//! frameworks.

// ============================================================================
// SECTION: Modules
// ============================================================================

pub mod core;
pub mod interfaces;
pub mod runtime;

// ============================================================================
// SECTION: Re-Exports
// ============================================================================

pub use core::*;

pub use interfaces::Artifact;
pub use interfaces::ArtifactError;
pub use interfaces::ArtifactReader;
pub use interfaces::ArtifactRef;
pub use interfaces::ArtifactSink;
pub use interfaces::DispatchError;
pub use interfaces::Dispatcher;
pub use interfaces::EvidenceContext;
pub use interfaces::EvidenceError;
pub use interfaces::EvidenceProvider;
pub use interfaces::PolicyDecider;
pub use interfaces::PolicyDecision;
pub use interfaces::PolicyError;
pub use interfaces::RunStateStore;
pub use interfaces::StoreError;
pub use interfaces::TriggerSource;
pub use runtime::ControlPlane;
pub use runtime::ControlPlaneConfig;
pub use runtime::ControlPlaneError;
pub use runtime::EvaluationResult;
pub use runtime::GateEvaluator;
pub use runtime::InMemoryRunStateStore;
pub use runtime::NextRequest;
pub use runtime::NextResult;
pub use runtime::RunpackBuilder;
pub use runtime::RunpackError;
pub use runtime::RunpackVerifier;
pub use runtime::ScenarioStatus;
pub use runtime::StatusRequest;
pub use runtime::SubmitRequest;
pub use runtime::SubmitResult;
pub use runtime::TriggerResult;
pub use runtime::VerificationReport;
pub use runtime::VerificationStatus;
