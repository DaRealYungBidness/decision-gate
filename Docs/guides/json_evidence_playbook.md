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

## Purpose
These are **example defaults** designed to make JSON evidence immediately
usable. They are not prescriptive standards. Treat them as templates you can
extend, replace, or document in your own organization.

## Template 1: Test Results
**Tool output**
```json
{
  "summary": { "failed": 0, "passed": 128 },
  "tool": "tests",
  "version": "1.0"
}
```

**Predicate**
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

## Template 2: Coverage Threshold
**Tool output**
```json
{ "coverage": { "percent": 92 } }
```

**Predicate**
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

## Template 3: Security Scan Summary
**Tool output**
```json
{
  "summary": { "critical": 0, "high": 0, "medium": 2 },
  "tool": "scanner"
}
```

**Predicate**
```json
{
  "predicate": "scan_ok",
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

## Template 4: Review Approval Count
**Tool output**
```json
{ "reviews": { "approvals": 2 } }
```

**Predicate**
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

## Template 5: Explicit Boolean Gates
**Tool output**
```json
{ "checks": { "lint_ok": true, "format_ok": true } }
```

**Predicate**
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

## Error Recovery Guidance
Missing JSONPath yields `value = null` with structured error metadata (e.g.,
`jsonpath_not_found`). Agents should treat this as a **pipeline mismatch** and
update either the JSONPath or the tool output.

## Design Guidance
- Keep summaries short and stable.
- Use explicit booleans or scalar counts whenever possible.
- Avoid deeply nested, unstable schemas unless absolutely necessary.
