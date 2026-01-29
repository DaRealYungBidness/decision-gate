// decision-gate-store-sqlite/src/lib.rs
// ============================================================================
// Module: SQLite Run State Store
// Description: Durable RunStateStore backend using SQLite WAL.
// Purpose: Provide production-grade persistence for Decision Gate run state.
// Dependencies: decision-gate-core, rusqlite
// ============================================================================

//! ## Overview
//! This crate provides a SQLite-backed [`RunStateStore`] implementation that
//! persists canonical run state snapshots and a versioned history table. It
//! is designed for deterministic serialization, crash recovery, and audit
//! readiness. Security posture: storage inputs are untrusted; see
//! `Docs/security/threat_model.md`.

// ============================================================================
// SECTION: Modules
// ============================================================================

pub mod store;

// ============================================================================
// SECTION: Re-Exports
// ============================================================================

pub use store::MAX_STATE_BYTES;
pub use store::SqliteRunStateStore;
pub use store::SqliteStoreConfig;
pub use store::SqliteStoreError;
pub use store::SqliteStoreMode;
pub use store::SqliteSyncMode;
