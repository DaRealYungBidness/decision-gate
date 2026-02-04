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
Last Updated: 2026-02-04 (UTC)
============================================================================
-->

# Comparator Validation Architecture Reference

## Overview
Decision Gate enforces comparator correctness in two layers:

1) **Authoring-time strict validation (default-on)** in the MCP layer rejects
   invalid comparator/type combinations before a scenario or precheck runs.
   [F:decision-gate-mcp/src/validation.rs L36-L673](decision-gate-mcp/src/validation.rs#L36-L673)
   [F:decision-gate-mcp/src/tools.rs L2027-L2050](decision-gate-mcp/src/tools.rs#L2027-L2050)
   [F:decision-gate-mcp/src/tools.rs L2663-L2707](decision-gate-mcp/src/tools.rs#L2663-L2707)
2) **Runtime comparator evaluation** in the core returns tri-state results and
   yields `Unknown` when evidence types do not match expectations.
   [F:decision-gate-core/src/runtime/comparator.rs L39-L259](decision-gate-core/src/runtime/comparator.rs#L39-L259)

The strict validator is the gatekeeper; runtime logic is the last line of
fail-closed behavior.
[F:decision-gate-mcp/src/validation.rs L36-L170](decision-gate-mcp/src/validation.rs#L36-L170)
[F:decision-gate-core/src/runtime/comparator.rs L39-L197](decision-gate-core/src/runtime/comparator.rs#L39-L197)

## Source of Truth Map

| Area | File | Notes |
| --- | --- | --- |
| Comparator enum + EvidenceQuery | [F:decision-gate-core/src/core/evidence.rs L32-L90](decision-gate-core/src/core/evidence.rs#L32-L90) | Canonical comparator list and query shape. |
| Runtime comparator semantics | [F:decision-gate-core/src/runtime/comparator.rs L39-L308](decision-gate-core/src/runtime/comparator.rs#L39-L308) | Decimal-aware ordering, lex/deep behavior, Unknown on mismatch. |
| Strict validation engine | [F:decision-gate-mcp/src/validation.rs L36-L673](decision-gate-mcp/src/validation.rs#L36-L673) | Type-class matrix, schema parsing, domain overrides. |
| MCP tool integration | [F:decision-gate-mcp/src/tools.rs L2027-L2050](decision-gate-mcp/src/tools.rs#L2027-L2050) [F:decision-gate-mcp/src/tools.rs L2663-L2707](decision-gate-mcp/src/tools.rs#L2663-L2707) | `scenario_define` + `precheck` invoke strict validation. |
| Validation config surface | [F:decision-gate-config/src/config.rs L1469-L1521](decision-gate-config/src/config.rs#L1469-L1521) | `ValidationConfig` toggles and profile identifiers. |
| Contract schemas | [F:decision-gate-contract/src/schemas.rs L1105-L1129](decision-gate-contract/src/schemas.rs#L1105-L1129) | Comparator schema and annotations. |
| Tooltips/docs | [F:decision-gate-contract/src/tooltips.rs L260-L296](decision-gate-contract/src/tooltips.rs#L260-L296) | Public-facing comparator and validation behavior. |
| Generated docs | [F:Docs/generated/decision-gate/tooling.md L1-L42](Docs/generated/decision-gate/tooling.md#L1-L42) [F:Docs/generated/decision-gate/tooltips.json L1-L80](Docs/generated/decision-gate/tooltips.json#L1-L80) | Regenerated after schema/tooltip updates. |
| MCP validation tests | [F:decision-gate-mcp/tests/validation.rs L1-L220](decision-gate-mcp/tests/validation.rs#L1-L220) | Unit coverage for strict mode. |
| System tests | [F:system-tests/tests/suites/validation.rs L1-L220](system-tests/tests/suites/validation.rs#L1-L220) | End-to-end validation behavior. |

## Validation Pipeline (MCP)

### Scenario definition (`scenario_define`)
- MCP validates provider contracts and check schemas.
  [F:decision-gate-mcp/src/tools.rs L2045-L2048](decision-gate-mcp/src/tools.rs#L2045-L2048)
- Strict validation enforces comparator/type compatibility and expected-value
  shape.
  [F:decision-gate-mcp/src/tools.rs L2045-L2048](decision-gate-mcp/src/tools.rs#L2045-L2048)
- Implementation: `ToolRouter::define_scenario`.
  [F:decision-gate-mcp/src/tools.rs L2027-L2093](decision-gate-mcp/src/tools.rs#L2027-L2093)

### Precheck (`precheck` tool)
- MCP validates payload against the registered data shape schema.
  [F:decision-gate-mcp/src/tools.rs L2670-L2680](decision-gate-mcp/src/tools.rs#L2670-L2680)
- Strict validation enforces condition compatibility against the data shape
  schema.
  [F:decision-gate-mcp/src/tools.rs L2690-L2692](decision-gate-mcp/src/tools.rs#L2690-L2692)
- Implementation: `ToolRouter::precheck`.
  [F:decision-gate-mcp/src/tools.rs L2663-L2707](decision-gate-mcp/src/tools.rs#L2663-L2707)

## Strict Validation Rules (Implementation Summary)

### Type-class compatibility
- Implemented in `decision-gate-mcp/src/validation.rs`.
  [F:decision-gate-mcp/src/validation.rs L234-L535](decision-gate-mcp/src/validation.rs#L234-L535)
- `schema_type_classes` and helpers derive type classes.
  [F:decision-gate-mcp/src/validation.rs L400-L535](decision-gate-mcp/src/validation.rs#L400-L535)
- `comparator_allowances` defines allowed/opt-in/forbidden combinations.
  [F:decision-gate-mcp/src/validation.rs L234-L398](decision-gate-mcp/src/validation.rs#L234-L398)
- `validate_expected_value` ensures expected-value shapes match schema.
  [F:decision-gate-mcp/src/validation.rs L587-L673](decision-gate-mcp/src/validation.rs#L587-L673)

### Optional comparator families
- Lexicographic ordering comparators are opt-in (config flag + schema override).
  [F:decision-gate-mcp/src/validation.rs L329-L340](decision-gate-mcp/src/validation.rs#L329-L340)
  [F:decision-gate-mcp/src/validation.rs L558-L585](decision-gate-mcp/src/validation.rs#L558-L585)
  [F:decision-gate-mcp/src/validation.rs L829-L838](decision-gate-mcp/src/validation.rs#L829-L838)
  [F:decision-gate-config/src/config.rs L1472-L1487](decision-gate-config/src/config.rs#L1472-L1487)
- Deep equality comparators are opt-in (config flag + schema override).
  [F:decision-gate-mcp/src/validation.rs L350-L365](decision-gate-mcp/src/validation.rs#L350-L365)
  [F:decision-gate-mcp/src/validation.rs L558-L585](decision-gate-mcp/src/validation.rs#L558-L585)
  [F:decision-gate-mcp/src/validation.rs L829-L838](decision-gate-mcp/src/validation.rs#L829-L838)
  [F:decision-gate-config/src/config.rs L1472-L1487](decision-gate-config/src/config.rs#L1472-L1487)

### Domain overrides
- `x-decision-gate.allowed_comparators` restricts allowed comparators to a
  subset of the type-class matrix.
  [F:decision-gate-mcp/src/validation.rs L537-L585](decision-gate-mcp/src/validation.rs#L537-L585)
- `x-decision-gate.dynamic_type = true` treats the schema as dynamic (no
  declared type) and allows comparator validation to proceed without a type
  restriction, subject to config toggles.
  [F:decision-gate-mcp/src/validation.rs L400-L414](decision-gate-mcp/src/validation.rs#L400-L414)

### Union handling
- `oneOf`/`anyOf`/multi-type unions intersect allowances across variants.
  [F:decision-gate-mcp/src/validation.rs L234-L270](decision-gate-mcp/src/validation.rs#L234-L270)
  [F:decision-gate-mcp/src/validation.rs L675-L701](decision-gate-mcp/src/validation.rs#L675-L701)
- Nullable unions allow null without expanding comparator set.
  [F:decision-gate-mcp/src/validation.rs L703-L755](decision-gate-mcp/src/validation.rs#L703-L755)

## Runtime Comparator Semantics (Core)

- Numeric comparisons are decimal-aware (no float rounding).
  [F:decision-gate-core/src/runtime/comparator.rs L148-L277](decision-gate-core/src/runtime/comparator.rs#L148-L277)
- RFC 3339 `date`/`date-time` ordering is supported for string values.
  [F:decision-gate-core/src/runtime/comparator.rs L168-L295](decision-gate-core/src/runtime/comparator.rs#L168-L295)
- Unsupported comparisons yield `TriState::Unknown`.
  [F:decision-gate-core/src/runtime/comparator.rs L65-L198](decision-gate-core/src/runtime/comparator.rs#L65-L198)
- `in_set` only applies to scalar evidence values; arrays/objects yield `Unknown`.
  [F:decision-gate-core/src/runtime/comparator.rs L250-L258](decision-gate-core/src/runtime/comparator.rs#L250-L258)
- Implementation: `decision-gate-core/src/runtime/comparator.rs`.
  [F:decision-gate-core/src/runtime/comparator.rs L39-L308](decision-gate-core/src/runtime/comparator.rs#L39-L308)

## Configuration Surface

- Strict validation is default-on. Disabling strict requires
  `validation.allow_permissive = true`.
  [F:decision-gate-config/src/config.rs L1472-L1511](decision-gate-config/src/config.rs#L1472-L1511)
- Optional comparator families are gated by config toggles.
  [F:decision-gate-config/src/config.rs L1472-L1487](decision-gate-config/src/config.rs#L1472-L1487)
- Implementation: `decision-gate-config/src/config.rs` (`ValidationConfig`).
  [F:decision-gate-config/src/config.rs L1469-L1521](decision-gate-config/src/config.rs#L1469-L1521)

## Contract + Docs Alignment

- Comparator schema and annotations are in
  `decision-gate-contract/src/schemas.rs`.
  [F:decision-gate-contract/src/schemas.rs L1105-L1129](decision-gate-contract/src/schemas.rs#L1105-L1129)
- Public tooltips and guidance are in
  `decision-gate-contract/src/tooltips.rs`.
  [F:decision-gate-contract/src/tooltips.rs L260-L296](decision-gate-contract/src/tooltips.rs#L260-L296)
- Regenerate contract artifacts after any validation or comparator changes:
  `Docs/generated/decision-gate/`.
  [F:Docs/generated/decision-gate/tooling.md L1-L42](Docs/generated/decision-gate/tooling.md#L1-L42)
  [F:Docs/generated/decision-gate/tooltips.json L1-L80](Docs/generated/decision-gate/tooltips.json#L1-L80)

## Change Checklist

1) Update comparator semantics in `decision-gate-core/src/runtime/comparator.rs`.
   [F:decision-gate-core/src/runtime/comparator.rs L39-L308](decision-gate-core/src/runtime/comparator.rs#L39-L308)
2) Update strict validation logic in `decision-gate-mcp/src/validation.rs`.
   [F:decision-gate-mcp/src/validation.rs L36-L673](decision-gate-mcp/src/validation.rs#L36-L673)
3) Update config toggles in `decision-gate-config/src/config.rs` if needed.
   [F:decision-gate-config/src/config.rs L1469-L1521](decision-gate-config/src/config.rs#L1469-L1521)
4) Align schemas/tooltips in `decision-gate-contract/src/schemas.rs` and
   `decision-gate-contract/src/tooltips.rs`.
   [F:decision-gate-contract/src/schemas.rs L1105-L1129](decision-gate-contract/src/schemas.rs#L1105-L1129)
   [F:decision-gate-contract/src/tooltips.rs L260-L296](decision-gate-contract/src/tooltips.rs#L260-L296)
5) Regenerate `Docs/generated/decision-gate/` artifacts.
   [F:Docs/generated/decision-gate/tooling.md L1-L42](Docs/generated/decision-gate/tooling.md#L1-L42)
6) Update unit + system tests.
   [F:decision-gate-mcp/tests/validation.rs L1-L220](decision-gate-mcp/tests/validation.rs#L1-L220)
   [F:system-tests/tests/suites/validation.rs L1-L220](system-tests/tests/suites/validation.rs#L1-L220)
