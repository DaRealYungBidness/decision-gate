// system-tests/tests/providers.rs
// ============================================================================
// Module: Providers Suite
// Description: Aggregates provider and AssetCore integration system tests.
// Purpose: Reduce binaries while keeping provider coverage centralized.
// Dependencies: suites/*, helpers
// ============================================================================

//! Providers suite entry point for system-tests.

mod helpers;

#[path = "suites/assetcore_integration.rs"]
mod assetcore_integration;
#[path = "suites/providers.rs"]
mod providers;
