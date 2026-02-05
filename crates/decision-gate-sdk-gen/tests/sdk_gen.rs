// crates/decision-gate-sdk-gen/tests/sdk_gen.rs
// ============================================================================
// Module: SDK Generator Tests
// Description: Integration tests for SDK output drift and input limits.
// Purpose: Validate generated artifacts and tooling.json size bounds.
// Dependencies: decision-gate-sdk-gen
// ============================================================================

//! ## Overview
//! Integration tests covering generator drift checks and input size limits.
//!
//! ### Security Posture
//! These tests exercise size limits to ensure untrusted tooling inputs are
//! bounded and fail closed. See `Docs/security/threat_model.md`.

use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;

use decision_gate_sdk_gen::DEFAULT_TOOLING_PATH;
use decision_gate_sdk_gen::MAX_TOOLING_BYTES;
use decision_gate_sdk_gen::SdkGenError;
use decision_gate_sdk_gen::SdkGenerator;

// ============================================================================
// SECTION: Test Helpers
// ============================================================================

// ============================================================================
// CONSTANTS: Temporary file tracking
// ============================================================================

static TEMP_COUNTER: AtomicUsize = AtomicUsize::new(0);

struct TempFile {
    path: PathBuf,
}

impl TempFile {
    fn new(label: &str) -> Self {
        let mut path = std::env::temp_dir();
        let attempt = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
        path.push(format!("decision-gate-sdk-gen-{label}-{}-{}.json", std::process::id(), attempt));
        Self {
            path,
        }
    }
}

impl Drop for TempFile {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

fn workspace_root() -> Result<PathBuf, SdkGenError> {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let root = manifest
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .ok_or_else(|| SdkGenError::Tooling("missing workspace root".to_string()))?;
    Ok(root)
}

fn read_string(path: &Path) -> Result<String, SdkGenError> {
    fs::read_to_string(path).map_err(|err| SdkGenError::Io(err.to_string()))
}

// ============================================================================
// SECTION: Tests
// ============================================================================

#[test]
fn python_sdk_matches_generated_output() -> Result<(), SdkGenError> {
    let root = workspace_root()?;
    let tooling_path = root.join(DEFAULT_TOOLING_PATH);
    let generator = SdkGenerator::load(tooling_path)?;
    let rendered = generator.generate_python()?;
    let expected_path = root.join("sdks/python/decision_gate/_generated.py");
    let expected = read_string(&expected_path)?;
    if rendered != expected {
        return Err(SdkGenError::Tooling(
            "Python SDK drift detected. Run decision-gate-sdk-gen generate.".to_string(),
        ));
    }
    Ok(())
}

#[test]
fn typescript_sdk_matches_generated_output() -> Result<(), SdkGenError> {
    let root = workspace_root()?;
    let tooling_path = root.join(DEFAULT_TOOLING_PATH);
    let generator = SdkGenerator::load(tooling_path)?;
    let rendered = generator.generate_typescript()?;
    let expected_path = root.join("sdks/typescript/src/_generated.ts");
    let expected = read_string(&expected_path)?;
    if rendered != expected {
        return Err(SdkGenError::Tooling(
            "TypeScript SDK drift detected. Run decision-gate-sdk-gen generate.".to_string(),
        ));
    }
    Ok(())
}

#[test]
fn openapi_matches_generated_output() -> Result<(), SdkGenError> {
    let root = workspace_root()?;
    let tooling_path = root.join(DEFAULT_TOOLING_PATH);
    let generator = SdkGenerator::load(tooling_path)?;
    let rendered = generator.generate_openapi()?;
    let expected_path = root.join("Docs/generated/openapi/decision-gate.json");
    let expected = read_string(&expected_path)?;
    if rendered != expected {
        return Err(SdkGenError::Tooling(
            "OpenAPI drift detected. Run decision-gate-sdk-gen generate.".to_string(),
        ));
    }
    Ok(())
}

#[test]
fn tooling_input_enforces_size_limit() -> Result<(), Box<dyn std::error::Error>> {
    let temp = TempFile::new("tooling-limit");
    let size = usize::try_from(MAX_TOOLING_BYTES + 1)?;
    let payload = vec![b'a'; size];
    fs::write(&temp.path, payload)?;
    let result = SdkGenerator::load(&temp.path);
    match result {
        Err(SdkGenError::Tooling(_)) => Ok(()),
        Ok(_) => Err(std::io::Error::other("expected tooling size limit error").into()),
        Err(other) => Err(std::io::Error::other(format!("unexpected error: {other}")).into()),
    }
}
