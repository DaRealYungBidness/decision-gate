<!--
Docs/roadmap/decision_gate_config_single_source_plan.md
============================================================================
Document: Decision Gate Config Single-Source Plan
Description: World-class plan to make config schema/docs/runtime drift-proof.
Purpose: Provide an execution-ready, agent-friendly roadmap and guardrails.
============================================================================
-->

# Decision Gate Config Single-Source Plan (World-Class)

## Why We Are Doing This

Decision Gate must be trustworthy and deterministic. Configuration is a
security boundary and the foundation for contracts, tooling, and user trust.
We will not accept drift between runtime validation, schema, examples, and
docs. This plan sets a single source of truth, deterministic generation, and
hard CI gates so configuration is always accurate and auditable.

This is explicitly designed to be "agent-runnable": an LLM agent should be
able to follow this plan, check the acceptance criteria, and proceed safely.

## World-Class Standards (Non-Negotiable)

- **Single source of truth.** Exactly one canonical config model in OSS.
- **Deterministic artifacts.** Schema, docs, and examples are generated and
  reproducible byte-for-byte.
- **Fail-closed security.** Runtime validation always rejects unsafe configs.
- **No drift allowed.** CI fails on any mismatch between generated outputs and
  committed artifacts.
- **Docs are contractual.** The website pulls generated docs, not hand edits.
- **OSS boundary preserved.** No enterprise deps in OSS crates.

## Current Problem (Summary)

There are three sources of truth today:
1) Runtime structs + validation in `decision-gate-mcp`.
2) Config schema + examples in `decision-gate-contract`.
3) Hand-authored docs in `Docs/configuration/decision-gate.toml.md`.

These sources already drift. We will eliminate that by consolidating to one
canonical source and generating the rest.

## Canonical Source of Truth (Decision)

Create a new OSS crate (e.g., `decision-gate-config`) that owns:
- Config structs and parsing.
- All validation rules and constraints.
- Metadata required to generate schema, docs, and examples.

`decision-gate-mcp` consumes the shared types for runtime behavior.
`decision-gate-contract` consumes the shared metadata for schema/examples.
Docs are generated from the same metadata.

## Architecture Overview

```
decision-gate-config (canonical)
  - config structs + validation
  - schema generator
  - doc generator (decision-gate.toml.md)
  - example generator (decision-gate.toml)

decision-gate-mcp
  - uses decision-gate-config for parsing/validation

decision-gate-contract
  - uses decision-gate-config generators for schema/examples
  - bundles artifacts into Docs/generated/decision-gate
```

## Phased Plan

### Phase 0: Design the Canonical Model

Deliverables:
- `decision-gate-config` crate scaffolded.
- Config model + validation moved from `decision-gate-config/src/config.rs`.
- Explicit metadata for every field (type, default, constraints, description).

Acceptance criteria:
- All config fields are represented once.
- No runtime validation remains outside the new crate.

### Phase 1: Runtime Integration

Deliverables:
- `decision-gate-mcp` uses `decision-gate-config` for config load/validate.
- Runtime behavior is unchanged (strict fail-closed semantics preserved).

Acceptance criteria:
- All existing tests pass unchanged.
- Config parsing and validation are unified in one crate.

### Phase 2: Artifact Generation

Deliverables:
- JSON schema generator in `decision-gate-config`.
- TOML example generator in `decision-gate-config`.
- Markdown doc generator producing `Docs/configuration/decision-gate.toml.md`.

Acceptance criteria:
- Generated schema includes every config field and constraint.
- Generated doc matches schema and example.
- No hand edits required for configuration docs.

### Phase 3: Contract Harmonization

Deliverables:
- `decision-gate-contract` removes hand-built config schema/example.
- Contract bundle includes generated schema + example from the canonical crate.

Acceptance criteria:
- `scripts/generate_all.sh --check` passes.
- Contract bundle is deterministic and drift-free.

### Phase 4: Drift Enforcement (CI Gates)

Deliverables:
- `scripts/generate_all.sh --check` fails if config docs/schema/example drift.
- Tests validate the TOML example against the JSON schema.
- Tests validate the schema against the runtime parser defaults.

Acceptance criteria:
- Any config change requires updating the canonical crate metadata.
- CI rejects PRs that change config behavior without regenerating docs.

### Phase 5: Tests (Unit + Integration + System)

Deliverables:
- Unit tests in `decision-gate-config` cover:
  - All validation constraints.
  - Doc generator completeness (every field documented).
  - Schema generator completeness (every field present).
- Integration tests in `decision-gate-mcp` cover:
  - Fail-closed behavior for unsafe configs.
  - Security-critical defaults (auth, TLS, non-loopback behavior).
- System tests verify:
  - Generated docs match committed outputs.
  - Minimal valid config starts MCP server and uses documented defaults.

Acceptance criteria:
- Config correctness is fully testable at three levels.
- No silent drift is possible.

## CI and Local Workflow

Local:
- `scripts/generate_all.sh` regenerates schema/docs/examples.
- `scripts/generate_all.sh --check` validates no drift.

CI (Required):
- Run generator drift checks.
- Run config unit tests.
- Run integration tests for config parsing and validation.

## Strict Design Requirements (For Implementers)

- All config fields and constraints must be declared in the canonical crate.
- No duplication of validation logic anywhere else.
- Docs and examples must be generated, never hand-edited.
- Generator output must be deterministic (order and formatting stable).
- Any new config field must include:
  - Type, default, constraints, description, and example.

## Agent Checklist (Must Pass Before Merge)

- `decision-gate-mcp` compiles and runs with the new config crate.
- `decision-gate-contract` emits schema/examples from the canonical source.
- `Docs/configuration/decision-gate.toml.md` is generated, not hand-edited.
- `scripts/generate_all.sh --check` passes.
- Tests confirm schema/example/docs all match runtime validation.

## Success Definition

We have exactly one source of truth for configuration, deterministic outputs
for schema/docs/examples, and CI guarantees that drift cannot occur. The DG
website can ingest generated docs safely without manual intervention.
