<!--
Docs/roadmap/json_evidence_playbook_roadmap.md
============================================================================
Document: JSON Evidence Playbook + Provider Expansion Roadmap
Description: World-class plan for JSON evidence workflows, comparator expansion,
             error semantics, and LLM-native playbooks.
Purpose: Provide an exhaustive implementation plan with tests and docs.
Dependencies:
  - decision-gate-contract (contract schemas + generated docs)
  - Docs/generated/decision-gate/ (tooling + providers + schemas)
  - Docs/security/threat_model.md
  - Docs/guides/evidence_flow_and_execution_model.md
  - Docs/guides/getting_started.md
  - Docs/guides/integration_patterns.md
  - Docs/guides/predicate_authoring.md
============================================================================
-->

# JSON Evidence Playbook + Provider Expansion Roadmap

## Overview
This roadmap defines a **world-class, industrial-grade** plan for:
- JSON evidence workflows (file-based + precheck).
- Structured error semantics for agents.
- Comparator expansion for `json.path`.
- LLM-native playbooks and onboarding.
- Single-source documentation generated from `decision-gate-contract`.

The goal is to remove ambiguity, maximize determinism, preserve security, and
make Decision Gate feel immediately usable for LLM and CI workflows.

## Non-Negotiable Quality Bar
- Deterministic evaluation across platforms.
- Fail-closed by default on untrusted inputs.
- Explicit, versioned contracts for tools and provider capabilities.
- Structured, machine-readable errors for recovery loops.
- Minimal and auditable semantics, even when expressive.
- Security-first defaults (limits, allowlists, root constraints).

## Scope and Constraints
- **OSS only**: all features here remain open-core (no enterprise dependencies).
- **No behavior forks**: changes apply uniformly via contracts and docs.
- **Single source of truth**: all public-facing provider/tool docs are generated
  from `decision-gate-contract` into `Docs/generated/decision-gate/`.

## Architectural Principles (Keep Stable)
- Decision Gate evaluates evidence; it does not execute tasks.
- Evidence is data. Meaning is defined by predicate + comparator + expected.
- Precheck is asserted evidence (fast iteration); live runs are provider-pulled
  evidence (audit-grade).

---

## Phase 0 — Error Semantics and Contract Alignment (World-Class)

### Feature Goals
1) **Structured error metadata** for JSON evidence and provider failures.
2) Preserve deterministic `null/Unknown` behavior (fail-closed).
3) Clear separation of **soft errors** (recoverable data issues) vs **hard
   errors** (provider failure, policy violation).

### Required Contract Updates (decision-gate-contract)
- Add error metadata fields to evidence query responses (tooling schema).
- Standardize error codes and optional `details` payloads.
- Update provider capability contracts to document error codes for `json.path`.

### Error Codes (Initial Set)
- `jsonpath_not_found`
- `invalid_json`
- `invalid_yaml`
- `file_not_found`
- `size_limit_exceeded`
- `path_outside_root`
- `invalid_jsonpath`

### Runtime Semantics
- Missing JSONPath yields:
  - `value = null`
  - `error = { code: "jsonpath_not_found", details: { jsonpath, file } }`
  - Comparator evaluates `Unknown` unless `exists`/`not_exists`.
- Invalid JSON/YAML yields:
  - `value = null`
  - `error = { code: "invalid_json", details: { file } }`

### Docs Updates
- `Docs/guides/evidence_flow_and_execution_model.md`
- `Docs/guides/predicate_authoring.md`
- `Docs/guides/getting_started.md`
- `Docs/security/threat_model.md` (error metadata in untrusted surfaces)
- Regenerate `Docs/generated/decision-gate/providers.md`
- Regenerate `Docs/generated/decision-gate/tooling.md`

### Unit Tests
- `decision-gate-providers`:
  - Missing JSONPath returns `null` + error metadata.
  - Invalid JSON/YAML returns error metadata.
  - File size limit is enforced with structured error.
  - Root escape is rejected with structured error.
- `decision-gate-core`:
  - EvidenceRecord carries structured error metadata.
  - Comparator behavior remains deterministic with `null`.
- `decision-gate-mcp`:
  - Tool responses include error metadata in a stable schema.

### System Tests
- New suite: `system-tests/tests/suites/json_evidence.rs`
  - JSONPath missing → `Unknown` + error metadata.
  - Invalid JSON → `Unknown` + error metadata.
  - Size limit exceeded → error metadata, fail-closed.

---

## Phase 1 — Canonical Example Schemas + Playbook Templates

### Feature Goals
- Publish **example templates** (not prescriptive standards).
- Provide “recipes” for:
  - Tests / lint / coverage
  - Security scans
  - Review approvals
  - Release readiness

### Deliverables
- New docs section (playbook):
  - JSON example schemas
  - JSONPath predicates
  - Comparator choices
  - Expected values
  - Error handling guidance

### Docs Updates
- New document: `Docs/roadmap/json_evidence_playbook.md` (playbook guide).
- Cross-link from:
  - `Docs/guides/integration_patterns.md`
  - `Docs/guides/getting_started.md`
  - `README.md`

### Tests
No code tests required unless schema registry is used for optional validation.

---

## Phase 2 — Comparator Expansion for `json.path`

### Feature Goals
Enable full, explicit comparator support for JSON evidence:
- Ordering: `greater_than`, `less_than`, `>=`, `<=`
- `contains`
- `in_set`
- `deep_equals`, `deep_not_equals`
- Maintain `exists`/`not_exists`

### Semantic Rules (Must Be Documented)
- Scalars: compare directly.
- Arrays:
  - `contains` means “needle subset contained in haystack array”.
  - `in_set` expects array of scalars.
- Objects:
  - Only deep equality comparisons permitted.
- Mixed types → `Unknown`.

### Contract Updates (decision-gate-contract)
- Expand allowed comparators for `json.path` in provider capabilities.
- Update example predicates in `providers.json` and `providers.md`.

### Docs Updates
- `Docs/generated/decision-gate/providers.md` (allowed comparators list)
- `Docs/guides/predicate_authoring.md` (comparators per provider)
- `Docs/guides/evidence_flow_and_execution_model.md` (examples)

### Unit Tests
- `decision-gate-core/tests/comparator.rs` (new ordering + contains cases).
- `decision-gate-providers/tests.rs` (JSONPath + comparator interaction).

### System Tests
- `system-tests/tests/suites/json_evidence.rs`:
  - Ordering on numeric JSONPath values.
  - Contains on array outputs.
  - Deep equality on object outputs.

---

## Phase 3 — Date/Time Ordering Support (RFC3339)

### Feature Goals
Allow ordering comparators on:
- RFC3339 timestamps
- Date-only values (YYYY-MM-DD)

### Contract Updates
- Document supported temporal formats in contract metadata for comparators.

### Docs Updates
- `Docs/guides/predicate_authoring.md`
- `Docs/generated/decision-gate/providers.md`

### Tests
- Comparator unit tests for RFC3339 date/time ordering.
- System tests verifying deterministic ordering.

---

## Phase 4 — LLM-Native Playbooks and UX

### Feature Goals
Make Decision Gate “LLM-native” in usage, not just in principle.

### Deliverables
- JSON-RPC walkthroughs for:
  - `precheck` with inline evidence (LLM-first)
  - `scenario_define` + `scenario_next` with file-based JSON evidence
- Python and curl examples (no Rust required).
- Full “agent loop” sample with error recovery (structured errors).

### Docs Updates
- New guide: `Docs/guides/llm_native_playbook.md`
- `Docs/guides/getting_started.md` (append LLM-native section)
- `Docs/guides/integration_patterns.md` (agent loop with precheck)

### Tests
- Tool schema validation for new example payloads (contract tests).
- Optional system test to simulate an “agent loop” using precheck.

---

## Phase 5 — Precheck-First Onboarding Guidance

### Feature Goals
Explicitly recommend precheck for onboarding, while preserving strict trust
language and audit guidance.

### Doc Guidance
State clearly:
- Precheck is fast, safe for iteration, but asserted evidence.
- Provider-pulled runs are required for audit-grade decisions.

### Docs Updates
- `Docs/guides/evidence_flow_and_execution_model.md`
- `Docs/guides/getting_started.md`
- `README.md` (short onboarding guidance)

---

## Cross-Cutting: Single-Source Documentation from decision-gate-contract

### Objective
All public-facing provider/tool documentation must be generated from
`decision-gate-contract` artifacts to ensure consistency and to support a
single, canonical source for the website.

### Required Steps
- Update contract schemas and examples in `decision-gate-contract`.
- Regenerate:
  - `Docs/generated/decision-gate/providers.json`
  - `Docs/generated/decision-gate/providers.md`
  - `Docs/generated/decision-gate/tooling.md`
  - `Docs/generated/decision-gate/schemas/*`
- Add/extend CI checks to prevent drift between contract and docs.

### Tests
- `decision-gate-contract/tests/schema_validation.rs` (schema correctness).
- `decision-gate-mcp/tests/contract_schema_e2e.rs` (tool payloads valid).
- `system-tests/tests/suites/contract.rs` (integration validation).

---

## Security and Audit Considerations
- Error metadata must not expose sensitive file paths or secrets unless
  explicitly permitted by policy.
- JSON root constraints and size limits remain mandatory.
- All new comparator paths must preserve determinism and fail-closed behavior.

---

## Open Decisions (Current Positions)
- **Error behavior**: include structured error metadata while returning `null`.
  (Recommended: yes, to enable agent recovery.)
- **Templates**: example defaults only, explicitly non-prescriptive.
  (Recommended: yes.)
- **Precheck guidance**: precheck-first onboarding with explicit trust warnings.
  (Recommended: yes.)

---

## Dependencies and Touchpoints (Non-Exhaustive)
- `decision-gate-providers/src/json.rs`
- `decision-gate-core/src/runtime/comparator.rs`
- `decision-gate-core/src/runtime/engine.rs`
- `decision-gate-mcp/src/tools.rs`
- `decision-gate-contract/src/*` (schemas + examples)
- `Docs/generated/decision-gate/*`
- `system-tests/tests/suites/*`

---

## Definition of Done (World-Class)
- Structured, machine-readable errors available across MCP tools.
- JSON comparator surface aligns with full math semantics and is documented.
- LLM-native playbooks exist and are verified against contracts.
- Generated documentation is the single source of truth.
- Unit tests + system tests cover error paths, comparator semantics, and
  JSONPath behavior with arrays, scalars, and objects.
