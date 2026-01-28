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

fn write_file(path: &Path, contents: &str) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, contents.as_bytes())?;
    Ok(())
}

#[test]
fn filesystem_runpack_store_roundtrips() -> Result<(), Box<dyn std::error::Error>> {
    let root = TempDir::new()?;
    let store = FilesystemRunpackStore::new(root.path().to_path_buf());
    let source = TempDir::new()?;
    write_file(&source.path().join("manifest.json"), "manifest")?;
    write_file(&source.path().join("logs/decisions.json"), "decisions")?;

    let key = RunpackKey {
        tenant_id: TenantId::new("tenant-1"),
        namespace_id: NamespaceId::new("default"),
        run_id: RunId::new("run-1"),
    };
    store.put_dir(&key, source.path())?;

    let dest = TempDir::new()?;
    store.get_dir(&key, dest.path())?;
    let manifest = fs::read_to_string(dest.path().join("manifest.json"))?;
    let decisions = fs::read_to_string(dest.path().join("logs/decisions.json"))?;
    if manifest != "manifest" {
        return Err("manifest content mismatch".into());
    }
    if decisions != "decisions" {
        return Err("decisions content mismatch".into());
    }
    Ok(())
}

#[test]
fn filesystem_runpack_store_rejects_invalid_key() -> Result<(), Box<dyn std::error::Error>> {
    let root = TempDir::new()?;
    let store = FilesystemRunpackStore::new(root.path().to_path_buf());
    let source = TempDir::new()?;
    write_file(&source.path().join("manifest.json"), "manifest")?;
    let key = RunpackKey {
        tenant_id: TenantId::new("bad/tenant"),
        namespace_id: NamespaceId::new("default"),
        run_id: RunId::new("run-1"),
    };
    let result = store.put_dir(&key, source.path());
    if result.is_ok() {
        return Err("expected invalid key to fail".into());
    }
    Ok(())
}

#[test]
fn filesystem_runpack_store_missing_runpack_fails() -> Result<(), Box<dyn std::error::Error>> {
    let root = TempDir::new()?;
    let store = FilesystemRunpackStore::new(root.path().to_path_buf());
    let dest = TempDir::new()?;
    let key = RunpackKey {
        tenant_id: TenantId::new("tenant-1"),
        namespace_id: NamespaceId::new("default"),
        run_id: RunId::new("missing"),
    };
    let result = store.get_dir(&key, dest.path());
    if result.is_ok() {
        return Err("expected missing runpack to fail".into());
    }
    Ok(())
}

// ============================================================================
// New segment validation tests
// ============================================================================

#[test]
fn runpack_validate_segment_empty_string() -> Result<(), Box<dyn std::error::Error>> {
    let root = TempDir::new()?;
    let store = FilesystemRunpackStore::new(root.path().to_path_buf());
    let source = TempDir::new()?;
    write_file(&source.path().join("manifest.json"), "manifest")?;
    let key = RunpackKey {
        tenant_id: TenantId::new(""),
        namespace_id: NamespaceId::new("default"),
        run_id: RunId::new("run-1"),
    };
    let result = store.put_dir(&key, source.path());
    if result.is_ok() {
        return Err("expected empty tenant id to fail".into());
    }
    Ok(())
}

#[test]
fn runpack_validate_segment_dot() -> Result<(), Box<dyn std::error::Error>> {
    let root = TempDir::new()?;
    let store = FilesystemRunpackStore::new(root.path().to_path_buf());
    let source = TempDir::new()?;
    write_file(&source.path().join("manifest.json"), "manifest")?;
    let key = RunpackKey {
        tenant_id: TenantId::new("."),
        namespace_id: NamespaceId::new("default"),
        run_id: RunId::new("run-1"),
    };
    let result = store.put_dir(&key, source.path());
    if result.is_ok() {
        return Err("expected '.' segment to fail".into());
    }
    Ok(())
}

#[test]
fn runpack_validate_segment_dotdot() -> Result<(), Box<dyn std::error::Error>> {
    let root = TempDir::new()?;
    let store = FilesystemRunpackStore::new(root.path().to_path_buf());
    let source = TempDir::new()?;
    write_file(&source.path().join("manifest.json"), "manifest")?;
    let key = RunpackKey {
        tenant_id: TenantId::new(".."),
        namespace_id: NamespaceId::new("default"),
        run_id: RunId::new("run-1"),
    };
    let result = store.put_dir(&key, source.path());
    if result.is_ok() {
        return Err("expected '..' segment to fail".into());
    }
    Ok(())
}

#[test]
fn runpack_validate_segment_overlength() -> Result<(), Box<dyn std::error::Error>> {
    let root = TempDir::new()?;
    let store = FilesystemRunpackStore::new(root.path().to_path_buf());
    let source = TempDir::new()?;
    write_file(&source.path().join("manifest.json"), "manifest")?;
    let key = RunpackKey {
        tenant_id: TenantId::new("a".repeat(256)),
        namespace_id: NamespaceId::new("default"),
        run_id: RunId::new("run-1"),
    };
    let result = store.put_dir(&key, source.path());
    if result.is_ok() {
        return Err("expected overlength segment to fail".into());
    }
    Ok(())
}

#[test]
fn runpack_validate_segment_backslash() -> Result<(), Box<dyn std::error::Error>> {
    let root = TempDir::new()?;
    let store = FilesystemRunpackStore::new(root.path().to_path_buf());
    let source = TempDir::new()?;
    write_file(&source.path().join("manifest.json"), "manifest")?;
    let key = RunpackKey {
        tenant_id: TenantId::new("bad\\seg"),
        namespace_id: NamespaceId::new("default"),
        run_id: RunId::new("run-1"),
    };
    let result = store.put_dir(&key, source.path());
    if result.is_ok() {
        return Err("expected backslash segment to fail".into());
    }
    Ok(())
}

#[test]
fn runpack_put_dir_overwrite_replaces() -> Result<(), Box<dyn std::error::Error>> {
    let root = TempDir::new()?;
    let store = FilesystemRunpackStore::new(root.path().to_path_buf());

    let key = RunpackKey {
        tenant_id: TenantId::new("tenant-1"),
        namespace_id: NamespaceId::new("default"),
        run_id: RunId::new("run-overwrite"),
    };

    // First put with v1 content.
    let source_v1 = TempDir::new()?;
    write_file(&source_v1.path().join("data.txt"), "v1")?;
    store.put_dir(&key, source_v1.path())?;

    // Second put with v2 content.
    let source_v2 = TempDir::new()?;
    write_file(&source_v2.path().join("data.txt"), "v2")?;
    store.put_dir(&key, source_v2.path())?;

    // Get and verify v2.
    let dest = TempDir::new()?;
    store.get_dir(&key, dest.path())?;
    let content = fs::read_to_string(dest.path().join("data.txt"))?;
    if content != "v2" {
        return Err("expected overwrite to preserve latest content".into());
    }
    Ok(())
}

#[test]
fn runpack_copy_dir_recursive_nested_3_levels() -> Result<(), Box<dyn std::error::Error>> {
    let root = TempDir::new()?;
    let store = FilesystemRunpackStore::new(root.path().to_path_buf());

    let source = TempDir::new()?;
    write_file(&source.path().join("a/b/c/file.txt"), "deep content")?;

    let key = RunpackKey {
        tenant_id: TenantId::new("tenant-1"),
        namespace_id: NamespaceId::new("default"),
        run_id: RunId::new("run-nested"),
    };
    store.put_dir(&key, source.path())?;

    let dest = TempDir::new()?;
    store.get_dir(&key, dest.path())?;
    let content = fs::read_to_string(dest.path().join("a/b/c/file.txt"))?;
    if content != "deep content" {
        return Err("nested content mismatch".into());
    }
    Ok(())
}
