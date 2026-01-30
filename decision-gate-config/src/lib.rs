// decision-gate-config/src/lib.rs
// ============================================================================
// Module: Decision Gate Config Library
// Description: Canonical config model, validation, and artifact generation.
// Purpose: Single source of truth for decision-gate.toml semantics.
// Dependencies: decision-gate-core, serde, toml
// ============================================================================

//! ## Overview
//! `decision-gate-config` defines the canonical configuration model for
//! Decision Gate. It provides strict, fail-closed validation and deterministic
//! generators for config schema, examples, and docs.
//!
//! Security posture: config inputs are untrusted; see
//! `Docs/security/threat_model.md`.

// ============================================================================
// SECTION: Modules
// ============================================================================

pub mod config;
pub mod docs;
pub mod examples;
pub mod policy;
pub mod schema;

// ============================================================================
// SECTION: Re-Exports
// ============================================================================

pub use config::*;
pub use docs::config_docs_markdown;
pub use docs::verify_config_docs;
pub use docs::write_config_docs;
pub use examples::config_toml_example;
pub use policy::*;
pub use schema::config_schema;
