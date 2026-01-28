// decision-gate-store-enterprise/src/lib.rs
// ============================================================================
// Private multi-tenant storage backends for Decision Gate enterprise.
// ============================================================================

//! Enterprise storage scaffolding.
//!
//! This crate will host production-grade storage backends (Postgres, object
//! storage) that implement Decision Gate's store and registry interfaces.

/// Postgres-backed run state and schema registry store.
pub mod postgres_store;
/// Runpack storage abstractions and implementations.
pub mod runpack_store;
/// Enterprise `SQLite` store wrapper.
pub mod sqlite_store;

#[cfg(feature = "s3")]
/// S3-backed runpack store implementation.
pub mod s3_runpack_store;
