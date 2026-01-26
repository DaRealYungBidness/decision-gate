# Decision Gate: Product Phasing, Open Core vs Enterprise, and Initial Cloud Strategy

This document outlines a pragmatic, founder‑efficient path for turning Decision Gate (DG) into a revenue‑generating product while preserving its long‑term role as a foundational governance layer alongside AssetCore.

It assumes:

- Initial wedge market: **AI agents and autonomous tooling**.
- Long‑term market: **general regulated automation, CI/CD governance, robotics, finance, research labs**.
- Constraint: **maximize revenue per unit of founder time**.
- Preference: **open core, productized monetization, minimal consulting**.

---

## 1. Strategic Frame

Decision Gate is positioned as:

> **A deterministic, evidence‑driven control plane that gates agent actions and disclosures and produces audit‑grade proof artifacts.**

It is not an agent runtime. It is what production agents graduate into.

DG must:

- Stand alone as a profitable product.
- Create a natural upgrade path into AssetCore.
- Remain mathematically principled at the core.

---

## 2. Market Entry: AI Agents as the Wedge

Agents create immediate pain:

- Untrusted tool calls.
- Production deployment risk.
- Data leakage.
- Regulatory anxiety.
- Post‑incident forensics.

DG’s differentiators map cleanly:

- Trust lanes.
- Verified providers.
- Deterministic runs.
- Runpacks.
- MCP integration.

Messaging:

- "Agents are easy to prototype. Production requires proof."
- "Put DG between agents and irreversible actions."

---

## 3. Product Phases

These phases are additive, not rewrites. The same core engine evolves into increasingly monetizable operational products.

---

### Phase 0 — OSS Kernel Completion and Hardening

Goal: Establish DG as a credible standard gating kernel.

Deliverables:

- Stable ScenarioSpec / GateSpec / RET model.
- Provider SDKs.
- Built‑in providers.
- MCP server.
- CLI.
- Runpack format + verifier.
- Contract generation.
- Docs + runnable agent examples.

No monetization yet. This is ecosystem gravity.

---

### Phase 1 — DG Cloud (Developer Tier)

Goal: First revenue with near‑zero handholding.

DG Cloud is:

- Hosted MCP endpoint.
- Hosted registry.
- Hosted run state store.
- Hosted runpack export + verify.
- API‑key auth.
- Usage quotas.
- Minimal web UI (list runs, download runpacks).

Pricing:

- Monthly subscription.
- Self‑serve signup.

Founder workload:

- Operate one fleet.
- No bespoke installs.
- No custom providers per customer.

---

### Phase 2 — DG Cloud (Org / Compliance Tier)

Goal: Higher ACV without consulting.

Adds:

- Org accounts.
- Namespace isolation UI.
- RBAC.
- SSO.
- Retention policies.
- Audit dashboards.
- SLA tier.

Still self‑serve upgrade.

---

### Phase 3 — Premium / AssetCore Tier

Goal: High‑stakes users and platform unification.

Adds:

- AssetCore‑anchored runs.
- Immutable log backends.
- Attestation chains.
- Air‑gapped / on‑prem SKU.
- FIPS builds.

These customers involve more engagement and pay accordingly.

---

## 4. Open Core vs Enterprise: Governing Principle

**Open Source:** deterministic semantics and extension hooks.

**Paid:** running this safely across organizations at scale.

If the feature answers:

- "Can I embed this in my system?" → open.
- "Can my company standardize on this and audit it?" → enterprise.

---

## 5. Open Source Core (Apache 2.0)

### Engine and Semantics

- RET logic engine.
- Gate and stage evaluation.
- Trust lane enforcement.
- Precheck mode.
- Deterministic evaluation.
- Runpack format.
- Offline verifier.

### Providers and SDK

- Built‑in providers.
- Provider SDKs.
- MCP federation.
- Provider registration schemas.

### Tooling

- MCP server implementation.
- CLI.
- Local registry.
- SQLite run store.
- Contract generator.
- Schema registry.

### Integrations and Examples

- Agent framework adapters.
- AssetCore interface layer.
- CI/CD examples.
- Disclosure examples.

### Specs and Formats

- Wire protocols.
- JSON schemas.
- Runpack spec.
- Manifest format.

---

## 6. Paid Enterprise / Cloud Features

These are **organizational capabilities**, not mathematical primitives.

---

### Governance and Identity

- Org accounts.
- RBAC.
- Namespace policy enforcement.
- Approval routing.
- SSO.
- Human review UIs.

---

### Operations and Scale

- HA deployments.
- Multi‑region.
- Managed storage.
- Backups.
- Disaster recovery.
- Rate limiting.
- Quotas.

---

### Compliance and Audit

- SOC2 artifacts.
- FIPS builds.
- Immutable logs.
- SIEM export.
- Legal‑hold retention.
- Attestation chains.

---

### DG Cloud Services

- Hosted MCP gateway.
- Hosted registry.
- Hosted run store.
- Runpack browser.
- Scenario diff tooling.
- UI dashboards.

---

### Commercial Layer

- SLA tiers.
- Priority support.
- LTS releases.
- Security advisories.
- Enterprise onboarding.

---

## 7. Minimum Enterprise / Cloud Feature Set to Charge

To charge money in Phase‑1/2, DG Cloud must include:

- Multi‑tenant isolation.
- API‑key auth.
- Usage metering.
- Hosted runpacks.
- Hosted registry.
- Audit log access.
- Basic UI.
- Reliability SLO.

SSO and RBAC unlock Phase‑2 pricing.

---

## 8. How AssetCore Fits

DG customers who ask for:

- full world‑state replay,
- cryptographic anchoring,
- simulation provenance,

become AssetCore prospects.

DG is the top of funnel; AssetCore is the cathedral.

---

## 9. Founder‑Efficiency Constraints

DG Cloud must:

- be declarative.
- auto‑provision tenants.
- enforce quotas.
- avoid bespoke installs.
- minimize support load.

Consulting is deferred deliberately.

---

## 10. Summary

- DG launches OSS first.
- DG Cloud monetizes operations.
- Org features unlock higher ACV.
- AssetCore becomes premium tier.
- Founder time is protected by automation.

---

## Appendix: Immediate Next Steps

- Audit codebase for single‑tenant assumptions.
- Add quota enforcement.
- Design tenant isolation model.
- Define runpack storage backend.
- Draft DG Cloud architecture diagram.
- Write pricing page.

