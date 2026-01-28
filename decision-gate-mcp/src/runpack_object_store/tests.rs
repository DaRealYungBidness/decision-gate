#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::panic_in_result_fn,
    clippy::unwrap_in_result,
    reason = "Test-only assertions favor direct unwrap/expect for clarity."
)]

use super::*;

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
    let bytes = reader
        .read_with_limit("scenario.json", MAX_RUNPACK_ARTIFACT_BYTES)
        .expect("read");
    assert_eq!(bytes, b"{\"ok\":true}");
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
