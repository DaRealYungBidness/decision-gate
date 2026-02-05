// crates/decision-gate-sdk-gen/src/lib.rs
// ============================================================================
// Module: SDK Generator Library
// Description: Deterministic generator for Decision Gate client SDK artifacts.
// Purpose: Render Python/TypeScript SDKs and OpenAPI view from tooling.json.
// Dependencies: decision-gate-contract, serde_json, thiserror
// ============================================================================

//! ## Overview
//! This crate generates Decision Gate client SDK artifacts from the canonical
//! `Docs/generated/decision-gate/tooling.json` contract. It produces
//! deterministic Python and TypeScript SDK surfaces plus an `OpenAPI` view of the
//! JSON-RPC `tools/call` surface.
//!
//! ### Design Notes
//! - Output is deterministic: schema properties and JSON object keys are sorted before rendering,
//!   and tool order follows the tooling contract input.
//! - The generator does not reach out to external schemas; `$ref` values are treated as opaque and
//!   rendered as `Any`.
//! - Schema-to-type mapping is best-effort and intentionally conservative to preserve compatibility
//!   across SDK consumers.
//!
//! ### Security Posture
//! Tooling contracts are treated as untrusted input. The generator enforces a
//! hard input size limit and fails closed on parsing errors. See
//! `Docs/security/threat_model.md` for the repository threat model.
//!
//! ## Index
//! - Public API: [`SdkGenerator`], [`SdkGenError`], [`DEFAULT_TOOLING_PATH`], [`MAX_TOOLING_BYTES`]
//! - Rendering: Python, TypeScript, `OpenAPI` (private helpers)
//! - Schema helpers: schema inspection, doc normalization, type mapping

use std::collections::BTreeMap;
use std::fmt;
use std::fmt::Write;
use std::fs;
use std::io::Read;
use std::path::Path;
use std::path::PathBuf;

use decision_gate_contract::types::ToolContract;
use decision_gate_contract::types::ToolExample;
use serde_json::Value;
use thiserror::Error;

// ============================================================================
// SECTION: Public API
// ============================================================================

// ============================================================================
// CONSTANTS: Tooling input defaults and limits
// ============================================================================

/// Default tooling.json path relative to the workspace root.
pub const DEFAULT_TOOLING_PATH: &str = "Docs/generated/decision-gate/tooling.json";

/// Maximum tooling.json size accepted by the generator.
pub const MAX_TOOLING_BYTES: u64 = 4 * 1024 * 1024;

/// Errors raised by the SDK generator.
///
/// # Invariants
/// - Variant meanings are stable for automation and tests.
///
/// # Examples
/// ```
/// use decision_gate_sdk_gen::SdkGenError;
///
/// let err = SdkGenError::Tooling("missing tooling".to_string());
/// assert!(matches!(err, SdkGenError::Tooling(message) if message == "missing tooling"));
/// ```
#[derive(Debug, Error)]
pub enum SdkGenError {
    /// IO error while reading or writing files.
    #[error("io error: {0}")]
    Io(String),
    /// JSON serialization or parsing error.
    #[error("json error: {0}")]
    Json(String),
    /// Tooling contract error.
    #[error("tooling error: {0}")]
    Tooling(String),
}

/// SDK generator loaded with tooling contracts.
///
/// # Invariants
/// - Tool order matches the tooling contract input.
/// - Rendering is deterministic for a fixed tooling contract.
///
/// # Examples
/// ```
/// use std::path::PathBuf;
///
/// use decision_gate_sdk_gen::DEFAULT_TOOLING_PATH;
/// use decision_gate_sdk_gen::SdkGenerator;
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
/// let workspace_root = manifest_dir
///     .parent()
///     .and_then(std::path::Path::parent)
///     .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::Other, "missing workspace root"))?;
/// let tooling_path = workspace_root.join(DEFAULT_TOOLING_PATH);
/// let generator = SdkGenerator::load(tooling_path)?;
/// let python = generator.generate_python()?;
/// assert!(python.contains("decision-gate-sdk-gen"));
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct SdkGenerator {
    /// Path to the tooling.json contract backing this generator.
    tooling_path: PathBuf,
    /// Loaded tooling contracts used to render SDK artifacts.
    tools: Vec<ToolContract>,
}

impl SdkGenerator {
    /// Loads tooling contracts from the given path.
    ///
    /// # Errors
    /// Returns [`SdkGenError`] when the tooling file cannot be read or parsed,
    /// or when the file exceeds [`MAX_TOOLING_BYTES`].
    ///
    /// # Notes
    /// This method performs JSON parsing only; semantic validation is expected
    /// to happen upstream when the tooling contract is built.
    pub fn load(tooling_path: impl AsRef<Path>) -> Result<Self, SdkGenError> {
        let tooling_path = tooling_path.as_ref().to_path_buf();
        let bytes = read_tooling_bytes(&tooling_path)?;
        let tools: Vec<ToolContract> =
            serde_json::from_slice(&bytes).map_err(|err| SdkGenError::Json(err.to_string()))?;
        Ok(Self {
            tooling_path,
            tools,
        })
    }

    /// Returns the tooling.json path used by the generator.
    #[must_use]
    pub fn tooling_path(&self) -> &Path {
        &self.tooling_path
    }

    /// Generates the Python SDK `_generated.py` content.
    ///
    /// # Errors
    /// Returns [`SdkGenError`] if JSON rendering fails.
    pub fn generate_python(&self) -> Result<String, SdkGenError> {
        render_python(&self.tools)
    }

    /// Generates the TypeScript SDK `_generated.ts` content.
    ///
    /// # Errors
    /// Returns [`SdkGenError`] if JSON rendering fails.
    pub fn generate_typescript(&self) -> Result<String, SdkGenError> {
        render_typescript(&self.tools)
    }

    /// Generates the `OpenAPI` JSON document.
    ///
    /// # Errors
    /// Returns [`SdkGenError`] if JSON serialization fails.
    pub fn generate_openapi(&self) -> Result<String, SdkGenError> {
        render_openapi(&self.tools)
    }
}

// ============================================================================
// SECTION: Tooling Input
// ============================================================================

/// Reads the tooling contract with size limits to avoid memory exhaustion.
fn read_tooling_bytes(path: &Path) -> Result<Vec<u8>, SdkGenError> {
    let file = fs::File::open(path).map_err(|err| SdkGenError::Io(err.to_string()))?;
    let metadata = file.metadata().map_err(|err| SdkGenError::Io(err.to_string()))?;
    if metadata.len() > MAX_TOOLING_BYTES {
        return Err(SdkGenError::Tooling(format!(
            "tooling input exceeds {MAX_TOOLING_BYTES} bytes"
        )));
    }
    let mut bytes = Vec::new();
    let mut limited = file.take(MAX_TOOLING_BYTES + 1);
    limited.read_to_end(&mut bytes).map_err(|err| SdkGenError::Io(err.to_string()))?;
    let size = u64::try_from(bytes.len()).map_err(|_| {
        SdkGenError::Tooling("tooling input size exceeds addressable memory".to_string())
    })?;
    if size > MAX_TOOLING_BYTES {
        return Err(SdkGenError::Tooling(format!(
            "tooling input exceeds {MAX_TOOLING_BYTES} bytes"
        )));
    }
    Ok(bytes)
}

// ============================================================================
// SECTION: Schema Model
// ============================================================================

/// Internal representation of a schema-derived type.
///
/// # Invariants
/// - `Union` variants contain at least two distinct, non-`Any` entries.
/// - `Array` always wraps a fully resolved inner [`TypeSpec`].
#[derive(Debug, Clone, PartialEq)]
enum TypeSpec {
    /// Arbitrary JSON value.
    Any,
    /// JSON null literal.
    Null,
    /// Boolean value.
    Bool,
    /// Integer number.
    Int,
    /// Floating point number.
    Number,
    /// String value.
    String,
    /// Array of items with a shared type.
    Array(Box<Self>),
    /// Object with arbitrary properties.
    Object,
    /// Union of multiple candidate types.
    Union(Vec<Self>),
    /// Literal set of JSON values.
    Literal(Vec<Value>),
}

/// Object property metadata for SDK type rendering.
///
/// # Invariants
/// - `name` matches the JSON Schema property key.
/// - `schema` is the original schema fragment for this property.
#[derive(Debug, Clone)]
struct Property {
    /// Property name in the schema.
    name: String,
    /// Resolved property type.
    ty: TypeSpec,
    /// True when the property is required.
    required: bool,
    /// Original schema fragment for this property.
    schema: Value,
}

// ============================================================================
// SECTION: Python SDK Rendering
// ============================================================================

/// Renders the Python SDK generated file from tooling contracts.
#[allow(
    clippy::too_many_lines,
    reason = "Generator output is assembled in one pass for determinism."
)]
fn render_python(tools: &[ToolContract]) -> Result<String, SdkGenError> {
    let mut out = String::new();
    out.push_str("# This file is @generated by decision-gate-sdk-gen. DO NOT EDIT.\n");
    out.push_str("# Source: ");
    out.push_str(DEFAULT_TOOLING_PATH);
    out.push('\n');
    out.push_str("# fmt: off\n\n");
    out.push_str("from __future__ import annotations\n\n");
    out.push_str("import json as _json\n");
    out.push_str(
        "from typing import Any, Dict, List, Mapping, Sequence, TypedDict, Union, Literal, cast\n",
    );
    out.push_str("try:\n");
    out.push_str("    from typing import NotRequired\n");
    out.push_str("except ImportError:\n");
    out.push_str("    try:\n");
    out.push_str("        from typing_extensions import NotRequired\n");
    out.push_str("    except ImportError:\n");
    out.push_str("        class _NotRequired:\n");
    out.push_str("            def __class_getitem__(cls, item):\n");
    out.push_str("                return item\n");
    out.push_str("        NotRequired = _NotRequired\n\n");
    out.push_str("JsonPrimitive = Union[str, int, float, bool, None]\n");
    out.push_str(
        "JsonValue = Union[JsonPrimitive, List[\"JsonValue\"], Dict[str, \"JsonValue\"]]\n\n",
    );

    out.push_str("TOOL_NAMES: Sequence[str] = (\n");
    for tool in tools {
        out.push_str("    \"");
        out.push_str(tool.name.as_str());
        out.push_str("\",\n");
    }
    out.push_str(")\n\n");

    out.push_str("TOOL_DESCRIPTIONS: Mapping[str, str] = {\n");
    for tool in tools {
        out.push_str("    \"");
        out.push_str(tool.name.as_str());
        out.push_str("\": ");
        out.push_str(&python_string_literal(&tool.description));
        out.push_str(",\n");
    }
    out.push_str("}\n\n");

    out.push_str("TOOL_NOTES: Mapping[str, Sequence[str]] = {\n");
    for tool in tools {
        out.push_str("    \"");
        out.push_str(tool.name.as_str());
        out.push_str("\": [\n");
        for note in &tool.notes {
            out.push_str("        ");
            out.push_str(&python_string_literal(note));
            out.push_str(",\n");
        }
        out.push_str("    ],\n");
    }
    out.push_str("}\n\n");

    for tool in tools {
        let pascal = pascal_case(tool.name.as_str());
        let input_type = format!("{pascal}Request");
        let output_type = format!("{pascal}Response");
        render_python_typed_dict(&mut out, &input_type, &tool.input_schema);
        render_python_typed_dict(&mut out, &output_type, &tool.output_schema);
        render_python_schema_constant(&mut out, &pascal, "INPUT_SCHEMA", &tool.input_schema)?;
        render_python_schema_constant(&mut out, &pascal, "OUTPUT_SCHEMA", &tool.output_schema)?;
    }

    out.push_str("class GeneratedDecisionGateClient:\n");
    out.push_str(
        "    \"\"\"Generated Decision Gate client methods. Implement `_call_tool`.\"\"\"\n\n",
    );
    out.push_str("    def _call_tool(self, name: str, arguments: JsonValue) -> JsonValue:\n");
    out.push_str(
        "        raise NotImplementedError(\"_call_tool must be implemented by subclasses\")\n\n",
    );

    for tool in tools {
        let pascal = pascal_case(tool.name.as_str());
        let input_type = format!("{pascal}Request");
        let output_type = format!("{pascal}Response");
        out.push_str("    def ");
        out.push_str(tool.name.as_str());
        out.push_str("(self, request: ");
        out.push_str(&input_type);
        out.push_str(") -> ");
        out.push_str(&output_type);
        out.push_str(":\n");
        out.push_str("        \"\"\"\n");
        out.push_str("        ");
        out.push_str(&normalize_doc(&tool.description));
        out.push('\n');
        if !tool.notes.is_empty() {
            out.push('\n');
            out.push_str("        Notes:\n");
            for note in &tool.notes {
                out.push_str("        - ");
                out.push_str(&normalize_doc(note));
                out.push('\n');
            }
        }
        if !tool.examples.is_empty() {
            out.push('\n');
            render_python_examples(&mut out, &tool.examples)?;
        }
        out.push_str("        \"\"\"\n");
        out.push_str("        return cast(");
        out.push_str(&output_type);
        out.push_str(", self._call_tool(\"");
        out.push_str(tool.name.as_str());
        out.push_str("\", request))\n\n");
    }

    render_python_validation_helpers(&mut out, tools);
    render_python_exports(&mut out, tools);
    Ok(out)
}

/// Renders a `TypedDict` for a JSON object schema.
fn render_python_typed_dict(out: &mut String, name: &str, schema: &Value) {
    out.push_str("class ");
    out.push_str(name);
    out.push_str("(TypedDict):\n");
    let class_doc = schema_doc(schema).unwrap_or_else(|| format!("Schema for {name}."));
    out.push_str("    \"\"\"");
    out.push_str(&class_doc);
    out.push_str("\"\"\"\n");
    match object_properties(schema) {
        Some(properties) if !properties.is_empty() => {
            for property in properties {
                if let Some(comment) = schema_doc(&property.schema) {
                    for line in wrap_doc(&comment, 88) {
                        out.push_str("    #: ");
                        out.push_str(&line);
                        out.push('\n');
                    }
                }
                out.push_str("    ");
                out.push_str(&property.name);
                out.push_str(": ");
                if property.required {
                    out.push_str(&python_type(&property.ty));
                } else {
                    out.push_str("NotRequired[");
                    out.push_str(&python_type(&property.ty));
                    out.push(']');
                }
                out.push('\n');
            }
        }
        _ => {
            out.push_str("    pass\n");
        }
    }
    out.push('\n');
}

/// Renders a Python constant holding the JSON schema.
fn render_python_schema_constant(
    out: &mut String,
    pascal: &str,
    suffix: &str,
    schema: &Value,
) -> Result<(), SdkGenError> {
    let constant_name = format!("{pascal}_{suffix}");
    let json =
        serde_json::to_string_pretty(schema).map_err(|err| SdkGenError::Json(err.to_string()))?;
    out.push_str(&constant_name);
    out.push_str(" = _json.loads(r\"\"\"\n");
    out.push_str(&json);
    out.push_str("\n\"\"\")\n\n");
    Ok(())
}

// ============================================================================
// SECTION: TypeScript SDK Rendering
// ============================================================================

/// Renders the TypeScript SDK generated file from tooling contracts.
fn render_typescript(tools: &[ToolContract]) -> Result<String, SdkGenError> {
    let mut out = String::new();
    out.push_str("// This file is @generated by decision-gate-sdk-gen. DO NOT EDIT.\n");
    out.push_str("// Source: ");
    out.push_str(DEFAULT_TOOLING_PATH);
    out.push('\n');
    out.push_str("// fmt: off\n\n");
    out.push_str("export type JsonPrimitive = string | number | boolean | null;\n");
    out.push_str(
        "export type JsonValue = JsonPrimitive | JsonValue[] | { [key: string]: JsonValue };\n\n",
    );
    out.push_str("export const TOOL_NAMES = [\n");
    for tool in tools {
        out.push_str("  \"");
        out.push_str(tool.name.as_str());
        out.push_str("\",\n");
    }
    out.push_str("] as const;\n\n");
    out.push_str("export const TOOL_DESCRIPTIONS: Record<string, string> = {\n");
    for tool in tools {
        out.push_str("  \"");
        out.push_str(tool.name.as_str());
        out.push_str("\": ");
        out.push_str(&typescript_string_literal(&tool.description));
        out.push_str(",\n");
    }
    out.push_str("};\n\n");
    out.push_str("export const TOOL_NOTES: Record<string, string[]> = {\n");
    for tool in tools {
        out.push_str("  \"");
        out.push_str(tool.name.as_str());
        out.push_str("\": [\n");
        for note in &tool.notes {
            out.push_str("    ");
            out.push_str(&typescript_string_literal(note));
            out.push_str(",\n");
        }
        out.push_str("  ],\n");
    }
    out.push_str("};\n\n");

    for tool in tools {
        let pascal = pascal_case(tool.name.as_str());
        let input_type = format!("{pascal}Request");
        let output_type = format!("{pascal}Response");
        render_typescript_interface(&mut out, &input_type, &tool.input_schema);
        render_typescript_interface(&mut out, &output_type, &tool.output_schema);
        render_typescript_schema_constant(&mut out, &pascal, "INPUT_SCHEMA", &tool.input_schema)?;
        render_typescript_schema_constant(&mut out, &pascal, "OUTPUT_SCHEMA", &tool.output_schema)?;
    }

    out.push_str("export abstract class GeneratedDecisionGateClient {\n");
    out.push_str(
        "  protected abstract callTool<T>(name: string, arguments_: object): Promise<T>;\n\n",
    );
    for tool in tools {
        let pascal = pascal_case(tool.name.as_str());
        let input_type = format!("{pascal}Request");
        let output_type = format!("{pascal}Response");
        out.push_str("  /**\n");
        out.push_str("   * ");
        out.push_str(&normalize_doc(&tool.description));
        out.push('\n');
        if !tool.notes.is_empty() {
            out.push_str("   *\n");
            out.push_str("   * Notes:\n");
            for note in &tool.notes {
                out.push_str("   * - ");
                out.push_str(&normalize_doc(note));
                out.push('\n');
            }
        }
        if !tool.examples.is_empty() {
            out.push_str("   *\n");
            render_typescript_examples(&mut out, &tool.examples)?;
        }
        out.push_str("   */\n");
        out.push_str("  public ");
        out.push_str(tool.name.as_str());
        out.push_str("(request: ");
        out.push_str(&input_type);
        out.push_str("): Promise<");
        out.push_str(&output_type);
        out.push_str("> {\n");
        out.push_str("    return this.callTool<");
        out.push_str(&output_type);
        out.push_str(">(\"");
        out.push_str(tool.name.as_str());
        out.push_str("\", request);\n");
        out.push_str("  }\n\n");
    }
    out.push_str("}\n");

    render_typescript_validation_helpers(&mut out, tools);
    Ok(out)
}

/// Renders a TypeScript interface for a JSON object schema.
fn render_typescript_interface(out: &mut String, name: &str, schema: &Value) {
    if let Some(doc) = schema_doc(schema) {
        for line in wrap_doc(&doc, 96) {
            out.push_str("/** ");
            out.push_str(&line);
            out.push_str(" */\n");
        }
    }
    out.push_str("export interface ");
    out.push_str(name);
    out.push_str(" {\n");
    match object_properties(schema) {
        Some(properties) if !properties.is_empty() => {
            for property in properties {
                if let Some(comment) = schema_doc(&property.schema) {
                    for line in wrap_doc(&comment, 96) {
                        out.push_str("  /** ");
                        out.push_str(&line);
                        out.push_str(" */\n");
                    }
                }
                out.push_str("  ");
                out.push_str(&property.name);
                if !property.required {
                    out.push('?');
                }
                out.push_str(": ");
                out.push_str(&typescript_type(&property.ty));
                out.push_str(";\n");
            }
        }
        _ => {
            out.push_str("  [key: string]: never;\n");
        }
    }
    out.push_str("}\n\n");
}

/// Renders a TypeScript constant holding the JSON schema.
fn render_typescript_schema_constant(
    out: &mut String,
    pascal: &str,
    suffix: &str,
    schema: &Value,
) -> Result<(), SdkGenError> {
    let constant_name = format!("{pascal}_{suffix}");
    let json =
        serde_json::to_string_pretty(schema).map_err(|err| SdkGenError::Json(err.to_string()))?;
    out.push_str("export const ");
    out.push_str(&constant_name);
    out.push_str(" = ");
    out.push_str(&json);
    out.push_str(" as const;\n\n");
    Ok(())
}

// ============================================================================
// SECTION: OpenAPI Rendering
// ============================================================================

/// Renders the `OpenAPI` JSON document for the JSON-RPC tools/call surface.
#[allow(
    clippy::too_many_lines,
    reason = "OpenAPI assembly is kept in one place to mirror the schema output."
)]
fn render_openapi(tools: &[ToolContract]) -> Result<String, SdkGenError> {
    let mut schemas = serde_json::Map::new();
    schemas.insert(
        "JsonRpcErrorData".to_string(),
        serde_json::json!({
            "type": "object",
            "properties": {
                "kind": { "type": "string" },
                "retryable": { "type": "boolean" },
                "request_id": { "type": "string" },
                "retry_after_ms": { "type": "integer" }
            },
            "required": ["kind", "retryable"],
            "additionalProperties": false
        }),
    );
    schemas.insert(
        "JsonRpcError".to_string(),
        serde_json::json!({
            "type": "object",
            "properties": {
                "code": { "type": "integer" },
                "message": { "type": "string" },
                "data": { "$ref": "#/components/schemas/JsonRpcErrorData" }
            },
            "required": ["code", "message"],
            "additionalProperties": false
        }),
    );

    let mut tool_param_refs = Vec::new();
    let mut tool_result_refs = Vec::new();

    for tool in tools {
        let pascal = pascal_case(tool.name.as_str());
        let params_name = format!("{pascal}ToolCallParams");
        let result_name = format!("{pascal}ToolCallResult");

        schemas.insert(
            params_name.clone(),
            serde_json::json!({
                "type": "object",
                "properties": {
                    "name": { "const": tool.name.as_str() },
                    "arguments": tool.input_schema.clone()
                },
                "required": ["name", "arguments"],
                "additionalProperties": false
            }),
        );

        schemas.insert(
            result_name.clone(),
            serde_json::json!({
                "type": "object",
                "properties": {
                    "content": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "type": { "const": "json" },
                                "json": tool.output_schema.clone()
                            },
                            "required": ["type", "json"],
                            "additionalProperties": false
                        },
                        "minItems": 1
                    }
                },
                "required": ["content"],
                "additionalProperties": false
            }),
        );

        tool_param_refs.push(serde_json::json!({
            "$ref": format!("#/components/schemas/{params_name}")
        }));
        tool_result_refs.push(serde_json::json!({
            "$ref": format!("#/components/schemas/{result_name}")
        }));
    }

    schemas.insert(
        "ToolCallParams".to_string(),
        serde_json::json!({
            "oneOf": tool_param_refs
        }),
    );
    schemas.insert(
        "ToolCallResult".to_string(),
        serde_json::json!({
            "oneOf": tool_result_refs
        }),
    );
    schemas.insert(
        "ToolCallRequest".to_string(),
        serde_json::json!({
            "type": "object",
            "properties": {
                "jsonrpc": { "const": "2.0" },
                "id": {
                    "oneOf": [
                        { "type": "string" },
                        { "type": "number" },
                        { "type": "null" }
                    ]
                },
                "method": { "const": "tools/call" },
                "params": { "$ref": "#/components/schemas/ToolCallParams" }
            },
            "required": ["jsonrpc", "id", "method", "params"],
            "additionalProperties": false
        }),
    );
    schemas.insert(
        "ToolCallResponse".to_string(),
        serde_json::json!({
            "type": "object",
            "properties": {
                "jsonrpc": { "const": "2.0" },
                "id": {
                    "oneOf": [
                        { "type": "string" },
                        { "type": "number" },
                        { "type": "null" }
                    ]
                },
                "result": { "$ref": "#/components/schemas/ToolCallResult" },
                "error": { "$ref": "#/components/schemas/JsonRpcError" }
            },
            "required": ["jsonrpc", "id"],
            "additionalProperties": false
        }),
    );

    let openapi = serde_json::json!({
        "openapi": "3.1.0",
        "jsonSchemaDialect": "https://json-schema.org/draft/2020-12/schema",
        "info": {
            "title": "Decision Gate MCP JSON-RPC",
            "version": env!("CARGO_PKG_VERSION"),
            "description": "Generated OpenAPI view of the Decision Gate tools/call JSON-RPC surface."
        },
        "paths": {
            "/rpc": {
                "post": {
                    "summary": "Invoke a Decision Gate tool via JSON-RPC.",
                    "requestBody": {
                        "required": true,
                        "content": {
                            "application/json": {
                                "schema": { "$ref": "#/components/schemas/ToolCallRequest" }
                            }
                        }
                    },
                    "responses": {
                        "200": {
                            "description": "JSON-RPC tool response.",
                            "content": {
                                "application/json": {
                                    "schema": { "$ref": "#/components/schemas/ToolCallResponse" }
                                }
                            }
                        }
                    }
                }
            }
        },
        "components": {
            "schemas": schemas
        }
    });

    let openapi = sorted_json_value(&openapi);
    serde_json::to_string_pretty(&openapi).map_err(|err| SdkGenError::Json(err.to_string()))
}

// ============================================================================
// SECTION: Schema Introspection and Documentation
// ============================================================================

/// Extracts top-level object properties from a JSON schema.
///
/// Properties are returned in sorted order for deterministic output.
fn object_properties(schema: &Value) -> Option<Vec<Property>> {
    let properties = schema.get("properties")?.as_object()?;
    let required_list = schema.get("required").and_then(|value| value.as_array());
    let mut required = BTreeMap::new();
    if let Some(list) = required_list {
        for value in list {
            if let Some(name) = value.as_str() {
                required.insert(name.to_string(), true);
            }
        }
    }
    let mut output = Vec::new();
    let mut names: Vec<&String> = properties.keys().collect();
    names.sort();
    for name in names {
        let schema = &properties[name];
        let ty = schema_to_typespec(schema);
        let is_required = required.contains_key(name.as_str());
        output.push(Property {
            name: name.clone(),
            ty,
            required: is_required,
            schema: schema.clone(),
        });
    }
    Some(output)
}

/// Builds a combined documentation string for a schema.
fn schema_doc(schema: &Value) -> Option<String> {
    let desc = schema_description(schema);
    let constraints = schema_constraints(schema);
    let doc = match (desc, constraints.is_empty()) {
        (None, true) => None,
        (Some(description), true) => Some(description),
        (None, false) => Some(format!("Constraints: {}.", constraints.join("; "))),
        (Some(description), false) => {
            Some(format!("{description} Constraints: {}.", constraints.join("; ")))
        }
    };
    doc.map(|value| normalize_doc(&value))
}

/// Extracts and normalizes the description field from a schema.
fn schema_description(schema: &Value) -> Option<String> {
    schema
        .get("description")
        .and_then(Value::as_str)
        .map(normalize_doc)
        .filter(|value| !value.is_empty())
}

/// Builds human-readable constraint notes from a schema.
fn schema_constraints(schema: &Value) -> Vec<String> {
    let mut items = Vec::new();
    if let Some(const_value) = schema.get("const") {
        items.push(format!("Const: {}", json_inline(const_value)));
    }
    if let Some(enum_values) = schema.get("enum").and_then(Value::as_array) {
        let values: Vec<String> = enum_values.iter().map(json_inline).collect();
        if !values.is_empty() {
            items.push(format!("Allowed values: {}", values.join(", ")));
        }
    }
    if let Some(format) = schema.get("format").and_then(Value::as_str) {
        items.push(format!("Format: {}", normalize_doc(format)));
    }
    if let Some(pattern) = schema.get("pattern").and_then(Value::as_str) {
        items.push(format!("Pattern: {}", normalize_doc(pattern)));
    }
    if let Some(minimum) = schema.get("minimum") {
        items.push(format!("Minimum: {}", json_inline(minimum)));
    }
    if let Some(maximum) = schema.get("maximum") {
        items.push(format!("Maximum: {}", json_inline(maximum)));
    }
    if let Some(minimum) = schema.get("exclusiveMinimum") {
        items.push(format!("Exclusive minimum: {}", json_inline(minimum)));
    }
    if let Some(maximum) = schema.get("exclusiveMaximum") {
        items.push(format!("Exclusive maximum: {}", json_inline(maximum)));
    }
    if let Some(min_length) = schema.get("minLength") {
        items.push(format!("Min length: {}", json_inline(min_length)));
    }
    if let Some(max_length) = schema.get("maxLength") {
        items.push(format!("Max length: {}", json_inline(max_length)));
    }
    if let Some(min_items) = schema.get("minItems") {
        items.push(format!("Min items: {}", json_inline(min_items)));
    }
    if let Some(max_items) = schema.get("maxItems") {
        items.push(format!("Max items: {}", json_inline(max_items)));
    }
    if let Some(default) = schema.get("default") {
        items.push(format!("Default: {}", json_inline(default)));
    }
    if let Some(title) = schema.get("title").and_then(Value::as_str) {
        items.push(format!("Title: {}", normalize_doc(title)));
    }
    if schema.get("deprecated").and_then(Value::as_bool) == Some(true) {
        items.push("Deprecated.".to_string());
    }
    items
}

/// Normalizes documentation strings by collapsing whitespace and defusing
/// comment or docstring terminators in generated outputs.
fn normalize_doc(value: &str) -> String {
    let collapsed = value.split_whitespace().collect::<Vec<_>>().join(" ");
    let collapsed = collapsed.replace("*/", "* /");
    collapsed.replace("\"\"\"", "\\\"\\\"\\\"")
}

/// Renders a JSON value as a compact inline string.
fn json_inline(value: &Value) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "<unprintable>".to_string())
}

/// Wraps documentation text to a target width.
fn wrap_doc(text: &str, width: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let mut current = String::new();
    for word in text.split_whitespace() {
        if current.is_empty() {
            current.push_str(word);
            continue;
        }
        if current.len() + 1 + word.len() > width {
            lines.push(current);
            current = word.to_string();
        } else {
            current.push(' ');
            current.push_str(word);
        }
    }
    if !current.is_empty() {
        lines.push(current);
    }
    lines
}

// ============================================================================
// SECTION: Examples Rendering
// ============================================================================

/// Renders example blocks into a Python docstring.
fn render_python_examples(out: &mut String, examples: &[ToolExample]) -> Result<(), SdkGenError> {
    out.push_str("        Examples:\n");
    for example in examples {
        out.push_str("        - ");
        out.push_str(&normalize_doc(&example.description));
        out.push('\n');
        out.push_str("          Input:\n");
        render_python_json_block(out, &example.input, "            ")?;
        out.push_str("          Output:\n");
        render_python_json_block(out, &example.output, "            ")?;
    }
    Ok(())
}

/// Renders pretty JSON with indentation for Python docs.
fn render_python_json_block(
    out: &mut String,
    value: &Value,
    indent: &str,
) -> Result<(), SdkGenError> {
    let json =
        serde_json::to_string_pretty(value).map_err(|err| SdkGenError::Json(err.to_string()))?;
    for line in json.lines() {
        out.push_str(indent);
        out.push_str(line);
        out.push('\n');
    }
    Ok(())
}

/// Renders example blocks into a TypeScript doc comment.
fn render_typescript_examples(
    out: &mut String,
    examples: &[ToolExample],
) -> Result<(), SdkGenError> {
    out.push_str("   * Examples:\n");
    for example in examples {
        out.push_str("   * - ");
        out.push_str(&normalize_doc(&example.description));
        out.push('\n');
        out.push_str("   *   Input:\n");
        render_typescript_json_block(out, &example.input)?;
        out.push_str("   *   Output:\n");
        render_typescript_json_block(out, &example.output)?;
    }
    Ok(())
}

/// Renders pretty JSON for a TypeScript doc comment.
fn render_typescript_json_block(out: &mut String, value: &Value) -> Result<(), SdkGenError> {
    let json =
        serde_json::to_string_pretty(value).map_err(|err| SdkGenError::Json(err.to_string()))?;
    out.push_str("   *   ```json\n");
    for line in json.lines() {
        out.push_str("   *   ");
        out.push_str(line);
        out.push('\n');
    }
    out.push_str("   *   ```\n");
    Ok(())
}

// ============================================================================
// SECTION: Runtime Validation Helpers
// ============================================================================

/// Emits Python runtime schema validation helpers.
fn render_python_validation_helpers(out: &mut String, tools: &[ToolContract]) {
    out.push_str("class SchemaValidationError(ValueError):\n");
    out.push_str("    \"\"\"Raised when payloads fail schema validation.\"\"\"\n\n");
    out.push_str("def _load_jsonschema() -> Any:\n");
    out.push_str("    try:\n");
    out.push_str("        import jsonschema  # type: ignore\n");
    out.push_str("    except ImportError as exc:\n");
    out.push_str("        raise RuntimeError(\n");
    out.push_str("            \"jsonschema is required for runtime validation. \"\n");
    out.push_str("            \"Install with: pip install decision-gate[validation]\"\n");
    out.push_str("        ) from exc\n");
    out.push_str("    return jsonschema\n\n");
    out.push_str(
        "def validate_schema(instance: JsonValue, schema: Mapping[str, JsonValue]) -> None:\n",
    );
    out.push_str("    \"\"\"Validate a payload against a JSON schema.\"\"\"\n");
    out.push_str("    jsonschema = _load_jsonschema()\n");
    out.push_str("    try:\n");
    out.push_str("        jsonschema.validate(instance=instance, schema=schema)\n");
    out.push_str("    except jsonschema.ValidationError as exc:\n");
    out.push_str("        raise SchemaValidationError(str(exc)) from exc\n\n");
    for tool in tools {
        let pascal = pascal_case(tool.name.as_str());
        out.push_str("def validate_");
        out.push_str(tool.name.as_str());
        out.push_str("_request(request: ");
        let _ = write!(out, "{pascal}Request");
        out.push_str(") -> None:\n");
        out.push_str("    \"\"\"Validate the request payload against the input schema.\"\"\"\n");
        out.push_str("    validate_schema(request, ");
        let _ = write!(out, "{pascal}_INPUT_SCHEMA");
        out.push_str(")\n\n");
        out.push_str("def validate_");
        out.push_str(tool.name.as_str());
        out.push_str("_response(response: ");
        let _ = write!(out, "{pascal}Response");
        out.push_str(") -> None:\n");
        out.push_str("    \"\"\"Validate the response payload against the output schema.\"\"\"\n");
        out.push_str("    validate_schema(response, ");
        let _ = write!(out, "{pascal}_OUTPUT_SCHEMA");
        out.push_str(")\n\n");
    }
}

/// Emits the Python `__all__` export list.
fn render_python_exports(out: &mut String, tools: &[ToolContract]) {
    let mut exports = vec![
        "JsonPrimitive".to_string(),
        "JsonValue".to_string(),
        "TOOL_NAMES".to_string(),
        "TOOL_DESCRIPTIONS".to_string(),
        "TOOL_NOTES".to_string(),
        "GeneratedDecisionGateClient".to_string(),
        "SchemaValidationError".to_string(),
        "validate_schema".to_string(),
    ];
    for tool in tools {
        let pascal = pascal_case(tool.name.as_str());
        exports.push(format!("{pascal}Request"));
        exports.push(format!("{pascal}Response"));
        exports.push(format!("{pascal}_INPUT_SCHEMA"));
        exports.push(format!("{pascal}_OUTPUT_SCHEMA"));
        exports.push(format!("validate_{}_request", tool.name.as_str()));
        exports.push(format!("validate_{}_response", tool.name.as_str()));
    }
    out.push_str("__all__ = [\n");
    for name in exports {
        out.push_str("    \"");
        out.push_str(&name);
        out.push_str("\",\n");
    }
    out.push_str("]\n\n");
}

/// Emits TypeScript runtime schema validation helpers.
fn render_typescript_validation_helpers(out: &mut String, tools: &[ToolContract]) {
    out.push_str("export type SchemaValidator = (schema: unknown, payload: unknown) => void;\n\n");
    out.push_str("export class SchemaValidationError extends Error {\n");
    out.push_str("  public readonly errors?: unknown;\n");
    out.push_str("  constructor(message: string, errors?: unknown) {\n");
    out.push_str("    super(message);\n");
    out.push_str("    this.name = \"SchemaValidationError\";\n");
    out.push_str("    this.errors = errors;\n");
    out.push_str("  }\n");
    out.push_str("}\n\n");
    out.push_str(
        "export function validateSchemaWith(validator: SchemaValidator, schema: unknown, payload: \
         unknown): void {\n",
    );
    out.push_str("  validator(schema, payload);\n");
    out.push_str("}\n\n");
    out.push_str("async function loadAjv(): Promise<any> {\n");
    out.push_str(
        "  const loader = new Function(\"moduleName\", \"return import(moduleName);\") as (name: \
         string) => Promise<any>;\n",
    );
    out.push_str("  const mod = await loader(\"ajv\");\n");
    out.push_str("  return mod.default ?? mod;\n");
    out.push_str("}\n\n");
    out.push_str(
        "export async function validateSchemaWithAjv(schema: unknown, payload: unknown): \
         Promise<void> {\n",
    );
    out.push_str("  const Ajv = await loadAjv();\n");
    out.push_str("  const ajv = new Ajv({ allErrors: true, strict: false });\n");
    out.push_str("  const validate = ajv.compile(schema);\n");
    out.push_str("  const ok = validate(payload);\n");
    out.push_str("  if (!ok) {\n");
    out.push_str(
        "    throw new SchemaValidationError(\"Schema validation failed.\", validate.errors);\n",
    );
    out.push_str("  }\n");
    out.push_str("}\n\n");
    for tool in tools {
        let pascal = pascal_case(tool.name.as_str());
        out.push_str("export function validate");
        out.push_str(&pascal);
        out.push_str("Request(payload: ");
        let _ = write!(out, "{pascal}Request");
        out.push_str(", validator: SchemaValidator): void {\n");
        out.push_str("  validateSchemaWith(validator, ");
        let _ = write!(out, "{pascal}_INPUT_SCHEMA");
        out.push_str(", payload);\n");
        out.push_str("}\n\n");
        out.push_str("export function validate");
        out.push_str(&pascal);
        out.push_str("Response(payload: ");
        let _ = write!(out, "{pascal}Response");
        out.push_str(", validator: SchemaValidator): void {\n");
        out.push_str("  validateSchemaWith(validator, ");
        let _ = write!(out, "{pascal}_OUTPUT_SCHEMA");
        out.push_str(", payload);\n");
        out.push_str("}\n\n");
        out.push_str("export async function validate");
        out.push_str(&pascal);
        out.push_str("RequestWithAjv(payload: ");
        let _ = write!(out, "{pascal}Request");
        out.push_str("): Promise<void> {\n");
        out.push_str("  return validateSchemaWithAjv(");
        let _ = write!(out, "{pascal}_INPUT_SCHEMA");
        out.push_str(", payload);\n");
        out.push_str("}\n\n");
        out.push_str("export async function validate");
        out.push_str(&pascal);
        out.push_str("ResponseWithAjv(payload: ");
        let _ = write!(out, "{pascal}Response");
        out.push_str("): Promise<void> {\n");
        out.push_str("  return validateSchemaWithAjv(");
        let _ = write!(out, "{pascal}_OUTPUT_SCHEMA");
        out.push_str(", payload);\n");
        out.push_str("}\n\n");
    }
}

// ============================================================================
// SECTION: Schema Sorting and Type Mapping
// ============================================================================

/// Returns a JSON value with deterministically sorted object keys.
///
/// This is used for `OpenAPI` output so that regeneration is stable across runs.
fn sorted_json_value(value: &Value) -> Value {
    match value {
        Value::Array(items) => Value::Array(items.iter().map(sorted_json_value).collect()),
        Value::Object(map) => {
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort();
            let mut sorted = serde_json::Map::new();
            for key in keys {
                if let Some(value) = map.get(key) {
                    sorted.insert(key.clone(), sorted_json_value(value));
                }
            }
            Value::Object(sorted)
        }
        _ => value.clone(),
    }
}

/// Maps a JSON schema to an internal type representation.
///
/// The mapping is intentionally conservative: `$ref` becomes `Any`, enums of
/// JSON literals become `Literal`, and unrecognized types fall back to `Any`.
fn schema_to_typespec(schema: &Value) -> TypeSpec {
    if let Some(one_of) = schema.get("oneOf").and_then(|value| value.as_array()) {
        return union_types(one_of.iter().map(schema_to_typespec));
    }
    if let Some(any_of) = schema.get("anyOf").and_then(|value| value.as_array()) {
        return union_types(any_of.iter().map(schema_to_typespec));
    }
    if let Some(enum_values) = schema.get("enum").and_then(|value| value.as_array())
        && enum_values.iter().all(is_literal_value)
    {
        return TypeSpec::Literal(enum_values.clone());
    }
    if schema.get("$ref").is_some() {
        return TypeSpec::Any;
    }
    match schema.get("type") {
        Some(Value::String(ty)) => type_from_name(ty, schema),
        Some(Value::Array(types)) => union_types(
            types.iter().filter_map(|value| value.as_str()).map(|ty| type_from_name(ty, schema)),
        ),
        _ => TypeSpec::Any,
    }
}

/// Maps a JSON schema type tag to an internal type representation.
fn type_from_name(name: &str, schema: &Value) -> TypeSpec {
    match name {
        "null" => TypeSpec::Null,
        "boolean" => TypeSpec::Bool,
        "integer" => TypeSpec::Int,
        "number" => TypeSpec::Number,
        "string" => TypeSpec::String,
        "array" => {
            let inner = schema.get("items").map_or(TypeSpec::Any, schema_to_typespec);
            TypeSpec::Array(Box::new(inner))
        }
        "object" => TypeSpec::Object,
        _ => TypeSpec::Any,
    }
}

/// Collapses multiple types into a union, deduplicating where possible.
///
/// If any type is `Any`, the union is treated as `Any` to avoid overconstraining.
fn union_types<I>(types: I) -> TypeSpec
where
    I: IntoIterator<Item = TypeSpec>,
{
    let mut output = Vec::new();
    for ty in types {
        if ty == TypeSpec::Any {
            return TypeSpec::Any;
        }
        if !output.contains(&ty) {
            output.push(ty);
        }
    }
    if output.is_empty() {
        TypeSpec::Any
    } else if output.len() == 1 {
        output.remove(0)
    } else {
        TypeSpec::Union(output)
    }
}

/// Renders a Python type annotation for the internal type representation.
///
/// Union members are sorted to keep generated output stable.
fn python_type(ty: &TypeSpec) -> String {
    match ty {
        TypeSpec::Any => "JsonValue".to_string(),
        TypeSpec::Null => "None".to_string(),
        TypeSpec::Bool => "bool".to_string(),
        TypeSpec::Int => "int".to_string(),
        TypeSpec::Number => "float".to_string(),
        TypeSpec::String => "str".to_string(),
        TypeSpec::Array(inner) => format!("List[{}]", python_type(inner)),
        TypeSpec::Object => "Dict[str, JsonValue]".to_string(),
        TypeSpec::Union(types) => {
            let mut rendered: Vec<String> = types.iter().map(python_type).collect();
            rendered.sort();
            if rendered.len() == 1 {
                rendered.remove(0)
            } else {
                format!("Union[{}]", rendered.join(", "))
            }
        }
        TypeSpec::Literal(values) => {
            let literals: Vec<String> = values.iter().map(python_literal_value).collect();
            format!("Literal[{}]", literals.join(", "))
        }
    }
}

/// Renders a JSON literal as a Python literal expression.
fn python_literal_value(value: &Value) -> String {
    match value {
        Value::Bool(true) => "True".to_string(),
        Value::Bool(false) => "False".to_string(),
        Value::Number(number) => number.to_string(),
        Value::String(value) => python_string_literal(value),
        _ => "None".to_string(),
    }
}

/// Renders a TypeScript type annotation for the internal type representation.
///
/// Union members are sorted to keep generated output stable.
fn typescript_type(ty: &TypeSpec) -> String {
    match ty {
        TypeSpec::Any => "JsonValue".to_string(),
        TypeSpec::Null => "null".to_string(),
        TypeSpec::Bool => "boolean".to_string(),
        TypeSpec::Int | TypeSpec::Number => "number".to_string(),
        TypeSpec::String => "string".to_string(),
        TypeSpec::Array(inner) => format!("Array<{}>", typescript_type(inner)),
        TypeSpec::Object => "Record<string, JsonValue>".to_string(),
        TypeSpec::Union(types) => {
            let mut rendered: Vec<String> = types.iter().map(typescript_type).collect();
            rendered.sort();
            if rendered.len() == 1 { rendered.remove(0) } else { rendered.join(" | ") }
        }
        TypeSpec::Literal(values) => {
            let literals: Vec<String> = values.iter().map(typescript_literal_value).collect();
            literals.join(" | ")
        }
    }
}

/// Renders a JSON literal as a TypeScript literal expression.
fn typescript_literal_value(value: &Value) -> String {
    match value {
        Value::Bool(true) => "true".to_string(),
        Value::Bool(false) => "false".to_string(),
        Value::Number(number) => number.to_string(),
        Value::String(value) => typescript_string_literal(value),
        _ => "null".to_string(),
    }
}

/// Renders a JSON string as a Python string literal.
///
/// Uses JSON encoding for correct escaping; falls back to a best-effort quoted
/// string on error.
fn python_string_literal(value: &str) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| format!("\"{}\"", value.replace('"', "\\\"")))
}

/// Renders a JSON string as a TypeScript string literal.
fn typescript_string_literal(value: &str) -> String {
    python_string_literal(value)
}

/// Returns true if a JSON value can be represented as a literal in SDK types.
const fn is_literal_value(value: &Value) -> bool {
    matches!(value, Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_))
}

// ============================================================================
// SECTION: Utilities
// ============================================================================

/// Converts a `snake_case` identifier into `PascalCase`.
fn pascal_case(value: &str) -> String {
    let mut output = String::new();
    for segment in value.split('_') {
        let mut chars = segment.chars();
        if let Some(first) = chars.next() {
            output.push(first.to_ascii_uppercase());
            for ch in chars {
                output.push(ch.to_ascii_lowercase());
            }
        }
    }
    if output.is_empty() { "Tool".to_string() } else { output }
}

impl fmt::Display for TypeSpec {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&python_type(self))
    }
}
