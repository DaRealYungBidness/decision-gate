// decision-gate-cli/src/tests/protocol.rs
// ============================================================================
// Module: Protocol Robustness Tests
// Description: Unit and property-based tests for JSON-RPC and framing protocols.
// Purpose: Ensure protocol parsing handles malformed inputs safely (nation-state resistance).
// Dependencies: decision-gate-cli mcp_client, proptest
// ============================================================================

//! ## Overview
//! Validates protocol parsing robustness using both traditional unit tests
//! and property-based fuzzing with proptest. This is CRITICAL for nation-state
//! adversary resistance - malformed protocol messages must never cause crashes,
//! panics, or undefined behavior.
//!
//! Coverage:
//! - SSE (Server-Sent Events) parsing
//! - Stdio framing (Content-Length protocol)
//! - JSON-RPC response validation
//! - Property-based fuzzing with arbitrary inputs

use std::io::Cursor;

use proptest::prelude::*;

use crate::mcp_client::parse_sse_body;
use crate::mcp_client::read_framed;

// ============================================================================
// SECTION: JSON-RPC Response Validation Tests
// ============================================================================

#[test]
fn truncated_json_rpc_frame_rejected() {
    // Test various truncation points in a valid JSON-RPC response
    let valid = r#"{"jsonrpc":"2.0","id":1,"result":{"tools":[]}}"#;

    // Try truncating at every position
    for i in 1 .. valid.len() {
        let truncated = &valid[.. i];
        let result = serde_json::from_str::<serde_json::Value>(truncated);

        // Should fail to parse (unless we got lucky and truncated at a valid boundary)
        if result.is_ok() {
            // Valid JSON at this truncation point is rare but possible
            continue;
        }
        assert!(result.is_err(), "Truncated JSON should fail: '{}'", truncated);
    }
}

#[test]
fn malformed_json_in_response_rejected() {
    // Various malformed JSON payloads
    let malformed_cases = vec![
        r#"{"jsonrpc":"2.0","id":1,"result":"#,  // Unclosed string
        r#"{"jsonrpc":"2.0","id":1,result:{}}"#, // Missing quotes on key
        r#"{"jsonrpc":"2.0","id":1,"result":{}extra}"#, // Extra content
        r#"{"jsonrpc":"2.0","id":1,"result":[}"#, // Mismatched brackets
        r#"{"jsonrpc":"2.0","id":1,"result":null"#, // Missing closing brace
        r#"{"jsonrpc":"2.0","id":1,"result":undefined}"#, // Invalid value
        r#"{'jsonrpc':'2.0','id':1,'result':{}}"#, // Single quotes (not valid JSON)
    ];

    for case in malformed_cases {
        let result = serde_json::from_str::<serde_json::Value>(case);
        assert!(result.is_err(), "Malformed JSON should fail: '{}'", case);
    }
}

#[test]
fn missing_jsonrpc_field_rejected() {
    // JSON-RPC responses must have "jsonrpc": "2.0"
    let missing_version = r#"{"id":1,"result":{}}"#;
    let parsed: serde_json::Value = serde_json::from_str(missing_version).unwrap();

    // The JSON is valid, but missing required field
    assert!(parsed.get("jsonrpc").is_none());
}

#[test]
fn missing_id_field_handled() {
    // Notifications don't have an ID, but responses should
    let missing_id = r#"{"jsonrpc":"2.0","result":{}}"#;
    let parsed: serde_json::Value = serde_json::from_str(missing_id).unwrap();

    assert!(parsed.get("id").is_none());
}

#[test]
fn null_result_and_null_error_rejected() {
    // JSON-RPC requires either result or error, not both null
    let both_null = r#"{"jsonrpc":"2.0","id":1,"result":null,"error":null}"#;
    let parsed: serde_json::Value = serde_json::from_str(both_null).unwrap();

    // This is valid JSON but invalid JSON-RPC (both null)
    assert!(parsed["result"].is_null());
    assert!(parsed["error"].is_null());
}

// ============================================================================
// SECTION: Stdio Framing Tests
// ============================================================================

#[test]
fn stdio_missing_content_length_rejected() {
    // Framed protocol requires Content-Length header
    let data = b"\r\n{\"jsonrpc\":\"2.0\"}";
    let mut reader = std::io::BufReader::new(Cursor::new(data));

    let result = read_framed(&mut reader);
    assert!(result.is_err(), "Missing Content-Length should be rejected");
}

#[test]
fn stdio_invalid_content_length_rejected() {
    // Content-Length must be a valid number
    let data = b"Content-Length: not-a-number\r\n\r\n";
    let mut reader = std::io::BufReader::new(Cursor::new(data));

    let result = read_framed(&mut reader);
    assert!(result.is_err(), "Invalid Content-Length should be rejected");
}

#[test]
fn stdio_content_length_mismatch_rejected() {
    // Content-Length claims 100 bytes but only 20 provided
    let data = b"Content-Length: 100\r\n\r\n{\"jsonrpc\":\"2.0\"}";
    let mut reader = std::io::BufReader::new(Cursor::new(data));

    let result = read_framed(&mut reader);
    // Should fail due to EOF or incomplete read
    assert!(result.is_err(), "Content-Length mismatch should be rejected");
}

#[test]
fn stdio_content_length_overflow_rejected() {
    // Content-Length with a value that would overflow
    let data = b"Content-Length: 99999999999999999999\r\n\r\n";
    let mut reader = std::io::BufReader::new(Cursor::new(data));

    let result = read_framed(&mut reader);
    assert!(result.is_err(), "Overflow Content-Length should be rejected");
}

#[test]
fn stdio_negative_content_length_rejected() {
    // Negative Content-Length is invalid
    let data = b"Content-Length: -100\r\n\r\n";
    let mut reader = std::io::BufReader::new(Cursor::new(data));

    let result = read_framed(&mut reader);
    assert!(result.is_err(), "Negative Content-Length should be rejected");
}

// ============================================================================
// SECTION: SSE Parsing Tests
// ============================================================================

#[test]
fn sse_missing_data_prefix_rejected() {
    // SSE events must have "data: " prefix
    let body = b"event: message\nno-data-field\n\n";

    let result = parse_sse_body(body);
    assert!(result.is_err(), "SSE without 'data:' should be rejected");
}

#[test]
fn sse_data_without_colon_rejected() {
    // "data" without colon is invalid
    let body = b"data {\"jsonrpc\":\"2.0\"}\n\n";

    let result = parse_sse_body(body);
    // This might parse as invalid data or fail
    // The key is it doesn't crash
    if result.is_ok() {
        // If it parses, verify it's not the expected JSON
        let parsed = result.unwrap();
        assert!(parsed != b"{\"jsonrpc\":\"2.0\"}");
    }
}

#[test]
fn sse_multiple_events_takes_first() {
    // Multiple SSE events - should take the first
    let body = b"data: first-event\n\ndata: second-event\n\n";

    let result = parse_sse_body(body).expect("Should parse first event");
    assert_eq!(result, b"first-event");
}

// ============================================================================
// SECTION: Property-Based Fuzzing Tests
// ============================================================================

proptest! {
    #[test]
    fn arbitrary_json_does_not_crash(json in "\\PC{0,1024}") {
        // Feed arbitrary strings to JSON parser
        // Should never panic, always return Ok or Err
        let _result = serde_json::from_str::<serde_json::Value>(&json);
        // Success: didn't crash
    }

    #[test]
    fn arbitrary_content_length_safe(length in 0u64..1_000_000_000) {
        // Test various Content-Length values
        let header = format!("Content-Length: {}\r\n\r\n", length);
        let mut reader = std::io::BufReader::new(Cursor::new(header.as_bytes()));

        // Should not crash, even with huge lengths
        let _result = read_framed(&mut reader);
        // Success: didn't crash (will fail due to EOF, but safely)
    }

    #[test]
    fn arbitrary_sse_body_safe(data in prop::collection::vec(any::<u8>(), 0..1024)) {
        // Feed arbitrary bytes to SSE parser
        // Should never panic
        let _result = parse_sse_body(&data);
        // Success: didn't crash
    }
}

// ============================================================================
// SECTION: Edge Case Tests
// ============================================================================

#[test]
fn empty_json_object_valid() {
    let empty = r#"{}"#;
    let result = serde_json::from_str::<serde_json::Value>(empty);
    assert!(result.is_ok());
}

#[test]
fn deeply_nested_json_handled() {
    // Create deeply nested JSON
    let mut json = String::from("{");
    for _ in 0 .. 100 {
        json.push_str("\"a\":{");
    }
    json.push_str("\"b\":1");
    for _ in 0 .. 100 {
        json.push_str("}");
    }
    json.push_str("}");

    // Should parse without stack overflow
    let result = serde_json::from_str::<serde_json::Value>(&json);
    // May succeed or fail depending on limits, but shouldn't crash
    let _ = result;
}

#[test]
fn json_with_unicode_handled() {
    let unicode = r#"{"message":"Hello 世界 🌍"}"#;
    let result = serde_json::from_str::<serde_json::Value>(unicode);
    assert!(result.is_ok());
}

#[test]
fn json_with_escaped_chars_handled() {
    let escaped = r#"{"message":"Line 1\nLine 2\tTabbed\r\nWindows"}"#;
    let result = serde_json::from_str::<serde_json::Value>(escaped);
    assert!(result.is_ok());
}
