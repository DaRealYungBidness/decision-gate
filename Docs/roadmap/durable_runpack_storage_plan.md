<!--
Docs/roadmap/durable_runpack_storage_plan.md
============================================================================
Document: Decision Gate Durable Runpack Storage Plan
Description: Pre-implementation plan for object-store and WORM runpack storage.
Purpose: Define scope, standards, tests, and open questions before execution.
Dependencies:
  - Docs/standards/codebase_engineering_standards.md
  - Docs/standards/codebase_formatting_standards.md
  - Docs/security/threat_model.md
  - Docs/architecture/decision_gate_runpack_architecture.md
============================================================================
-->

# Decision Gate Durable Runpack Storage Plan

## Overview
This document is the pre-implementation plan for durable runpack storage.
It defines scope, why we are doing the work, standards to meet, and the
test strategy for both OSS and enterprise layers.

## Why This Exists
Decision Gate runpacks are audit bundles. File-backed storage is sufficient
for local development, but production environments (cloud, containerized,
ephemeral disks) require durable object storage and optional WORM
immutability/compliance controls. The system must remain deterministic,
auditable, and fail-closed under adversarial conditions.

## Standards (Authoritative)
We must build to hyperscaler and DoD-ready expectations, assuming
nation-state adversaries. The following standards govern all work:

1. **NASA-Grade Engineering Standards**:
   - `Docs/standards/codebase_engineering_standards.md`
   - Zero trust, fail-closed behavior, typed errors, no panics/unwraps,
     deterministic replay invariants, and explicit threat model alignment.
2. **Formatting & Documentation Standards**:
   - `Docs/standards/codebase_formatting_standards.md`
   - Required file headers, overview blocks, sectioning, and cross-references.
3. **OSS vs Enterprise Boundary (Authoritative)**:
   - `AGENTS.md` in this repo
   - No enterprise deps in OSS crates; enterprise code under `enterprise/`;
     seams, not forks; OSS stays deterministic and auditable.
4. **Threat Model Alignment**:
   - `Docs/security/threat_model.md` must be updated when new storage
     surfaces or trust boundaries are introduced.

## Scope

### OSS Scope (Open-Core Base)
Objective: Provide a production-grade object-store adapter that preserves
determinism, strong validation, and offline verification.

**OSS deliverables**:
- Object-store `ArtifactSink` and `ArtifactReader` implementations.
- Strict path/key derivation (no caller-controlled paths).
- Bounded IO: size caps, streaming reads/writes, memory limits.
- Fail-closed verification and typed errors.
- Optional `storage_uri` return on runpack export.
- Config surface for selecting storage backend.
- Documentation updates to runpack architecture and threat model.

### Enterprise Scope (Compliance / WORM)
Objective: Provide WORM, immutability, retention, and compliance controls
without changing core semantics or OSS determinism.

**Enterprise deliverables**:
- WORM / object-lock support (retention, legal hold) via enterprise S3 config
  (`runpacks.s3.object_lock`).
- Compliance metadata capture and attestation.
- Multi-region durability or replication policy integration.
- Enterprise-only system tests under `enterprise/`.

## World-Class Decisions (Locked)
These are the agreed implementation decisions for OSS and enterprise layering.

1. **Provider-agnostic interface, S3-first implementation**
   - Define an OSS, provider-agnostic object-store abstraction.
   - Implement S3-compatible storage as the first concrete provider.
2. **Single bucket + strict prefix (default)**
   - Default to a single bucket with deterministic per-tenant/namespace prefixes.
   - Per-tenant bucket mapping is deferred to enterprise policy routing.
3. **Manifest-last completion**
   - Runpack exports are incomplete until the manifest is written.
   - No additional commit marker in OSS; enterprise may add if needed.
4. **Deterministic key derivation**
   - Object keys are derived from tenant, namespace, scenario, run_id, and spec_hash.
   - Caller-provided keys are rejected.
5. **Bounded IO + hard per-artifact limits**
   - Enforce `MAX_RUNPACK_ARTIFACT_BYTES` for every artifact.
   - Fail closed on size violations.
6. **Storage URI semantics**
   - `storage_uri` is informational, scheme-based (e.g., `s3://`), and never
     authoritative for verification. The manifest remains canonical.
7. **Enterprise-only compliance**
   - WORM, retention, legal hold, and compliance attestations are enterprise-only.
   - OSS remains deterministic and auditable without compliance promises.

## Architecture Plan (High-Level)

### 1) Storage Backend Abstraction
Reuse existing `ArtifactSink`/`ArtifactReader` traits from
`decision-gate-core/src/interfaces/mod.rs`. Implement:
- `FileArtifactSink`/`FileArtifactReader` (already present).
- `ObjectStoreArtifactSink`/`ObjectStoreArtifactReader` (new in OSS).
- Enterprise WORM is enforced in the S3 runpack store via Object Lock config.

### 2) Key / Path Derivation
All object-store keys are derived and validated. No caller-provided keys.
Canonical prefix:
`tenant/{tenant_id}/namespace/{namespace_id}/scenario/{scenario_id}/run/{run_id}/{spec_hash}/...`

### 3) Atomic Finalization
Write all artifacts first, manifest last. Optional commit marker.
If manifest missing, runpack is treated as incomplete.

### 4) Integrity Guarantees
All artifacts are hashed. Manifest hash includes every file hash. Reads must
verify hashes; mismatches fail closed.

### 5) Security Posture
Treat storage as untrusted. Always verify. No implicit trust in the object
store or its metadata. Validate sizes, keys, and MIME constraints.

## Test Plan (Broad Strokes)

### OSS Unit Tests
- Key derivation correctness and invariants.
- Path validation: absolute paths, traversal attempts, overly long components.
- Read/write size caps and bounded memory usage.
- Manifest ordering and deterministic hash outputs.
- Fail-closed error paths (missing artifacts, hash mismatch).

### OSS System Tests
- End-to-end `runpack_export` → `runpack_verify` with object store adapter.
- Corruption test: alter a stored artifact, ensure verify fails.
- Partial runpack test: missing manifest or missing artifact fails.

### Enterprise Unit Tests
- WORM policy enforcement: retention and legal hold logic.
- Compliance metadata capture correctness.
- Deny deletion or overwrite under lock.

### Enterprise System Tests
- WORM export/verify flow in enterprise test harness.
- Retention policy enforcement over time window.
- Compliance report generation and audit log coverage.

## Documentation Updates (Planned)
- `Docs/architecture/decision_gate_runpack_architecture.md`:
  add object-store and WORM flows, finalize semantics.
- `Docs/security/threat_model.md`:
  include object-store as an untrusted persistence boundary.
- `Docs/generated/decision-gate/tooling.md`:
  confirm `storage_uri` semantics and verify flow wording.

## Open Questions (Resolve Before Implementation)
None. World-class decisions are locked in the section above.

## Execution Phases
1. **Phase 1 (OSS adapter)**: object store sink/reader, config, tests, docs.
2. **Phase 2 (Enterprise WORM)**: enterprise-only adapter + tests.
3. **Phase 3 (Operational hardening)**: metrics, audit logs, integration guidance.

## Success Criteria
- Object-store runpacks verify offline with deterministic outcomes.
- No OSS crate depends on enterprise code.
- Tests cover adversarial cases and fail closed.
- Threat model updated with new storage boundary.
- Documentation reflects the new storage flows and constraints.
