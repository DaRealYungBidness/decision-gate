// crates/decision-gate-cli/src/tests/i18n.rs
// ============================================================================
// Module: CLI i18n Tests
// Description: Unit tests for catalog parity and locale parsing.
// Purpose: Ensure CLI localization remains consistent across supported locales.
// Dependencies: decision-gate-cli i18n module
// ============================================================================

//! ## Overview
//! Verifies the CLI message catalogs stay in sync, locale parsing is tolerant,
//! and locale templates preserve placeholder parity with English.

use std::collections::BTreeSet;

use crate::i18n::Locale;
use crate::i18n::MessageArg;
use crate::i18n::SUPPORTED_LOCALES;
use crate::i18n::catalog_entries_for;
use crate::i18n::catalog_for;
use crate::i18n::translate;

fn parse_placeholder_names(template: &str) -> Result<BTreeSet<String>, String> {
    let mut placeholders = BTreeSet::new();
    let bytes = template.as_bytes();
    let mut index = 0usize;
    while index < bytes.len() {
        match bytes[index] {
            b'{' => {
                let mut end = index + 1;
                while end < bytes.len() && bytes[end] != b'}' {
                    if bytes[end] == b'{' {
                        return Err(format!("nested '{{' at byte {end}"));
                    }
                    end += 1;
                }
                if end >= bytes.len() {
                    return Err(format!("unclosed '{{' at byte {index}"));
                }
                let name = &template[index + 1 .. end];
                if name.is_empty() {
                    return Err(format!("empty placeholder at byte {index}"));
                }
                let mut chars = name.chars();
                let Some(first) = chars.next() else {
                    return Err(format!("empty placeholder at byte {index}"));
                };
                if !first.is_ascii_lowercase() {
                    return Err(format!(
                        "placeholder '{name}' at byte {index} must start with lowercase ASCII \
                         letter"
                    ));
                }
                if chars.any(|ch| !(ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '_')) {
                    return Err(format!("placeholder '{name}' at byte {index} must use [a-z0-9_]"));
                }
                placeholders.insert(name.to_string());
                index = end + 1;
            }
            b'}' => {
                return Err(format!("unmatched '}}' at byte {index}"));
            }
            _ => {
                index += 1;
            }
        }
    }
    Ok(placeholders)
}

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
fn catalogs_have_unique_keys_per_locale() {
    for locale in SUPPORTED_LOCALES {
        let mut seen = BTreeSet::new();
        let mut duplicates = BTreeSet::new();
        let entries = catalog_entries_for(*locale);
        for (key, _) in entries {
            if !seen.insert(*key) {
                duplicates.insert(*key);
            }
        }
        assert!(
            duplicates.is_empty(),
            "locale catalogs must not contain duplicate keys ({locale:?}): {duplicates:?}"
        );
        assert_eq!(
            seen.len(),
            entries.len(),
            "locale catalogs must preserve one entry per key ({locale:?})"
        );
    }
}

#[test]
fn catalog_templates_have_valid_placeholder_syntax() {
    for locale in SUPPORTED_LOCALES {
        for (key, template) in catalog_entries_for(*locale) {
            parse_placeholder_names(template).unwrap_or_else(|error| {
                panic!("invalid placeholder syntax for key '{key}' in locale {locale:?}: {error}")
            });
        }
    }
}

#[test]
fn catalogs_have_placeholder_shape_parity_with_english() {
    let en_catalog = catalog_for(Locale::En);
    let en_keys: BTreeSet<&'static str> = en_catalog.keys().copied().collect();
    for key in en_keys {
        let en_template = en_catalog.get(key).copied().expect("en key exists");
        let en_placeholders = parse_placeholder_names(en_template).unwrap_or_else(|error| {
            panic!("invalid placeholder syntax for key '{key}' in locale En: {error}")
        });
        for locale in SUPPORTED_LOCALES {
            if *locale == Locale::En {
                continue;
            }
            let locale_template = catalog_for(*locale)
                .get(key)
                .copied()
                .unwrap_or_else(|| panic!("missing key '{key}' in locale {locale:?}"));
            let locale_placeholders =
                parse_placeholder_names(locale_template).unwrap_or_else(|error| {
                    panic!(
                        "invalid placeholder syntax for key '{key}' in locale {locale:?}: {error}"
                    )
                });
            assert_eq!(
                en_placeholders, locale_placeholders,
                "placeholder set mismatch for key '{key}' in locale {locale:?}"
            );
        }
    }
}

#[test]
fn non_english_locales_differ_for_curated_keys() {
    const CURATED_KEYS: &[&str] =
        &["config.validate.ok", "authoring.validate.ok", "i18n.disclaimer.machine_translated"];
    for locale in SUPPORTED_LOCALES {
        if *locale == Locale::En {
            continue;
        }
        for key in CURATED_KEYS {
            let en = catalog_for(Locale::En).get(key).copied().expect("en key exists");
            let localized = catalog_for(*locale)
                .get(key)
                .copied()
                .unwrap_or_else(|| panic!("missing key '{key}' in locale {locale:?}"));
            assert_ne!(
                en, localized,
                "non-English locale must differ from English for curated key '{key}' ({locale:?})"
            );
        }
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
    assert_eq!(Locale::parse("en_US"), Some(Locale::En));
    assert_eq!(Locale::parse("ca_ES"), Some(Locale::Ca));
}

#[test]
fn translate_missing_placeholder_handled() {
    let output =
        translate("serve.bind.non_loopback_opt_in", vec![MessageArg::new("bind", "0.0.0.0:8080")]);
    assert!(!output.is_empty());
}

#[test]
fn translate_extra_placeholder_ignored() {
    let output = translate("config.validate.ok", vec![MessageArg::new("extra", "value")]);
    assert!(!output.is_empty());
}

#[test]
fn fallback_chain_en_to_key() {
    let nonexistent_key = "nonexistent.key.does.not.exist";
    let output = translate(nonexistent_key, vec![]);
    assert_eq!(output, nonexistent_key);
}
