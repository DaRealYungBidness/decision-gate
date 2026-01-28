// enterprise/enterprise-system-tests/tests/storage_postgres.rs
// ============================================================================
// Module: Storage Postgres Suite
// Description: Aggregates enterprise Postgres storage system tests.
// Purpose: Reduce binaries while keeping Postgres storage coverage centralized.
// Dependencies: suites/*, helpers
// ============================================================================

//! Postgres storage suite entry point for enterprise system-tests.

mod helpers;

#[path = "suites/postgres_store.rs"]
mod postgres_store;
