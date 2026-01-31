<!--
Docs/roadmap/decision_gate_world_class_onboarding_plan.md
============================================================================
Document: Decision Gate World-Class Onboarding Plan
Description: End-to-end plan for best-possible OSS onboarding experience.
Purpose: Deliver frictionless first-run experiences (source + binaries) with
         strict semantic-drift prevention and release gating.
Dependencies:
  - Docs/roadmap/decision_gate_world_class_integration_readiness_plan.md
  - Docs/roadmap/decision_gate_sdk_invariance_plan.md
  - Docs/roadmap/decision_gate_publish_readiness_checklist.md
  - system-tests/TEST_MATRIX.md
============================================================================
-->

# Decision Gate World-Class Onboarding Plan

## Executive Intent

Deliver the smoothest, least-brittle onboarding flow possible while making
semantic drift impossible. New users must be able to run a real Decision Gate
workflow in minutes, with clear success signals and no hidden setup traps.

This plan focuses on **onboarding**, not publishing. Publishing happens only
after all gates in this plan are green.

---

## Glossary (Onboarding Modes)

- **Download-and-go**: prebuilt artifacts (binaries or container) that run
  without local toolchains (no Rust/Python/Node installs).
- **Clone-and-go**: source-first path that bootstraps toolchains and runs a
  working server + sample in a single command.
- **Package-and-go**: installing SDKs/adapters via package managers once
  published (pip/npm), plus minimal code samples.

---

## Non-Negotiable Invariants

1. **Single source of truth**: MCP tool definitions are canonical; SDKs,
   adapters, and docs are generated or validated from them.
2. **Semantic drift is blocked**: any mismatch between MCP spec, SDK, adapters,
   docs, or examples must fail CI.
3. **Deterministic outputs**: same inputs produce byte-identical outputs for
   core flows and runpacks.
4. **Clear success signal**: onboarding always ends with a verified tool call
   (e.g., precheck or scenario status) and an explicit green signal.
5. **Security by default**: no onboarding step weakens security defaults or
   hides policy requirements.
6. **One-command entry**: each onboarding mode has a single primary command.

---

## Target User Journeys

### Journey A — Download-and-go (fastest trial)
1. Download a release artifact (or run a container).
2. Start the server with a sample config (bundled or auto-generated).
3. Run a single tool invocation (CLI helper or sample script).
4. See a deterministic success output + pointer to next steps.

### Journey B — Clone-and-go (source-first)
1. Clone repo.
2. Run a bootstrap script that installs toolchains + dependencies.
3. Start server and execute a sample tool call.
4. See a deterministic success output + pointer to next steps.

### Journey C — Package-and-go (SDK/adapters)
1. Install SDK or adapter via package manager.
2. Run a 10-line sample script.
3. See a deterministic success output + pointer to advanced usage.

---

## Milestones

### Milestone 0 — Define the Onboarding Contract
**Goal:** Align on what “success” means for each onboarding path.

Deliverables:
- Onboarding success criteria (explicit, testable).
- Canonical “hello flow” scenario (spec + run config + trigger).
- A single success output format shared across SDKs/adapters.

Acceptance Criteria:
- A newcomer can verify a working setup in < 5 minutes.
- Success output is deterministic and validated in tests.

---

### Milestone 1 — Canonical Tool Surface and Drift Gates
**Goal:** Eliminate any divergence between MCP spec, SDKs, adapters, and docs.

Deliverables:
- MCP tool spec remains canonical (`Docs/generated/.../tooling.json`).
- SDKs and adapters validated against the spec (tool list + signatures).
- Docs verification updated to fail on drift.

Acceptance Criteria:
- CI fails if any tool surface mismatch exists.
- `docs_verify` passes on clean checkouts.

---

### Milestone 2 — Adapter Completeness and Conformance
**Goal:** All adapters expose full MCP tool surface with consistent behavior.

Deliverables:
- Adapter tool coverage extended to all MCP tools.
- Adapter conformance tests (tool presence + minimal roundtrip calls).
- Framework-specific output conventions verified (e.g., JSON strings for CrewAI).

Acceptance Criteria:
- Each adapter passes conformance tests in CI.
- No missing MCP tool is tolerated.

---

### Milestone 3 — Clone-and-go Bootstrap
**Goal:** One-command developer onboarding from source.

Deliverables:
- `scripts/bootstrap.sh` (or equivalent) that:
  - installs toolchains (if missing)
  - sets up Python venv + installs SDK/adapters
  - builds or runs the server
  - executes the canonical “hello flow”
- README “Quickstart” referencing this script.

Acceptance Criteria:
- Fresh clone on Linux/macOS succeeds with one command.
- Output shows a clean success signal.

---

### Milestone 4 — Download-and-go Artifacts
**Goal:** Run without local toolchains.

Deliverables:
- Prebuilt binaries for `decision-gate-cli` (major OS/arch).
- Minimal config + sample pack bundled with releases.
- “Quickstart (binary)” section with a single command.
- Optional container image with the same quickstart.

Acceptance Criteria:
- A user can run a working server + hello flow without installing Rust/Python.

---

### Milestone 5 — Package-and-go (SDKs + Adapters)
**Goal:** Make SDK/adapters painless to install and use.

Deliverables:
- Packaging fixes for `src/` layouts (editable installs work).
- SDK and adapter install docs (pip/npm) with minimal examples.
- Optional “extras” documented (e.g., validation dependencies).

Acceptance Criteria:
- SDK + adapter examples run in under 3 minutes after install.

---

### Milestone 6 — Onboarding QA and Release Gate
**Goal:** Onboarding is provably stable before any release.

Deliverables:
- A dedicated onboarding test suite:
  - binary quickstart
  - clone-and-go script
  - SDK + adapter samples
- CI gates: onboarding suite must pass for release.

Acceptance Criteria:
- No release candidate can be built if onboarding fails.

---

## Semantic Drift Prevention (How It Becomes Impossible)

- **Canonical spec**: MCP tool definitions are the source of truth.
- **Generated/validated surfaces**: SDKs/adapters are generated or checked
  against the spec on every CI run.
- **Docs verification**: docs are required to match current behavior.
- **System tests**: agentic harness runs raw MCP + SDKs + adapters and compares
  outcomes.
- **Release gating**: publishing is blocked unless all drift checks pass.

---

## Success Metrics (Objective)

- Median time-to-first-success: **< 5 minutes**.
- Onboarding failure rate in CI: **0%**.
- Adapter tool surface coverage: **100%** of MCP tools.
- Drift incidents post-release: **0** (blocked by CI).

---

## Immediate Next Steps (Actionable)

1. Define the canonical hello flow scenario pack.
2. Add a conformance test that compares adapter tool lists to MCP spec.
3. Fix adapter packaging to ensure editable installs work.
4. Draft a minimal Quickstart for clone-and-go (script + doc).

---

## Out of Scope (Explicitly Deferred)

- Marketing site updates
- Enterprise-only onboarding
- Telemetry or analytics

