// crates/decision-gate-providers/src/lib.rs
// ============================================================================
// Module: Decision Gate Providers
// Description: Built-in evidence providers and registry utilities.
// Purpose: Provide zero-config evidence sources aligned with Decision Gate core.
// Dependencies: decision-gate-core, serde, reqwest, time
// ============================================================================

//! ## Overview
//! This crate ships built-in evidence providers (time, env, json, http) and a
//! registry implementation that routes evidence queries by provider identifier.
//! Providers are deterministic with respect to the supplied trigger context and
//! enforce strict validation and size limits for untrusted inputs.
//! Invariants:
//! - Evidence queries are routed via [`ProviderRegistry`] by provider identifier.
//! - Built-in providers enforce size limits and fail closed on invalid inputs.
//!
//! Security posture: evidence inputs are untrusted; see `Docs/security/threat_model.md`.

// ============================================================================
// SECTION: Modules
// ============================================================================

pub mod env;
pub mod http;
pub mod json;
pub mod registry;
pub mod time;

// ============================================================================
// SECTION: Re-Exports
// ============================================================================

pub use env::EnvProvider;
pub use env::EnvProviderConfig;
pub use http::HttpProvider;
pub use http::HttpProviderConfig;
pub use json::JsonProvider;
pub use json::JsonProviderConfig;
pub use registry::BuiltinProviderConfigs;
pub use registry::ProviderAccessPolicy;
pub use registry::ProviderRegistry;
pub use time::TimeProvider;
pub use time::TimeProviderConfig;

#[cfg(test)]
mod tests;
