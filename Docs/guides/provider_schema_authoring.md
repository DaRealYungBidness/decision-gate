<!--
Docs/guides/provider_schema_authoring.md
============================================================================
Document: Provider Schema Authoring Guide
Description: LLM-ready guide for authoring Decision Gate provider contracts.
Purpose: Help teams map code/OpenAPI into provider contracts and validate them.
Dependencies:
  - Docs/generated/decision-gate/providers.json
  - Docs/guides/provider_development.md
  - Docs/guides/provider_protocol.md
  - Docs/guides/predicate_authoring.md
============================================================================
-->

# Provider Schema Authoring Guide

## Overview
Decision Gate providers must ship a **provider contract** (capability contract).
This contract is the schema for every provider check: params, result schema,
comparator allow-list, evidence outputs, and examples. It is required for MCP
providers and is used to validate ScenarioSpec predicates before any run starts.

This guide is designed to be LLM-friendly and can be given to an agent along
with your codebase or OpenAPI. The goal is to produce a correct JSON contract
that you can import into the Decision Gate builder for inspection.

If you can express your evidence as JSON files instead of a custom MCP service,
prefer the JSON provider path. It is the lowest-friction integration and still
supports strict comparator validation.

## Inputs the Authoring Process Needs
Provide these to the author (human or LLM):

1. **Provider identity**
   - `provider_id` (snake_case)
   - display name and description

2. **Evidence source inventory**
   - What systems or datasets the provider queries
   - How each query maps to a provider check

3. **Per-check examples**
   - Sample params payloads
   - Sample results (JSON, numbers, strings, booleans, null)

4. **Schemas**
   - Params schema for each check (JSON Schema)
   - Result schema for each check (JSON Schema)

5. **Output metadata**
   - `content_types` for evidence outputs (MIME types)
   - `anchor_types` if the provider emits anchors

6. **Determinism classification**
   - `deterministic`, `time_dependent`, or `external`

If you have OpenAPI, share the request/response schemas for the endpoints that
represent evidence checks. If you have code, share the types or DTOs used for
inputs and outputs. Provide at least one example per check whenever possible.

## Output: Provider Contract JSON
A provider contract is a single JSON object with this top-level shape:

```json
{
  "provider_id": "example",
  "name": "Example Provider",
  "description": "Short summary of evidence source.",
  "transport": "mcp",
  "notes": ["Optional notes"],
  "config_schema": { "type": "object", "additionalProperties": false, "properties": {} },
  "predicates": [
    {
      "name": "check_name",
      "description": "What this check returns.",
      "determinism": "external",
      "params_required": true,
      "params_schema": { "type": "object", "additionalProperties": false, "properties": {} },
      "result_schema": { "type": "string" },
      "allowed_comparators": ["equals", "not_equals", "exists", "not_exists"],
      "anchor_types": [],
      "content_types": ["application/json"],
      "examples": [
        { "description": "Happy path.", "params": {}, "result": "ok" }
      ]
    }
  ]
}
```

Use `Docs/generated/decision-gate/providers.json` as a canonical reference for
field names and examples.

## Predicate Field Guidance (Per Check)
Each entry in `predicates` defines a **provider check**.

- `name`: stable identifier referenced by ScenarioSpec predicates.
- `description`: describe the returned value, not the query mechanics.
- `determinism`:
  - `deterministic`: output depends only on inputs.
  - `time_dependent`: output varies by time but is deterministic given time.
  - `external`: output depends on external state or network.
- `params_required`: true if params are required for this check.
- `params_schema`: JSON Schema for params. Use `additionalProperties: false`.
- `result_schema`: JSON Schema describing the evidence value.
- `allowed_comparators`: allow-list of comparator strings.
- `anchor_types`: stable anchor type strings (if emitted).
- `content_types`: MIME types of the evidence payload.
- `examples`: array of sample `{description, params, result}` entries.

## Evidence Outputs: anchor_types and content_types
These fields describe what the provider emits:

- `anchor_types`: Strings describing anchor kinds emitted in evidence anchors.
  Examples: `file_path`, `url`, `receipt_id`, `log_offset`. Use empty array if
  the provider does not emit anchors.

- `content_types`: MIME types of the evidence value. Use `application/json` for
  JSON outputs, `text/plain` for raw strings, or multiple types if applicable.
  Always include at least one content type.

Decision Gate uses these for auditing, disclosure policy, and downstream tools.

## Examples (Strongly Recommended)
Examples are not required for runtime validation but are essential for:

- LLM-guided authoring
- Builder previews and sanity checks
- Scenario authoring guidance

Include at least one example per predicate with representative params and
results. Keep examples deterministic and free of secrets.

## Comparator Compatibility (Strict Mode)
Comparator allow-lists must be compatible with the result schema. Decision Gate
validates this in strict mode and fails closed on mismatches.

Key rules:
- Numeric comparators only work with `integer` or `number` result schemas.
- `contains` is for strings and arrays.
- `in_set` requires `expected` to be an array in ScenarioSpec.
- `exists` / `not_exists` ignore `expected`.

### Opt-in comparators
Lexicographic and deep equality comparators are opt-in and require two steps:

1) Enable in `decision-gate.toml`:
   - `validation.enable_lexicographic = true`
   - `validation.enable_deep_equals = true`

2) Declare them in the result schema under `x-decision-gate.allowed_comparators`.

Example:

```json
"result_schema": {
  "type": "string",
  "x-decision-gate": {
    "allowed_comparators": ["lex_greater_than", "lex_less_than"]
  }
}
```

### Dynamic results
If a predicate can return any JSON shape (for example, JSONPath output), mark
it as dynamic:

```json
"result_schema": {
  "description": "Dynamic JSON result",
  "x-decision-gate": { "dynamic_type": true }
}
```

Dynamic schemas permit the full comparator set (still gated by config). Use
this only when you cannot express a fixed JSON schema.

## OpenAPI to Contract Mapping (Practical Recipe)
If you have OpenAPI:

1. **Identify endpoints** that represent evidence checks.
2. For each endpoint, define a predicate name and description.
3. Map request params to `params_schema`.
4. Map the response field (or subset) to `result_schema`.
5. Choose `allowed_comparators` based on the result schema type.
6. Add `examples` using real sample requests/responses.

If the response is a large object, consider:
- creating a narrower predicate for a single field, or
- using the JSON provider with JSONPath for file-backed snapshots.

## LLM Authoring Prompt (Copy/Paste)
Use this with an LLM along with your codebase/OpenAPI and example payloads.

```
You are generating a Decision Gate provider contract JSON.

Requirements:
- Output JSON only, no markdown.
- Include provider_id, name, description, transport="mcp", config_schema, notes, predicates.
- For each predicate include: name, description, determinism, params_required,
  params_schema, result_schema, allowed_comparators, anchor_types, content_types, examples.
- Use additionalProperties: false for params_schema.
- allowed_comparators must be compatible with result_schema type.
- If you include lex_* or deep_* comparators, add them to result_schema under
  x-decision-gate.allowed_comparators and note that server config must enable them.
- Provide at least one example per predicate.

Inputs:
- Provider description:
- Endpoint list and OpenAPI schemas:
- Example requests/responses:
- Anchor/content types (if any):

Return the final contract JSON.
```

## Validation Checklist
Before shipping:
- Import the JSON into the Decision Gate builder and confirm no errors.
- Ensure every predicate has non-empty `allowed_comparators`.
- Ensure every predicate has a valid `params_schema` and `result_schema`.
- Verify `content_types` includes at least one MIME type.
- Confirm opt-in comparators are enabled in config and in `x-decision-gate`.

## Related Guides
- `Docs/guides/provider_development.md`
- `Docs/guides/provider_protocol.md`
- `Docs/guides/predicate_authoring.md`
- `Docs/generated/decision-gate/providers.json`
