// enterprise/decision-gate-enterprise/tests/audit_chain.rs
// ============================================================================
// Module: Audit Chain Tests
// Description: Unit tests for hash-chained audit sink behavior.
// Purpose: Validate hash chaining and append-only semantics.
// ============================================================================

//! Audit chain unit tests.

use decision_gate_contract::ToolName;
use decision_gate_core::hashing::HashAlgorithm;
use decision_gate_core::hashing::hash_bytes;
use decision_gate_enterprise::audit_chain::AuditChainError;
use decision_gate_enterprise::audit_chain::HashChainedAuditSink;
use decision_gate_mcp::McpAuditEvent;
use decision_gate_mcp::McpAuditSink;
use decision_gate_mcp::PrecheckAuditEvent;
use decision_gate_mcp::TenantAuthzEvent;
use decision_gate_mcp::UsageAuditEvent;
use decision_gate_mcp::audit::RegistryAuditEvent;
use decision_gate_mcp::audit::SecurityAuditEvent;
use decision_gate_mcp::config::RegistryAclAction;
use decision_gate_mcp::config::ServerTransport;
use decision_gate_mcp::telemetry::McpMethod;
use decision_gate_mcp::telemetry::McpOutcome;
use tempfile::NamedTempFile;

fn sample_event(timestamp_ms: u128) -> McpAuditEvent {
    McpAuditEvent {
        event: "test_event",
        timestamp_ms,
        request_id: Some(format!("req-{timestamp_ms}")),
        transport: ServerTransport::Stdio,
        peer_ip: None,
        method: McpMethod::ToolsCall,
        tool: Some(ToolName::RunpackExport),
        outcome: McpOutcome::Ok,
        error_code: None,
        error_kind: None,
        request_bytes: 0,
        response_bytes: 0,
        client_subject: None,
        redaction: "none",
    }
}

#[test]
fn audit_chain_links_hashes() {
    let file = NamedTempFile::new().expect("temp file");
    let sink = HashChainedAuditSink::new(file.path()).expect("sink");
    sink.record(&sample_event(1));
    sink.record(&sample_event(2));

    let contents = std::fs::read_to_string(file.path()).expect("read log");
    let lines: Vec<&str> = contents.lines().filter(|line| !line.trim().is_empty()).collect();
    assert_eq!(lines.len(), 2);
    let first: serde_json::Value = serde_json::from_str(lines[0]).expect("parse first");
    let second: serde_json::Value = serde_json::from_str(lines[1]).expect("parse second");
    let first_prev = first.get("prev_hash").and_then(|v| v.as_str()).unwrap_or("");
    let first_hash = first.get("hash").and_then(|v| v.as_str()).unwrap_or("");
    let second_prev = second.get("prev_hash").and_then(|v| v.as_str()).unwrap_or("");
    assert_eq!(first_prev, "0");
    assert!(!first_hash.is_empty());
    assert_eq!(second_prev, first_hash);
}

#[test]
fn audit_chain_hash_is_sha256_of_prev_plus_payload() {
    let file = NamedTempFile::new().expect("temp file");
    let sink = HashChainedAuditSink::new(file.path()).expect("sink");
    sink.record(&sample_event(10));
    sink.record(&sample_event(20));
    sink.record(&sample_event(30));

    let contents = std::fs::read_to_string(file.path()).expect("read log");
    let lines: Vec<&str> = contents.lines().filter(|line| !line.trim().is_empty()).collect();
    assert_eq!(lines.len(), 3);

    for line in &lines {
        let envelope: serde_json::Value = serde_json::from_str(line).expect("parse envelope");
        let prev_hash =
            envelope.get("prev_hash").and_then(|v| v.as_str()).expect("prev_hash field");
        let stored_hash = envelope.get("hash").and_then(|v| v.as_str()).expect("hash field");
        let payload = envelope.get("payload").expect("payload field");

        let payload_bytes = serde_json::to_vec(payload).expect("serialize payload");
        let mut combined = prev_hash.as_bytes().to_vec();
        combined.extend_from_slice(&payload_bytes);
        let expected_digest = hash_bytes(HashAlgorithm::Sha256, &combined);
        assert_eq!(
            stored_hash, expected_digest.value,
            "hash mismatch for envelope with prev_hash={prev_hash}"
        );
    }
}

#[test]
fn audit_chain_resumes_after_reopen() {
    let file = NamedTempFile::new().expect("temp file");
    let path = file.path().to_path_buf();

    // Write 2 events, then drop the sink.
    {
        let sink = HashChainedAuditSink::new(&path).expect("sink");
        sink.record(&sample_event(100));
        sink.record(&sample_event(200));
    }

    // Re-open on the same path and write 1 more event.
    {
        let sink = HashChainedAuditSink::new(&path).expect("sink reopen");
        sink.record(&sample_event(300));
    }

    let contents = std::fs::read_to_string(&path).expect("read log");
    let lines: Vec<&str> = contents.lines().filter(|line| !line.trim().is_empty()).collect();
    assert_eq!(lines.len(), 3, "expected 3 total audit lines");

    let second: serde_json::Value = serde_json::from_str(lines[1]).expect("parse second");
    let third: serde_json::Value = serde_json::from_str(lines[2]).expect("parse third");
    let second_hash = second.get("hash").and_then(|v| v.as_str()).expect("second hash");
    let third_prev = third.get("prev_hash").and_then(|v| v.as_str()).expect("third prev_hash");
    assert_eq!(
        third_prev, second_hash,
        "third event must chain from second event's hash after reopen"
    );
}

#[test]
fn audit_chain_corrupt_json_returns_parse_error() {
    let file = NamedTempFile::new().expect("temp file");
    std::fs::write(file.path(), "not-json\n").expect("write corrupt data");

    let result = HashChainedAuditSink::new(file.path());
    assert!(result.is_err(), "expected error for corrupt JSON");
    let err = match result {
        Ok(_) => panic!("expected error for corrupt JSON"),
        Err(err) => err,
    };
    match err {
        AuditChainError::Parse(msg) => {
            assert!(!msg.is_empty(), "parse error message should not be empty");
        }
        other => panic!("expected AuditChainError::Parse, got: {other:?}"),
    }
}

#[test]
fn audit_chain_empty_file_starts_at_zero() {
    let file = NamedTempFile::new().expect("temp file");
    // File is already empty by default.
    let sink = HashChainedAuditSink::new(file.path()).expect("sink");
    sink.record(&sample_event(1));

    let contents = std::fs::read_to_string(file.path()).expect("read log");
    let lines: Vec<&str> = contents.lines().filter(|line| !line.trim().is_empty()).collect();
    assert_eq!(lines.len(), 1);

    let first: serde_json::Value = serde_json::from_str(lines[0]).expect("parse first");
    let prev_hash = first.get("prev_hash").and_then(|v| v.as_str()).expect("prev_hash");
    assert_eq!(prev_hash, "0", "first event in empty file must have prev_hash == \"0\"");
}

#[test]
fn audit_chain_all_sink_methods_emit_envelopes() {
    let file = NamedTempFile::new().expect("temp file");
    let sink = HashChainedAuditSink::new(file.path()).expect("sink");

    let dummy_hash = hash_bytes(HashAlgorithm::Sha256, b"test");

    // 1. record_precheck
    sink.record_precheck(&PrecheckAuditEvent {
        event: "precheck_audit",
        timestamp_ms: 1,
        tenant_id: "t1".to_string(),
        namespace_id: "ns1".to_string(),
        scenario_id: None,
        stage_id: None,
        schema_id: "schema-1".to_string(),
        schema_version: "1.0.0".to_string(),
        request_hash: dummy_hash.clone(),
        response_hash: dummy_hash.clone(),
        request: None,
        response: None,
        redaction: "none",
    });

    // 2. record_registry
    sink.record_registry(&RegistryAuditEvent {
        event: "registry_audit",
        timestamp_ms: 2,
        request_id: None,
        tenant_id: "t1".to_string(),
        namespace_id: "ns1".to_string(),
        action: RegistryAclAction::List,
        allowed: true,
        reason: "test".to_string(),
        principal_id: "p1".to_string(),
        roles: vec!["admin".to_string()],
        policy_class: None,
        schema_id: None,
        schema_version: None,
    });

    // 3. record_tenant_authz
    sink.record_tenant_authz(&TenantAuthzEvent {
        event: "tenant_authz",
        timestamp_ms: 3,
        request_id: None,
        tool: Some(ToolName::RunpackExport),
        allowed: true,
        reason: "test".to_string(),
        principal_id: "p1".to_string(),
        tenant_id: Some("t1".to_string()),
        namespace_id: Some("ns1".to_string()),
    });

    // 4. record_usage
    sink.record_usage(&UsageAuditEvent {
        event: "usage_audit",
        timestamp_ms: 4,
        request_id: None,
        tool: Some(ToolName::RunpackExport),
        tenant_id: Some("t1".to_string()),
        namespace_id: Some("ns1".to_string()),
        principal_id: "p1".to_string(),
        metric: "api_calls".to_string(),
        units: 1,
        allowed: true,
        reason: "test".to_string(),
    });

    // 5. record_security
    sink.record_security(&SecurityAuditEvent {
        event: "security_audit",
        timestamp_ms: 5,
        kind: "startup".to_string(),
        message: Some("test startup".to_string()),
        dev_permissive: false,
        namespace_authority: "config".to_string(),
        namespace_mapping_mode: None,
    });

    let contents = std::fs::read_to_string(file.path()).expect("read log");
    let lines: Vec<&str> = contents.lines().filter(|line| !line.trim().is_empty()).collect();
    assert_eq!(lines.len(), 5, "expected exactly 5 audit envelopes");

    for (i, line) in lines.iter().enumerate() {
        let envelope: serde_json::Value =
            serde_json::from_str(line).unwrap_or_else(|_| panic!("parse line {i}"));
        assert!(envelope.get("prev_hash").is_some(), "line {i} missing prev_hash");
        assert!(envelope.get("hash").is_some(), "line {i} missing hash");
    }
}

#[test]
fn audit_chain_tolerates_empty_lines_in_log() {
    let file = NamedTempFile::new().expect("temp file");
    let path = file.path().to_path_buf();

    // Write one valid envelope manually, then an empty line.
    {
        let sink = HashChainedAuditSink::new(&path).expect("sink");
        sink.record(&sample_event(1));
    }

    // Read the first envelope's hash so we know what to expect.
    let contents_before = std::fs::read_to_string(&path).expect("read");
    let first_line = contents_before.lines().find(|l| !l.trim().is_empty()).expect("first line");
    let first_env: serde_json::Value = serde_json::from_str(first_line).expect("parse first");
    let first_hash = first_env.get("hash").and_then(|v| v.as_str()).expect("hash").to_string();

    // Append a blank line to the log file.
    {
        use std::io::Write;
        let mut f = std::fs::OpenOptions::new().append(true).open(&path).expect("open for append");
        writeln!(f).expect("write blank line");
    }

    // Re-open the sink on the file with the blank line and write 1 more event.
    {
        let sink = HashChainedAuditSink::new(&path).expect("sink after blank");
        sink.record(&sample_event(2));
    }

    let contents_after = std::fs::read_to_string(&path).expect("read final");
    let all_lines: Vec<&str> = contents_after.lines().filter(|l| !l.trim().is_empty()).collect();
    assert_eq!(all_lines.len(), 2, "expected 2 non-empty lines");

    let second_env: serde_json::Value = serde_json::from_str(all_lines[1]).expect("parse second");
    let second_prev =
        second_env.get("prev_hash").and_then(|v| v.as_str()).expect("second prev_hash");
    assert_eq!(
        second_prev, first_hash,
        "chain must continue from the first valid envelope's hash, skipping blank lines"
    );
}
