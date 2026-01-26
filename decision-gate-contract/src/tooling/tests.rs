// decision-gate-contract/src/tooling/tests.rs
// ============================================================================
// Module: Tooling Schema Unit Tests
// Description: Validates tool examples against their JSON schemas.
// Purpose: Ensure contract examples are kept in sync with schema definitions.
// Dependencies: decision-gate-contract
// ============================================================================

//! ## Overview
//! Verifies that tool input/output examples satisfy their JSON schemas.
//!
//! Security posture: Tests validate untrusted input contracts; see
//! `Docs/security/threat_model.md`.

// ============================================================================
// SECTION: Lint Configuration
// ============================================================================

#![allow(
    clippy::panic,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic_in_result_fn,
    clippy::unwrap_in_result,
    clippy::missing_docs_in_private_items,
    reason = "Test-only validation helpers use panic-based assertions for clarity."
)]

// ============================================================================
// SECTION: Imports
// ============================================================================

use std::collections::BTreeMap;
use std::io;
use std::sync::Arc;

use jsonschema::CompilationOptions;
use jsonschema::Draft;
use jsonschema::JSONSchema;
use jsonschema::SchemaResolver;
use jsonschema::SchemaResolverError;
use serde_json::Value;
use url::Url;

use super::tool_contracts;
use super::tool_examples;
use crate::schemas;

// ============================================================================
// SECTION: Fixtures
// ============================================================================

#[derive(Clone)]
struct ContractSchemaResolver {
    registry: Arc<BTreeMap<String, Value>>,
}

impl ContractSchemaResolver {
    fn new(registry: BTreeMap<String, Value>) -> Self {
        Self {
            registry: Arc::new(registry),
        }
    }
}

impl SchemaResolver for ContractSchemaResolver {
    fn resolve(
        &self,
        _root_schema: &Value,
        url: &Url,
        _original_reference: &str,
    ) -> Result<Arc<Value>, SchemaResolverError> {
        let key = url.as_str();
        self.registry.get(key).map_or_else(
            || Err(io::Error::new(io::ErrorKind::NotFound, key.to_string()).into()),
            |schema| Ok(Arc::new(schema.clone())),
        )
    }
}

fn compile_schema(schema: &Value, resolver: &ContractSchemaResolver) -> JSONSchema {
    let mut options = CompilationOptions::default();
    options.with_draft(Draft::Draft202012);
    options.with_resolver(resolver.clone());
    options.compile(schema).expect("schema compilation failed")
}

// ============================================================================
// SECTION: Tests
// ============================================================================

#[test]
fn tool_examples_match_tool_schemas() {
    let scenario_schema = schemas::scenario_schema();
    let mut registry = BTreeMap::new();
    let id =
        scenario_schema.get("$id").and_then(Value::as_str).expect("scenario schema missing $id");
    registry.insert(id.to_string(), scenario_schema);
    let resolver = ContractSchemaResolver::new(registry);

    for contract in tool_contracts() {
        let input_schema = compile_schema(&contract.input_schema, &resolver);
        let output_schema = compile_schema(&contract.output_schema, &resolver);
        let examples = tool_examples(contract.name);
        assert!(!examples.is_empty(), "tool examples missing for {}", contract.name);
        for example in examples {
            let result = input_schema.validate(&example.input);
            assert!(result.is_ok(), "input example failed for {}", contract.name);
            let result = output_schema.validate(&example.output);
            assert!(result.is_ok(), "output example failed for {}", contract.name);
        }
    }
}
