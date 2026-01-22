// decision-gate-broker/tests/sources/inline_tests.rs
// ============================================================================
// Module: InlineSource Unit Tests
// Description: Comprehensive tests for the inline/embedded payload source.
// ============================================================================

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

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use decision_gate_broker::InlineSource;
use decision_gate_broker::Source;
use decision_gate_broker::SourceError;
use decision_gate_core::ContentRef;
use decision_gate_core::hashing::DEFAULT_HASH_ALGORITHM;
use decision_gate_core::hashing::hash_bytes;

// ============================================================================
// SECTION: Constructor Tests
// ============================================================================

/// Tests inline source new creates source.
#[test]
fn inline_source_new_creates_source() {
    let _source = InlineSource::new();
    // Source created successfully
}

/// Tests inline source default creates source.
#[test]
fn inline_source_default_creates_source() {
    let source = InlineSource::new();
    assert!(matches!(source, InlineSource));
}

// ============================================================================
// SECTION: Success Path Tests - inline: scheme
// ============================================================================

/// Tests inline source decodes plain inline scheme.
#[test]
fn inline_source_decodes_plain_inline_scheme() {
    let data = b"plain inline data";
    let encoded = STANDARD.encode(data);
    let content_hash = hash_bytes(DEFAULT_HASH_ALGORITHM, data);

    let content_ref = ContentRef {
        uri: format!("inline:{encoded}"),
        content_hash,
        encryption: None,
    };

    let source = InlineSource::new();
    let payload = source.fetch(&content_ref).expect("inline fetch");

    assert_eq!(payload.bytes, data);
    assert!(payload.content_type.is_none());
}

/// Tests inline source decodes empty payload.
#[test]
fn inline_source_decodes_empty_payload() {
    let data = b"";
    let encoded = STANDARD.encode(data);
    let content_hash = hash_bytes(DEFAULT_HASH_ALGORITHM, data);

    let content_ref = ContentRef {
        uri: format!("inline:{encoded}"),
        content_hash,
        encryption: None,
    };

    let source = InlineSource::new();
    let payload = source.fetch(&content_ref).expect("inline fetch");

    assert!(payload.bytes.is_empty());
    assert!(payload.content_type.is_none());
}

// ============================================================================
// SECTION: Success Path Tests - inline+json: scheme
// ============================================================================

/// Tests inline source decodes json scheme.
#[test]
fn inline_source_decodes_json_scheme() {
    let data = br#"{"key": "value"}"#;
    let encoded = STANDARD.encode(data);
    let content_hash = hash_bytes(DEFAULT_HASH_ALGORITHM, data);

    let content_ref = ContentRef {
        uri: format!("inline+json:{encoded}"),
        content_hash,
        encryption: None,
    };

    let source = InlineSource::new();
    let payload = source.fetch(&content_ref).expect("inline fetch");

    assert_eq!(payload.bytes, data);
    assert_eq!(payload.content_type.as_deref(), Some("application/json"));
}

/// Tests inline source json scheme with array.
#[test]
fn inline_source_json_scheme_with_array() {
    let data = br"[1, 2, 3]";
    let encoded = STANDARD.encode(data);
    let content_hash = hash_bytes(DEFAULT_HASH_ALGORITHM, data);

    let content_ref = ContentRef {
        uri: format!("inline+json:{encoded}"),
        content_hash,
        encryption: None,
    };

    let source = InlineSource::new();
    let payload = source.fetch(&content_ref).expect("inline fetch");

    assert_eq!(payload.bytes, data);
    assert_eq!(payload.content_type.as_deref(), Some("application/json"));
}

// ============================================================================
// SECTION: Success Path Tests - inline+bytes: scheme
// ============================================================================

/// Tests inline source decodes bytes scheme.
#[test]
fn inline_source_decodes_bytes_scheme() {
    let data = b"\x00\x01\x02\x03\xff\xfe";
    let encoded = STANDARD.encode(data);
    let content_hash = hash_bytes(DEFAULT_HASH_ALGORITHM, data);

    let content_ref = ContentRef {
        uri: format!("inline+bytes:{encoded}"),
        content_hash,
        encryption: None,
    };

    let source = InlineSource::new();
    let payload = source.fetch(&content_ref).expect("inline fetch");

    assert_eq!(payload.bytes, data);
    assert_eq!(payload.content_type.as_deref(), Some("application/octet-stream"));
}

/// Tests inline source bytes scheme with binary data.
#[test]
fn inline_source_bytes_scheme_with_binary_data() {
    // Test with various binary patterns
    let data: Vec<u8> = (0 ..= 255).collect();
    let encoded = STANDARD.encode(&data);
    let content_hash = hash_bytes(DEFAULT_HASH_ALGORITHM, &data);

    let content_ref = ContentRef {
        uri: format!("inline+bytes:{encoded}"),
        content_hash,
        encryption: None,
    };

    let source = InlineSource::new();
    let payload = source.fetch(&content_ref).expect("inline fetch");

    assert_eq!(payload.bytes, data);
}

// ============================================================================
// SECTION: Error Path Tests
// ============================================================================

/// Tests inline source rejects invalid base64.
#[test]
fn inline_source_rejects_invalid_base64() {
    let content_hash = hash_bytes(DEFAULT_HASH_ALGORITHM, b"phantom");

    let content_ref = ContentRef {
        uri: "inline:not-valid-base64!!!".to_string(),
        content_hash,
        encryption: None,
    };

    let source = InlineSource::new();
    let err = source.fetch(&content_ref).unwrap_err();

    assert!(matches!(err, SourceError::Decode(_)));
}

/// Tests inline source rejects invalid base64 in json scheme.
#[test]
fn inline_source_rejects_invalid_base64_in_json_scheme() {
    let content_hash = hash_bytes(DEFAULT_HASH_ALGORITHM, b"phantom");

    let content_ref = ContentRef {
        uri: "inline+json:!!!invalid!!!".to_string(),
        content_hash,
        encryption: None,
    };

    let source = InlineSource::new();
    let err = source.fetch(&content_ref).unwrap_err();

    assert!(matches!(err, SourceError::Decode(_)));
}

/// Tests inline source rejects invalid base64 in bytes scheme.
#[test]
fn inline_source_rejects_invalid_base64_in_bytes_scheme() {
    let content_hash = hash_bytes(DEFAULT_HASH_ALGORITHM, b"phantom");

    let content_ref = ContentRef {
        uri: "inline+bytes:@@@bad@@@".to_string(),
        content_hash,
        encryption: None,
    };

    let source = InlineSource::new();
    let err = source.fetch(&content_ref).unwrap_err();

    assert!(matches!(err, SourceError::Decode(_)));
}

/// Tests inline source rejects unsupported scheme.
#[test]
fn inline_source_rejects_unsupported_scheme() {
    let content_hash = hash_bytes(DEFAULT_HASH_ALGORITHM, b"phantom");

    let content_ref = ContentRef {
        uri: "http://example.com".to_string(),
        content_hash,
        encryption: None,
    };

    let source = InlineSource::new();
    let err = source.fetch(&content_ref).unwrap_err();

    assert!(matches!(err, SourceError::UnsupportedScheme(_)));
}

/// Tests inline source rejects unknown inline subscheme.
#[test]
fn inline_source_rejects_unknown_inline_subscheme() {
    let encoded = STANDARD.encode(b"data");
    let content_hash = hash_bytes(DEFAULT_HASH_ALGORITHM, b"phantom");

    let content_ref = ContentRef {
        uri: format!("inline+unknown:{encoded}"),
        content_hash,
        encryption: None,
    };

    let source = InlineSource::new();
    let err = source.fetch(&content_ref).unwrap_err();

    // Should fail because inline+unknown: doesn't match any known pattern
    assert!(matches!(err, SourceError::UnsupportedScheme(_)));
}

/// Tests inline source rejects bare inline without colon.
#[test]
fn inline_source_rejects_bare_inline_without_colon() {
    let content_hash = hash_bytes(DEFAULT_HASH_ALGORITHM, b"phantom");

    let content_ref = ContentRef {
        uri: "inline".to_string(),
        content_hash,
        encryption: None,
    };

    let source = InlineSource::new();
    let err = source.fetch(&content_ref).unwrap_err();

    assert!(matches!(err, SourceError::UnsupportedScheme(_)));
}

// ============================================================================
// SECTION: Edge Case Tests
// ============================================================================

/// Tests inline source handles large payload.
#[test]
fn inline_source_handles_large_payload() {
    let data: Vec<u8> =
        (0 .. 10000).map(|i| u8::try_from(i % 256).expect("u8 conversion")).collect();
    let encoded = STANDARD.encode(&data);
    let content_hash = hash_bytes(DEFAULT_HASH_ALGORITHM, &data);

    let content_ref = ContentRef {
        uri: format!("inline+bytes:{encoded}"),
        content_hash,
        encryption: None,
    };

    let source = InlineSource::new();
    let payload = source.fetch(&content_ref).expect("inline fetch");

    assert_eq!(payload.bytes, data);
}

/// Tests inline source handles unicode in json.
#[test]
fn inline_source_handles_unicode_in_json() {
    let data = r#"{"emoji": "🎉", "chinese": "中文"}"#.as_bytes();
    let encoded = STANDARD.encode(data);
    let content_hash = hash_bytes(DEFAULT_HASH_ALGORITHM, data);

    let content_ref = ContentRef {
        uri: format!("inline+json:{encoded}"),
        content_hash,
        encryption: None,
    };

    let source = InlineSource::new();
    let payload = source.fetch(&content_ref).expect("inline fetch");

    assert_eq!(payload.bytes, data);
}

/// Tests inline source handles base64 padding variations.
#[test]
fn inline_source_handles_base64_padding_variations() {
    // Test different padding scenarios
    let test_cases = [
        b"a".as_slice(),    // 1 byte - needs 2 padding chars
        b"ab".as_slice(),   // 2 bytes - needs 1 padding char
        b"abc".as_slice(),  // 3 bytes - no padding needed
        b"abcd".as_slice(), // 4 bytes - needs 2 padding chars
    ];

    for data in test_cases {
        let encoded = STANDARD.encode(data);
        let content_hash = hash_bytes(DEFAULT_HASH_ALGORITHM, data);

        let content_ref = ContentRef {
            uri: format!("inline:{encoded}"),
            content_hash,
            encryption: None,
        };

        let source = InlineSource::new();
        let payload = source.fetch(&content_ref).expect("inline fetch");

        assert_eq!(payload.bytes, data);
    }
}
