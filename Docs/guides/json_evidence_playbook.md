<!--
Docs/guides/json_evidence_playbook.md
============================================================================
Document: JSON Evidence Playbook
Description: Template JSON schemas and predicate recipes for common workflows.
Purpose: Provide example defaults without prescribing a single canonical format.
Dependencies:
  - Docs/guides/predicate_authoring.md
  - Docs/generated/decision-gate/providers.md
============================================================================
-->

# JSON Evidence Playbook (Template Recipes)

## At a Glance

**What:** Bridge tool outputs to Decision Gate using JSON/YAML files and JSONPath queries
**Why:** Avoid arbitrary code execution while enabling gates for any tool that can emit JSON
**Who:** Developers integrating tools (tests, linters, scanners) with Decision Gate
**Prerequisites:** Predicate basics (see [predicate_authoring.md](predicate_authoring.md))

---

## Why JSON Evidence?

The built-in `json` provider reads local JSON/YAML files and evaluates JSONPath expressions. That gives you:

1. **No code execution in Decision Gate**: DG only reads artifacts.
2. **Tool-agnostic integration**: any tool that emits JSON/YAML can be gated.
3. **Deterministic evaluation**: same file + same JSONPath -> same evidence.

---

## JSONPath Essentials

JSONPath extracts values from a JSON document.

| JSONPath | Meaning | Example |
|----------|---------|---------|
| `$` | Root object | `$` (entire document) |
| `.field` | Access object field | `$.version` -> `"1.2.3"` |
| `.nested.field` | Access nested field | `$.summary.failed` -> `0` |
| `[index]` | Array element | `$.items[0]` -> first item |
| `[*]` | All array elements | `$.items[*].id` -> all IDs |
| `..field` | Recursive descent | `$..errors` -> all `errors` fields |

**Example JSON document:**
```json
{
  "version": "1.0",
  "summary": { "failed": 0, "passed": 128 },
  "items": [
    { "id": 1, "status": "pass" },
    { "id": 2, "status": "fail" }
  ]
}
```

**Example queries:**
- `$.version` -> `"1.0"`
- `$.summary.failed` -> `0`
- `$.items[*].id` -> `[1, 2]`

---

## JSON Provider Match Behavior

The `json` provider behaves as follows:

- **If `params.jsonpath` is omitted** -> returns the entire JSON/YAML document.
- **If JSONPath matches one value** -> returns that value.
- **If JSONPath matches multiple values** -> returns an array of matched values.
- **If JSONPath matches nothing** -> returns `EvidenceResult.error` with code `jsonpath_not_found` and `value = None`.
- **If JSONPath is invalid** -> returns `EvidenceResult.error` with code `invalid_jsonpath` and `value = None`.

The provider does **not** return JSON `null` for missing paths; it returns **no value** (`value = None`).

---

## JSON Provider File and Parsing Rules

- **File size limit:** `max_bytes` (default `1_048_576`). Oversize returns `size_limit_exceeded`.
- **YAML support:** enabled when `allow_yaml = true` and file extension is `.yaml`/`.yml`.
- **Root restriction:** if `root` is set, all paths must resolve within it; otherwise `path_outside_root`.
- **Parsing errors:** invalid JSON -> `invalid_json`; invalid YAML -> `invalid_yaml`; YAML disabled -> `yaml_disabled`.

All file/JSONPath errors are returned via `EvidenceResult.error` (not JSON-RPC errors).

---

## Workflow Pattern: Tool -> JSON -> Gate

```
1. Run tool (outside DG) -> emit JSON file
2. Define predicate (json.path + comparator + expected)
3. scenario_next -> DG reads file, extracts value, evaluates predicate
```

Example predicate:
```json
{
  "predicate": "tests_ok",
  "query": {
    "provider_id": "json",
    "predicate": "path",
    "params": { "file": "test-results.json", "jsonpath": "$.summary.failed" }
  },
  "comparator": "equals",
  "expected": 0,
  "policy_tags": []
}
```

---

## Template Recipes

These are **examples**, not standards. Use them as starting points.

### Template 1: Test Results

**Tool output (example):**
```json
{
  "summary": { "failed": 0, "passed": 128 },
  "tool": "tests",
  "version": "1.0"
}
```

**Predicate:**
```json
{
  "predicate": "tests_ok",
  "query": {
    "provider_id": "json",
    "predicate": "path",
    "params": { "file": "report.json", "jsonpath": "$.summary.failed" }
  },
  "comparator": "equals",
  "expected": 0,
  "policy_tags": []
}
```

**Type mismatch (exact behavior):**
```json
{ "summary": { "failed": "0" } }
```
- Evidence value is **string**; expected is **number**.
- `equals` returns **false** (not `unknown`).

### Template 2: Coverage Threshold

**Tool output (example):**
```json
{ "coverage": { "percent": 92 } }
```

**Predicate:**
```json
{
  "predicate": "coverage_ok",
  "query": {
    "provider_id": "json",
    "predicate": "path",
    "params": { "file": "coverage.json", "jsonpath": "$.coverage.percent" }
  },
  "comparator": "greater_than_or_equal",
  "expected": 85,
  "policy_tags": []
}
```

**Type mismatch for ordering comparators:**
```json
{ "coverage": { "percent": "92%" } }
```
- Evidence value is **string**, not a number or RFC3339 date.
- `greater_than_or_equal` returns **unknown**.

### Template 3: Security Scan Summary

**Tool output (example):**
```json
{ "summary": { "critical": 0, "high": 0, "medium": 2 } }
```

**Predicate:**
```json
{
  "predicate": "no_critical",
  "query": {
    "provider_id": "json",
    "predicate": "path",
    "params": { "file": "scan.json", "jsonpath": "$.summary.critical" }
  },
  "comparator": "equals",
  "expected": 0,
  "policy_tags": []
}
```

### Template 4: Review Approval Count

**Tool output (example):**
```json
{ "reviews": { "approvals": 2 } }
```

**Predicate:**
```json
{
  "predicate": "approvals_ok",
  "query": {
    "provider_id": "json",
    "predicate": "path",
    "params": { "file": "reviews.json", "jsonpath": "$.reviews.approvals" }
  },
  "comparator": "greater_than_or_equal",
  "expected": 2,
  "policy_tags": []
}
```

### Template 5: Explicit Boolean Flags

**Tool output (example):**
```json
{ "checks": { "lint_ok": true, "format_ok": true } }
```

**Predicate:**
```json
{
  "predicate": "lint_ok",
  "query": {
    "provider_id": "json",
    "predicate": "path",
    "params": { "file": "quality.json", "jsonpath": "$.checks.lint_ok" }
  },
  "comparator": "equals",
  "expected": true,
  "policy_tags": []
}
```

---

## Debugging JSON Evidence Precisely

### Use `evidence_query` for Provider Errors

`scenario_next` does **not** return evidence values or provider errors. To debug JSON evidence:
1. Call `evidence_query` with the same `query` and any valid `context`.
2. Inspect `EvidenceResult.error` and `evidence_anchor`.

### Common JSON Provider Error Codes

These codes come from the built-in `json` provider:

- `params_missing`, `params_invalid`
- `file_not_found`, `file_open_failed`, `file_read_failed`
- `size_limit_invalid`, `size_limit_exceeded`
- `invalid_json`, `invalid_yaml`, `yaml_disabled`
- `invalid_jsonpath`, `jsonpath_not_found`
- `invalid_root`, `path_outside_root`

Other providers may return `provider_error` (a wrapper around internal failures).

---

## Design Guidance (Stable JSON Schemas)

- Keep summaries **flat** and **stable**.
- Prefer **scalars** (numbers, booleans) for comparator friendliness.
- Avoid deep nesting (`$.a.b.c.d`) unless required.
- Include a `version` field if you control the schema and expect evolution.

---

## Cross-Reference Learning Paths

- [getting_started.md](getting_started.md) -> [predicate_authoring.md](predicate_authoring.md) -> **THIS GUIDE**
- **THIS GUIDE** -> [provider_development.md](provider_development.md) (when JSON isn't enough)
- **THIS GUIDE** -> [llm_native_playbook.md](llm_native_playbook.md) (precheck workflows)

---

## Glossary

**JSONPath:** Query language for extracting values from JSON documents.
**Comparator:** Operator comparing evidence to expected values.
**Evidence:** Provider output plus metadata (hashes, anchors, errors).
**Predicate:** Evidence check definition in a scenario.
