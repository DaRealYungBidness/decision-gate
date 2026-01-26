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

## Immediate Messaging Tasks
- **Root README positioning**: Add a concise section that states:
  - DG is a deterministic checkpoint / requirement evaluator.
  - DG runs independently.
  - For deterministic evidence + replay against a world-state substrate, plug
    DG into Asset Core.
- **Clarity on overlap**: Add a "Use DG with ASC when..." versus "ASC without DG"
  matrix to avoid confusion.

## Integration Collateral
- **Integration hub**: Create `Docs/integrations/assetcore/` as the canonical
  entry point with:
  - Overview and positioning.
  - Architecture diagrams (namespace + evidence anchors).
  - Integration patterns (read-only, streaming triggers, optional precheck).
  - Deployment topology (DG MCP + ASC MCP side-by-side).
- **Runbook consolidation**: Link `Docs/guides/assetcore_interop_runbook.md` to
  the hub and update the runbook to be implementation-focused, not marketing.

## Ecosystem Framing
- **Adoption-first narrative**: Emphasize that DG is open and standalone; ASC is
  the best-in-class substrate when the problem needs deterministic evidence and
  replay.
- **Terminology alignment**: Standardize phrasing across docs:
  - "Deterministic checkpoint / requirement evaluation"
  - "World-state substrate" (ASC)
  - "Evidence anchors" (world_seq, commit_id)

## Open Questions (Must Answer Before Publishing)
- What is the official integration tagline? (Short, repeatable line for README)
- Which DG/ASC terms should be trademarked or treated as compatibility labels?
- Where should the integration hub live if DG is open and ASC is closed?
- How explicit should we be about ASC being proprietary in DG docs?
- Which real example should lead the integration story (agent planning,
  disclosure gating, deterministic replay)?

