// enterprise/decision-gate-store-enterprise/tests/runpack_store.rs
// ============================================================================
// Module: Runpack Store Tests
// Description: Unit tests for filesystem runpack store behavior.
// Purpose: Validate copy, retrieval, and error handling semantics.
// ============================================================================

//! Runpack store unit tests.

use std::fs;
use std::path::Path;

use decision_gate_core::NamespaceId;
use decision_gate_core::RunId;
use decision_gate_core::TenantId;
use decision_gate_store_enterprise::runpack_store::FilesystemRunpackStore;
use decision_gate_store_enterprise::runpack_store::RunpackKey;
use decision_gate_store_enterprise::runpack_store::RunpackStore;
use tempfile::TempDir;

fn write_file(path: &Path, contents: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("create dir");
    }
    fs::write(path, contents.as_bytes()).expect("write file");
}

#[test]
fn filesystem_runpack_store_roundtrips() {
    let root = TempDir::new().expect("root");
    let store = FilesystemRunpackStore::new(root.path().to_path_buf());
    let source = TempDir::new().expect("source");
    write_file(&source.path().join("manifest.json"), "manifest");
    write_file(&source.path().join("logs/decisions.json"), "decisions");

    let key = RunpackKey {
        tenant_id: TenantId::new("tenant-1"),
        namespace_id: NamespaceId::new("default"),
        run_id: RunId::new("run-1"),
    };
    store.put_dir(&key, source.path()).expect("put dir");

    let dest = TempDir::new().expect("dest");
    store.get_dir(&key, dest.path()).expect("get dir");
    let manifest = fs::read_to_string(dest.path().join("manifest.json")).expect("manifest");
    let decisions = fs::read_to_string(dest.path().join("logs/decisions.json")).expect("decisions");
    assert_eq!(manifest, "manifest");
    assert_eq!(decisions, "decisions");
}

#[test]
fn filesystem_runpack_store_rejects_invalid_key() {
    let root = TempDir::new().expect("root");
    let store = FilesystemRunpackStore::new(root.path().to_path_buf());
    let source = TempDir::new().expect("source");
    write_file(&source.path().join("manifest.json"), "manifest");
    let key = RunpackKey {
        tenant_id: TenantId::new("bad/tenant"),
        namespace_id: NamespaceId::new("default"),
        run_id: RunId::new("run-1"),
    };
    let result = store.put_dir(&key, source.path());
    assert!(result.is_err());
}

#[test]
fn filesystem_runpack_store_missing_runpack_fails() {
    let root = TempDir::new().expect("root");
    let store = FilesystemRunpackStore::new(root.path().to_path_buf());
    let dest = TempDir::new().expect("dest");
    let key = RunpackKey {
        tenant_id: TenantId::new("tenant-1"),
        namespace_id: NamespaceId::new("default"),
        run_id: RunId::new("missing"),
    };
    let result = store.get_dir(&key, dest.path());
    assert!(result.is_err());
}

// ============================================================================
// New segment validation tests
// ============================================================================

#[test]
fn runpack_validate_segment_empty_string() {
    let root = TempDir::new().expect("root");
    let store = FilesystemRunpackStore::new(root.path().to_path_buf());
    let source = TempDir::new().expect("source");
    write_file(&source.path().join("manifest.json"), "manifest");
    let key = RunpackKey {
        tenant_id: TenantId::new(""),
        namespace_id: NamespaceId::new("default"),
        run_id: RunId::new("run-1"),
    };
    let result = store.put_dir(&key, source.path());
    assert!(result.is_err());
}

#[test]
fn runpack_validate_segment_dot() {
    let root = TempDir::new().expect("root");
    let store = FilesystemRunpackStore::new(root.path().to_path_buf());
    let source = TempDir::new().expect("source");
    write_file(&source.path().join("manifest.json"), "manifest");
    let key = RunpackKey {
        tenant_id: TenantId::new("."),
        namespace_id: NamespaceId::new("default"),
        run_id: RunId::new("run-1"),
    };
    let result = store.put_dir(&key, source.path());
    assert!(result.is_err());
}

#[test]
fn runpack_validate_segment_dotdot() {
    let root = TempDir::new().expect("root");
    let store = FilesystemRunpackStore::new(root.path().to_path_buf());
    let source = TempDir::new().expect("source");
    write_file(&source.path().join("manifest.json"), "manifest");
    let key = RunpackKey {
        tenant_id: TenantId::new(".."),
        namespace_id: NamespaceId::new("default"),
        run_id: RunId::new("run-1"),
    };
    let result = store.put_dir(&key, source.path());
    assert!(result.is_err());
}

#[test]
fn runpack_validate_segment_overlength() {
    let root = TempDir::new().expect("root");
    let store = FilesystemRunpackStore::new(root.path().to_path_buf());
    let source = TempDir::new().expect("source");
    write_file(&source.path().join("manifest.json"), "manifest");
    let key = RunpackKey {
        tenant_id: TenantId::new(&"a".repeat(256)),
        namespace_id: NamespaceId::new("default"),
        run_id: RunId::new("run-1"),
    };
    let result = store.put_dir(&key, source.path());
    assert!(result.is_err());
}

#[test]
fn runpack_validate_segment_backslash() {
    let root = TempDir::new().expect("root");
    let store = FilesystemRunpackStore::new(root.path().to_path_buf());
    let source = TempDir::new().expect("source");
    write_file(&source.path().join("manifest.json"), "manifest");
    let key = RunpackKey {
        tenant_id: TenantId::new("bad\\seg"),
        namespace_id: NamespaceId::new("default"),
        run_id: RunId::new("run-1"),
    };
    let result = store.put_dir(&key, source.path());
    assert!(result.is_err());
}

#[test]
fn runpack_put_dir_overwrite_replaces() {
    let root = TempDir::new().expect("root");
    let store = FilesystemRunpackStore::new(root.path().to_path_buf());

    let key = RunpackKey {
        tenant_id: TenantId::new("tenant-1"),
        namespace_id: NamespaceId::new("default"),
        run_id: RunId::new("run-overwrite"),
    };

    // First put with v1 content.
    let source_v1 = TempDir::new().expect("source v1");
    write_file(&source_v1.path().join("data.txt"), "v1");
    store.put_dir(&key, source_v1.path()).expect("put v1");

    // Second put with v2 content.
    let source_v2 = TempDir::new().expect("source v2");
    write_file(&source_v2.path().join("data.txt"), "v2");
    store.put_dir(&key, source_v2.path()).expect("put v2");

    // Get and verify v2.
    let dest = TempDir::new().expect("dest");
    store.get_dir(&key, dest.path()).expect("get dir");
    let content = fs::read_to_string(dest.path().join("data.txt")).expect("read data");
    assert_eq!(content, "v2");
}

#[test]
fn runpack_copy_dir_recursive_nested_3_levels() {
    let root = TempDir::new().expect("root");
    let store = FilesystemRunpackStore::new(root.path().to_path_buf());

    let source = TempDir::new().expect("source");
    write_file(&source.path().join("a/b/c/file.txt"), "deep content");

    let key = RunpackKey {
        tenant_id: TenantId::new("tenant-1"),
        namespace_id: NamespaceId::new("default"),
        run_id: RunId::new("run-nested"),
    };
    store.put_dir(&key, source.path()).expect("put dir");

    let dest = TempDir::new().expect("dest");
    store.get_dir(&key, dest.path()).expect("get dir");
    let content = fs::read_to_string(dest.path().join("a/b/c/file.txt")).expect("read nested file");
    assert_eq!(content, "deep content");
}
