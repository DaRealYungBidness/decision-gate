// enterprise/decision-gate-store-enterprise/tests/s3_runpack_store.rs
// ============================================================================
// Module: S3 Runpack Store Tests
// Description: Unit tests for S3 runpack store configuration.
// Purpose: Validate config validation without real AWS services.
// ============================================================================

//! S3 runpack store unit tests.

#![cfg(feature = "s3")]

use decision_gate_store_enterprise::s3_runpack_store::S3ObjectLockConfig;
use decision_gate_store_enterprise::s3_runpack_store::S3ObjectLockLegalHold;
use decision_gate_store_enterprise::s3_runpack_store::S3ObjectLockMode;
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
        object_lock: None,
    }
}

#[test]
fn s3_store_rejects_empty_bucket() {
    let mut config = base_config();
    config.bucket = String::new();
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

#[test]
fn s3_store_accepts_object_lock_with_mode_and_retain_until() {
    let mut config = base_config();
    config.object_lock = Some(S3ObjectLockConfig {
        mode: Some(S3ObjectLockMode::Governance),
        retain_until: Some("2030-01-01T00:00:00Z".to_string()),
        legal_hold: None,
    });
    let result = S3RunpackStore::new(config);
    assert!(result.is_ok());
}

#[test]
fn s3_store_accepts_object_lock_legal_hold_only() {
    let mut config = base_config();
    config.object_lock = Some(S3ObjectLockConfig {
        mode: None,
        retain_until: None,
        legal_hold: Some(S3ObjectLockLegalHold::On),
    });
    let result = S3RunpackStore::new(config);
    assert!(result.is_ok());
}

#[test]
fn s3_store_rejects_object_lock_without_mode() {
    let mut config = base_config();
    config.object_lock =
        Some(decision_gate_store_enterprise::s3_runpack_store::S3ObjectLockConfig {
            mode: None,
            retain_until: Some("2030-01-01T00:00:00Z".to_string()),
            legal_hold: None,
        });
    let result = S3RunpackStore::new(config);
    assert!(result.is_err());
}

#[test]
fn s3_store_rejects_object_lock_without_retain_until() {
    let mut config = base_config();
    config.object_lock =
        Some(decision_gate_store_enterprise::s3_runpack_store::S3ObjectLockConfig {
            mode: Some(S3ObjectLockMode::Governance),
            retain_until: None,
            legal_hold: None,
        });
    let result = S3RunpackStore::new(config);
    assert!(result.is_err());
}

#[test]
fn s3_store_rejects_invalid_object_lock_date() {
    let mut config = base_config();
    config.object_lock =
        Some(decision_gate_store_enterprise::s3_runpack_store::S3ObjectLockConfig {
            mode: Some(S3ObjectLockMode::Compliance),
            retain_until: Some("not-a-date".to_string()),
            legal_hold: None,
        });
    let result = S3RunpackStore::new(config);
    assert!(result.is_err());
}

// ============================================================================
// New tests
// ============================================================================

#[test]
fn s3_config_serde_roundtrip() -> Result<(), Box<dyn std::error::Error>> {
    let original = base_config();
    let json = serde_json::to_string(&original)?;
    let restored: S3RunpackStoreConfig = serde_json::from_str(&json)?;
    if original.bucket != restored.bucket {
        return Err("bucket mismatch after serde roundtrip".into());
    }
    if original.region != restored.region {
        return Err("region mismatch after serde roundtrip".into());
    }
    if original.prefix != restored.prefix {
        return Err("prefix mismatch after serde roundtrip".into());
    }
    if original.endpoint != restored.endpoint {
        return Err("endpoint mismatch after serde roundtrip".into());
    }
    if original.force_path_style != restored.force_path_style {
        return Err("force_path_style mismatch after serde roundtrip".into());
    }
    if original.max_archive_bytes != restored.max_archive_bytes {
        return Err("max_archive_bytes mismatch after serde roundtrip".into());
    }
    Ok(())
}
