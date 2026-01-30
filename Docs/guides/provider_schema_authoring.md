<!--
Docs/guides/provider_schema_authoring.md
============================================================================
Document: Provider Schema Authoring Guide (Self-Contained)
Description: Exhaustive, standalone guide for authoring Decision Gate provider contracts.
Purpose: Enable accurate provider contract authoring without any other documents.
Dependencies: None required (this file is standalone).
============================================================================
-->

# Provider Schema Authoring Guide (Self-Contained)

This document is exhaustive and standalone. You can upload this file alone
and it is sufficient to produce a correct Decision Gate provider contract JSON.

If you only remember one sentence:
**Decision Gate evaluates evidence. It does not execute arbitrary tasks.**

---

## 1) What Decision Gate Is (Goal and Mental Model)

Decision Gate (DG) answers a single question:
**"Has X been done?"**

It does this by evaluating **evidence** against **predicates**, combining the
results with a **Requirement Evaluation Tree (RET)**, and deciding whether a
**gate** passes.

DG is **data-first**:
- Tools, scripts, or services run **outside** DG.
- Those tools emit evidence (JSON or bytes).
- DG evaluates that evidence deterministically.

### Core evaluation pipeline
```
EvidenceQuery -> Provider -> EvidenceResult -> Comparator -> Tri-state
               -> RET -> Gate outcome
```

- **Tri-state outcomes** are `true`, `false`, or `unknown`.
- **Missing evidence** or **provider error** yields `unknown` (fail-closed).
- Comparator type mismatches may yield `unknown` or `false/true` depending on the
  comparator (see Section 7).
- Gates only pass when the RET evaluates to `true`.

### Evidence sourcing modes
1. **Provider-pulled evidence (live runs)**
   - DG calls a provider to fetch evidence.
2. **Asserted evidence (precheck only)**
   - The caller supplies evidence directly; no run state is mutated.
3. **Audit submissions (`scenario_submit`)**
   - Stored for audit, but does not affect evaluation.

This guide focuses on **providers** and their **provider contracts**.

---

## 2) Glossary (Essential Terms)

- **Provider**: A data source that can answer evidence queries.
- **Provider contract**: A JSON document describing provider capabilities,
  schemas, and allowed comparators (this is what you are authoring).
- **Provider check**: A single queryable capability exposed by a provider.
  Provider checks live inside the provider contract under `predicates`.
- **Scenario predicate**: A predicate in a ScenarioSpec that references a
  provider check via `query.predicate`. Do not confuse these two concepts.
- **EvidenceQuery**: `{ provider_id, predicate, params }` sent to a provider.
- **EvidenceResult**: Provider response with `value` plus optional hash/anchor.
- **Comparator**: Operator that compares evidence to an expected value.
- **RET**: Requirement Evaluation Tree that combines predicate outcomes.
- **Determinism**: Classification of provider checks:
  - `deterministic`, `time_dependent`, or `external`.

---

## 3) Provider Protocol (What Providers Actually Do)

Providers implement a single MCP tool named `evidence_query` and return an
`EvidenceResult` inside the tool response.

### EvidenceQuery shape
```json
{
  "provider_id": "string",
  "predicate": "string",
  "params": { "any": "json" }
}
```

- `params` is optional. If present, it must match the predicate's `params_schema`.
- Most built-in providers require `params` to be a JSON object.

### EvidenceResult shape (value is what your contract describes)
```json
{
  "value": { "kind": "json|bytes", "value": "any" } | null,
  "lane": "verified|asserted",
  "error": { "code": "string", "message": "string", "details": "any|null" } | null,
  "evidence_hash": { "algorithm": "sha256", "value": "hex" } | null,
  "evidence_ref": { "uri": "string" } | null,
  "evidence_anchor": { "anchor_type": "string", "anchor_value": "string" } | null,
  "signature": { "scheme": "string", "key_id": "string", "signature": [0] } | null,
  "content_type": "string|null"
}
```

Notes:
- All fields are required in the schema; use `null` when absent.
- `evidence_anchor.anchor_value` is a **string**. If an anchor policy requires
  fields, this string must be canonical JSON for an object (see Section 9).
- `signature.signature` is a byte array (JSON array of integers 0-255).

Your provider contract **must** describe the `params` and the `value` shape
returned here. DG never trusts providers to decide gate outcomes.

---

## 4) What a Provider Contract Is (Why It Exists)

A provider contract (capability contract) is the **schema of evidence**.
It declares:
- Which provider checks exist (names + descriptions).
- Required params for each check.
- Result schemas for each check.
- Which comparators are allowed for each check.
- Determinism classification, anchors, content types, and examples.

DG uses the contract to:
- Validate ScenarioSpec predicates.
- Enforce comparator allow-lists per predicate.
- Generate tooling and UI forms.
- Support discovery for LLMs and authoring tools.

If a provider contract is wrong or incomplete, **Scenario authoring will be
wrong**, and evaluation will fail closed.

---

## 5) Provider Contract JSON: Complete Specification

A contract is a single JSON object with the following **required** fields.
Fields marked optional are allowed to be omitted.

### 5.1 Top-level fields (required unless noted)

- `provider_id` (string, required)
  - Stable identifier used in `EvidenceQuery.provider_id`.
  - Use `snake_case` and keep it stable.

- `name` (string, required)
  - Human-readable name for UI and tooling.

- `description` (string, required)
  - What evidence source this provider represents.

- `transport` (string, required)
  - Must be either `"mcp"` (external providers) or `"builtin"` (built-ins).
  - External provider contracts **must** set `transport: "mcp"` or they will be rejected.

- `notes` (array of strings, required)
  - Freeform notes for humans and LLMs.
  - Use `[]` if there are no notes.

- `config_schema` (JSON Schema, required)
  - Schema for the provider's config block in `decision-gate.toml`.
  - The contract schema accepts any JSON Schema value, but **Decision Gate tooling
    assumes an object schema**. Use an object schema with `additionalProperties: false`.
  - If the provider has no config, use:
    ```json
    { "type": "object", "additionalProperties": false, "properties": {} }
    ```

- `predicates` (array, required)
  - Each entry is a **provider check** definition.
  - The array may be empty, but then the provider exposes no usable checks.

### 5.2 Predicate fields (all required)

Each entry in `predicates` defines one provider check.

- `name` (string)
  - Stable identifier used in `EvidenceQuery.predicate`.
  - Use `snake_case` and keep it stable.

- `description` (string)
  - Describe **what the returned value means**, not how it is fetched.

- `determinism` (string)
  - One of: `deterministic`, `time_dependent`, `external`.

- `params_required` (boolean)
  - True if params must be provided; false if params are optional or none.

- `params_schema` (JSON Schema)
  - Schema for `EvidenceQuery.params`.
  - Use an object schema and set `additionalProperties: false` (recommended).
  - If no params, use:
    ```json
    { "type": "object", "additionalProperties": false, "properties": {} }
    ```

- `result_schema` (JSON Schema)
  - Schema for the evidence value (`EvidenceResult.value.value`).
  - Use the **tightest** schema possible.

- `allowed_comparators` (array of strings)
  - Comparator allow-list for this predicate.
  - Must be **non-empty** and in **canonical order** (see Section 7 list).
  - Should be compatible with `result_schema` and strict validation rules.

- `anchor_types` (array of strings)
  - Anchor kinds this predicate may emit.
  - Use `[]` if none.

- `content_types` (array of strings)
  - MIME types for `EvidenceResult.content_type`.
  - Can be empty (`[]`) to mean "unspecified".
  - If present, use valid MIME types (e.g., `application/json`).

- `examples` (array of objects)
  - Each example is `{ "description": "...", "params": { ... }, "result": ... }`.
  - The schema allows an empty array, but **examples are strongly recommended**.

---

## 6) JSON Schema Guidance (Params and Results)

DG uses JSON Schema (draft 2020-12) to validate params and results.
Keep schemas **precise** and **minimal**.

### Params schema rules (recommended)
- Use `type: "object"`.
- Set `additionalProperties: false`.
- Declare `required` for every required field.
- Prefer explicit types and bounds.

### Result schema rules
- Describe the actual evidence value, not the EvidenceResult wrapper.
- Use the narrowest possible type (`boolean`, `integer`, `number`, `string`,
  `array`, `object`, or `null`).
- If you need multiple types, use `oneOf` / `anyOf`. Strict validation
  intersects comparator allowances across all variants.

### Bytes results (special case)
If the provider returns `EvidenceValue::Bytes`, the evidence value is compared
as bytes. Recommended schema:
```json
{
  "type": "array",
  "items": { "type": "integer", "minimum": 0, "maximum": 255 }
}
```
For byte evidence, **only** `equals` and `not_equals` are valid at runtime.
Constrain `allowed_comparators` accordingly.

### Dynamic results (escape hatch)
If a predicate can return arbitrary JSON shapes that cannot be expressed, mark
the result schema as dynamic:
```json
{
  "description": "Dynamic JSON result",
  "x-decision-gate": { "dynamic_type": true }
}
```
Dynamic schemas allow all comparators in strict validation, but **lex/deep**
comparators still require config flags (see Section 8).

---

## 7) Comparator Semantics and Compatibility

Comparators convert evidence into tri-state outcomes. For comparators that
require `expected`, missing `expected` yields `unknown`.

### Comparator list (canonical order)
- `equals`
- `not_equals`
- `greater_than`
- `greater_than_or_equal`
- `less_than`
- `less_than_or_equal`
- `lex_greater_than`
- `lex_greater_than_or_equal`
- `lex_less_than`
- `lex_less_than_or_equal`
- `contains`
- `in_set`
- `deep_equals`
- `deep_not_equals`
- `exists`
- `not_exists`

Use this order when authoring `allowed_comparators`.

### Runtime semantics (Decision Gate core)
- **equals / not_equals**: JSON equality. Numbers are compared as decimals
  (`10 == 10.0`). Type mismatches return `false` / `true` respectively.
- **greater_than / less_than (and _or_equal)**:
  - Numeric ordering for JSON numbers.
  - Temporal ordering for strings that parse as RFC3339 date-time or date-only
    (`YYYY-MM-DD`) **on both sides**. Otherwise `unknown`.
- **lex_***: Lexicographic ordering for strings only; otherwise `unknown`.
- **contains**:
  - Strings: substring containment.
  - Arrays: evidence array contains **all** elements of the expected array
    (membership-only, not multiset counts).
- **in_set**:
  - Expected must be an array.
  - Evidence must be scalar (not object/array). Membership uses JSON equality.
- **exists / not_exists**:
  - Ignore `expected`.
  - Check only whether `EvidenceResult.value` is present.
  - JSON `null` counts as **present**.
- **deep_equals / deep_not_equals**:
  - Only for arrays or objects; otherwise `unknown`.

### Strict validation compatibility (default in decision-gate-mcp)

Decision Gate MCP performs strict comparator/type validation by default.
A ScenarioSpec comparator must:
1. Be present in the provider contract's `allowed_comparators` list.
2. Be allowed by the result schema type classification.
3. Be enabled by config (lex/deep families are disabled by default).

**Allowances by result schema type (strict validation):**

- **boolean**
  - Allowed: `equals`, `not_equals`, `in_set`, `exists`, `not_exists`

- **integer / number**
  - Allowed: `equals`, `not_equals`, `greater_than`, `greater_than_or_equal`,
    `less_than`, `less_than_or_equal`, `in_set`, `exists`, `not_exists`

- **string (no format)**
  - Allowed: `equals`, `not_equals`, `contains`, `in_set`, `exists`, `not_exists`
  - Opt-in: `lex_*` (see Section 8)

- **string with `format: "date"` or `format: "date-time"`**
  - Allowed: `equals`, `not_equals`, `greater_than`, `greater_than_or_equal`,
    `less_than`, `less_than_or_equal`, `in_set`, `exists`, `not_exists`

- **string with `format: "uuid"`**
  - Allowed: `equals`, `not_equals`, `in_set`, `exists`, `not_exists`

- **enum (scalar values only)**
  - Allowed: `equals`, `not_equals`, `in_set`, `exists`, `not_exists`

- **array with scalar items**
  - Allowed: `contains`, `exists`, `not_exists`
  - Opt-in: `deep_equals`, `deep_not_equals` (see Section 8)

- **array with complex items**
  - Allowed: `exists`, `not_exists`
  - Opt-in: `deep_equals`, `deep_not_equals`

- **object**
  - Allowed: `exists`, `not_exists`
  - Opt-in: `deep_equals`, `deep_not_equals`

- **null**
  - Allowed: `equals`, `not_equals`, `exists`, `not_exists`

- **dynamic (`x-decision-gate.dynamic_type: true`)**
  - Allowed: all comparators (subject to config flags)

If strict validation is disabled (`validation.strict = false` with
`validation.allow_permissive = true`), schema compatibility checks are skipped,
but runtime comparator semantics still apply.

---

## 8) Opt-in Comparators (Lex and Deep)

Lexicographic and deep equality comparators are **opt-in** in strict validation
for non-dynamic schemas and require two steps:

1. **Enable in `decision-gate.toml`:**
   - `validation.enable_lexicographic = true`
   - `validation.enable_deep_equals = true`

2. **Declare opt-ins in the result schema** (for non-dynamic schemas):
```json
"result_schema": {
  "type": "string",
  "x-decision-gate": {
    "allowed_comparators": ["lex_greater_than", "lex_less_than"]
  }
}
```

Additional rules:
- The comparator must also appear in the provider contract's
  `allowed_comparators` list.
- If `x-decision-gate.allowed_comparators` is present, **every** comparator
  used by scenarios must be listed there.
- For dynamic schemas (`dynamic_type: true`), schema opt-in is not required,
  but config flags still apply.

---

## 9) Evidence Metadata: Anchors and Content Types

Contracts declare what **evidence metadata** a provider can emit.

- **anchor_types**: Strings describing anchor kinds (for audit or external
  references). Examples: `file_path`, `url`, `receipt_id`, `log_offset`.
  Use `[]` if no anchors are emitted.

- **content_types**: MIME types of the evidence value. Examples:
  - `application/json`
  - `text/plain`
  - `application/octet-stream`

`EvidenceResult.content_type` values should match this list when present.
An empty list means "unspecified" (policies treat it as a wildcard).

**Anchor value note:**
`EvidenceResult.evidence_anchor.anchor_value` is a **string**. If you configure
an anchor policy that requires fields, this string must be canonical JSON for a
single object with scalar fields.

---

## 10) Determinism Classification (Be Honest)

- **deterministic**: Output depends only on inputs and internal state.
- **time_dependent**: Output depends on time, but is deterministic given time.
- **external**: Output depends on external state (network, DB, APIs).

This classification is used for audit and trust reasoning. Do not mislabel.

---

## 11) End-to-End Authoring Workflow (Humans or LLMs)

### Step 1: Gather required inputs
You should have all of the following for each provider check:
- Predicate name and description (what the result means).
- Determinism classification.
- Params schema (JSON Schema or full param list with types/required).
- Result schema (JSON Schema for the evidence value).
- Allowed comparators (or allow the author to derive them strictly).
- At least one example params + result pair (recommended).
- Anchor types and content types (or explicit "none").

### Step 2: Map inputs to contract fields
- Convert names to stable `snake_case` identifiers.
- Translate request types into `params_schema`.
- Translate response values into `result_schema`.
- Choose comparator allow-lists from Section 7 and strict validation rules.

### Step 3: Validate internally
- Ensure `params_schema` and `result_schema` are valid JSON Schemas.
- Ensure `allowed_comparators` is non-empty and in canonical order.
- Ensure comparator lists are compatible with strict validation rules.
- Ensure examples (if provided) match schemas.
- Ensure `content_types` and `anchor_types` are accurate.

### Step 4: Produce the final JSON
- Output JSON only.
- Avoid markdown in the final output.

---

## 12) Provider Intake Worksheet (Template)

Copy/paste and fill this out before authoring the contract:

```
Provider
- provider_id:
- name:
- description:
- transport: mcp | builtin
- config_schema (JSON Schema):
- notes (required; use [] if none):

Predicates (repeat per check)
- name:
- description (what the value means):
- determinism: deterministic | time_dependent | external
- params_required: true | false
- params_schema (JSON Schema):
- result_schema (JSON Schema):
- allowed_comparators (if known):
- anchor_types (array or empty):
- content_types (array; can be empty):
- examples (recommended):
  - description:
  - params:
  - result:
```

If any field is missing, do not guess. Ask for it explicitly.

---

## 13) LLM Authoring Prompt (Strict, Self-Contained)

Use this prompt **verbatim** with an LLM, along with the completed worksheet
and any OpenAPI or type definitions. This prompt is designed to prevent
hallucination and force explicit questions when information is missing.

```
You are generating a Decision Gate provider contract JSON.

Rules:
- Output JSON only (no markdown).
- Do not assume missing information. If any required field is missing or
  ambiguous, output a JSON object with a single key "questions" containing an
  array of clarifying questions, then stop.
- Use this exact contract shape:
  provider_id, name, description, transport, config_schema, notes, predicates.
- Each predicate must include:
  name, description, determinism, params_required, params_schema, result_schema,
  allowed_comparators, anchor_types, content_types, examples.
- allowed_comparators must be non-empty and in canonical order.
- Comparators must be compatible with strict validation rules for the result_schema.
- If you include lex_* or deep_* comparators and the result_schema is not dynamic,
  also add result_schema.x-decision-gate.allowed_comparators.
- Note that server config must enable lex/deep comparator families for them to be used.
- Provide at least one example per predicate unless explicitly told to omit.

Inputs:
- Provider worksheet:
- OpenAPI/DTO schemas (if any):
- Example requests/responses (if any):

Return the final contract JSON.
```

---
