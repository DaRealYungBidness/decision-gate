<!--
Docs/integrations/assetcore/README.md
============================================================================
Document: DG + AssetCore Integration Hub
Description: Canonical entry point for DG/ASC overlap and positioning.
Purpose: Explain how DG integrates with ASC without implying dependency.
Dependencies:
  - ../architecture/decision_gate_assetcore_integration_contract.md
  - ../guides/assetcore_interop_runbook.md
============================================================================
-->

# Decision Gate + AssetCore Integration

**Tagline**: DG evaluates requirements. ASC provides the world-state substrate
for deterministic evidence.

**Compatibility**: Compatible with AssetCore.

## Table of Contents

- [Positioning](#positioning)
- [Integration Boundaries](#integration-boundaries)
- [Data Flow](#data-flow)
- [When to Use ASC](#when-to-use-asc)
- [Starting Points](#starting-points)
- [References](#references)

## Positioning

- **Decision Gate**: deterministic checkpoint and requirement-evaluation control
  plane.
- **AssetCore**: proprietary world-state substrate (namespaces, proofs, replay).
- **Integration**: optional and explicit; no code coupling between repos.

## Integration Boundaries

- DG remains authoritative for scenarios, gates, decisions, and runpacks.
- ASC remains authoritative for world-state and namespace validity.
- Auth tokens from ASC are not parsed by DG; an integration layer maps
  principals to DG tool permissions.

## Data Flow

```mermaid
%% alt: {{diagram.assetcore.readme.alt}}
flowchart TB
  Client[{{diagram.assetcore.readme.client}}] --> DG[{{diagram.assetcore.readme.dg}}]
  DG -->|{{diagram.assetcore.readme.namespace_check}}| ASC[{{diagram.assetcore.readme.namespace_authority}}]
  DG -->|{{diagram.assetcore.readme.evidence_query}}| ASCRead[{{diagram.assetcore.readme.read_daemon}}]
  DG --> Runpack[{{diagram.assetcore.readme.runpack_artifacts}}]
  ASCRead --> DG
```

- `diagram.assetcore.readme.alt`: Decision Gate + AssetCore data flow showing namespace checks, evidence queries, and runpack artifacts.
- `diagram.assetcore.readme.client`: Caller
- `diagram.assetcore.readme.dg`: Decision Gate
- `diagram.assetcore.readme.namespace_check`: namespace check
- `diagram.assetcore.readme.namespace_authority`: AssetCore namespace authority
- `diagram.assetcore.readme.evidence_query`: evidence query
- `diagram.assetcore.readme.read_daemon`: AssetCore read daemon
- `diagram.assetcore.readme.runpack_artifacts`: Runpack artifacts

## When to Use ASC

Use DG with ASC when:
- Evidence must be replayable against a deterministic world-state snapshot.
- Namespace authority must be enforced by a system of record.
- Auditable anchors must reference ASC commit or sequence metadata.

Use DG without ASC when:
- Evidence comes from local artifacts or non-ASC services.
- A lightweight gating layer is sufficient.

## Starting Points

- Contract: `Docs/architecture/decision_gate_assetcore_integration_contract.md`
- Architecture diagrams: `Docs/integrations/assetcore/architecture.md`
- Integration examples: `Docs/integrations/assetcore/examples.md`

## References

- `Docs/architecture/decision_gate_assetcore_integration_contract.md`
- `Docs/integrations/assetcore/architecture.md`
- `Docs/integrations/assetcore/deployment.md`
- `Docs/integrations/assetcore/examples.md`
