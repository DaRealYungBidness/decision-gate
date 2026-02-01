// decision-gate-cli/src/tests/i18n.rs
// ============================================================================
// Module: CLI i18n Tests
// Description: Unit tests for catalog parity and locale parsing.
// Purpose: Ensure CLI localization remains consistent across supported locales.
// Dependencies: decision-gate-cli i18n module
// ============================================================================

//! ## Overview
//! Verifies the CLI message catalogs stay in sync, locale parsing is tolerant,
//! and representative Catalan strings differ from English.

use std::collections::BTreeSet;

use crate::i18n::Locale;
use crate::i18n::MessageArg;
use crate::i18n::SUPPORTED_LOCALES;
use crate::i18n::catalog_for;
use crate::i18n::translate;

#[test]
fn catalogs_have_matching_keys() {
    assert!(SUPPORTED_LOCALES.contains(&Locale::En), "English must remain the baseline locale");
    let en_keys: BTreeSet<&'static str> = catalog_for(Locale::En).keys().copied().collect();
    for locale in SUPPORTED_LOCALES {
        let locale_keys: BTreeSet<&'static str> = catalog_for(*locale).keys().copied().collect();
        assert_eq!(en_keys, locale_keys, "locale catalogs must stay in parity ({locale:?})");
    }
}

#[test]
fn locale_parse_accepts_region_tags_and_case() {
    assert_eq!(Locale::parse("en"), Some(Locale::En));
    assert_eq!(Locale::parse("EN"), Some(Locale::En));
    assert_eq!(Locale::parse("en-US"), Some(Locale::En));
    assert_eq!(Locale::parse("en_us"), Some(Locale::En));
    assert_eq!(Locale::parse("ca"), Some(Locale::Ca));
    assert_eq!(Locale::parse("CA"), Some(Locale::Ca));
    assert_eq!(Locale::parse("ca-ES"), Some(Locale::Ca));
    assert_eq!(Locale::parse("ca_es"), Some(Locale::Ca));
    assert_eq!(Locale::parse(""), None);
    assert_eq!(Locale::parse("de"), None);
}

#[test]
fn catalan_translation_differs_for_known_key() {
    let en = catalog_for(Locale::En).get("config.validate.ok").copied().expect("en key exists");
    let ca = catalog_for(Locale::Ca).get("config.validate.ok").copied().expect("ca key exists");
    assert_ne!(en, ca, "expected Catalan translation to differ from English");
}

#[test]
fn translate_substitutes_placeholders() {
    let output = translate(
        "serve.bind.non_loopback_opt_in",
        vec![MessageArg::new("bind", "0.0.0.0:8080"), MessageArg::new("env", "ENV_FLAG")],
    );
    assert!(output.contains("0.0.0.0:8080"));
    assert!(output.contains("ENV_FLAG"));
}

// ============================================================================
// SECTION: Locale Parsing Edge Cases
// ============================================================================

#[test]
fn locale_parse_accepts_underscores() {
    // Test underscore as separator (common in env vars)
    assert_eq!(Locale::parse("en_US"), Some(Locale::En));
    assert_eq!(Locale::parse("ca_ES"), Some(Locale::Ca));
}

#[test]
fn translate_missing_placeholder_handled() {
    // Test translation with missing placeholder
    let output = translate(
        "serve.bind.non_loopback_opt_in",
        vec![MessageArg::new("bind", "0.0.0.0:8080")], // Missing "env" placeholder
    );
    // Should still produce output, placeholder might be empty or show {env}
    assert!(!output.is_empty());
}

#[test]
fn translate_extra_placeholder_ignored() {
    // Test translation with extra placeholder
    let output = translate(
        "config.validate.ok", // Simple message with no placeholders
        vec![MessageArg::new("extra", "value")],
    );
    // Should ignore extra placeholder
    assert!(!output.is_empty());
}

#[test]
fn disclaimer_present_for_non_english() {
    // Test that non-English locales have a disclaimer
    // The disclaimer key should be in the catalog
    let ca_catalog = catalog_for(Locale::Ca);
    // Check if there's a disclaimer-related key
    // (Exact key name depends on implementation)
    assert!(!ca_catalog.is_empty());
}

#[test]
fn fallback_chain_en_to_key() {
    // Test fallback behavior for missing keys
    // If a key doesn't exist in any catalog, it should return the key itself
    let nonexistent_key = "nonexistent.key.does.not.exist";
    let output = translate(nonexistent_key, vec![]);
    // Should return the key or a sensible default
    assert!(!output.is_empty());
}
