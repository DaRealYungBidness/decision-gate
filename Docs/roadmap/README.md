<!--
Docs/roadmap/README.md
============================================================================
Document: Decision Gate Roadmap (Public)
Description: Public, future-facing roadmap for OSS launch and early adoption.
Purpose: Track priorities and status for launch and early adoption work.
Dependencies:
  - Docs/roadmap/foundational_correctness_roadmap.md
  - Docs/roadmap/decision_gate_agentic_flow_harness_plan.md
============================================================================
-->

# Decision Gate Roadmap

## Purpose

This roadmap is future-facing and focused on OSS launch and early adoption.
We are not accepting external PRs at this time; feedback via issues and
discussion is welcome.

Primary audience: adopters, partners evaluating integrations, and maintainers.

The authoritative correctness gate checklist lives in
[F:Docs/roadmap/foundational_correctness_roadmap.md L21-L30](foundational_correctness_roadmap.md#L21-L30).

## Strategic Direction

Current focus:

1. Adapter usage, ergonomics, correctness (see
   [F:adapters/README.md L1-L12](../../adapters/README.md#L1-L12)).
2. External provider integration hardening and integration
   maturity (provider contracts, MCP interoperability, and compatibility).

## Roadmap Conventions

Priority indicates importance, not sequence. Work can happen out of order.

| Priority | Meaning                                                                       |
| -------- | ----------------------------------------------------------------------------- |
| P0       | Highest priority. These items materially affect launch confidence and safety. |
| P1       | Important for early adoption and operator readiness.                          |
| P2       | Valuable improvements and guidance, but not required for initial launch.      |
| P3       | Low priority. Nice-to-have items that may happen opportunistically.           |

Status indicates active state, not priority.

| Status      | Meaning                                                 |
| ----------- | ------------------------------------------------------- |
| Planned     | Tracked and intended, but not active.                   |
| In progress | Actively being worked.                                  |
| Ongoing     | Continuous or recurring work without a clear end state. |

## P0) Highest Priority

| Item                                                                             | Why                                                                                         | Notes                                                                                                                                               | Status      |
| -------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------- | ----------- |
| Foundational correctness gates                                                   | Security and determinism require adversarial depth and cross-surface confidence.            | Track detailed checklist in [F:Docs/roadmap/foundational_correctness_roadmap.md L21-L30](foundational_correctness_roadmap.md#L21-L30). | In progress |
| External provider integration hardening                                          | Provider integrations are experimental; hardening and compatibility reduce regression risk. | Ongoing focus with evolving scope; see Strategic Direction.                                                                                         | Ongoing     |
| Metamorphic determinism coverage for provider order and evidence arrival reorder | Determinism must hold under reordered inputs and concurrency.                               | Focus on provider-order shuffle and evidence-arrival reorder cases.                                                                                 | In progress |
| Agentic flow harness parity for deterministic runs                               | Live-mode parity across OS ensures deterministic replay and auditability.                   | See [F:Docs/roadmap/decision_gate_agentic_flow_harness_plan.md L17-L45](decision_gate_agentic_flow_harness_plan.md#L17-L45).           | In progress |

## P1) High Priority

| Item                                                 | Why                                                                    | Notes                                                       | Status  |
| ---------------------------------------------------- | ---------------------------------------------------------------------- | ----------------------------------------------------------- | ------- |
| Performance and scaling targets with gated benchmark | Establish baseline capacity expectations and detect regressions early. | At least one gated benchmark with documented thresholds.    | Planned |
| Reproducible build guidance and version stamping     | Helps adopters verify artifacts and reproduce releases.                | Focus on CLI and runpack tooling.                           | Planned |
| Quick Start validation on Linux and Windows          | Cross-OS onboarding must be predictable for adopters.                  | Validate scripts and documentation end to end.              | Planned |
| Capacity and limits documentation                    | Operators need clear size and payload boundaries.                      | Include runpack sizes, evidence payload caps, and timeouts. | Planned |
| Ergonomics and bug-fix sweep                         | Reduce sharp edges before broader adoption.                            | Focus on CLI, SDKs, and error messaging.                    | Ongoing |

## P2) Medium Priority

| Item                                                     | Why                                                               | Notes                                                                   | Status  |
| -------------------------------------------------------- | ----------------------------------------------------------------- | ----------------------------------------------------------------------- | ------- |
| Scenario examples for hold, unknown, and branch outcomes | Authors need precise examples for tri-state routing.              | Add canonical scenarios under `Docs/generated/decision-gate/examples/`. | Planned |
| Agent progress vs plan state guidance                    | Keep agent planning external while modeling progress as evidence. | Document guidance for common workflows.                                 | Planned |
| Runpack verification with evidence replay (optional)     | Adds an audit mode when evidence sources are stable.              | Optional CLI or MCP flow.                                               | Planned |
| Public integration docs hygiene                          | OSS docs should not depend on private repo content.               | Remove or replace AssetCore placeholders with OSS-safe examples.        | Planned |
| AssetCore example and deployment placeholders            | Avoid public TODOs that imply private dependencies.               | Remove or replace if OSS-safe guidance is available.                    | Planned |

## P3) Low Priority

| Item                          | Why                                                                        | Notes                                                                                      | Status  |
| ----------------------------- | -------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------ | ------- |
| Provider contract bulk export | Useful for offline indexing and caching, but high disclosure and DoS risk. | Requires explicit opt-in, pagination, strict size limits, and authz gating before revisit. | Planned |
