<!--
Docs/roadmap/openapi_to_dg_mapping_deep_dive.md
============================================================================
Document: OpenAPI to Decision Gate Mapping (Deep Dive, Deferred)
Description: Feasibility analysis, pain points, and future approach for
             deriving DG comparator defaults from arbitrary OpenAPI schemas.
Purpose: Capture the reasoning now, so we can revisit later without redoing
         the full analysis.
Dependencies:
  - Docs/roadmap/scenario_builder_plan.md
  - Docs/roadmap/README.md
  - Docs/architecture/comparator_validation_architecture.md
  - Docs/architecture/decision_gate_namespace_registry_rbac_architecture.md
  - Docs/architecture/decision_gate_provider_capability_architecture.md
============================================================================
-->

# OpenAPI to Decision Gate Mapping (Deep Dive, Deferred)

## Executive Summary
We can mechanistically derive default comparator allowances from OpenAPI/JSON
Schema types. That logic already exists in strict validation (the type-class
matrix). However, making this work for *arbitrary* OpenAPI schemas requires a
non-trivial normalization and policy layer (dialect conversion, ref resolution,
union handling, and explicit schema typing). This document captures the
obstacles and a phased approach, but the work is deferred for now.

## Why This Is Deferred
- It is a multi-surface change (schema parsing, registry, builder UX, and
  contracts), not a single feature drop.
- It intersects with existing roadmap priorities (trust lanes, schema registry,
  builder plan, and world-class readiness). See references below.
- The highest risk is correctness drift when schemas are ambiguous or untyped
  (a common case for real-world OpenAPI documents).

## Core Idea (What We Would Build Later)
Given any OpenAPI schema, derive a DG-compatible data-shape schema and default
comparator allowances per field. The builder then only *removes* comparators or
opts into special families (lexicographic, deep equals) where safe.

This aligns with the Scenario Builder Plan: the builder already expects to
derive allowed comparators from schema type classes. See:
`Docs/roadmap/scenario_builder_plan.md`.

## Current Ground Truth (Decision Gate Defaults)
Comparator defaults come from strict validation (type-class matrix), and runtime
semantics define which comparisons are meaningful. The matrix is already the
mechanical baseline for "defaults." See:
`Docs/architecture/comparator_validation_architecture.md`.

Key policy reality: OpenAPI gives *shape*, but not *intent*. We still need a
policy layer for opt-ins (lexicographic, deep equality, date ordering).

## Known Obstacles (Pain Points)
1) OpenAPI dialect mismatch
- OpenAPI 3.0 is not JSON Schema 2020-12. The current validator compiles 2020-12.
- Requires conversion for: nullable, oneOf/anyOf, and schema defaults.

2) Missing or implicit types
- Many OpenAPI schemas omit explicit `type`. The strict validator requires it.
- We must infer and normalize types (e.g., object with properties -> type: object).

3) $ref resolution and allOf composition
- Real-world specs rely on $ref and allOf. We need a resolver + merge step.
- Without normalization, defaults become inconsistent or impossible to compute.

4) additionalProperties ambiguity
- Untyped additionalProperties are common and are rejected in strict precheck.
- We must choose a policy: reject, default to a typed schema, or require user
  confirmation in the builder.

5) Union semantics shrink allowances
- oneOf/anyOf intersect allowances. Mixed unions can reduce comparators to
  existence-only, which will look like a bug to users without explanation.

6) Nested object paths
- DG conditions are top-level identifiers. OpenAPI often describes nested
  fields. We must decide: flatten paths, require explicit projection, or only
  support top-level objects with object-level comparators.

7) Format handling beyond date/uuid
- OpenAPI formats (email, uri, byte, binary, etc.) do not have dedicated
  comparator semantics today. We must document or restrict these formats.

8) Enum and mixed-type pitfalls
- The validator rejects enums with non-scalar values or mixed scalar types.
- Many OpenAPI enums violate these constraints in practice.

## Dependencies and Roadmap Anchors
This work intersects with multiple roadmap items and should be scheduled only
when these foundations are either complete or explicitly scoped:

- Scenario builder derivation rules:
  `Docs/roadmap/scenario_builder_plan.md`
- Schema registry and RBAC foundation:
  `Docs/architecture/decision_gate_namespace_registry_rbac_architecture.md`
- Provider capability registry:
  `Docs/architecture/decision_gate_provider_capability_architecture.md`
- Release readiness and open items:
  `Docs/roadmap/README.md`

## Future Approach (Phased Proposal)
Phase 0: Policy decisions
- Decide which OpenAPI versions are supported (3.0 vs 3.1).
- Decide how to handle nested fields and untyped additionalProperties.
- Decide default opt-in policy for lexicographic and deep equality.

Phase 1: Normalization layer
- Resolve $ref and merge allOf.
- Convert OpenAPI 3.0 schemas into JSON Schema 2020-12.
- Enforce explicit type annotations on all schema fragments.

Phase 2: Comparator derivation
- Reuse strict validation matrix for defaults.
- Emit per-field comparator defaults and optional overrides.
- Generate a DG-compatible data shape schema.

Phase 3: Builder UX + validation
- UI defaults match computed allowances.
- Users may only remove defaults or explicitly opt into special comparator
  families.
- Surface warnings for ambiguous/unsupported schema constructs.

Phase 4: Contract emission
- Emit provider contract or data-shape registry entries derived from the
  normalized schema.
- Preserve provenance (which OpenAPI operation/schema produced each condition).

## Criteria for Re-Opening This Work
We should re-open this once at least one of these is true:
- The scenario builder is being actively implemented (and needs a robust
  import path), or
- The schema registry is being expanded and we need ingestion from external
  sources, or
- We want to ship a public "upload OpenAPI" feature and can staff the
  normalization work.

## Explicit Non-Goals (for the deferred scope)
- Not a UI or frontend spec.
- Not a replacement for provider contract workflows.
- Not a commitment to support every OpenAPI dialect in v1.

## Notes
This deep dive is intentionally conservative. The mechanical mapping exists, but
schema normalization and semantic policy are the real work. Until we fund that,
we should avoid shipping a partial feature that breaks on real-world specs.
