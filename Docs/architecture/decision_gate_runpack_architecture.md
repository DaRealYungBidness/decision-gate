<!--
Docs/architecture/decision_gate_runpack_architecture.md
============================================================================
Document: Decision Gate Runpack Architecture
Description: Current-state reference for runpack manifest structure, export
             pipeline, artifact integrity, and verification flow.
Purpose: Provide an implementation-grade map of how runpacks are built and
         verified in DG.
Dependencies:
  - decision-gate-core/src/core/runpack.rs
  - decision-gate-core/src/runtime/runpack.rs
  - decision-gate-mcp/src/tools.rs
  - decision-gate-mcp/src/runpack.rs
  - decision-gate-mcp/src/runpack_object_store.rs
  - decision-gate-config/src/config.rs
============================================================================
Last Updated: 2026-02-04 (UTC)
============================================================================
-->

# Decision Gate Runpack Architecture

> **Audience:** Engineers implementing runpack export/verification and
> filesystem/object-store artifact handling.

---

## Table of Contents

1. [Executive Overview](#executive-overview)
2. [Runpack Manifest Structure](#runpack-manifest-structure)
3. [Runpack Export Pipeline](#runpack-export-pipeline)
4. [Artifact Integrity Model](#artifact-integrity-model)
5. [Runpack Verification Flow](#runpack-verification-flow)
6. [Filesystem Sink/Reader Safety](#filesystem-sinkreader-safety)
7. [Object Store Sink/Reader Safety](#object-store-sinkreader-safety)
8. [File-by-File Cross Reference](#file-by-file-cross-reference)

---

## Executive Overview

Runpacks are deterministic bundles of scenario specs, control-plane logs, and
integrity metadata. The builder writes canonical JSON artifacts and computes
hashes for every file, plus a root hash over the file hash list. Verification
replays integrity checks, validates decision log uniqueness, and optionally
validates evidence anchors when an anchor policy is present.
[F:decision-gate-core/src/runtime/runpack.rs L83-L374](decision-gate-core/src/runtime/runpack.rs#L83-L374)

Evidence anchors are stored verbatim in the runpack logs. For file-based
anchors (`file_path_rooted`), anchors include a stable `root_id` plus a
POSIX-style relative path to ensure hashes are consistent across operating
systems.

Runpack exports select a sink in this order: an optional `RunpackStorage`
backend (exports to a temp directory and delegates storage), an OSS object-store
backend when configured, or a filesystem export requiring `output_dir`.
[F:decision-gate-mcp/src/runpack_storage.rs L31-L67](decision-gate-mcp/src/runpack_storage.rs#L31-L67) [F:decision-gate-mcp/src/tools.rs L2342-L2480](decision-gate-mcp/src/tools.rs#L2342-L2480)

---

## Runpack Manifest Structure

The manifest is the canonical index for runpack artifacts. Key fields include:

- Scenario/run identifiers and spec hash
- Hash algorithm and verifier mode
- Optional anchor policy and security context
- File hash list and root hash
- Artifact index entries

[F:decision-gate-core/src/core/runpack.rs L57-L116](decision-gate-core/src/core/runpack.rs#L57-L116)

Manifest versioning is explicit. The verifier currently supports `v1` and fails
closed on unknown `manifest_version` values to preserve backward compatibility
guarantees as new versions are introduced.
[F:decision-gate-core/src/runtime/runpack.rs L60-L374](decision-gate-core/src/runtime/runpack.rs#L60-L374)

Security context metadata captures dev-permissive and namespace authority
posture when provided by the MCP server.
[F:decision-gate-core/src/core/runpack.rs L94-L104](decision-gate-core/src/core/runpack.rs#L94-L104)

---

## Runpack Export Pipeline

Runpack export is initiated via the MCP tool `runpack_export`:

1. Tool router loads run state from the configured store.
2. A `RunpackBuilder` is created with the active anchor policy and optional
   security context metadata.
3. Artifacts are written via filesystem or object-store sinks depending on
   configuration.
4. Optional in-line verification can be requested during export.

[F:decision-gate-mcp/src/tools.rs L2342-L2480](decision-gate-mcp/src/tools.rs#L2342-L2480)
[F:decision-gate-mcp/src/runpack_object_store.rs L94-L260](decision-gate-mcp/src/runpack_object_store.rs#L94-L260)

When a `RunpackStorage` override is configured, MCP builds the runpack on disk
and delegates storage to the backend. Otherwise, object-store exports write
per-artifact objects directly; filesystem exports require `output_dir`.

The builder writes deterministic JSON artifacts for:

- Scenario spec
- Trigger log
- Gate evaluation log
- Decision log
- Packet log
- Submission log
- Tool call log

[F:decision-gate-core/src/runtime/runpack.rs L130-L214](decision-gate-core/src/runtime/runpack.rs#L130-L214)

---

## Artifact Integrity Model

For each artifact, the builder:

- Serializes using JCS
- Rejects non-finite floats during canonicalization (NaN/±Infinity)
- Computes a file hash
- Adds a file hash entry and artifact record

A root hash is computed over the canonical list of file hashes to guard against
artifact reordering or omission.
[F:decision-gate-core/src/runtime/runpack.rs L433-L480](decision-gate-core/src/runtime/runpack.rs#L433-L480)

---

## Runpack Verification Flow

Verification validates integrity and structural invariants:

- All artifact hashes match the manifest.
- The root hash matches the file-hash list.
- Decision log contains no duplicate decisions per trigger id.
- Anchor policy validation runs when present in the manifest.

[F:decision-gate-core/src/runtime/runpack.rs L314-L567](decision-gate-core/src/runtime/runpack.rs#L314-L567)

The `runpack_verify` tool parses the manifest, reads artifacts from disk, and
returns a structured verification report.
[F:decision-gate-mcp/src/tools.rs L2497-L2513](decision-gate-mcp/src/tools.rs#L2497-L2513)

**Note on `include_verification`:** `runpack_export` generates the verification
report before the report artifact is added to the manifest, so
`report.checked_files` excludes `verifier_report.json`. `runpack_verify`
validates all manifest file hashes (including the report) and may report
`checked_files` as +1 compared to the export-time report.

---

## Filesystem Sink/Reader Safety

Filesystem artifacts are handled by hardened sink/reader implementations:

- Paths must be relative and cannot escape the runpack root.
- Path component and total path length limits are enforced.
- Reads enforce max byte limits and fail closed on violations.

[F:decision-gate-mcp/src/runpack.rs L43-L217](decision-gate-mcp/src/runpack.rs#L43-L217)

---

## Object Store Sink/Reader Safety

Object-store runpack adapters enforce the same safety guarantees as filesystem
storage:

- Keys are derived from tenant/namespace/scenario/run/spec_hash.
- Path segments are validated and length-bounded.
- Artifacts are capped at `MAX_RUNPACK_ARTIFACT_BYTES`.
- Reads fail closed when size limits are exceeded.

[F:decision-gate-mcp/src/runpack_object_store.rs L94-L260](decision-gate-mcp/src/runpack_object_store.rs#L94-L260)

---

## File-by-File Cross Reference

| Area | File | Notes |
| --- | --- | --- |
| Manifest schema | `decision-gate-core/src/core/runpack.rs` | Manifest fields, integrity, security context. |
| Builder + verifier | `decision-gate-core/src/runtime/runpack.rs` | Artifact writing and verification logic. |
| Tool integration | `decision-gate-mcp/src/tools.rs` | runpack_export/runpack_verify flows. |
| Filesystem IO | `decision-gate-mcp/src/runpack.rs` | Safe artifact sink/reader with path validation. |
| Object store IO | `decision-gate-mcp/src/runpack_object_store.rs` | Object-store sink/reader for runpack artifacts. |
