<!--
Docs/roadmap/asc_dg_alignment_marketing_now.md
============================================================================
Document: Decision Gate + Asset Core Alignment (Marketing and Integration Now)
Description: Near-term messaging and integration collateral tasks.
Purpose: Make the DG/ASC overlap clear while preserving independence.
Dependencies:
  - README.md
  - Docs/guides/assetcore_interop_runbook.md
  - Docs/business/open_core_strategy.md
============================================================================
-->

# Decision Gate + Asset Core Alignment (Marketing and Integration Now)

## Overview
This roadmap lists the immediate marketing and integration documentation tasks
needed to make the DG/ASC overlap first-class without implying dependency.
The message should be: DG runs anywhere, and ASC makes DG deterministic and
audit-grade when a world-state substrate is required.

## Current Status (As Implemented)
Marketing/integration framing is implemented with the integration hub and README
positioning updates. The only remaining open item is the lead example narrative.

## Immediate Messaging Tasks
- **Root README positioning**: Add a concise section that states:
  - DG is a deterministic checkpoint / requirement evaluator.
  - DG runs independently.
  - For deterministic evidence + replay against a world-state substrate, plug
    DG into Asset Core.
- **Clarity on overlap**: Add a "Use DG with ASC when..." versus "ASC without DG"
  matrix to avoid confusion.

Implemented:
- Root README now includes an AssetCore integration section and links to the
  integration hub.
- Integration hub includes the overlap matrix in
  `Docs/integrations/assetcore/README.md`.

## Integration Collateral
- **Integration hub**: Create `Docs/integrations/assetcore/` as the canonical
  entry point with:
  - Overview and positioning.
  - Architecture diagrams (namespace + evidence anchors).
  - Integration patterns (read-only, streaming triggers, optional precheck).
  - Deployment topology (DG MCP + ASC MCP side-by-side).
- **Runbook consolidation**: Link `Docs/guides/assetcore_interop_runbook.md` to
  the hub and update the runbook to be implementation-focused, not marketing.

Implemented artifacts:
- `Docs/integrations/assetcore/README.md`
- `Docs/integrations/assetcore/architecture.md`
- `Docs/integrations/assetcore/deployment.md` (conceptual placeholder)
- `Docs/integrations/assetcore/examples.md` (TODO placeholder)

## Ecosystem Framing
- **Adoption-first narrative**: Emphasize that DG is open and standalone; ASC is
  the best-in-class substrate when the problem needs deterministic evidence and
  replay.
- **Terminology alignment**: Standardize phrasing across docs:
  - "Deterministic checkpoint / requirement evaluation"
  - "World-state substrate" (ASC)
  - "Evidence anchors" (world_seq, commit_id)

## Locked Decisions (Now)
- **Tagline**: "DG evaluates requirements. ASC provides the world-state substrate
  for deterministic evidence."
- **Integration phrasing**: "DG integrates with ASC." Keep the canonical
  integration hub in the DG repo.
- **Compatibility label**: Use "Compatible with AssetCore" (no trademark
  assumptions).
- **Diagrams**: Approved for inclusion in the integration hub (namespace +
  evidence anchors + topology).
- **Deployment recipes**: Conceptual only in V1; mark as TODO until validated.
- **Lead example**: Placeholder only (explicit TODO) until a world-class example
  is chosen.

## Open Questions (Must Answer Before Publishing)
- **Lead example**: Select a real-world, AssetCore-native scenario that proves
  DG strength and naturally motivates ASC. (Placeholder in docs until chosen.)
