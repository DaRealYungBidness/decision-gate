// decision-gate-enterprise/src/lib.rs
// ============================================================================
// Private enterprise control-plane extensions for Decision Gate.
// ============================================================================

//! Enterprise control-plane scaffolding.
//!
//! This crate will host tenant authz enforcement, usage metering, quota
//! enforcement, and admin lifecycle APIs. It must not alter Decision Gate
//! core semantics or weaken security defaults.

/// Minimal admin UI scaffolding.
pub mod admin_ui;
/// Hash-chained audit sink implementation.
pub mod audit_chain;
/// Enterprise config loader and wiring helpers.
pub mod config;
/// Runpack storage adapters.
pub mod runpack_storage;
/// Enterprise server builder with overrides.
pub mod server;
/// Tenant lifecycle administration primitives.
pub mod tenant_admin;
/// Tenant authorization policy implementation.
pub mod tenant_authz;
/// Usage metering and quota enforcement utilities.
pub mod usage;
/// SQLite-backed usage ledger implementation.
pub mod usage_sqlite;
