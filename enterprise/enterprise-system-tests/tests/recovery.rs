// enterprise/enterprise-system-tests/tests/recovery.rs
// ============================================================================
// Module: Recovery Suite
// Description: Aggregates enterprise backup/restore system tests.
// Purpose: Reduce binaries while keeping recovery coverage centralized.
// Dependencies: suites/*, helpers
// ============================================================================

//! Recovery suite entry point for enterprise system-tests.

mod helpers;

#[path = "suites/backup_restore.rs"]
mod backup_restore;
