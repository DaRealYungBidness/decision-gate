// decision-gate-core/src/runtime/mod.rs
// ============================================================================
// Module: Decision Gate Runtime
// Description: Deterministic evaluation engine, runpack builder, and helpers.
// Purpose: Execute Decision Gate scenarios against evidence providers and dispatchers.
// Dependencies: crate::{core, interfaces}, ret-logic
// ============================================================================

//! ## Overview
//! Runtime modules implement Decision Gate evaluation, tool-call APIs, and runpack
//! generation/verifier helpers. All external interfaces must call into the
//! same engine logic to preserve invariance.

// ============================================================================
// SECTION: Submodules
// ============================================================================

pub mod comparator;
pub mod engine;
pub mod gate;
pub mod runpack;
pub mod store;

// ============================================================================
// SECTION: Re-Exports
// ============================================================================

pub use engine::ControlPlane;
pub use engine::ControlPlaneConfig;
pub use engine::ControlPlaneError;
pub use engine::EvaluationResult;
pub use engine::NextRequest;
pub use engine::NextResult;
pub use engine::ScenarioStatus;
pub use engine::StatusRequest;
pub use engine::SubmitRequest;
pub use engine::SubmitResult;
pub use engine::TriggerResult;
pub use gate::GateEvaluator;
pub use runpack::RunpackBuilder;
pub use runpack::RunpackError;
pub use runpack::RunpackVerifier;
pub use runpack::VerificationReport;
pub use runpack::VerificationStatus;
pub use store::InMemoryRunStateStore;
