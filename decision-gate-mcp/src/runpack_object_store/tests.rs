// decision-gate-mcp/src/runpack_object_store/tests.rs
// ============================================================================
// Module: Runpack Object Store Tests
// Description: Unit tests for object-store-backed runpack helpers.
// Purpose: Validate path normalization, size limits, and deterministic prefixes.
// Dependencies: decision-gate-mcp, decision-gate-core
// ============================================================================

//! ## Overview
//! Exercises object-store runpack helpers for safe path handling, size limits,
//! and deterministic storage prefixes.
//!
//! Security posture: Tests validate fail-closed handling for untrusted artifact
//! paths; see `Docs/security/threat_model.md`.

// ============================================================================
// SECTION: Lint Configuration
// ============================================================================

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::panic_in_result_fn,
    clippy::unwrap_in_result,
    reason = "Test-only assertions favor direct unwrap/expect for clarity."
)]

// ============================================================================
// SECTION: Imports
// ============================================================================

use decision_gate_core::AdvanceTo;
use decision_gate_core::ArtifactKind;
use decision_gate_core::ArtifactRecord;
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
use decision_gate_core::PacketEnvelope;
use decision_gate_core::PacketId;
use decision_gate_core::PacketPayload;
use decision_gate_core::PacketRecord;
use decision_gate_core::ProviderId;
use decision_gate_core::RunState;
use decision_gate_core::RunpackIntegrity;
use decision_gate_core::RunpackVersion;
use decision_gate_core::ScenarioSpec;
use decision_gate_core::SchemaId;
use decision_gate_core::SpecVersion;
use decision_gate_core::StageId;
use decision_gate_core::StageSpec;
use decision_gate_core::SubmissionRecord;
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
use ret_logic::TriState;
use serde_json::json;

use super::*;

// ============================================================================
// SECTION: Fixtures
// ============================================================================

fn sample_key() -> RunpackObjectKey {
    RunpackObjectKey {
        tenant_id: TenantId::from_raw(1).expect("nonzero tenantid"),
        namespace_id: NamespaceId::from_raw(1).expect("nonzero namespaceid"),
        scenario_id: ScenarioId::from("scenario-001"),
        run_id: RunId::from("run-001"),
        spec_hash: HashDigest {
            algorithm: HashAlgorithm::Sha256,
            value: "abc123".to_string(),
        },
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
        scenario_id: ScenarioId::from("scenario-001"),
        tenant_id: TenantId::from_raw(1).expect("nonzero tenantid"),
        namespace_id: NamespaceId::from_raw(1).expect("nonzero namespaceid"),
        run_id: RunId::from("run-001"),
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

#[test]
fn runpack_prefix_is_deterministic() {
    let key = sample_key();
    let prefix = runpack_prefix(&key).expect("prefix");
    assert!(prefix.contains("tenant/1"));
    assert!(prefix.ends_with('/'));
}

#[test]
fn validate_relative_path_rejects_traversal() {
    let result = validate_relative_path("../escape");
    assert!(result.is_err());
}

#[test]
fn validate_relative_path_rejects_backslashes() {
    let result = validate_relative_path("bad\\path");
    assert!(result.is_err());
}

#[test]
fn normalize_prefix_rejects_backslashes() {
    let result = normalize_prefix("bad\\prefix");
    assert!(result.is_err());
}

#[test]
fn runpack_prefix_rejects_invalid_segments() {
    let mut key = sample_key();
    key.run_id = RunId::from("bad/run");
    let result = runpack_prefix(&key);
    assert!(result.is_err());
}

#[test]
fn storage_uri_contains_bucket_and_prefix() {
    let store = Arc::new(InMemoryObjectStore::new());
    let backend = ObjectStoreRunpackBackend::from_client("unit-bucket", store);
    let key = sample_key();
    let uri = backend.storage_uri(&key).expect("uri");
    assert!(uri.starts_with("memory://unit-bucket/tenant/1"));
    assert!(uri.ends_with('/'));
}

#[test]
fn sink_rejects_invalid_manifest_name() {
    let store = Arc::new(InMemoryObjectStore::new());
    let backend = ObjectStoreRunpackBackend::from_client("test-bucket", store);
    let key = sample_key();
    let result = backend.sink(&key, "../manifest.json");
    assert!(result.is_err());
}

#[test]
fn object_store_sink_writes_and_reads() {
    let store = Arc::new(InMemoryObjectStore::new());
    let backend = ObjectStoreRunpackBackend::from_client("test-bucket", store);
    let key = sample_key();
    let mut sink = backend.sink(&key, "manifest.json").expect("sink");
    let artifact = Artifact {
        kind: decision_gate_core::ArtifactKind::ScenarioSpec,
        path: "scenario.json".to_string(),
        content_type: Some("application/json".to_string()),
        bytes: b"{\"ok\":true}".to_vec(),
        required: true,
    };
    sink.write(&artifact).expect("write");
    let reader = backend.reader(&key).expect("reader");
    let bytes = reader.read_with_limit("scenario.json", MAX_RUNPACK_ARTIFACT_BYTES).expect("read");
    assert_eq!(bytes, b"{\"ok\":true}");
}

#[test]
fn object_store_sink_writes_canonical_manifest() {
    let store = Arc::new(InMemoryObjectStore::new());
    let backend = ObjectStoreRunpackBackend::from_client("test-bucket", store);
    let key = sample_key();
    let mut sink = backend.sink(&key, "manifest.json").expect("sink");
    let manifest = sample_manifest();

    sink.finalize(&manifest).expect("finalize");

    let reader = backend.reader(&key).expect("reader");
    let bytes = reader.read_with_limit("manifest.json", MAX_RUNPACK_ARTIFACT_BYTES).expect("read");
    let expected = canonical_json_bytes(&manifest).expect("canonical manifest");
    assert_eq!(bytes, expected);
}

#[test]
fn object_store_sink_writes_canonical_artifacts() {
    let store = Arc::new(InMemoryObjectStore::new());
    let backend = ObjectStoreRunpackBackend::from_client("test-bucket", store);
    let spec = canonical_spec();
    let state = canonical_state(&spec);
    let key = RunpackObjectKey {
        tenant_id: state.tenant_id,
        namespace_id: state.namespace_id,
        scenario_id: spec.scenario_id.clone(),
        run_id: state.run_id.clone(),
        spec_hash: state.spec_hash.clone(),
    };

    let mut sink = backend.sink(&key, "manifest.json").expect("sink");
    let builder = RunpackBuilder::default();
    let manifest =
        builder.build(&mut sink, &spec, &state, Timestamp::Logical(99)).expect("runpack build");

    let reader = backend.reader(&key).expect("reader");
    for artifact in &manifest.artifacts {
        let expected = expected_bytes_for_kind(artifact.kind, &spec, &state);
        let bytes =
            reader.read_with_limit(&artifact.path, MAX_RUNPACK_ARTIFACT_BYTES).expect("read");
        assert_eq!(bytes, expected, "artifact mismatch: {}", artifact.path);
    }
}

#[test]
fn sink_rejects_large_artifacts() {
    let store = Arc::new(InMemoryObjectStore::new());
    let backend = ObjectStoreRunpackBackend::from_client("test-bucket", store);
    let key = sample_key();
    let mut sink = backend.sink(&key, "manifest.json").expect("sink");
    let bytes = vec![0u8; MAX_RUNPACK_ARTIFACT_BYTES + 1];
    let artifact = Artifact {
        kind: decision_gate_core::ArtifactKind::ScenarioSpec,
        path: "scenario.json".to_string(),
        content_type: None,
        bytes,
        required: true,
    };
    let result = sink.write(&artifact);
    assert!(matches!(result, Err(ArtifactError::TooLarge { .. })));
}

#[test]
fn reader_rejects_over_limit_reads() {
    let store = Arc::new(InMemoryObjectStore::new());
    let backend = ObjectStoreRunpackBackend::from_client("test-bucket", store);
    let key = sample_key();
    let mut sink = backend.sink(&key, "manifest.json").expect("sink");
    let artifact = Artifact {
        kind: decision_gate_core::ArtifactKind::ScenarioSpec,
        path: "scenario.json".to_string(),
        content_type: None,
        bytes: vec![1u8; 32],
        required: true,
    };
    sink.write(&artifact).expect("write");
    let reader = backend.reader(&key).expect("reader");
    let result = reader.read_with_limit("scenario.json", 16);
    assert!(matches!(result, Err(ArtifactError::TooLarge { .. })));
}

#[test]
fn object_key_rejects_overlong_paths() {
    let store = Arc::new(InMemoryObjectStore::new());
    let sink = ObjectStoreArtifactSink::new(
        store,
        String::new(),
        "a".repeat(MAX_TOTAL_PATH_LENGTH),
        "manifest.json",
    )
    .expect("sink");
    let result = sink.object_key("artifact.json");
    assert!(result.is_err());
}
