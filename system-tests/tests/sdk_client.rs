// system-tests/tests/sdk_client.rs
// ============================================================================
// Module: SDK Client Suite
// Description: Aggregates Decision Gate SDK system tests.
// Purpose: Reduce binaries while keeping SDK coverage centralized.
// Dependencies: suites/*
// ============================================================================

//! SDK client suite entry point for system-tests.

mod helpers;

#[path = "suites/sdk_client.rs"]
mod sdk_client;
