// enterprise/decision-gate-store-enterprise/tests/s3_runpack_store.rs
// ============================================================================
// Module: S3 Runpack Store Tests
// Description: Unit tests for S3 runpack store configuration.
// Purpose: Validate config validation without real AWS services.
// ============================================================================

//! S3 runpack store unit tests.

#![cfg(feature = "s3")]

use decision_gate_store_enterprise::s3_runpack_store::S3RunpackStore;
use decision_gate_store_enterprise::s3_runpack_store::S3RunpackStoreConfig;
use decision_gate_store_enterprise::s3_runpack_store::S3ServerSideEncryption;

fn base_config() -> S3RunpackStoreConfig {
    S3RunpackStoreConfig {
        bucket: "decision-gate-test".to_string(),
        region: Some("us-east-1".to_string()),
        prefix: None,
        endpoint: None,
        force_path_style: false,
        server_side_encryption: None,
        kms_key_id: None,
        max_archive_bytes: Some(10 * 1024 * 1024),
    }
}

#[test]
fn s3_store_rejects_empty_bucket() {
    let mut config = base_config();
    config.bucket = "".to_string();
    let result = S3RunpackStore::new(config);
    assert!(result.is_err());
}

#[test]
fn s3_store_rejects_kms_without_key() {
    let mut config = base_config();
    config.server_side_encryption = Some(S3ServerSideEncryption::AwsKms);
    config.kms_key_id = None;
    let result = S3RunpackStore::new(config);
    assert!(result.is_err());
}

#[test]
fn s3_store_rejects_invalid_prefix() {
    let mut config = base_config();
    config.prefix = Some("invalid/../prefix".to_string());
    let result = S3RunpackStore::new(config);
    assert!(result.is_err());
}

// ============================================================================
// New tests
// ============================================================================

#[test]
fn s3_config_serde_roundtrip() {
    let original = base_config();
    let json = serde_json::to_string(&original).expect("serialize");
    let restored: S3RunpackStoreConfig = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(original.bucket, restored.bucket);
    assert_eq!(original.region, restored.region);
    assert_eq!(original.prefix, restored.prefix);
    assert_eq!(original.endpoint, restored.endpoint);
    assert_eq!(original.force_path_style, restored.force_path_style);
    assert_eq!(original.max_archive_bytes, restored.max_archive_bytes);
}
