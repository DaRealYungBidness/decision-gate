// system-tests/tests/reliability.rs
// ============================================================================
// Module: Reliability Suite
// Description: Aggregates determinism, persistence, and stress system tests.
// Purpose: Reduce binaries while keeping reliability coverage centralized.
// Dependencies: suites/*, helpers
// ============================================================================

//! Reliability suite entry point for system-tests.

mod helpers;

#[path = "suites/determinism.rs"]
mod determinism;
#[path = "suites/metamorphic.rs"]
mod metamorphic;
#[path = "suites/reliability.rs"]
mod reliability;
#[path = "suites/sqlite_registry_runpack.rs"]
mod sqlite_registry_runpack;
#[path = "suites/store_persistence.rs"]
mod store_persistence;
#[path = "suites/stress.rs"]
mod stress;
