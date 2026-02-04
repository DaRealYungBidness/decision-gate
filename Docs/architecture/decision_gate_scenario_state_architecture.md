<!--
Docs/architecture/decision_gate_scenario_state_architecture.md
============================================================================
Document: Decision Gate Scenario Lifecycle + State Store Architecture
Description: Current-state reference for scenario specs, runtime lifecycle,
             and run state persistence.
Purpose: Provide an implementation-grade map of scenario execution and storage.
Dependencies:
  - decision-gate-core/src/core/spec.rs
  - decision-gate-core/src/core/state.rs
  - decision-gate-core/src/runtime/engine.rs
  - decision-gate-core/src/runtime/store.rs
  - decision-gate-mcp/src/tools.rs
  - decision-gate-config/src/config.rs
  - decision-gate-store-sqlite/src/store.rs
============================================================================
Last Updated: 2026-02-04 (UTC)
============================================================================
-->

# Decision Gate Scenario Lifecycle + State Store Architecture

> **Audience:** Engineers implementing scenario execution, run lifecycle, and
> run state persistence.

---

## Table of Contents

1. [Executive Overview](#executive-overview)
2. [Scenario Specification](#scenario-specification)
3. [Scenario Runtime Lifecycle (MCP)](#scenario-runtime-lifecycle-mcp)
4. [Run State Model](#run-state-model)
5. [Control Plane Execution Flow](#control-plane-execution-flow)
6. [Run State Stores](#run-state-stores)
7. [File-by-File Cross Reference](#file-by-file-cross-reference)

---

## Executive Overview

Scenarios define staged disclosure workflows in a deterministic spec. The MCP
layer validates and registers scenarios, then instantiates a control plane
runtime for each scenario. Runs are persisted via a `RunStateStore` implementation
(in-memory or SQLite). The run state is append-only, logging triggers, gate
outcomes, decisions, packets, submissions, and tool calls.
[F:decision-gate-core/src/core/spec.rs L51-L114](decision-gate-core/src/core/spec.rs#L51-L114) [F:decision-gate-mcp/src/tools.rs L721-L895](decision-gate-mcp/src/tools.rs#L721-L895) [F:decision-gate-core/src/core/state.rs L357-L394](decision-gate-core/src/core/state.rs#L357-L394)

---

## Scenario Specification

`ScenarioSpec` is the canonical scenario definition:

- Identifiers (scenario, namespace, spec version)
- Stage definitions and gate logic
- Condition definitions and evidence queries
- Optional schema references and default tenant id

Specs are validated on load to ensure uniqueness and internal consistency.
[F:decision-gate-core/src/core/spec.rs L51-L114](decision-gate-core/src/core/spec.rs#L51-L114)

Stage-level behavior is defined by `StageSpec` (entry packets, gates, branching,
optional timeout).
[F:decision-gate-core/src/core/spec.rs L121-L140](decision-gate-core/src/core/spec.rs#L121-L140)

---

## Scenario Runtime Lifecycle (MCP)

### Define Scenario
`scenario_define` registers a scenario and caches a runtime in the tool router:

- Namespace enforcement
- Capability registry validation
- Strict comparator validation
- ControlPlane instantiation with current trust + anchor policy

[F:decision-gate-mcp/src/tools.rs L721-L749](decision-gate-mcp/src/tools.rs#L721-L749) [F:decision-gate-mcp/src/tools.rs L2045-L2066](decision-gate-mcp/src/tools.rs#L2045-L2066)

### Start Run
`scenario_start` creates a new run state using the control plane and persists it
via the configured store.
[F:decision-gate-mcp/src/tools.rs L760-L814](decision-gate-mcp/src/tools.rs#L760-L814)

### Status / Next / Submit / Trigger
Subsequent tools operate on the cached runtime and persisted run state:

- `scenario_status` reads the current status
- `scenario_next` advances based on available evidence
- `scenario_submit` uploads external artifacts
- `scenario_trigger` injects an external trigger event

[F:decision-gate-mcp/src/tools.rs L816-L977](decision-gate-mcp/src/tools.rs#L816-L977)

`scenario_next` can optionally include feedback (summary/trace/evidence) in the
tool response when permitted by server feedback policy. Trace feedback reuses
stored gate evaluations; evidence feedback can surface gate evaluation records
with disclosure policy applied.
[F:decision-gate-mcp/src/tools.rs L2144-L2257](decision-gate-mcp/src/tools.rs#L2144-L2257) [F:decision-gate-core/src/core/state.rs L357-L394](decision-gate-core/src/core/state.rs#L357-L394)

---

## Run State Model

Run state is a structured, append-only log containing:

- Tenant, namespace, run, scenario identifiers
- Current stage and lifecycle status
- Dispatch targets
- Trigger log, gate evaluation log, decision log
- Packets, submissions, and tool call transcripts

[F:decision-gate-core/src/core/state.rs L357-L394](decision-gate-core/src/core/state.rs#L357-L394)

Run lifecycle status is a closed enum: `active`, `completed`, `failed`.
[F:decision-gate-core/src/core/state.rs L72-L85](decision-gate-core/src/core/state.rs#L72-L85)

---

## Control Plane Execution Flow

The control plane engine executes scenario transitions, evaluates evidence, and
records decisions. It persists run state after key transitions and uses
trust/anchor policies configured at runtime.
[F:decision-gate-core/src/runtime/engine.rs L153-L178](decision-gate-core/src/runtime/engine.rs#L153-L178) [F:decision-gate-mcp/src/tools.rs L2029-L2066](decision-gate-mcp/src/tools.rs#L2029-L2066)

---

## Run State Stores

### In-Memory Store
The in-memory store is intended for tests and demos. It implements `RunStateStore`
with a mutex-protected map.
[F:decision-gate-core/src/runtime/store.rs L53-L127](decision-gate-core/src/runtime/store.rs#L53-L127)

### SQLite Store
The SQLite store provides durable snapshots:

- Each save stores canonical JSON plus a hash.
- Loads verify hash integrity and key consistency.
- Versions are tracked per run, with optional retention pruning.

[F:decision-gate-store-sqlite/src/store.rs L540-L640](decision-gate-store-sqlite/src/store.rs#L540-L640)

Store configuration supports WAL mode, sync mode, busy timeout, and retention
limits.
[F:decision-gate-store-sqlite/src/store.rs L135-L156](decision-gate-store-sqlite/src/store.rs#L135-L156)

### MCP Configuration
The MCP layer selects store type via `run_state_store` configuration.
[F:decision-gate-config/src/config.rs L1523-L1582](decision-gate-config/src/config.rs#L1523-L1582)

---

## File-by-File Cross Reference

| Area | File | Notes |
| --- | --- | --- |
| Scenario spec + validation | `decision-gate-core/src/core/spec.rs` | Canonical scenario structure + invariants. |
| Run state model | `decision-gate-core/src/core/state.rs` | Run status + append-only logs. |
| Control plane engine | `decision-gate-core/src/runtime/engine.rs` | Execution and decision flow. |
| MCP tool lifecycle | `decision-gate-mcp/src/tools.rs` | scenario_define/start/next/submit/trigger/status. |
| In-memory store | `decision-gate-core/src/runtime/store.rs` | Test/deterministic store implementation. |
| SQLite store | `decision-gate-store-sqlite/src/store.rs` | Durable store with hash verification + retention. |
| Store config | `decision-gate-config/src/config.rs` | run_state_store selection + validation. |
