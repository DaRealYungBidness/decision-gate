<!--
Docs/roadmap/decision_gate_sdk_invariance_plan.md
============================================================================
Document: Decision Gate SDK + Docs Invariance Plan
Description: Formal plan for contract-driven SDK, OpenAPI, and web doc generation.
Purpose: Enforce deterministic, drift-free external projections of Decision Gate.
Dependencies:
  - decision-gate-contract (canonical contract generator)
  - Docs/generated/decision-gate/tooling.json
  - Docs/generated/decision-gate/schemas/*
  - Docs/generated/decision-gate/examples/*
  - Docs/generated/decision-gate/tooltips.json
============================================================================
-->

# Decision Gate SDK + Docs Invariance Plan

## 1) Purpose

Establish a **single-source-of-truth, deterministic, drift-free** pipeline for
all external Decision Gate projections:

- Client SDKs (Python + TypeScript)
- OpenAPI (generated view)
- Website-ready docs/artifacts

Goal: **build once, never hand-edit again**. All outputs must be generated from
canonical contract artifacts, with CI enforcing zero drift.

## 2) Scope

This plan covers:

- Codegen tooling (Rust-based generator)
- Generated SDK outputs
- OpenAPI generation
- Website documentation sync inputs
- CI checks and test strategy

It does **not** cover enterprise-only features or private AssetCore crates.

## 3) Canonical Inputs (Single Source of Truth)

All generated outputs must derive from **contract artifacts** emitted by
`decision-gate-contract`:

- `Docs/generated/decision-gate/tooling.json`
- `Docs/generated/decision-gate/schemas/*`
- `Docs/generated/decision-gate/examples/*`
- `Docs/generated/decision-gate/tooltips.json`

No SDK method signatures, OpenAPI definitions, or web docs are hand-authored.

## 4) Outputs

### 4.1 Client SDKs (Generated)

**Python**
- `sdks/python/decision_gate/_generated.py` (methods + types + field docs + examples + validation helpers; jsonschema optional).
- `sdks/python/decision_gate/client.py` (transport; handwritten)
- `sdks/python/decision_gate/errors.py` (handwritten)
- `sdks/python/decision_gate/__init__.py` (exports; handwritten)
- `sdks/python/pyproject.toml` (handwritten)

**TypeScript**
- `sdks/typescript/src/_generated.ts` (methods + types + field docs + examples + validation helpers; Ajv optional).
- `sdks/typescript/src/client.ts` (transport; handwritten)
- `sdks/typescript/src/errors.ts` (handwritten)
- `sdks/typescript/src/index.ts` (exports; handwritten)
- `sdks/typescript/package.json` (handwritten)

**Rule:** only `_generated.*` changes when tools change.

### 4.2 OpenAPI (Generated)

OpenAPI is generated from the same contract inputs. It is a view of the
JSON-RPC surface, not a parallel API definition.

Output:
- `Docs/generated/openapi/decision-gate.json`

### 4.3 Website Docs

Website sync must consume only canonical and generated outputs:

- `Docs/generated/decision-gate/**`
- `Docs/generated/openapi/**`
- `Docs/guides/**`
- `Docs/architecture/**`
- `Docs/business/decision_gate_integration_landscape.md`

No hand-authored fragments outside those directories should be required.

## 5) Invariants (Non-Negotiable)

1. **Single source of truth**: `decision-gate-contract` output only.
2. **Deterministic output**: identical inputs must yield byte-identical outputs.
3. **No manual edits** to generated files.
4. **CI drift checks** must fail any mismatch.
5. **OSS remains deterministic and auditable** (no enterprise coupling).

## 6) Generation Pipeline

### 6.1 Canonical Contract Generation (Existing)

```
decision-gate-contract generate
  -> Docs/generated/decision-gate/tooling.json
  -> Docs/generated/decision-gate/schemas/*
  -> Docs/generated/decision-gate/examples/*
  -> Docs/generated/decision-gate/tooltips.json
```

### 6.2 SDK Codegen (Implemented)

Rust generator reads `tooling.json` (and schemas as needed) and writes:

- `sdks/python/decision_gate/_generated.py`
- `sdks/typescript/src/_generated.ts`

Generator lives in this repo as `decision-gate-sdk-gen`, implemented in Rust
for consistency, determinism, and internal maintainability.

### 6.3 OpenAPI Codegen (Implemented)

Rust generator emits `Docs/generated/openapi/decision-gate.json` from the same
contract artifacts.

## 7) CI and Drift Enforcement

Current enforcement:

- `decision-gate-contract check` (existing)
- `decision-gate-sdk-gen check` (new)
- Generator drift tests in `decision-gate-sdk-gen` (`cargo test -p decision-gate-sdk-gen`)

CI must fail if SDK outputs drift from contract inputs.

## 8) Testing Strategy

### 8.1 Unit Tests (SDK transport)

- Mock JSON-RPC and assert request shape
- Validate error handling
- Validate timeouts and retry behavior (if implemented)
 - Implemented system-test-backed transport validation for Python/TypeScript SDKs (live MCP server)

### 8.2 Contract Tests (SDK)

- Generated method count matches tooling.json
- Parameter and return type models match schemas
- Example payloads validate against generated models
- Generator drift tests in `decision-gate-sdk-gen` (implemented)

### 8.3 System Tests (SDK)

- Bring up a local DG MCP server
- Execute full scenario lifecycle via SDK
- Validate expected responses and run status
 - Implemented for Python + TypeScript SDKs via `system-tests/tests/suites/sdk_client.rs`
- Repository example suites (Python + TypeScript) execute as system tests via
  `system-tests/tests/suites/sdk_examples.rs`.

## 9) Website Sync Integration

Ensure `Asset-Core-Web/scripts/sync-decision-gate-docs.mjs` pulls:

- `Docs/generated/decision-gate/**`
- `Docs/guides/**`
- `Docs/architecture/**`
- `Docs/business/decision_gate_integration_landscape.md`

Include OpenAPI (`Docs/generated/openapi/decision-gate.json`) in the sync bundle.

## 10) Milestones

1. **Milestone 1**: Rust codegen tool (SDK outputs) + deterministic outputs ✅
2. **Milestone 2**: SDK transport layer + unit tests ✅
3. **Milestone 3**: SDK system tests (MCP server live) ✅
4. **Milestone 4**: CI drift enforcement + SDK publish readiness 🟡 (drift checks done; publishing pending)
5. **Milestone 5**: OpenAPI generator + web sync integration ✅ (OpenAPI generated; web sync pending)

## 11) Ownership

- Contract: `decision-gate-contract`
- SDK generator: `decision-gate-sdk-gen`
- SDK packages: `sdks/`
- Docs: `Docs/generated/decision-gate/`, `Docs/generated/openapi/`

## 12) Open Questions

- Preferred SDK naming on PyPI/npm (`decision-gate` vs scoped)? (currently `decision-gate`)
- Do we publish SDK docs in `Docs/generated/decision-gate/sdk/*` for website sync?
