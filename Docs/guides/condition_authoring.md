<!--
Docs/guides/condition_authoring.md
============================================================================
Document: Condition Authoring Cookbook
Description: Practical guidance for writing ScenarioSpec conditions.
Purpose: Help authors define conditions that are correct, deterministic, and auditable.
Dependencies:
  - Docs/generated/decision-gate/providers.json
  - Docs/generated/decision-gate/schemas/scenario.schema.json
  - crates/decision-gate-core/src/runtime/comparator.rs
============================================================================
-->

# Condition Authoring Cookbook

## At a Glance

**What:** Define conditions that evaluate deterministically and validate cleanly
**Why:** Conditions connect gates to evidence; precision matters
**Who:** Developers and operators authoring Decision Gate scenarios
**Prerequisites:** [evidence_flow_and_execution_model.md](evidence_flow_and_execution_model.md)

---

## What is a Condition?

A condition is an evidence check:

```json dg-parse dg-level=fast
{
  "condition_id": "tests_ok",
  "query": {
    "provider_id": "json",
    "check_id": "path",
    "params": { "file": "report.json", "jsonpath": "$.summary.failed" }
  },
  "comparator": "equals",
  "expected": 0,
  "policy_tags": []
}
```

It consists of:
1. **Query**: what evidence to fetch.
2. **Comparator**: how to compare.
3. **Expected value**: the value to compare against (if required).

---

## Tri-State Outcomes

Comparators return **TriState**:
- `true`
- `false`
- `unknown`

Key rules (exact):
- If `EvidenceResult.value` is **missing**, result is `unknown` (except `exists`/`not_exists`).
- If `expected` is **missing**, result is `unknown` (except `exists`/`not_exists`).
- `equals` / `not_equals` return **false/true** on type mismatch.
- Ordering, lexicographic, contains, in_set, deep_* return `unknown` on type mismatch.

---

## Comparator Reference

### General Rules

- `exists`/`not_exists` check **presence of EvidenceResult.value**, not JSON `null`.
- JSON `null` is a **present value** (so `exists` returns true).

### Comparator Table

| Comparator | Evidence Types | Expected Required | Type Mismatch Behavior |
|-----------|----------------|-------------------|------------------------|
| `equals` | any JSON value, bytes | yes | returns **false** |
| `not_equals` | any JSON value, bytes | yes | returns **true** |
| `greater_than` / `>=` / `<` / `<=` | number or RFC3339 date/datetime string | yes | `unknown` |
| `lex_*` | string | yes | `unknown` |
| `contains` | string or array | yes | `unknown` |
| `in_set` | scalar JSON | yes (array) | `unknown` |
| `deep_equals` / `deep_not_equals` | object or array | yes | `unknown` |
| `exists` / `not_exists` | any | no | n/a |

### Details

**Equality (`equals`, `not_equals`)**
- Numbers compare via decimal-aware equality (`10` == `10.0`).
- For non-numeric types, JSON equality is used.

**Ordering (`greater_than`, etc.)**
- Accepts **numbers** or **RFC3339 date/time strings** (including date-only `YYYY-MM-DD`).
- Any other types -> `unknown`.

**Lexicographic (`lex_*`)**
- Strings only; compares Unicode code points.
- Requires explicit config + schema opt-in (see "Strict Validation").

**Contains**
- String: substring match.
- Array: evidence array must contain **all** elements of expected array.

**In Set (`in_set`)**
- Expected must be an array.
- Evidence must be scalar (not array/object).

**Deep Equals**
- Objects/arrays only; mismatched types -> `unknown`.
- Requires explicit config + schema opt-in.

**Bytes**
- Only `equals` / `not_equals` are defined.
- Expected must be JSON array of integers `0..255`.

---

## Strict Validation (Default On)

Decision Gate validates conditions at `scenario_define` time:

- For provider-based conditions, the **provider contract** defines allowed comparators and result types.
- For precheck, the **data shape schema** defines allowed comparators and types.

Special comparators require **both**:
1. Config flags (`validation.enable_lexicographic` / `validation.enable_deep_equals`), and
2. Explicit schema opt-in via `x-decision-gate.allowed_comparators` (precheck) or contract `allowed_comparators`.

If validation fails, `scenario_define` or `precheck` is rejected.

---

## Provider Patterns

### time Provider
- **Checks:** `now`, `after`, `before`.
- `after`/`before` accept `timestamp` as **unix millis** or **RFC3339 string**.
- `after` is **strictly greater-than**; `before` is **strictly less-than**.

### env Provider
- **Check:** `get`.
- Missing key returns `value = None` with **no error**.
- Blocked or invalid keys return a **provider error** (`provider_error` at the tool boundary).

### json Provider
- **Check:** `path`.
- JSONPath no-match returns `error.code = "jsonpath_not_found"` and `value = None`.

### http Provider
- **Checks:** `status`, `body_hash`.
- `status` returns an integer HTTP status.
- `body_hash` returns a HashDigest object `{ algorithm, value }`.
- `body_hash` allows only `exists` / `not_exists` (per contract).

---

## Avoiding `unknown`

To minimize `unknown` outcomes:

1. **Provide `expected`** for all comparators except `exists`/`not_exists`.
2. **Match types** exactly for non-equality comparators.
3. **Use provider contracts** to confirm allowed comparators.

---

## Evidence Disclosure

Evidence values are **not** returned by `scenario_next` by default. Use `feedback: "evidence"` if permitted, or inspect evidence via:
- Use `evidence_query` (subject to disclosure policy), or
- Export a runpack with `runpack_export`.

Disclosure policy is configured in `decision-gate.toml`:
```toml dg-parse dg-level=fast
[evidence]
allow_raw_values = false
require_provider_opt_in = true

[[providers]]
name = "json"
type = "builtin"
allow_raw = true
```

`allow_raw` is a per-provider **config** flag (not part of the provider contract).
Provider names are unique; built-in identifiers (`time`, `env`, `json`, `http`) are reserved.

---

## Checklist

- [ ] Condition IDs are unique within the scenario.
- [ ] Provider check name matches provider contract.
- [ ] Comparator is allowed for the result schema.
- [ ] Expected value type matches evidence type.
- [ ] `policy_tags` present (required by schema, may be empty).

---

## Cross-References

- [getting_started.md](getting_started.md)
- [json_evidence_playbook.md](json_evidence_playbook.md)
- [provider_schema_authoring.md](provider_schema_authoring.md)
- [ret_logic.md](ret_logic.md)
