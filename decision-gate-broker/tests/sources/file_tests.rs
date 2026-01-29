// decision-gate-broker/tests/sources/file_tests.rs
// ============================================================================
// Module: FileSource Unit Tests
// Description: Comprehensive tests for the file-backed payload source.
// Purpose: Validate file source path handling and size enforcement.
// Dependencies: decision-gate-broker, decision-gate-core, tempfile, url
// ============================================================================

//! ## Overview
//! Exercises [`decision_gate_broker::FileSource`] file resolution paths.

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

use decision_gate_broker::FileSource;
use decision_gate_broker::MAX_SOURCE_BYTES;
use decision_gate_broker::Source;
use decision_gate_broker::SourceError;
use decision_gate_core::ContentRef;
use decision_gate_core::hashing::DEFAULT_HASH_ALGORITHM;
use decision_gate_core::hashing::hash_bytes;
use tempfile::tempdir;
use url::Url;

// ============================================================================
// SECTION: Constructor Tests
// ============================================================================

/// Tests file source new creates rooted source.
#[test]
fn file_source_new_creates_rooted_source() {
    let dir = tempdir().expect("temp dir");
    let _source = FileSource::new(dir.path());
    // Source created successfully - validates constructor works
}

/// Tests file source unrestricted creates unrooted source.
#[test]
fn file_source_unrestricted_creates_unrooted_source() {
    let _source = FileSource::unrestricted();
    // Source created successfully - validates unrestricted constructor works
}

// ============================================================================
// SECTION: Success Path Tests
// ============================================================================

/// Tests file source reads bytes from valid file.
#[test]
fn file_source_reads_bytes_from_valid_file() {
    let dir = tempdir().expect("temp dir");
    let path = dir.path().join("payload.bin");
    let content = b"hello world";
    std::fs::write(&path, content).expect("write file");

    let uri = Url::from_file_path(&path).expect("file url").to_string();
    let content_hash = hash_bytes(DEFAULT_HASH_ALGORITHM, content);
    let content_ref = ContentRef {
        uri,
        content_hash,
        encryption: None,
    };

    let source = FileSource::new(dir.path());
    let payload = source.fetch(&content_ref).expect("file fetch");

    assert_eq!(payload.bytes, content);
    assert!(payload.content_type.is_none());
}

/// Tests file source reads empty file.
#[test]
fn file_source_reads_empty_file() {
    let dir = tempdir().expect("temp dir");
    let path = dir.path().join("empty.bin");
    std::fs::write(&path, b"").expect("write file");

    let uri = Url::from_file_path(&path).expect("file url").to_string();
    let content_hash = hash_bytes(DEFAULT_HASH_ALGORITHM, b"");
    let content_ref = ContentRef {
        uri,
        content_hash,
        encryption: None,
    };

    let source = FileSource::new(dir.path());
    let payload = source.fetch(&content_ref).expect("file fetch");

    assert!(payload.bytes.is_empty());
}

/// Tests file source reads large file.
#[test]
fn file_source_reads_large_file() {
    let dir = tempdir().expect("temp dir");
    let path = dir.path().join("large.bin");
    let content: Vec<u8> =
        (0 .. 10000).map(|i| u8::try_from(i % 256).expect("u8 conversion")).collect();
    std::fs::write(&path, &content).expect("write file");

    let uri = Url::from_file_path(&path).expect("file url").to_string();
    let content_hash = hash_bytes(DEFAULT_HASH_ALGORITHM, &content);
    let content_ref = ContentRef {
        uri,
        content_hash,
        encryption: None,
    };

    let source = FileSource::new(dir.path());
    let payload = source.fetch(&content_ref).expect("file fetch");

    assert_eq!(payload.bytes, content);
}

/// Tests file source rejects oversized files.
#[test]
fn file_source_rejects_oversized_file() {
    let dir = tempdir().expect("temp dir");
    let path = dir.path().join("oversized.bin");
    let content = vec![0_u8; MAX_SOURCE_BYTES + 1];
    std::fs::write(&path, &content).expect("write file");

    let uri = Url::from_file_path(&path).expect("file url").to_string();
    let content_hash = hash_bytes(DEFAULT_HASH_ALGORITHM, &content);
    let content_ref = ContentRef {
        uri,
        content_hash,
        encryption: None,
    };

    let source = FileSource::new(dir.path());
    let result = source.fetch(&content_ref);

    assert!(matches!(result, Err(SourceError::TooLarge { .. })));
}

/// Tests file source unrestricted reads any path.
#[test]
fn file_source_unrestricted_reads_any_path() {
    let dir = tempdir().expect("temp dir");
    let path = dir.path().join("unrestricted.bin");
    let content = b"unrestricted content";
    std::fs::write(&path, content).expect("write file");

    let uri = Url::from_file_path(&path).expect("file url").to_string();
    let content_hash = hash_bytes(DEFAULT_HASH_ALGORITHM, content);
    let content_ref = ContentRef {
        uri,
        content_hash,
        encryption: None,
    };

    let source = FileSource::unrestricted();
    let payload = source.fetch(&content_ref).expect("file fetch");

    assert_eq!(payload.bytes, content);
}

// ============================================================================
// SECTION: Error Path Tests
// ============================================================================

/// Tests file source rejects file not found.
#[test]
fn file_source_rejects_file_not_found() {
    let dir = tempdir().expect("temp dir");
    let nonexistent_path = dir.path().join("nonexistent.bin");

    let uri = Url::from_file_path(&nonexistent_path).expect("file url").to_string();
    let content_hash = hash_bytes(DEFAULT_HASH_ALGORITHM, b"phantom");
    let content_ref = ContentRef {
        uri,
        content_hash,
        encryption: None,
    };

    let source = FileSource::new(dir.path());
    let err = source.fetch(&content_ref).unwrap_err();

    assert!(matches!(err, SourceError::NotFound(_)));
}

/// Tests file source rejects path traversal attack.
#[test]
fn file_source_rejects_path_traversal_attack() {
    let dir = tempdir().expect("temp dir");
    let safe_subdir = dir.path().join("safe");
    std::fs::create_dir(&safe_subdir).expect("create subdir");

    // Create a file outside the safe directory
    let unsafe_path = dir.path().join("secret.txt");
    std::fs::write(&unsafe_path, b"secret").expect("write secret");

    // Try to access it via path traversal
    let traversal_path = safe_subdir.join("..").join("secret.txt");
    let uri = Url::from_file_path(&traversal_path).expect("file url").to_string();
    let content_hash = hash_bytes(DEFAULT_HASH_ALGORITHM, b"secret");
    let content_ref = ContentRef {
        uri,
        content_hash,
        encryption: None,
    };

    let source = FileSource::new(&safe_subdir);
    let err = source.fetch(&content_ref).unwrap_err();

    assert!(matches!(err, SourceError::InvalidUri(_)));
    assert!(err.to_string().contains("escapes configured root"));
}

/// Tests file source rejects non file scheme.
#[test]
fn file_source_rejects_non_file_scheme() {
    let dir = tempdir().expect("temp dir");
    let content_hash = hash_bytes(DEFAULT_HASH_ALGORITHM, b"data");
    let content_ref = ContentRef {
        uri: "http://example.com/file.bin".to_string(),
        content_hash,
        encryption: None,
    };

    let source = FileSource::new(dir.path());
    let err = source.fetch(&content_ref).unwrap_err();

    assert!(matches!(err, SourceError::UnsupportedScheme(_)));
    assert!(err.to_string().contains("http"));
}

/// Tests file source rejects malformed uri.
#[test]
fn file_source_rejects_malformed_uri() {
    let dir = tempdir().expect("temp dir");
    let content_hash = hash_bytes(DEFAULT_HASH_ALGORITHM, b"data");
    let content_ref = ContentRef {
        uri: "not a valid uri at all".to_string(),
        content_hash,
        encryption: None,
    };

    let source = FileSource::new(dir.path());
    let err = source.fetch(&content_ref).unwrap_err();

    assert!(matches!(err, SourceError::InvalidUri(_)));
}

/// Tests file source rejects directory as file.
#[test]
fn file_source_rejects_directory_as_file() {
    let dir = tempdir().expect("temp dir");
    let subdir = dir.path().join("subdir");
    std::fs::create_dir(&subdir).expect("create subdir");

    let uri = Url::from_file_path(&subdir).expect("file url").to_string();
    let content_hash = hash_bytes(DEFAULT_HASH_ALGORITHM, b"phantom");
    let content_ref = ContentRef {
        uri,
        content_hash,
        encryption: None,
    };

    let source = FileSource::new(dir.path());
    let err = source.fetch(&content_ref).unwrap_err();

    // Reading a directory should fail with IO error
    assert!(matches!(err, SourceError::Io(_)));
}

// ============================================================================
// SECTION: Edge Case Tests
// ============================================================================

/// Tests file source handles special characters in filename.
#[test]
fn file_source_handles_special_characters_in_filename() {
    let dir = tempdir().expect("temp dir");
    let path = dir.path().join("file with spaces & special.bin");
    let content = b"special chars";
    std::fs::write(&path, content).expect("write file");

    let uri = Url::from_file_path(&path).expect("file url").to_string();
    let content_hash = hash_bytes(DEFAULT_HASH_ALGORITHM, content);
    let content_ref = ContentRef {
        uri,
        content_hash,
        encryption: None,
    };

    let source = FileSource::new(dir.path());
    let payload = source.fetch(&content_ref).expect("file fetch");

    assert_eq!(payload.bytes, content);
}

/// Tests file source handles nested subdirectories.
#[test]
fn file_source_handles_nested_subdirectories() {
    let dir = tempdir().expect("temp dir");
    let nested = dir.path().join("a").join("b").join("c");
    std::fs::create_dir_all(&nested).expect("create nested dirs");
    let path = nested.join("deep.bin");
    let content = b"deeply nested";
    std::fs::write(&path, content).expect("write file");

    let uri = Url::from_file_path(&path).expect("file url").to_string();
    let content_hash = hash_bytes(DEFAULT_HASH_ALGORITHM, content);
    let content_ref = ContentRef {
        uri,
        content_hash,
        encryption: None,
    };

    let source = FileSource::new(dir.path());
    let payload = source.fetch(&content_ref).expect("file fetch");

    assert_eq!(payload.bytes, content);
}
