<!--
Docs/roadmap/decision_gate_world_class_integration_readiness_plan.md
============================================================================
Document: Decision Gate World-Class Integration Readiness Plan
Description: Pre-publish plan for SDKs, examples, and integration surfaces.
Purpose: Reach best-in-class ergonomics, security, performance, and clarity
         before any public package releases.
Dependencies:
  - Docs/business/decision_gate_integration_landscape.md
  - Docs/roadmap/decision_gate_sdk_invariance_plan.md
  - system-tests/TEST_MATRIX.md
============================================================================
-->

# Decision Gate World-Class Integration Readiness Plan

## Executive Intent

We will **not publish** any SDKs or adapters until every surface area is
contract-driven, exhaustively tested, and manually reviewed. The goal is
world-class ergonomics, security posture, performance, and understanding.

This plan delivers **pre-publish readiness**: everything is package-ready,
examples are runnable and validated, and system tests gate correctness. Actual
publishing is a separate, explicit decision and is not part of this plan.

---

## Non-Negotiable Invariants

1. **Single source of truth**: generated outputs derive only from
   `decision-gate-contract` artifacts.
2. **Deterministic outputs**: identical inputs produce byte-identical outputs.
3. **No manual edits** to generated SDK files.
4. **All examples are runnable**: SDK examples are gated in system tests; adapter
   examples require external framework deps and are gated via optional adapter
   test harness (planned).
5. **Security-by-default**: unsafe defaults are forbidden; opt-ins must be
   explicit and loudly documented.
6. **Integration proof, not promises**: the agentic flow harness must validate
   real scenarios across every projection (raw MCP + SDKs + adapters).

---

## Scope

**In scope (OSS readiness):**
- Python + TypeScript SDKs
- OpenAPI view
- Example suites (Python + TypeScript)
- Framework adapters (LangChain, CrewAI, AutoGen, OpenAI Agents SDK)
- System-test gating for all SDKs and examples
- Agentic flow harness + scenario library (deterministic + live modes)
- Packaging dry-run checks (no publishing)
- Documentation updates aligned to reality

**Out of scope (explicitly deferred):**
- Publishing to PyPI/npm
- Enterprise integrations

---

## Milestones

### Milestone 1 — SDK Surface Excellence (Ergonomics + Clarity)

**Goal:** The SDKs are fully typed, documented, and self-explanatory.

Status: **Implemented** (field docs, examples, optional validation helpers).

Deliverables:
- Field-level docs + examples in generated Python/TypeScript SDKs.
- Optional runtime validation helpers (jsonschema + Ajv).
- Public exports for validation helpers and schema constants.
- SDK READMEs updated with validation usage.

Acceptance Criteria:
- `decision-gate-sdk-gen check` passes with zero drift.
- `cargo test -p decision-gate-sdk-gen` passes.
- SDK docstrings/JSDoc include tool descriptions, notes, examples, and
  field-level constraints.

---

### Milestone 2 — Runnable Examples as Tests (Trustworthy by Default)

**Goal:** All examples are executable and validated against a live DG server.

Status: **Implemented** (Python/TypeScript example suites + system tests).

Deliverables:
- Python example suite under `examples/python/`:
  - Basic lifecycle (define → start → status)
  - Agent loop
  - CI/CD gating
  - Precheck with asserted evidence
- TypeScript example suite under `examples/typescript/`:
  - Same coverage as Python
- Example runner in system-tests that:
  - launches DG MCP server
  - executes each example
  - validates non-empty, schema-valid outputs

Acceptance Criteria:
- Each example has a corresponding system test entry.
- Examples pass under `scripts/verify_all.sh --system-tests=...`.
- Example payloads validate against generated schemas.

---

### Milestone 3 — Packaging Dry-Run Verification (No Publishing)

**Goal:** Packages are shippable without actually shipping them.

Status: **Implemented**.

Deliverables:
- Packaging validation script(s) (`scripts/package_dry_run.sh`):
  - Python: build sdist/wheel → install into temp venv → run SDK tests/examples.
  - TypeScript: `npm pack` → install tarball in temp project → run tests/examples.
- Add packaging dry-run step to `scripts/verify_all.sh` (optional flag).

Acceptance Criteria:
- Dry-run packaging passes locally and in CI.
- No publish step is executed; artifacts are built and installed from local outputs.

---

### Milestone 4 — Documentation and Integration Alignment

**Goal:** Docs reflect the exact current surface area, with no speculation.

Status: **In progress**.

Deliverables:
- Update `Docs/business/decision_gate_integration_landscape.md`
  to include local CI scripts, SDK validation helpers, and examples.
- Update SDK invariance plan with test coverage expectations.
- Add a “Publish Readiness Checklist” doc (manual sign-off) at
  `Docs/roadmap/decision_gate_publish_readiness_checklist.md`.

Acceptance Criteria:
- Docs match actual on-disk artifacts and tests.
- Manual review checklist is complete and versioned in the repo.

---

### Milestone 5 — Framework Adapters (Local, Unpublished)

**Goal:** Provide native-feel adapters for the dominant agent frameworks without publishing.

Status: **Implemented** (LangChain, CrewAI, AutoGen, OpenAI Agents SDK).

Deliverables:
- Adapter packages under `adapters/` with minimal, thin wrappers.
- Adapter examples under `examples/frameworks/`.
- Adapter READMEs with local install instructions.

Acceptance Criteria:
- Adapters import cleanly when dependencies are installed.
- Adapter examples are runnable against a local DG server.

---

### Milestone 6 — Adapter Test Harness (Optional)

**Goal:** Make adapter examples runnable under a gated, opt-in test harness.

Status: **Implemented**.

Deliverables:
- Script to install adapter dependencies in an isolated venv and run adapter examples (`scripts/adapter_tests.sh`).
- Optional wiring into `scripts/verify_all.sh` (flagged).

Acceptance Criteria:
- Adapter example runs are repeatable and isolated.
- Failures are loud and actionable.

---

### Milestone 7 — Agentic Flow Harness (Deterministic + Live)

**Goal:** Prove ecosystem integration by running canonical scenarios across
raw MCP, SDKs, and framework adapters.

Status: **Deterministic implemented; live-mode pending**.

Deliverables:
- Deterministic harness and scenario registry in system-tests.
- Canonical scenario packs mirrored into `examples/agentic/`.
- Cross-driver invariance check (runpack hash parity across projections).
- Live-mode harness (report-only) with:
  - pluggable LLM providers
  - allowlisted network access
  - transcript and artifact capture

Acceptance Criteria:
- Deterministic scenarios pass across all drivers.
- Runpack hashes match across projections and OS for deterministic mode.
- Live-mode runs are opt-in, report-only, and emit transcripts + runpacks.

---

## World-Class Quality Bars

**Ergonomics**
- Every tool method has concise, precise docs + examples.
- Examples are short, readable, and correct.
- Errors are actionable (clear, structured, consistent).

**Security**
- Unsafe features require explicit opt-in.
- Docs warn against insecure deployments.
- Example configs are safe-by-default.

**Performance**
- SDKs avoid heavy dependencies by default.
- Optional runtime validation is opt-in only.

**Understanding**
- Docs explain “why” not just “what.”
- Example outputs are validated and deterministic.

---

## Test Strategy

1) **Unit tests**
- Generator drift tests (already enforced).
- SDK transport tests (already enforced).

2) **System tests**
- Live MCP server + SDK clients (already enforced).
- Example suites must run in the same harness.

3) **Packaging dry-run**
- Install built artifacts into isolated environments.
- Run examples + SDK tests from installed packages (not repo source).

4) **Adapter examples (optional)**
- Run framework adapter examples in an isolated venv with external deps.
- Treat as opt-in gating due to third-party dependency footprint.

5) **Agentic flow harness**
- Deterministic harness is gating: scenario registry runs across projections.
- Live-mode harness is report-only: transcripts + artifacts for analysis.

---

## Local CI Entrypoints

- `scripts/generate_all.sh` — regenerate artifacts (or `--check`).
- `scripts/verify_all.sh` — drift checks + workspace tests (optional system tests).
- `scripts/package_dry_run.sh` — build/install packages without publishing.
- `scripts/verify_all.sh --package-dry-run` — run packaging verification.
- `scripts/adapter_tests.sh` — run adapter examples in an isolated venv (external deps).
- `scripts/verify_all.sh --adapter-tests=...` — run adapter verification.
- `scripts/agentic_harness.sh` — run the deterministic agentic flow harness.
- `scripts/agentic_harness_bootstrap.sh` — install harness driver deps.

---

## Publish Readiness Checklist (Manual Gate)

See `Docs/roadmap/decision_gate_publish_readiness_checklist.md`.

- [ ] All SDK + example tests pass (including system tests).
- [ ] Packaging dry-run passes for Python and TypeScript.
- [ ] Docs updated and reviewed.
- [ ] Security review complete (no unsafe defaults).
- [ ] Manual review sign-off.
- [ ] Agentic harness passes deterministically across drivers.

---

## Next Action (Suggested)

Start Milestone 2: build runnable Python + TypeScript example suites and wire
them into system-tests. This provides the most trust per unit of effort and
is the strongest signal of real-world readiness.
