// crates/decision-gate-cli/tests/i18n.rs
// ============================================================================
// Module: CLI i18n Tests
// Description: Exercises the translation catalog and placeholder substitution.
// Purpose: Ensure CLI user-facing strings route through stable i18n helpers.
// Dependencies: decision-gate-cli i18n module and the `t!` macro.
// ============================================================================

//! ## Overview
//! Validates the Decision Gate CLI i18n catalog behavior:
//! - Message arguments capture key/value substitutions.
//! - Translation falls back to keys on misses.
//! - The [`t!`](decision_gate_cli::t) macro formats placeholders correctly.

#![allow(
    clippy::panic,
    clippy::print_stdout,
    clippy::print_stderr,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::use_debug,
    clippy::dbg_macro,
    clippy::panic_in_result_fn,
    clippy::unwrap_in_result,
    reason = "Test-only output and panic-based assertions are permitted."
)]

// ============================================================================
// SECTION: Imports
// ============================================================================

use decision_gate_cli::i18n::MessageArg;
use decision_gate_cli::i18n::translate;
use decision_gate_cli::t;

// ============================================================================
// SECTION: Tests
// ============================================================================

/// Confirms message arguments capture key/value pairs.
#[test]
fn message_arg_new_captures_key_and_value() {
    let arg = MessageArg::new("path", "/tmp/runpack.json");
    assert_eq!(arg.key, "path");
    assert_eq!(arg.value, "/tmp/runpack.json");
}

/// Confirms catalog entries resolve and replace placeholders.
#[test]
fn translate_substitutes_placeholders() {
    let args = vec![MessageArg::new("path", "/tmp/runpack.json")];
    let result = translate("runpack.export.ok", args);
    assert_eq!(result, "Runpack manifest written to /tmp/runpack.json");
}

/// Confirms missing keys fall back to the key string.
#[test]
fn translate_falls_back_to_key() {
    let result = translate("missing.key", Vec::new());
    assert_eq!(result, "missing.key");
}

/// Confirms the t! macro formats named arguments.
#[test]
fn t_macro_formats_message() {
    let rendered = t!("main.version", version = "0.1.0");
    assert!(rendered.contains("decision-gate"));
    assert!(rendered.contains("0.1.0"));
}
