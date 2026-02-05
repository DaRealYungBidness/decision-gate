<!--
Docs/enterprise/decision_gate_enterprise.md
============================================================================
Document: Decision Gate Enterprise
Description: Enterprise feature overview and positioning for Decision Gate OSS users.
Purpose: Explain enterprise capabilities, deployment models, and contact paths.
Dependencies: None (standalone).
============================================================================
-->

# Decision Gate Enterprise

This document describes Decision Gate Enterprise (DG-E) for teams who want
self-hosted control or who embed Decision Gate into their own platforms. It is
intentionally honest and aligned with what exists today.

---

## Who DG-E Is For

DG-E is built for two customer types:

1. **Self-hosted enterprises**
   - You run Decision Gate in your own cloud or data center.
   - You control identity, storage, policies, and operations.

2. **Agentic platforms and framework providers**
   - You embed Decision Gate into a larger product or workflow system.
   - You operate DG-E as part of your own service and control the customer
     experience, billing model, and lifecycle.

---

## What DG-E Is Not

- **No public managed cloud service yet.** We do not operate a generally
  available DG-Cloud today. For select customers, a limited managed cloud
  offering may be available while we continue hardening it.
- **No promise of 24/7 operations.** You operate your deployment.
- **No hidden coupling.** DG-E integrates with standard enterprise primitives
  (OIDC, API keys, Postgres, S3, Prometheus-style metrics) without requiring a
  separate hosted platform.

If you need a managed offering, we can discuss it during early access.

---

## Enterprise Capabilities

DG-E adds enterprise-grade control-plane wiring while keeping OSS semantics
unchanged. Everything below is implemented in the enterprise repositories.

### Authentication and Authorization

- **OIDC** token validation (JWT and opaque tokens).
- **API key** issuance, rotation, revocation, and listing (tenant-scoped).
- **mTLS subject** authentication for HTTP deployments (also available in OSS).
- **Role-based authorization** with namespace context.

### Tenant and Namespace Administration

- Tenant lifecycle scaffolding and namespace membership tracking.
- Namespace authority enforcement (existence, lifecycle state, tenant matching).

### Docs Governance and Tool Visibility

- Tenant-scoped docs catalogs with RBAC and policy enforcement.
- Tool visibility policies per tenant.
- Optional rate limits, encryption, and audit trails for docs access.

### Audit and Compliance

- **Hash-chained JSONL audit log** with retention policies.
- **Audit export** with deterministic manifests.

### Usage and Quotas

- **Quota enforcement** (check + consume) for all MCP tool usage.
- **Usage ledger** for analytics (append-only, idempotent).
- **Usage export** to JSONL with SHA-256 manifest (billing enablement).

### Storage and Runpacks

- **Postgres run state and schema registry** with deterministic serialization.
- **Runpack storage** backends:
  - Filesystem (strict path validation, no symlinks).
  - S3-compatible object storage with integrity checks and optional object lock.

### Admin Surface

- **Admin API + lightweight HTML console** for tenants, API keys, runs, docs
  governance, and runpack download workflows. This is a minimal, deterministic
  UI, not a full GUI app.

### Telemetry

- **Prometheus-compatible metrics** adapter with safe, stable labels.
- Correlation IDs are not emitted as metric labels.

---

## Deployment Model (Self-Hosted by Default)

DG-E is designed for self-hosted control and embedded platform deployments.
You provide:

- Infrastructure (compute, storage, network, TLS).
- Identity provider (OIDC) and/or API key strategy.
- Operational runbooks and incident response.

We provide:

- Software, documentation, and best-effort integration help.
- Clear boundaries so you retain control of your environment.

Managed cloud is invite-only and limited to select customers while we harden it.

---

## OSS Boundary and Semantics

DG-E is an extension of Decision Gate OSS, not a fork of its core semantics:

- **OSS behavior remains deterministic and auditable.**
- **Enterprise features do not alter core evaluation logic.**
- **Enterprise-only dependencies stay outside OSS crates.**

---

## Managed Cloud (Select Customers)

We are actively developing a managed cloud version of Decision Gate. For select
customers, we may offer a managed cloud service while it is still being
hardened. This is not generally available. If a managed option is important to
your team, reach out and we can discuss fit, timeline, and prerequisites.

---

## Contact and Licensing

If you want DG-E, reach out directly:

- Email: `license@assetcore.io`
- Managed cloud inquiries: `license@assetcore.io`
- Decision Gate Enterprise page: `assetcore.io/decision-gate`
- Direct contact: `https://www.linkedin.com/in/michael-campbell-73159b5a/`

We can discuss self-hosted licensing and embedded platform partnerships.
