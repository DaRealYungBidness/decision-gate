<!--
Docs/roadmap/scenario_builder_plan.md
============================================================================
Document: Decision Gate Scenario Builder Plan
Description: Implementation plan for a scenario authoring builder that
             produces ScenarioSpec from provider contracts and data shapes.
Purpose: Provide a spec to hand to an LLM for implementing a scenario builder
         with strict validation and canonical output.
Dependencies:
  - Docs/architecture/comparator_validation_architecture.md
  - Docs/generated/decision-gate/schemas/scenario.schema.json
  - decision-gate-core/src/core/spec.rs
  - decision-gate-mcp/src/capabilities.rs
  - decision-gate-mcp/src/validation.rs
  - ret-logic/src/dsl.rs
============================================================================
-->

# Decision Gate Scenario Builder Plan

This document defines the implementation plan for a scenario builder that
constructs `ScenarioSpec` from provider contracts and data shapes. The builder
is meant to be a deterministic authoring assistant: it collects inputs, derives
allowed comparators and expected-value shapes from schemas, and emits canonical
ScenarioSpec JSON that passes strict validation.

## Goals

- Produce valid `ScenarioSpec` that passes:
  - JSON Schema validation (`schemas/scenario.schema.json`),
  - spec semantic validation (`ScenarioSpec::validate`), and
  - MCP validation (`CapabilityRegistry` + `StrictValidator`).
- Make condition authoring type-driven by provider result schemas or data shape
  schemas, so allowed comparators and expected values "fall out" of the type.
- Expose gate requirements as a structured tree (RET) or DSL that only accepts
  existing condition identifiers.
- Support asserted-lane precheck flows by aligning data shapes with condition
  identifiers.

## Non-Goals

- Not a UI design doc; this is a data/logic plan for an LLM or CLI builder.
- Not a provider contract registration tool; provider contracts are assumed to
  exist (built-ins or external `capabilities_path`).
- Not a run-config builder (run IDs, dispatch targets, etc are out of scope).

## Terminology

- ScenarioSpec: canonical scenario definition (`decision-gate-core/src/core/spec.rs`).
- ConditionSpec: provider query + comparator + expected value definition.
- GateSpec: RET requirement tree over condition identifiers.
- StageSpec: ordered stages, gates, packets, and advancement rules.
- Provider contract: schema and comparator metadata for provider checks.
- Data shape: JSON Schema record for asserted evidence (precheck).
- Strict validation: comparator/type compatibility and schema validation enforced
  at `scenario_define` and `precheck` time.

## Source of Truth (Builder Inputs)

The builder must load the same sources of truth used at runtime:

- Scenario schema: `Docs/generated/decision-gate/schemas/scenario.schema.json`
- Provider contracts:
  - Built-ins: `Docs/generated/decision-gate/providers.json`
  - External: provider `capabilities_path` JSON loaded by MCP config
    (`decision-gate-mcp/src/capabilities.rs`)
- Strict validation rules: `decision-gate-mcp/src/validation.rs`
- Comparator semantics: `decision-gate-core/src/runtime/comparator.rs`
- Data shape registry schema rules: `decision-gate-core/src/core/data_shape.rs`

## Output

- Canonical ScenarioSpec JSON (RFC 8785) with computed hash.
- Optional authoring format (RON) may be supported, but JSON is canonical.

## Builder Flow (Type-Driven)

1) Scenario metadata
- Collect `scenario_id`, `namespace_id`, `spec_version`.
- Optional: `default_tenant_id` (single-tenant shortcut).

2) Evidence sources and type selection
- For each condition, select:
  - Provider `provider_id` and check `check_id` (from contracts), or
  - Asserted payload property (for precheck scenarios).
- The selected result schema determines allowed comparators and expected values.

3) Condition authoring
- Create `ConditionSpec`:
  - `condition_id`: stable identifier used by gates.
  - `query`: provider_id + check_id + params (schema-validated).
  - `comparator`: derived from type rules and allowed lists (below).
  - `expected`: required for most comparators (see rules below).
  - `policy_tags`: optional.
  - `trust`: optional override.

Comparator selection algorithm:
- Start with provider contract allowlist:
  - `CheckContract.allowed_comparators`.
- Intersect with strict schema allowances:
  - Derived from provider result schema or data shape schema.
  - Respect `x-decision-gate.allowed_comparators` overrides.
- Intersect with config toggles:
  - `validation.enable_lexicographic` and `validation.enable_deep_equals`.
- Result: allowed comparators displayed to the user.

Expected value rules:
- `exists` / `not_exists`: expected must be omitted.
- `in_set`: expected must be an array of valid values.
- All others: expected must validate against the result schema (or schema
  property for asserted payloads).

4) Gate authoring (RET)
- Create `GateSpec` per stage with:
  - `gate_id`
  - `requirement`: RET tree over condition identifiers.
  - optional `trust`.
- Use ret-logic DSL (`ret-logic/src/dsl.rs`) or build trees directly.
- Validate that all conditions referenced by gates exist.

5) Stage authoring
- Define `StageSpec` with:
  - `stage_id`, `entry_packets`, `gates`, `advance_to`, `timeout`, `on_timeout`.
- `advance_to` can be `linear`, `fixed`, `branch`, or `terminal`.
- Branch rules must reference gate IDs in the same stage and stage targets that
  exist in the spec.
- Ensure unique stage, gate, packet, and condition identifiers.

6) Optional references
- `policies`: list of `PolicyRef` entries.
- `schemas`: list of `SchemaRef` entries (packet schema metadata).

7) Validation pipeline (must match runtime)
- JSON schema validation: `schemas/scenario.schema.json`.
- Spec semantic validation: `ScenarioSpec::validate`.
- Capability validation:
  - Provider and check exist.
  - Params validate.
  - Comparator allowed by provider contract.
  - Expected matches result schema.
- Strict validation:
  - Comparator/type compatibility.
  - `x-decision-gate.allowed_comparators` enforced.

8) Canonicalization
- Normalize to canonical JSON (RFC 8785).
- Compute spec hash (SHA-256).

## Precheck and Asserted Payload Mapping

The precheck path maps asserted payloads to condition identifiers:

- If the data shape schema is an object:
  - Properties must include every condition identifier, or a typed
    `additionalProperties` schema.
  - Un-typed `additionalProperties: true` is rejected.
- If there is exactly one condition:
  - The data shape can be non-object; the payload maps to that condition.

Builder guidance:
- When targeting precheck, prefer object schemas keyed by condition IDs so the
  mapping is explicit and validation passes.

## Builder State Model (Suggested)

A minimal in-memory model for the builder:

- ScenarioDraft:
  - metadata: scenario_id, namespace_id, spec_version, default_tenant_id
  - conditions: ConditionDraft[]
  - stages: StageDraft[]
  - policies: PolicyRef[]
  - schemas: SchemaRef[]
- ConditionDraft:
  - condition_id, provider_id, check_id, params, comparator, expected, policy_tags, trust
  - derived: allowed_comparators (filtered list), expected_schema
- GateDraft:
  - id, requirement (RET or DSL string), trust
- StageDraft:
  - id, entry_packets, gates, advance_to, timeout, on_timeout

## Implementation Plan (Phased)

Phase 0: Contract ingestion
- Load provider contracts (built-ins + external).
- Load strict validation config.
- Load data shapes (if precheck builder is enabled).

Phase 1: Condition builder
- Build provider check selection UI/flow from provider contracts.
- Implement comparator filtering + expected value validation.
- Surface schema-derived type info to the user.

Phase 2: Gate and requirement builder
- Provide RET builder and/or DSL parsing.
- Validate referenced condition identifiers.

Phase 3: Stage and packet builder
- Add stage advancement rules and timeout policies.
- Validate unique IDs and branch targets.

Phase 4: Validation and output
- Run full validation pipeline.
- Emit canonical JSON and spec hash.

Phase 5: Precheck alignment
- Enforce data shape mapping rules for asserted payloads.
- Provide warnings when schema does not align with condition IDs.

Phase 6: Tests
- Unit tests for comparator filtering and expected value validation.
- Integration tests that `scenario_define` accepts builder output.
- Precheck tests for asserted payload mapping and strict validation errors.

## Decisions (Rigor)

- Offline-only inputs: the builder must never query MCP at runtime. All inputs
  are local, versioned artifacts (provider contracts, config snapshot, data
  shapes). This guarantees deterministic output and auditability.
- Canonical output: JSON only, with RFC 8785 canonicalization and spec hash.
  RON is explicitly out of scope for now.
- No runtime registration: the builder does not call `schemas_register` or
  mutate any runtime registry. Data shapes are treated as offline inputs only.
- Formal draft format: define a round-trip "ScenarioDraft" schema that is a
  strict superset of `ScenarioSpec` and is not executable until compiled.
  This eliminates ambiguity and preserves full authoring intent.
