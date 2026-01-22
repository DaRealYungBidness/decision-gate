<!--
Docs/guides/integration_patterns.md
============================================================================
Document: Decision Gate Integration Patterns
Description: Common integration patterns for Decision Gate.
Purpose: Provide guidance for CI, agent loops, and disclosure workflows.
Dependencies:
  - examples/agent-loop
  - examples/ci-gate
  - examples/data-disclosure
============================================================================
-->

# Integration Patterns

## Overview
Decision Gate integrates into workflows as a deterministic gate evaluator.
These patterns mirror common deployment scenarios and map directly to the
examples in `examples/`.

## Agent Loop Targets
Use `scenario_status` to surface unmet predicates and let the agent plan
toward satisfying them. Once predicates are satisfied, call `scenario_next`
to advance the stage.

Example: `examples/agent-loop`

## CI/CD Gate
Use predicates backed by CI provider evidence (build status, test results,
review approvals). Gate advancement signals readiness to deploy.

Example: `examples/ci-gate`

## Controlled Disclosure
Use a review gate to unlock a disclosure stage that emits packet payloads.
This creates a verifiable audit trail of what was released and when.

Example: `examples/data-disclosure`

