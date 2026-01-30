<!--
Docs/architecture/comparator_validation_architecture.md
============================================================================
Document: Comparator Validation Architecture Reference
Description: Implementation map for strict comparator validation and runtime
             comparator semantics.
Purpose: Provide an architectural file reference and change guide for
         comparator validation behavior across MCP and core.
Dependencies:
  - decision-gate-mcp/src/validation.rs
  - decision-gate-mcp/src/tools.rs
  - decision-gate-config/src/config.rs
  - decision-gate-core/src/core/evidence.rs
  - decision-gate-core/src/runtime/comparator.rs
  - decision-gate-contract/src/schemas.rs
  - decision-gate-contract/src/tooltips.rs
  - Docs/generated/decision-gate/
============================================================================
-->

# Comparator Validation Architecture Reference

## Overview
Decision Gate enforces comparator correctness in two layers:

1) **Authoring-time strict validation (default-on)** in the MCP layer rejects
   invalid comparator/type combinations before a scenario or precheck runs.
2) **Runtime comparator evaluation** in the core returns tri-state results and
   yields `Unknown` when evidence types do not match expectations.

The strict validator is the gatekeeper; runtime logic is the last line of
fail-closed behavior.

## Source of Truth Map

| Area | File | Notes |
| --- | --- | --- |
| Comparator enum + EvidenceQuery | `decision-gate-core/src/core/evidence.rs` | Canonical comparator list and query shape. |
| Runtime comparator semantics | `decision-gate-core/src/runtime/comparator.rs` | Decimal-aware ordering, lex/deep behavior, Unknown on mismatch. |
| Strict validation engine | `decision-gate-mcp/src/validation.rs` | Type-class matrix, schema parsing, domain overrides. |
| MCP tool integration | `decision-gate-mcp/src/tools.rs` | `scenario_define` + `precheck` invoke strict validation. |
| Validation config surface | `decision-gate-config/src/config.rs` | `ValidationConfig` toggles and profile identifiers. |
| Contract schemas | `decision-gate-contract/src/schemas.rs` | Comparator schema and annotations. |
| Tooltips/docs | `decision-gate-contract/src/tooltips.rs` | Public-facing comparator and validation behavior. |
| Generated docs | `Docs/generated/decision-gate/` | Regenerated after schema/tooltip updates. |
| MCP validation tests | `decision-gate-mcp/tests/validation.rs` | Unit/system coverage for strict mode. |
| System tests | `system-tests/tests/suites/validation.rs` | End-to-end validation behavior. |

## Validation Pipeline (MCP)

### Scenario definition (`scenario_define`)
- MCP validates provider contracts and predicate schemas.
- Strict validation enforces comparator/type compatibility and expected-value
  shape.
- Implementation: `decision-gate-mcp/src/tools.rs` (calls `StrictValidator`).

### Precheck (`precheck` tool)
- MCP validates payload against the registered data shape schema.
- Strict validation enforces predicate compatibility against the data shape
  schema.
- Implementation: `decision-gate-mcp/src/tools.rs` (calls `validate_precheck`).

## Strict Validation Rules (Implementation Summary)

### Type-class compatibility
- Implemented in `decision-gate-mcp/src/validation.rs`.
- Core functions:
  - `schema_type_classes` and helpers derive type classes.
  - `comparator_allowances` defines allowed/opt-in/forbidden combinations.
  - `validate_expected_value` ensures expected-value shapes match schema.

### Optional comparator families
- Lexicographic ordering comparators are opt-in (config flag + schema override).
- Deep equality comparators are opt-in (config flag + schema override).
- Implemented in `decision-gate-mcp/src/validation.rs` and controlled by
  `decision-gate-config/src/config.rs`.

### Domain overrides
- `x-decision-gate.allowed_comparators` restricts allowed comparators to a
  subset of the type-class matrix.
- `x-decision-gate.dynamic_type = true` treats the schema as dynamic (no
  declared type) and allows comparator validation to proceed without a type
  restriction, subject to config toggles.
- Implemented in `decision-gate-mcp/src/validation.rs`.

### Union handling
- `oneOf`/`anyOf`/multi-type unions intersect allowances across variants.
- Nullable unions allow null without expanding comparator set.
- Implemented in `decision-gate-mcp/src/validation.rs`.

## Runtime Comparator Semantics (Core)

- Numeric comparisons are decimal-aware (no float rounding).
- RFC 3339 `date`/`date-time` ordering is supported for string values.
- Unsupported comparisons yield `TriState::Unknown`.
- `in_set` only applies to scalar evidence values; arrays/objects yield `Unknown`.
- Implementation: `decision-gate-core/src/runtime/comparator.rs`.

## Configuration Surface

- Strict validation is default-on. Disabling strict requires
  `validation.allow_permissive = true`.
- Optional comparator families are gated by config toggles.
- Implementation: `decision-gate-config/src/config.rs` (`ValidationConfig`).

## Contract + Docs Alignment

- Comparator schema and annotations are in
  `decision-gate-contract/src/schemas.rs`.
- Public tooltips and guidance are in
  `decision-gate-contract/src/tooltips.rs`.
- Regenerate contract artifacts after any validation or comparator changes:
  `Docs/generated/decision-gate/`.

## Change Checklist

1) Update comparator semantics in `decision-gate-core/src/runtime/comparator.rs`.
2) Update strict validation logic in `decision-gate-mcp/src/validation.rs`.
3) Update config toggles in `decision-gate-config/src/config.rs` if needed.
4) Align schemas/tooltips in `decision-gate-contract/src/schemas.rs` and
   `decision-gate-contract/src/tooltips.rs`.
5) Regenerate `Docs/generated/decision-gate/` artifacts.
6) Update unit + system tests:
   - `decision-gate-mcp/tests/validation.rs`
   - `system-tests/tests/suites/validation.rs`
