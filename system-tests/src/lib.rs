// system-tests/src/lib.rs
// ============================================================================
// Module: Decision Gate System Tests Library
// Description: Shared configuration and helpers for system test scenarios.
// Purpose: Provide common utilities for Decision Gate system-test binaries.
// Dependencies: std
// ============================================================================

//! ## Overview
//! This crate hosts shared configuration and helper utilities used by the
//! Decision Gate system-tests binaries in `system-tests/tests`.
//! Security posture: system-test inputs are untrusted; see `Docs/security/threat_model.md`.

// ============================================================================
// SECTION: Modules
// ============================================================================

pub mod config;
