//! Enterprise S3 runpack store system tests.
// enterprise-system-tests/tests/s3_runpack_store.rs
// ============================================================================
// Module: S3 Runpack Store Tests
// Description: Validate S3-backed runpack storage integrity and hardening.
// Purpose: Ensure object storage isolation, encryption, and tamper detection.
// Dependencies: enterprise system-test helpers
// ============================================================================

mod helpers;

use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::Path;

use aws_sdk_s3::primitives::ByteStream;
use aws_sdk_s3::types::MetadataDirective;
use aws_sdk_s3::types::ServerSideEncryption;
use decision_gate_core::NamespaceId;
use decision_gate_core::RunId;
use decision_gate_core::TenantId;
use decision_gate_core::hashing::HashAlgorithm;
use decision_gate_core::hashing::hash_bytes;
use decision_gate_store_enterprise::runpack_store::RunpackKey;
use decision_gate_store_enterprise::runpack_store::RunpackStore;
use decision_gate_store_enterprise::runpack_store::RunpackStoreError;
use decision_gate_store_enterprise::s3_runpack_store::S3RunpackStore;
use decision_gate_store_enterprise::s3_runpack_store::S3RunpackStoreConfig;
use decision_gate_store_enterprise::s3_runpack_store::S3ServerSideEncryption;
use helpers::artifacts::TestReporter;
use helpers::infra::S3Fixture;
use helpers::infra::ensure_bucket_policy_enforces_sse;
use helpers::infra::head_object_sse;
use tar::EntryType;
use tempfile::NamedTempFile;
use tempfile::TempDir;

const RUNPACK_ARCHIVE_NAME: &str = "runpack.tar";

#[tokio::test(flavor = "multi_thread")]
async fn s3_runpack_store_roundtrip() -> Result<(), Box<dyn std::error::Error>> {
    let mut reporter = TestReporter::new("s3_runpack_store_roundtrip")?;

    let s3 = S3Fixture::start().await?;
    set_s3_env(&s3);
    let store = S3RunpackStore::new(store_config(&s3, "roundtrip", None, None))?;

    let source_dir = TempDir::new()?;
    let nested_dir = source_dir.path().join("nested");
    fs::create_dir_all(&nested_dir)?;
    fs::write(source_dir.path().join("a.txt"), b"alpha")?;
    fs::write(nested_dir.join("b.txt"), b"bravo")?;

    let key = runpack_key("tenant-1", "default", "run-1");
    store.put_dir(&key, source_dir.path())?;

    let dest_dir = TempDir::new()?;
    store.get_dir(&key, dest_dir.path())?;

    let alpha = fs::read(dest_dir.path().join("a.txt"))?;
    let bravo = fs::read(dest_dir.path().join("nested").join("b.txt"))?;
    if alpha != b"alpha" || bravo != b"bravo" {
        return Err("runpack roundtrip content mismatch".into());
    }

    reporter.artifacts().write_json("tool_transcript.json", &Vec::<serde_json::Value>::new())?;
    reporter.finish(
        "pass",
        vec!["s3 runpack roundtrip succeeded".to_string()],
        vec![
            "summary.json".to_string(),
            "summary.md".to_string(),
            "tool_transcript.json".to_string(),
        ],
    )?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn s3_runpack_encryption_enforced() -> Result<(), Box<dyn std::error::Error>> {
    let mut reporter = TestReporter::new("s3_runpack_encryption_enforced")?;

    let s3 = S3Fixture::start().await?;
    set_s3_env(&s3);
    let client = s3.client().await?;
    ensure_bucket_policy_enforces_sse(&client, &s3.bucket).await?;

    let store =
        S3RunpackStore::new(store_config(&s3, "sse", Some(S3ServerSideEncryption::Aes256), None))?;
    let source_dir = TempDir::new()?;
    fs::write(source_dir.path().join("a.txt"), b"alpha")?;
    let key = runpack_key("tenant-1", "default", "run-1");
    store.put_dir(&key, source_dir.path())?;

    let object_key = runpack_object_key("sse", &key);
    let sse = head_object_sse(&client, &s3.bucket, &object_key).await?;
    if sse != Some(ServerSideEncryption::Aes256) {
        return Err("expected SSE-S3 metadata on runpack object".into());
    }

    let insecure_store = S3RunpackStore::new(store_config(&s3, "sse-deny", None, None))?;
    let deny_dir = TempDir::new()?;
    fs::write(deny_dir.path().join("b.txt"), b"bravo")?;
    let deny_key = runpack_key("tenant-1", "default", "run-2");
    let result = insecure_store.put_dir(&deny_key, deny_dir.path());
    if result.is_ok() {
        return Err("expected SSE bucket policy to reject unencrypted upload".into());
    }

    reporter.artifacts().write_json("tool_transcript.json", &Vec::<serde_json::Value>::new())?;
    reporter.finish(
        "pass",
        vec!["s3 SSE enforcement verified".to_string()],
        vec![
            "summary.json".to_string(),
            "summary.md".to_string(),
            "tool_transcript.json".to_string(),
        ],
    )?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn s3_runpack_metadata_tamper() -> Result<(), Box<dyn std::error::Error>> {
    let mut reporter = TestReporter::new("s3_runpack_metadata_tamper")?;

    let s3 = S3Fixture::start().await?;
    set_s3_env(&s3);
    let client = s3.client().await?;
    let store = S3RunpackStore::new(store_config(&s3, "tamper", None, None))?;

    let source_dir = TempDir::new()?;
    fs::write(source_dir.path().join("a.txt"), b"alpha")?;
    let key = runpack_key("tenant-1", "default", "run-1");
    store.put_dir(&key, source_dir.path())?;

    let object_key = runpack_object_key("tamper", &key);
    client
        .copy_object()
        .bucket(s3.bucket.clone())
        .key(object_key.clone())
        .copy_source(format!("{}/{}", s3.bucket, object_key))
        .metadata_directive(MetadataDirective::Replace)
        .set_metadata(Some(HashMap::from([("sha256".to_string(), "deadbeef".to_string())])))
        .send()
        .await?;

    let dest_dir = TempDir::new()?;
    let result = store.get_dir(&key, dest_dir.path());
    match result {
        Err(RunpackStoreError::Invalid(_)) => {}
        Err(err) => return Err(format!("unexpected error: {err}").into()),
        Ok(_) => return Err("expected metadata tamper detection".into()),
    }

    reporter.artifacts().write_json("tool_transcript.json", &Vec::<serde_json::Value>::new())?;
    reporter.finish(
        "pass",
        vec!["s3 runpack metadata tamper detected".to_string()],
        vec![
            "summary.json".to_string(),
            "summary.md".to_string(),
            "tool_transcript.json".to_string(),
        ],
    )?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn runpack_archive_path_traversal() -> Result<(), Box<dyn std::error::Error>> {
    let mut reporter = TestReporter::new("runpack_archive_path_traversal")?;

    let s3 = S3Fixture::start().await?;
    set_s3_env(&s3);
    let store = S3RunpackStore::new(store_config(&s3, "traversal", None, None))?;
    let client = s3.client().await?;

    let temp = build_tar_with_path("../evil", EntryType::Regular, Some(b"malicious"))?;
    let digest = digest_hex(temp.path())?;
    let object_key = runpack_object_key("traversal", &runpack_key("tenant-1", "default", "run-1"));
    put_object_with_hash(&client, &s3.bucket, &object_key, temp.path(), &digest).await?;

    let dest_dir = TempDir::new()?;
    let result = store.get_dir(&runpack_key("tenant-1", "default", "run-1"), dest_dir.path());
    if result.is_ok() {
        return Err("expected path traversal rejection".into());
    }

    reporter.artifacts().write_json("tool_transcript.json", &Vec::<serde_json::Value>::new())?;
    reporter.finish(
        "pass",
        vec!["runpack archive traversal rejected".to_string()],
        vec![
            "summary.json".to_string(),
            "summary.md".to_string(),
            "tool_transcript.json".to_string(),
        ],
    )?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn runpack_archive_symlink_specials() -> Result<(), Box<dyn std::error::Error>> {
    let mut reporter = TestReporter::new("runpack_archive_symlink_specials")?;

    let s3 = S3Fixture::start().await?;
    set_s3_env(&s3);
    let store = S3RunpackStore::new(store_config(&s3, "symlink", None, None))?;
    let client = s3.client().await?;

    let temp = build_tar_with_path("link", EntryType::Symlink, None)?;
    let digest = digest_hex(temp.path())?;
    let object_key = runpack_object_key("symlink", &runpack_key("tenant-1", "default", "run-1"));
    put_object_with_hash(&client, &s3.bucket, &object_key, temp.path(), &digest).await?;

    let dest_dir = TempDir::new()?;
    let result = store.get_dir(&runpack_key("tenant-1", "default", "run-1"), dest_dir.path());
    if result.is_ok() {
        return Err("expected symlink/special entry rejection".into());
    }

    reporter.artifacts().write_json("tool_transcript.json", &Vec::<serde_json::Value>::new())?;
    reporter.finish(
        "pass",
        vec!["runpack archive symlink rejection enforced".to_string()],
        vec![
            "summary.json".to_string(),
            "summary.md".to_string(),
            "tool_transcript.json".to_string(),
        ],
    )?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn runpack_archive_size_limit() -> Result<(), Box<dyn std::error::Error>> {
    let mut reporter = TestReporter::new("runpack_archive_size_limit")?;

    let s3 = S3Fixture::start().await?;
    set_s3_env(&s3);
    let store = S3RunpackStore::new(store_config(&s3, "size-limit", None, Some(32)))?;

    let source_dir = TempDir::new()?;
    let mut file = fs::File::create(source_dir.path().join("payload.bin"))?;
    file.write_all(&vec![0u8; 128])?;

    let key = runpack_key("tenant-1", "default", "run-1");
    let result = store.put_dir(&key, source_dir.path());
    match result {
        Err(RunpackStoreError::Invalid(_)) => {}
        Err(err) => return Err(format!("unexpected error: {err}").into()),
        Ok(_) => return Err("expected archive size limit rejection".into()),
    }

    reporter.artifacts().write_json("tool_transcript.json", &Vec::<serde_json::Value>::new())?;
    reporter.finish(
        "pass",
        vec!["runpack archive size limit enforced".to_string()],
        vec![
            "summary.json".to_string(),
            "summary.md".to_string(),
            "tool_transcript.json".to_string(),
        ],
    )?;
    Ok(())
}

fn store_config(
    s3: &S3Fixture,
    prefix: &str,
    sse: Option<S3ServerSideEncryption>,
    max_archive_bytes: Option<u64>,
) -> S3RunpackStoreConfig {
    S3RunpackStoreConfig {
        bucket: s3.bucket.clone(),
        region: Some(s3.region.clone()),
        prefix: Some(prefix.to_string()),
        endpoint: Some(s3.endpoint.clone()),
        force_path_style: s3.force_path_style,
        server_side_encryption: sse,
        kms_key_id: None,
        max_archive_bytes,
    }
}

fn runpack_key(tenant: &str, namespace: &str, run: &str) -> RunpackKey {
    RunpackKey {
        tenant_id: TenantId::new(tenant),
        namespace_id: NamespaceId::new(namespace),
        run_id: RunId::new(run),
    }
}

fn runpack_object_key(prefix: &str, key: &RunpackKey) -> String {
    let trimmed = prefix.trim_matches('/');
    let mut path = String::new();
    if !trimmed.is_empty() {
        path.push_str(trimmed);
        path.push('/');
    }
    path.push_str(key.tenant_id.as_str());
    path.push('/');
    path.push_str(key.namespace_id.as_str());
    path.push('/');
    path.push_str(key.run_id.as_str());
    path.push('/');
    path.push_str(RUNPACK_ARCHIVE_NAME);
    path
}

fn set_s3_env(s3: &S3Fixture) {
    helpers::env::set_var("AWS_EC2_METADATA_DISABLED", "true");
    helpers::env::set_var("AWS_ACCESS_KEY_ID", &s3.access_key);
    helpers::env::set_var("AWS_SECRET_ACCESS_KEY", &s3.secret_key);
    helpers::env::set_var("AWS_REGION", &s3.region);
}

fn build_tar_with_path(
    path: &str,
    entry_type: EntryType,
    payload: Option<&[u8]>,
) -> Result<NamedTempFile, Box<dyn std::error::Error>> {
    let temp = NamedTempFile::new()?;
    let file = temp.reopen()?;
    let mut builder = tar::Builder::new(file);
    let mut header = tar::Header::new_gnu();
    let data = payload.unwrap_or(&[]);
    header.set_entry_type(entry_type);
    header.set_mode(0o644);
    header.set_size(u64::try_from(data.len())?);
    if entry_type == EntryType::Symlink {
        header.set_link_name("target")?;
    }
    header.set_cksum();
    builder.append_data(&mut header, path, data)?;
    builder.finish()?;
    Ok(temp)
}

fn digest_hex(path: &Path) -> Result<String, Box<dyn std::error::Error>> {
    let bytes = fs::read(path)?;
    let digest = hash_bytes(HashAlgorithm::Sha256, &bytes);
    Ok(digest.value)
}

async fn put_object_with_hash(
    client: &aws_sdk_s3::Client,
    bucket: &str,
    key: &str,
    path: &Path,
    hash: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let body = ByteStream::from_path(path).await?;
    client
        .put_object()
        .bucket(bucket)
        .key(key)
        .body(body)
        .set_metadata(Some(HashMap::from([("sha256".to_string(), hash.to_string())])))
        .content_type("application/x-tar")
        .send()
        .await?;
    Ok(())
}
