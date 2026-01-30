<!--
Docs/roadmap/decision_gate_docs_sync_and_diagrams_plan.md
============================================================================
Document: Decision Gate Docs Sync + Diagram Localization Plan
Description: Plan to publish Decision Gate architecture + AssetCore integration
             docs on Asset-Core-Web with localized diagrams and OpenAPI download.
Purpose: Provide a world-class, CSP-safe, i18n-ready documentation pipeline.
Dependencies:
  - Docs/architecture/*
  - Docs/integrations/assetcore/*
  - Docs/generated/openapi/decision-gate.json
  - Asset-Core-Web/scripts/sync-decision-gate-docs.mjs
  - Asset-Core-Web/i18n/README.md
============================================================================
-->

# Decision Gate Docs Sync + Diagram Localization Plan

## Purpose

Publish Decision Gate architecture + AssetCore integration docs as a public
"engineering rigor" showcase, while keeping Asset-Core-Web fully static,
CSP-compliant, and build-time i18n-only. Add OpenAPI downloads and localized
diagram rendering without client-side networking or inline scripts.

## Principles

- Read-only OSS documentation (no contributions accepted).
- CSP strict: no inline scripts/styles, no runtime networking.
- Diagrams render at build time with locale-specific text.
- English remains canonical; localized docs generated via i18n pipeline.
- Generated artifacts are synced, not hand-edited.

## Status (Last Updated: 2026-01-29)

- [x] Scope confirmed (architecture + AssetCore integration + OpenAPI download).
- [x] Sync script extended in Asset-Core-Web to include new docs + OpenAPI.
- [x] Architecture + integrations index pages drafted in Asset-Core-Web.
- [x] Mermaid label placeholders added to DG AssetCore integration diagrams.
- [x] Build-time mermaid renderer + remark transformer implemented.
- [x] CSP documentation updated for diagram rendering.
- [x] Run sync + i18n annotate/scan (apply intentionally deferred).
- [x] Run build + lint gates (diagram output validated for EN build).
- [ ] Re-run i18n:apply and verify localized diagrams after naming/translation lock.

## Scope

**In scope**
- Sync all `Docs/architecture/*` into Asset-Core-Web.
- Sync `Docs/integrations/assetcore/*` to support AssetCore positioning.
- Publish OpenAPI artifact as a download.
- Add diagram rendering with localized SVGs (build-time only).
- Add navigation + index pages for new sections.

**Out of scope**
- Config single-source-of-truth work (handled separately).
- Roadmap/business/standards docs (internal or pending deletion).

---

## Phase 0 — Content Gate + Messaging

**Objective:** confirm public positioning and set expectation of read-only OSS.

Checklist:
- Confirm the public list of architecture docs (see Phase 1).
- Approve AssetCore integration docs for publication.
- Add a short, consistent “read-only OSS” disclaimer on Architecture index page.
- Confirm OpenAPI download placement in Reference.

Deliverables:
- Final list of docs to sync.
- Approved copy for the Architecture index disclaimer.

---

## Phase 1 — Sync Expansion (Docs + Downloads)

**Objective:** expand sync surface in `Asset-Core-Web/scripts/sync-decision-gate-docs.mjs`.

### 1.1 Architecture docs
Sync `Docs/architecture/*` to:
- `src/content/en/docs/decision-gate/architecture/*.md`

Suggested set (public value, not purely internal):
- `Docs/architecture/decision_gate_provider_capability_architecture.md`
- `Docs/architecture/decision_gate_runpack_architecture.md`
- `Docs/architecture/decision_gate_scenario_state_architecture.md`
- `Docs/architecture/decision_gate_evidence_trust_anchor_architecture.md`
- `Docs/architecture/decision_gate_auth_disclosure_architecture.md`
- `Docs/architecture/decision_gate_namespace_registry_rbac_architecture.md`
- `Docs/architecture/comparator_validation_architecture.md`
- `Docs/architecture/decision_gate_assetcore_integration_contract.md`

Optional (only if desired as public rigor proof):
- `Docs/architecture/decision_gate_system_test_architecture.md`

### 1.2 AssetCore integration docs
Sync `Docs/integrations/assetcore/*` to:
- `src/content/en/docs/decision-gate/integrations/assetcore/*.md`

Expected files:
- `Docs/integrations/assetcore/README.md`
- `Docs/integrations/assetcore/architecture.md`
- `Docs/integrations/assetcore/deployment.md`
- `Docs/integrations/assetcore/examples.md`

### 1.3 OpenAPI download
Copy `Docs/generated/openapi/decision-gate.json` to:
- `public/downloads/decision-gate/openapi/decision-gate.json`

Add references in:
- `src/content/en/docs/decision-gate/reference/index.md` (new “Machine-readable specs”)
- `src/content/en/docs/decision-gate/reference/tooling.md` (optional link)

### 1.4 Navigation + index pages
Add index pages to Asset-Core-Web:
- `src/content/en/docs/decision-gate/architecture/index.md`
- `src/content/en/docs/decision-gate/integrations/assetcore/index.md`

Ensure doc nav uses English anchors for i18n stability.

Deliverables:
- Sync additions in `scripts/sync-decision-gate-docs.mjs`.
- New download + updated reference links.
- New architecture + integration index pages.

---

## Phase 2 — Diagram Rendering + Localization (Build-Time)

**Objective:** render diagrams as localized SVGs with no runtime JS or network.

### 2.1 Diagram content format
- Keep Mermaid source in DG docs.
- Introduce **label placeholders** in Mermaid:
  - Example: `{{diagram.label_gate_eval}}`
- Add **diagram label entries** as list items with inline code keys:
  - Example:
    - `` `diagram.label_gate_eval`: Gate evaluation ``
    - `` `diagram.label_evidence_query`: Evidence query ``
- Add a required alt line in each Mermaid block:
  - `%% alt: {{diagram.alt.key}}`
- The i18n pipeline will translate these strings as normal segments.

### 2.2 Build-time renderer
Add a new build script in Asset-Core-Web (e.g., `scripts/render-diagrams.mts`):
- Input: localized markdown (post-i18n) + Mermaid code blocks.
- Extract label map per locale.
- Substitute placeholders → render Mermaid → SVG output per locale:
  - `public/diagrams/{locale}/decision-gate/.../*.svg`
- Replace Mermaid code blocks (or add a custom MD transformer) to render
  `<img src="/diagrams/{locale}/...">`.

### 2.3 CSP + i18n compliance
- Rendering occurs **before** Astro build.
- No runtime JS required for diagram rendering.
- Add a build check: fail if any `{{diagram.*}}` placeholders remain.

Deliverables:
- `scripts/render-diagrams.mjs` (build step).
- Updated build pipeline to run after `npm run i18n:apply`.
- Documentation updates in `docs/security.md` or `docs/csp-plan.md` describing
  the build-time diagram process.

---

## Phase 3 — QA + Verification Gates

**Objective:** ensure docs are correct, stable, and localized.

- Run `npm run lint:no-inline` after new templates.
- Run i18n pipeline: `npm run i18n:scan`, `npm run i18n:apply`.
- Build and verify localized routes: `npm run build && npm run preview`.
- Validate:
  - Architecture + AssetCore integration pages render.
  - Diagrams render in `en`, `ca`, `es` with translated labels.
  - OpenAPI download link is present and correct.

Deliverables:
- Confirmed build for `/docs/decision-gate/**` and localized counterparts.
- Verified OpenAPI download asset in `public/downloads/decision-gate/openapi/`.

---

## Phase 4 — Polish + Messaging

**Objective:** tighten narrative for “engineering rigor” and AssetCore positioning.

- Add a concise “Rigor + Read-Only OSS” callout on Architecture index page.
- Add a short “Why AssetCore” paragraph in the integration index.
- Ensure cross-links from DG docs to Asset Core docs (English anchors).

---

## Risks / Watchouts

- Mermaid rendering dependency introduces new build tooling; verify CSP rules.
- Diagram text must be localized via labels list (avoid hard-coded text).
- Some architecture docs are deeply internal; ensure phrasing is acceptable.

---

## Success Criteria

- Architecture and AssetCore integration docs are visible and navigable.
- Diagrams render as localized SVGs without client-side JS.
- OpenAPI artifact is downloadable from the docs site.
- All new content passes CSP and i18n checks.

---

## Follow-ups (Explicitly Deferred)

- Config single source-of-truth (handled separately).
