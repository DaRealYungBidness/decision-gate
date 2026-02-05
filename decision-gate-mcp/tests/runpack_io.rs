// decision-gate-mcp/tests/runpack_io.rs
// ============================================================================
// Module: Runpack IO Tests
// Description: Tests for file-backed runpack artifact IO.
// Purpose: Validate path safety and IO round-trips for runpack artifacts.
// Dependencies: decision-gate-core, decision-gate-mcp
// ============================================================================
//! ## Overview
//! Exercises filesystem-backed runpack artifact IO with adversarial paths.
//!
//! Security posture: Runpack paths are untrusted; all IO must fail closed.
//! Threat model: TM-RUNPACK-001 - Path traversal or path length abuse.

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

use std::fs;
use std::path::PathBuf;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use decision_gate_core::AdvanceTo;
use decision_gate_core::Artifact;
use decision_gate_core::ArtifactKind;
use decision_gate_core::ArtifactReader;
use decision_gate_core::ArtifactRecord;
use decision_gate_core::ArtifactSink;
use decision_gate_core::ConditionSpec;
use decision_gate_core::DecisionOutcome;
use decision_gate_core::DecisionRecord;
use decision_gate_core::EvidenceRecord;
use decision_gate_core::EvidenceResult;
use decision_gate_core::EvidenceValue;
use decision_gate_core::FileHashEntry;
use decision_gate_core::GateEvalRecord;
use decision_gate_core::GateEvaluation;
use decision_gate_core::GateId;
use decision_gate_core::GateSpec;
use decision_gate_core::GateTraceEntry;
use decision_gate_core::HashDigest;
use decision_gate_core::NamespaceId;
use decision_gate_core::PacketEnvelope;
use decision_gate_core::PacketId;
use decision_gate_core::PacketPayload;
use decision_gate_core::PacketRecord;
use decision_gate_core::ProviderId;
use decision_gate_core::RunId;
use decision_gate_core::RunState;
use decision_gate_core::RunpackIntegrity;
use decision_gate_core::RunpackManifest;
use decision_gate_core::RunpackVersion;
use decision_gate_core::ScenarioId;
use decision_gate_core::ScenarioSpec;
use decision_gate_core::SchemaId;
use decision_gate_core::SpecVersion;
use decision_gate_core::StageId;
use decision_gate_core::StageSpec;
use decision_gate_core::SubmissionRecord;
use decision_gate_core::TenantId;
use decision_gate_core::Timestamp;
use decision_gate_core::ToolCallRecord;
use decision_gate_core::TriggerEvent;
use decision_gate_core::TriggerId;
use decision_gate_core::TriggerKind;
use decision_gate_core::TriggerRecord;
use decision_gate_core::VerifierMode;
use decision_gate_core::VisibilityPolicy;
use decision_gate_core::hashing::DEFAULT_HASH_ALGORITHM;
use decision_gate_core::hashing::canonical_json_bytes;
use decision_gate_core::hashing::hash_canonical_json;
use decision_gate_core::runtime::RunpackBuilder;
use decision_gate_mcp::FileArtifactReader;
use decision_gate_mcp::FileArtifactSink;
use ret_logic::TriState;
use serde_json::json;

// ========================================================================
// SECTION: Helpers
// ========================================================================

fn temp_root(label: &str) -> PathBuf {
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH).expect("clock drift").as_nanos();
    let mut path = std::env::temp_dir();
    path.push(format!("decision-gate-mcp-{label}-{nanos}"));
    fs::create_dir_all(&path).expect("create temp dir");
    path
}

fn cleanup(path: &PathBuf) {
    let _ = fs::remove_dir_all(path);
}

fn sample_artifact(path: &str, bytes: &[u8]) -> Artifact {
    Artifact {
        kind: ArtifactKind::ScenarioSpec,
        path: path.to_string(),
        content_type: Some("application/json".to_string()),
        bytes: bytes.to_vec(),
        required: true,
    }
}

fn sample_manifest() -> RunpackManifest {
    let artifact_hash = HashDigest::new(DEFAULT_HASH_ALGORITHM, b"artifact");
    let file_hashes = vec![FileHashEntry {
        path: "artifacts/sample.json".to_string(),
        hash: artifact_hash.clone(),
    }];
    let root_hash = hash_canonical_json(DEFAULT_HASH_ALGORITHM, &file_hashes).expect("root hash");
    RunpackManifest {
        manifest_version: RunpackVersion("v1".to_string()),
        generated_at: Timestamp::Logical(1),
        scenario_id: ScenarioId::new("scenario"),
        tenant_id: TenantId::from_raw(1).expect("nonzero tenantid"),
        namespace_id: NamespaceId::from_raw(1).expect("nonzero namespaceid"),
        run_id: RunId::new("run-1"),
        spec_hash: HashDigest::new(DEFAULT_HASH_ALGORITHM, b"spec"),
        hash_algorithm: DEFAULT_HASH_ALGORITHM,
        verifier_mode: VerifierMode::OfflineStrict,
        anchor_policy: None,
        security: None,
        integrity: RunpackIntegrity {
            file_hashes,
            root_hash,
        },
        artifacts: vec![ArtifactRecord {
            artifact_id: "artifact-1".to_string(),
            kind: ArtifactKind::ScenarioSpec,
            path: "artifacts/sample.json".to_string(),
            content_type: Some("application/json".to_string()),
            hash: artifact_hash,
            required: true,
        }],
    }
}

fn canonical_spec() -> ScenarioSpec {
    ScenarioSpec {
        scenario_id: ScenarioId::new("runpack-canonical"),
        namespace_id: NamespaceId::from_raw(1).expect("nonzero namespaceid"),
        spec_version: SpecVersion::new("1"),
        stages: vec![StageSpec {
            stage_id: StageId::new("stage-1"),
            entry_packets: Vec::new(),
            gates: vec![
                GateSpec {
                    gate_id: GateId::new("gate-a"),
                    requirement: ret_logic::Requirement::condition("cond-a".into()),
                    trust: None,
                },
                GateSpec {
                    gate_id: GateId::new("gate-b"),
                    requirement: ret_logic::Requirement::condition("cond-b".into()),
                    trust: None,
                },
            ],
            advance_to: AdvanceTo::Terminal,
            timeout: None,
            on_timeout: decision_gate_core::TimeoutPolicy::Fail,
        }],
        conditions: vec![
            ConditionSpec {
                condition_id: "cond-a".into(),
                query: decision_gate_core::EvidenceQuery {
                    provider_id: ProviderId::new("test"),
                    check_id: "check-a".to_string(),
                    params: None,
                },
                comparator: decision_gate_core::Comparator::Equals,
                expected: Some(json!(true)),
                policy_tags: Vec::new(),
                trust: None,
            },
            ConditionSpec {
                condition_id: "cond-b".into(),
                query: decision_gate_core::EvidenceQuery {
                    provider_id: ProviderId::new("test"),
                    check_id: "check-b".to_string(),
                    params: None,
                },
                comparator: decision_gate_core::Comparator::Equals,
                expected: Some(json!(true)),
                policy_tags: Vec::new(),
                trust: None,
            },
        ],
        policies: Vec::new(),
        schemas: Vec::new(),
        default_tenant_id: None,
    }
}

#[allow(
    clippy::too_many_lines,
    reason = "Test fixture builds full run state for canonical artifact checks."
)]
fn canonical_state(spec: &ScenarioSpec) -> RunState {
    let spec_hash = spec.canonical_hash_with(DEFAULT_HASH_ALGORITHM).expect("spec hash");
    let tenant_id = TenantId::from_raw(1).expect("nonzero tenantid");
    let namespace_id = NamespaceId::from_raw(1).expect("nonzero namespaceid");
    let run_id = RunId::new("run-1");
    let stage_id = StageId::new("stage-1");

    let trigger_one = TriggerEvent {
        trigger_id: TriggerId::new("trigger-1"),
        tenant_id,
        namespace_id,
        run_id: run_id.clone(),
        kind: TriggerKind::ExternalEvent,
        time: Timestamp::Logical(10),
        source_id: "source-a".to_string(),
        payload: None,
        correlation_id: None,
    };
    let trigger_two = TriggerEvent {
        trigger_id: TriggerId::new("trigger-2"),
        tenant_id,
        namespace_id,
        run_id: run_id.clone(),
        kind: TriggerKind::ExternalEvent,
        time: Timestamp::Logical(20),
        source_id: "source-b".to_string(),
        payload: None,
        correlation_id: None,
    };

    let evidence_a = EvidenceRecord {
        condition_id: "cond-a".into(),
        status: TriState::True,
        result: EvidenceResult {
            value: Some(EvidenceValue::Json(json!(true))),
            lane: decision_gate_core::TrustLane::Verified,
            error: None,
            evidence_hash: Some(
                hash_canonical_json(DEFAULT_HASH_ALGORITHM, &json!(true)).expect("hash"),
            ),
            evidence_ref: None,
            evidence_anchor: None,
            signature: None,
            content_type: Some("application/json".to_string()),
        },
    };
    let evidence_b = EvidenceRecord {
        condition_id: "cond-b".into(),
        status: TriState::True,
        result: EvidenceResult {
            value: Some(EvidenceValue::Json(json!(true))),
            lane: decision_gate_core::TrustLane::Verified,
            error: None,
            evidence_hash: Some(
                hash_canonical_json(DEFAULT_HASH_ALGORITHM, &json!(true)).expect("hash"),
            ),
            evidence_ref: None,
            evidence_anchor: None,
            signature: None,
            content_type: Some("application/json".to_string()),
        },
    };

    let gate_eval = |trigger_id: &str, gate_id: &str| GateEvalRecord {
        trigger_id: TriggerId::new(trigger_id),
        stage_id: stage_id.clone(),
        evaluation: GateEvaluation {
            gate_id: GateId::new(gate_id),
            status: TriState::True,
            trace: vec![
                GateTraceEntry {
                    condition_id: "cond-a".into(),
                    status: TriState::True,
                },
                GateTraceEntry {
                    condition_id: "cond-b".into(),
                    status: TriState::True,
                },
            ],
        },
        evidence: vec![evidence_a.clone(), evidence_b.clone()],
    };

    let decisions = vec![
        DecisionRecord {
            decision_id: decision_gate_core::DecisionId::new("decision-1"),
            seq: 1,
            trigger_id: TriggerId::new("trigger-1"),
            stage_id: stage_id.clone(),
            decided_at: Timestamp::Logical(11),
            outcome: DecisionOutcome::Hold {
                summary: decision_gate_core::SafeSummary {
                    status: "hold".to_string(),
                    unmet_gates: vec![],
                    retry_hint: None,
                    policy_tags: Vec::new(),
                },
            },
            correlation_id: None,
        },
        DecisionRecord {
            decision_id: decision_gate_core::DecisionId::new("decision-2"),
            seq: 2,
            trigger_id: TriggerId::new("trigger-2"),
            stage_id: stage_id.clone(),
            decided_at: Timestamp::Logical(21),
            outcome: DecisionOutcome::Hold {
                summary: decision_gate_core::SafeSummary {
                    status: "hold".to_string(),
                    unmet_gates: vec![],
                    retry_hint: None,
                    policy_tags: Vec::new(),
                },
            },
            correlation_id: None,
        },
    ];

    let packet_payload = PacketPayload::Json {
        value: json!({"packet": true}),
    };
    let packet_hash =
        hash_canonical_json(DEFAULT_HASH_ALGORITHM, &json!({"packet": true})).expect("packet hash");
    let packets = vec![
        PacketRecord {
            envelope: PacketEnvelope {
                scenario_id: spec.scenario_id.clone(),
                run_id: run_id.clone(),
                stage_id: stage_id.clone(),
                packet_id: PacketId::new("packet-1"),
                schema_id: SchemaId::new("schema-1"),
                content_type: "application/json".to_string(),
                content_hash: packet_hash.clone(),
                visibility: VisibilityPolicy::new(Vec::new(), Vec::new()),
                expiry: None,
                correlation_id: None,
                issued_at: Timestamp::Logical(12),
            },
            payload: packet_payload.clone(),
            receipts: Vec::new(),
            decision_id: decision_gate_core::DecisionId::new("decision-1"),
        },
        PacketRecord {
            envelope: PacketEnvelope {
                scenario_id: spec.scenario_id.clone(),
                run_id: run_id.clone(),
                stage_id: stage_id.clone(),
                packet_id: PacketId::new("packet-2"),
                schema_id: SchemaId::new("schema-2"),
                content_type: "application/json".to_string(),
                content_hash: packet_hash,
                visibility: VisibilityPolicy::new(Vec::new(), Vec::new()),
                expiry: None,
                correlation_id: None,
                issued_at: Timestamp::Logical(22),
            },
            payload: packet_payload,
            receipts: Vec::new(),
            decision_id: decision_gate_core::DecisionId::new("decision-2"),
        },
    ];

    let submission_payload = PacketPayload::Json {
        value: json!({"submission": 1}),
    };
    let submission_hash = hash_canonical_json(DEFAULT_HASH_ALGORITHM, &json!({"submission": 1}))
        .expect("submission hash");
    let submissions = vec![
        SubmissionRecord {
            submission_id: "submission-1".to_string(),
            run_id: run_id.clone(),
            payload: submission_payload.clone(),
            content_type: "application/json".to_string(),
            content_hash: submission_hash.clone(),
            submitted_at: Timestamp::Logical(13),
            correlation_id: None,
        },
        SubmissionRecord {
            submission_id: "submission-2".to_string(),
            run_id: run_id.clone(),
            payload: submission_payload,
            content_type: "application/json".to_string(),
            content_hash: submission_hash,
            submitted_at: Timestamp::Logical(23),
            correlation_id: None,
        },
    ];

    let tool_calls = vec![ToolCallRecord {
        call_id: "call-1".to_string(),
        method: "scenario.next".to_string(),
        request_hash: hash_canonical_json(
            DEFAULT_HASH_ALGORITHM,
            &json!({"tool": "scenario.next"}),
        )
        .expect("request hash"),
        response_hash: hash_canonical_json(DEFAULT_HASH_ALGORITHM, &json!({"ok": true}))
            .expect("response hash"),
        called_at: Timestamp::Logical(14),
        correlation_id: None,
        error: None,
    }];

    RunState {
        tenant_id,
        namespace_id,
        run_id,
        scenario_id: spec.scenario_id.clone(),
        spec_hash,
        current_stage_id: stage_id.clone(),
        stage_entered_at: Timestamp::Logical(0),
        status: decision_gate_core::RunStatus::Active,
        dispatch_targets: vec![],
        triggers: vec![
            TriggerRecord {
                seq: 1,
                event: trigger_one,
            },
            TriggerRecord {
                seq: 2,
                event: trigger_two,
            },
        ],
        gate_evals: vec![
            gate_eval("trigger-1", "gate-a"),
            gate_eval("trigger-1", "gate-b"),
            gate_eval("trigger-2", "gate-a"),
            gate_eval("trigger-2", "gate-b"),
        ],
        decisions,
        packets,
        submissions,
        tool_calls,
    }
}

fn expected_bytes_for_kind(kind: ArtifactKind, spec: &ScenarioSpec, state: &RunState) -> Vec<u8> {
    match kind {
        ArtifactKind::ScenarioSpec => canonical_json_bytes(spec).expect("spec bytes"),
        ArtifactKind::TriggerLog => canonical_json_bytes(&state.triggers).expect("trigger bytes"),
        ArtifactKind::GateEvalLog => canonical_json_bytes(&state.gate_evals).expect("gate bytes"),
        ArtifactKind::DecisionLog => {
            canonical_json_bytes(&state.decisions).expect("decision bytes")
        }
        ArtifactKind::PacketLog => canonical_json_bytes(&state.packets).expect("packet bytes"),
        ArtifactKind::SubmissionLog => {
            canonical_json_bytes(&state.submissions).expect("submission bytes")
        }
        ArtifactKind::ToolTranscript => {
            canonical_json_bytes(&state.tool_calls).expect("tool call bytes")
        }
        _ => panic!("unexpected artifact kind: {kind:?}"),
    }
}

fn assert_canonical_artifacts<R: ArtifactReader>(
    reader: &R,
    manifest: &RunpackManifest,
    spec: &ScenarioSpec,
    state: &RunState,
) {
    for artifact in &manifest.artifacts {
        let expected = expected_bytes_for_kind(artifact.kind, spec, state);
        let bytes = reader
            .read_with_limit(
                &artifact.path,
                decision_gate_core::runtime::MAX_RUNPACK_ARTIFACT_BYTES,
            )
            .unwrap();
        assert_eq!(bytes, expected, "artifact mismatch: {}", artifact.path);
    }
}

// ========================================================================
// SECTION: Round-Trip IO Tests
// ========================================================================

/// Verifies file-backed sinks and readers round-trip bytes successfully.
#[test]
fn file_artifact_sink_and_reader_round_trip() {
    let root = temp_root("roundtrip");
    let mut sink = FileArtifactSink::new(root.clone(), "runpack.json").unwrap();
    let artifact = sample_artifact("evidence/log.json", b"hello");

    let reference = sink.write(&artifact).unwrap();
    let reference_path = PathBuf::from(&reference.uri);
    let expected = PathBuf::from("evidence").join("log.json");
    assert!(
        reference_path.ends_with(&expected),
        "unexpected artifact reference path: {}",
        reference.uri
    );

    let reader = FileArtifactReader::new(root.clone()).unwrap();
    let bytes = reader.read("evidence/log.json").unwrap();
    assert_eq!(bytes, b"hello");

    cleanup(&root);
}

/// Verifies runpack manifests are written as canonical JSON (RFC 8785).
#[test]
fn file_artifact_sink_writes_canonical_manifest() {
    let root = temp_root("manifest");
    let mut sink = FileArtifactSink::new(root.clone(), "runpack.json").unwrap();
    let manifest = sample_manifest();

    sink.finalize(&manifest).unwrap();
    let bytes = fs::read(root.join("runpack.json")).unwrap();
    let expected = canonical_json_bytes(&manifest).expect("canonical manifest");
    assert_eq!(bytes, expected);

    cleanup(&root);
}

/// Verifies file-backed runpack artifacts are serialized as canonical JSON.
#[test]
fn file_artifact_sink_writes_canonical_artifacts() {
    let root = temp_root("canonical-artifacts");
    let mut sink = FileArtifactSink::new(root.clone(), "runpack.json").unwrap();
    let spec = canonical_spec();
    let state = canonical_state(&spec);

    let builder = RunpackBuilder::default();
    let manifest =
        builder.build(&mut sink, &spec, &state, Timestamp::Logical(99)).expect("runpack build");

    let reader = FileArtifactReader::new(root.clone()).unwrap();
    assert_canonical_artifacts(&reader, &manifest, &spec, &state);

    cleanup(&root);
}

// ========================================================================
// SECTION: Path Safety Tests
// ========================================================================

/// Verifies absolute artifact paths are rejected.
#[test]
fn file_artifact_sink_rejects_absolute_paths() {
    let root = temp_root("absolute");
    let mut sink = FileArtifactSink::new(root.clone(), "runpack.json").unwrap();

    let absolute =
        if cfg!(windows) { "C:\\Windows\\System32\\drivers\\etc\\hosts" } else { "/etc/passwd" };

    let artifact = sample_artifact(absolute, b"nope");
    let err = sink.write(&artifact).unwrap_err();
    assert!(err.to_string().contains("absolute artifact path"), "unexpected error: {err}");

    cleanup(&root);
}

/// Verifies manifest path traversal is rejected.
#[test]
fn file_artifact_sink_rejects_manifest_traversal() {
    let root = temp_root("manifest-traversal");
    let result = FileArtifactSink::new(root.clone(), "../manifest.json");
    assert!(result.is_err());

    cleanup(&root);
}

/// Verifies parent traversal is rejected for readers.
#[test]
fn file_artifact_reader_rejects_parent_traversal() {
    let root = temp_root("traversal");
    fs::write(root.join("safe.json"), b"safe").unwrap();
    let reader = FileArtifactReader::new(root.clone()).unwrap();

    let result = reader.read("../escape.json");
    assert!(result.is_err());

    cleanup(&root);
}

// ========================================================================
// SECTION: Path Limit Tests
// ========================================================================

/// Verifies overly long path components are rejected.
#[test]
fn file_artifact_sink_rejects_overlong_component() {
    let mut root = std::env::temp_dir();
    root.push("a".repeat(256));
    let result = FileArtifactSink::new(root, "runpack.json");
    assert!(result.is_err());
}

/// Verifies overly long total paths are rejected.
#[test]
fn file_artifact_sink_rejects_overlong_total_path() {
    let too_long = "a".repeat(5000);
    let root = PathBuf::from(too_long);
    let result = FileArtifactSink::new(root, "runpack.json");
    assert!(result.is_err());
}

// ========================================================================
// SECTION: Path Edge Cases
// ========================================================================

/// Verifies that Windows UNC paths are rejected.
#[cfg(windows)]
#[test]
fn file_artifact_sink_rejects_unc_paths() {
    let root = temp_root("unc");
    let mut sink = FileArtifactSink::new(root.clone(), "runpack.json").unwrap();

    // Test verbatim UNC path
    let verbatim_unc = r"\\?\C:\Windows\System32\config";
    let artifact = sample_artifact(verbatim_unc, b"nope");
    let err = sink.write(&artifact).unwrap_err();
    assert!(
        err.to_string().contains("absolute artifact path")
            || err.to_string().contains("artifact path"),
        "unexpected error for verbatim UNC path: {err}"
    );

    // Test server UNC path
    let server_unc = r"\\server\share\file.txt";
    let artifact2 = sample_artifact(server_unc, b"nope");
    let err2 = sink.write(&artifact2).unwrap_err();
    assert!(
        err2.to_string().contains("absolute artifact path")
            || err2.to_string().contains("artifact path"),
        "unexpected error for server UNC path: {err2}"
    );

    cleanup(&root);
}

/// Verifies that path traversal attempts are rejected even with normalization tricks.
#[test]
fn file_artifact_sink_rejects_traversal_with_normalization() {
    let root = temp_root("normalized");
    let mut sink = FileArtifactSink::new(root.clone(), "runpack.json").unwrap();

    // Test path attempting traversal through relative components
    // This should be rejected because it contains .. component
    let traversal = "artifacts/../escape.json";
    let artifact = sample_artifact(traversal, b"test");
    let err = sink.write(&artifact).unwrap_err();
    assert!(
        err.to_string().contains("artifact path"),
        "expected path error for traversal, got: {err}"
    );

    cleanup(&root);
}

/// Verifies that certain non-normalized paths are safely handled.
#[test]
fn file_artifact_sink_normalizes_safe_paths() {
    let root = temp_root("safe-normalized");
    let mut sink = FileArtifactSink::new(root.clone(), "runpack.json").unwrap();

    // These paths should be normalized and allowed (not rejected)
    // The implementation safely handles ./ and // by normalizing them

    // Path with ./ should be normalized to artifacts/log.json
    let dot_slash = "artifacts/./log.json";
    let artifact = sample_artifact(dot_slash, b"test-dot");
    let result = sink.write(&artifact);
    assert!(result.is_ok(), "safe path normalization should allow ./, got error: {result:?}");

    cleanup(&root);
}

/// Verifies path component and total path length boundaries.
#[test]
fn file_artifact_sink_path_component_boundaries() {
    let root = temp_root("boundaries");

    // Test exact boundary for component length (255 chars should pass)
    let component_254 = "a".repeat(254);
    let result_254 = FileArtifactSink::new(root.join(&component_254), "runpack.json");
    assert!(result_254.is_ok(), "254-char component should be accepted");

    // Test exact boundary (255 chars should pass)
    let component_255 = "a".repeat(255);
    let result_255 = FileArtifactSink::new(root.join(&component_255), "runpack.json");
    assert!(result_255.is_ok(), "255-char component should be accepted");

    // Test over boundary (256 chars should fail)
    let component_256 = "a".repeat(256);
    let result_256 = FileArtifactSink::new(root.join(&component_256), "runpack.json");
    assert!(result_256.is_err(), "256-char component should be rejected");

    cleanup(&root);
}
