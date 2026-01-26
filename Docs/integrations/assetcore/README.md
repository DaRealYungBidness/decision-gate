<!--
Docs/integrations/assetcore/README.md
============================================================================
Document: DG + AssetCore Integration Hub
Description: Canonical entry point for DG/ASC overlap and positioning.
Purpose: Explain how DG integrates with ASC without implying dependency.
Dependencies:
  - Docs/architecture/decision_gate_assetcore_integration_contract.md
  - Docs/guides/assetcore_interop_runbook.md
============================================================================
-->

# Decision Gate + AssetCore Integration

**Tagline**: DG evaluates requirements. ASC provides the world-state substrate
for deterministic evidence.

**Compatibility**: Compatible with AssetCore.

## Positioning
- **DG is standalone**: A deterministic checkpoint and requirement-evaluation
  control plane.
- **ASC is standalone**: A proprietary world-state substrate.
- **Integration is optional**: Use ASC when deterministic evidence, replay, or
  audited world-state proofs are required.

## What Integration Means (At a Glance)
- **DG integrates with ASC** through explicit interfaces (no code coupling).
- **ASC remains the source of truth** for world-state and namespaces.
- **DG remains the source of truth** for decisions, gates, and runpacks.

## Integration Patterns
- **Read-only evidence**: DG queries ASC read daemon for predicates.
- **Namespace authority**: DG validates namespaces against ASC (fail-closed).
- **Evidence anchors**: ASC anchors (`namespace_id`, `commit_id`, `world_seq`)
  are recorded in runpacks for offline verification.
- **Auth mapping**: ASC principals are mapped to DG tool permissions by an
  integration layer (DG does not parse ASC auth tokens).

## When to Use ASC with DG
| Use DG with ASC when... | Use DG without ASC when... |
| --- | --- |
| You need deterministic evidence and replay across world-state snapshots. | Evidence comes from non-ASC providers or simple asserted inputs. |
| You require audit-grade anchors tied to a stateful substrate. | You only need lightweight gating for internal workflows. |
| Namespace authority must be enforced against a system of record. | Namespace authority can be handled by DG's own registry. |

## Where to Start
- Canonical contract: `Docs/architecture/decision_gate_assetcore_integration_contract.md`
- Implementation runbook: `Docs/guides/assetcore_interop_runbook.md`
- Architecture diagrams: `Docs/integrations/assetcore/architecture.md`

## Lead Example
**TODO (placeholder)**: Select and document a world-class example that proves
DG value and naturally motivates ASC adoption.
