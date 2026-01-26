<!--
Docs/integrations/assetcore/architecture.md
============================================================================
Document: DG + AssetCore Integration Architecture
Description: Architecture and data flow for DG/ASC alignment.
Purpose: Provide crisp, implementation-ready diagrams and flow.
Dependencies:
  - Docs/architecture/decision_gate_assetcore_integration_contract.md
============================================================================
-->

# DG + AssetCore Architecture

## High-Level Topology
```mermaid
flowchart TB
  Client[Agent / Client] -->|MCP tools| DG[Decision Gate MCP]
  DG -->|scenario_*| Core[Decision Gate Core]
  DG -->|evidence_query| ASCRead[AssetCore Read Daemon]
  Core --> Runpack[Runpack Builder]
  Runpack --> Artifacts[Deterministic Runpack + Manifest]

  ASCRead -->|anchors| Core
  ASCRead -->|world-state proofs| Core
```

## Namespace Authority Flow
```mermaid
sequenceDiagram
  participant Client as Client
  participant DG as Decision Gate MCP
  participant ASC as AssetCore Namespace Authority

  Client->>DG: scenario_define / scenario_start (namespace_id)
  DG->>ASC: validate namespace_id
  ASC-->>DG: allow/deny
  DG-->>Client: fail-closed if denied
```

## Evidence Anchoring Flow
```mermaid
sequenceDiagram
  participant DG as Decision Gate MCP
  participant Core as Decision Gate Core
  participant ASC as AssetCore Read Daemon
  participant Runpack as Runpack Builder

  DG->>Core: evidence_query (provider_id=assetcore_read)
  Core->>ASC: predicate query + params
  ASC-->>Core: EvidenceResult + anchors
  Core->>Runpack: record evidence + anchors
  Runpack-->>DG: manifest + integrity root hash
```

## Auth Mapping (Integration Layer)
DG does not parse ASC auth tokens. An external integration layer verifies ASC
principals and forwards a minimal principal context (tenant_id, principal_id,
roles, policy_class, groups). Mapping defaults are conservative and fail-closed.

## Schema Registry ACL (DG Internal)
Schema registry access is enforced inside DG after tool allowlists. Integration
layer RBAC determines which tools are callable; DG's registry ACL determines
per-tenant/namespace read/write permission for `schemas_*`.

Reference: `Docs/architecture/decision_gate_assetcore_integration_contract.md`
