<!--
Docs/roadmap/strict_comparator_validation_plan.md
============================================================================
Document: Decision Gate Strict Comparator Validation Plan
Description: Proposal for default-on strict validation of predicate comparator
             compatibility using schema-derived type classes and optional
             domain constraints.
Purpose: Define the authoring-time rules to prevent invalid comparator/type
         usage from reaching runtime, with a formal compatibility matrix.
Dependencies:
  - Docs/roadmap/trust_lanes_registry_plan.md
  - decision-gate-core/src/runtime/comparator.rs
============================================================================
-->

# Decision Gate Strict Comparator Validation Plan

This proposal defines a strict, default-on validation layer that prevents
invalid predicate comparator usage from being registered or evaluated. It
introduces a formal type-class compatibility matrix, optional domain-specific
comparator constraints, and explicit semantics for numerics, formats, and
unions. The intent is to eliminate authoring footguns while preserving maximum
expressiveness through explicit opt-ins.

## Goals

- Reject invalid comparator/type combinations at scenario registration and
  precheck time (no invalid specs reach runtime).
- Provide a formal, documented comparator compatibility matrix.
- Support decimal numeric comparison with explicit semantics.
- Allow domain owners to further restrict comparators per field (subset only).
- Keep strict validation default-on and opt-out for permissive/legacy behavior.

## Non-Goals

- Replace runtime evaluator semantics; this is an authoring/validation layer.
- Add UI; this plan defines the validation rules the UI will implement.
- Implicit type coercions or "best effort" conversions.

## Terminology

- Type class: Normalized semantic type derived from JSON Schema (string,
  integer, number, boolean, enum, array<scalar>, object, null).
- Comparator: Predicate operator (equals, greater_than, contains, etc.).
- Possible comparators: Comparator set that is semantically valid for a type
  class (engine-compatible).
- Allowed comparators: Domain-specific subset of possible comparators.
- Strict validation mode: Default-on MCP behavior that rejects invalid specs.

## Strict Validation Mode (Default)

When enabled, the MCP layer rejects invalid predicate definitions before they
are registered:

- `scenario_define`: reject any predicate whose comparator is not valid for the
  schema-derived type class or violates domain constraints.
- `precheck`: reject any asserted payload or predicate that violates the same
  constraints (no runtime Unknown for invalid combos).

Permissive mode may still exist for legacy/experiments; it allows invalid
combos to reach runtime, where comparator evaluation yields `Unknown`.

## World-Class Defaults (Strict Core V1)

- Decimal comparisons use exact decimal arithmetic; integer schemas reject
  decimal values.
- No implicit coercions; comparator and expected value must match the schema
  type.
- Numeric ordering uses `gt/gte/lt/lte` for integer/number only.
- String ordering is opt-in only via lexicographic comparators.
- Array/object equality is opt-in only via deep equality comparators.
- Union types are rejected unless comparator validity is guaranteed across all
  branches.
- Formats are explicit and standards-aligned: `date`/`date-time` use RFC 3339
  and allow ordering; `uuid` uses RFC 4122 and is equality only.
- Null equality is opt-in only when schema explicitly permits null.

## Comparator Compatibility Matrix (Strict Core V1)

Legend:

- Y = allowed
- N = rejected
- O = allowed only with explicit opt-in

Type class \ Comparator:

| Type class       | equals | not_equals | gt  | gte | lt  | lte | contains | in_set | exists | not_exists |
| ---------------- | ------ | ---------- | --- | --- | --- | --- | -------- | ------ | ------ | ---------- |
| boolean          | Y      | Y          | N   | N   | N   | N   | N        | Y      | Y      | Y          |
| integer          | Y      | Y          | Y   | Y   | Y   | Y   | N        | Y      | Y      | Y          |
| number (decimal) | Y      | Y          | Y   | Y   | Y   | Y   | N        | Y      | Y      | Y          |
| string           | Y      | Y          | N   | N   | N   | N   | Y        | Y      | Y      | Y          |
| enum             | Y      | Y          | N   | N   | N   | N   | N        | Y      | Y      | Y          |
| array<scalar>    | N      | N          | N   | N   | N   | N   | Y        | N      | Y      | Y          |
| object           | N      | N          | N   | N   | N   | N   | N        | N      | Y      | Y          |
| null             | O      | O          | N   | N   | N   | N   | N        | N      | Y      | Y          |

Notes:

- string ordering is opt-in (lexicographic) via optional comparators.
- array/object equality is opt-in (deep equality) via optional comparators.
- null equality is opt-in and only if schema explicitly permits null.

## Schema to Type Class Mapping

Type class is derived deterministically from JSON Schema:

- boolean: `type: "boolean"`
- integer: `type: "integer"`
- number: `type: "number"` (decimal semantics)
- string: `type: "string"` without enum/format override
- enum: `enum: [...]` (string or integer only)
- array<scalar>: `type: "array"` with `items` that map to a scalar type class
- object: `type: "object"`
- null: `type: "null"`

Union types:

- `oneOf`/`anyOf`/`type: [..]` are rejected unless all branches support the
  comparator and the expected value is valid for all branches. Otherwise the
  predicate is invalid in strict mode.
- Nullable unions (e.g., `type: ["string", "null"]` or `oneOf` with a null
  branch) treat null as optional and do not further restrict comparator
  compatibility; validation still rejects comparator-specific null expectations
  (e.g., `contains` with null).

Formats:

- `format: "date-time"` implies comparable timestamps (ordering allowed),
  parsed as RFC 3339 with required timezone offset or `Z`.
- `format: "date"` implies comparable dates (ordering allowed), parsed as RFC
  3339 full-date (`YYYY-MM-DD`).
- `format: "uuid"` implies equality only (RFC 4122).
- Other formats default to base string rules unless explicitly mapped.

## Comparator Semantics (Strict Core V1)

- equals/not_equals: strict type match; no coercion (scalar types only).
- gt/gte/lt/lte:
  - integer: numeric ordering on signed integers.
  - number: decimal ordering (arbitrary precision). No binary float rounding.
- contains:
  - string: substring containment.
  - array: expected must be an array; all expected elements must appear in
    the evidence array.
- in_set:
  - expected must be an array of the same scalar type as the evidence value.
- exists/not_exists:
  - valid for all types; presence of value determines result.

## Optional Comparator Families (Strict Core V1 Opt-Ins)

These comparators are explicit opt-ins and must be enabled by profile/config
and allowed by `x-decision-gate.allowed_comparators`.

- lex_greater_than / lex_greater_than_or_equal / lex_less_than / lex_less_than_or_equal:
  - type: string only.
  - semantics: lexicographic ordering by Unicode scalar value; no locale, no
    case folding, no normalization.
- deep_equals / deep_not_equals:
  - type: array/object only.
  - semantics: structural JSON equality (recursive comparison of elements and
    fields).

## Decimal Number Semantics

To support decimals safely:

- Parse JSON numbers into a decimal type (e.g., BigDecimal) using the numeric
  string representation to avoid binary float error.
- Comparisons are exact decimal comparisons.
- NaN/Infinity are rejected (JSON does not encode them).

## Compliance and Parsing Rules

- RFC 3339 parsing is strict: timezone required for `date-time`; no locale or
  ambiguous formats; normalize to UTC for ordering comparisons.
- RFC 3339 `date` uses full-date only (`YYYY-MM-DD`).
- RFC 4122 UUID parsing is strict; non-canonical forms are rejected.

## Domain-Specific Comparator Constraints

Domains may restrict allowed comparators per field (subset only). This is the
primary "semantic" control layer.

Proposed schema annotation:

```json
{
  "type": "string",
  "x-decision-gate": {
    "type_alias": "team_id",
    "allowed_comparators": ["equals", "in_set"]
  }
}
```

Rules:

- `allowed_comparators` must be a subset of the type class compatibility
  matrix (after format refinements).
- If absent, all comparators from the matrix are allowed.
- If present, predicates using disallowed comparators are rejected.

## Validation Rules (Strict Mode)

At `scenario_define` time:

- Derive type class for each predicate from schema context.
- Validate comparator against the compatibility matrix.
- Validate expected value type and shape.
- Apply domain constraints (`allowed_comparators`).
- Reject union types unless comparator is valid for all branches.

At `precheck` time:

- Validate payload against data shape schema.
- Validate predicates against the same strict rules.
- Reject invalid payloads or predicates before evaluation.

Provider contracts:

- For verified evidence, comparator allowlists already exist in provider
  contracts; strict validation must intersect provider allowlists with the
  matrix and schema-derived type class.

## Configuration Surface (Proposed)

Default behavior: strict validation enabled.

Example config:

```toml
[validation]
strict = true
profile = "strict_core_v1"
allow_permissive = false
```

Permissive mode:

- Allows invalid comparator/type usage to proceed (evaluation yields Unknown).
- Intended only for legacy compatibility or explicit experimentation.

## Examples

Valid:

- Schema: `wins: integer`
- Predicate: `wins >= 10`
- Result: accepted

Invalid (strict mode):

- Schema: `team_id: string`
- Predicate: `team_id > "ARS"`
- Result: rejected unless lexicographic ordering is explicitly allowed.

Domain restriction:

- Schema: `team_id` allows only `equals` and `in_set`
- Predicate: `team_id contains "ARS"` -> rejected

## Implementation Plan (Phased)

Phase 0: Spec and Contracts

- Publish this matrix and schema annotation spec.
- Add tooling examples for `x-decision-gate.allowed_comparators`.

Phase 1: MCP Validation Layer

- Enforce strict validation in `scenario_define`.
- Enforce strict validation in `precheck`.
- Provide clear error messages with predicate IDs and field paths.

Phase 2: Comparator Semantics Update

- Update comparator evaluation to support decimal comparisons.
- Add lexicographic ordering comparators (opt-in).
- Add deep equality comparators (opt-in).

Phase 3: Documentation + Builder Guidance

- Document the matrix and annotations in docs.
- Provide example schemas and scenario specs.

## Testing Plan

- Unit tests for type class derivation and matrix validation.
- Unit tests for decimal comparisons (ordering and equality).
- System tests: invalid predicate comparator rejected at `scenario_define`.
- System tests: `precheck` rejects payload or predicates that violate policy.

## Decisions in This Proposal

- Strict validation is default-on.
- Decimal numeric comparisons are supported and exact (no float drift).
- String ordering is opt-in only via explicit lexicographic comparators:
  `lex_greater_than`, `lex_greater_than_or_equal`, `lex_less_than`,
  `lex_less_than_or_equal`.
- Array/object equality is opt-in only via deep equality comparators.
- Formats are explicit: RFC 3339 date/date-time ordering allowed; RFC 4122 uuid
  equality only.
- Union types require comparator validity across all branches.
- Null equality requires explicit schema permission.
- Domain constraints are a subset of the compatibility matrix.
- No implicit type coercions.
