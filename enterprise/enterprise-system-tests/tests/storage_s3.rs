// enterprise/enterprise-system-tests/tests/storage_s3.rs
// ============================================================================
// Module: Storage S3 Suite
// Description: Aggregates enterprise S3 runpack storage system tests.
// Purpose: Reduce binaries while keeping S3 storage coverage centralized.
// Dependencies: suites/*, helpers
// ============================================================================

//! S3 storage suite entry point for enterprise system-tests.

mod helpers;

#[path = "suites/s3_runpack_store.rs"]
mod s3_runpack_store;
