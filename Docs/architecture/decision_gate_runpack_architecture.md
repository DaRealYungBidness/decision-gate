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
============================================================================
Last Updated: 2026-01-26 (UTC)
============================================================================
-->

# Decision Gate Runpack Architecture

> **Audience:** Engineers implementing runpack export/verification and
> filesystem artifact handling.

---

## Table of Contents

1. [Executive Overview](#executive-overview)
2. [Runpack Manifest Structure](#runpack-manifest-structure)
3. [Runpack Export Pipeline](#runpack-export-pipeline)
4. [Artifact Integrity Model](#artifact-integrity-model)
5. [Runpack Verification Flow](#runpack-verification-flow)
6. [Filesystem Sink/Reader Safety](#filesystem-sinkreader-safety)
7. [File-by-File Cross Reference](#file-by-file-cross-reference)

---

## Executive Overview

Runpacks are deterministic bundles of scenario specs, control-plane logs, and
integrity metadata. The builder writes canonical JSON artifacts and computes
hashes for every file, plus a root hash over the file hash list. Verification
replays integrity checks, validates decision log uniqueness, and optionally
validates evidence anchors when an anchor policy is present.
[F:decision-gate-core/src/runtime/runpack.rs L80-L365]

---

## Runpack Manifest Structure

The manifest is the canonical index for runpack artifacts. Key fields include:

- Scenario/run identifiers and spec hash
- Hash algorithm and verifier mode
- Optional anchor policy and security context
- File hash list and root hash
- Artifact index entries

[F:decision-gate-core/src/core/runpack.rs L49-L127]

Security context metadata captures dev-permissive and namespace authority
posture when provided by the MCP server.
[F:decision-gate-core/src/core/runpack.rs L82-L92]

---

## Runpack Export Pipeline

Runpack export is initiated via the MCP tool `runpack_export`:

1. Tool router loads run state from the configured store.
2. A `RunpackBuilder` is created with the active anchor policy and optional
   security context metadata.
3. Artifacts are written via a filesystem sink.
4. Optional in-line verification can be requested during export.

[F:decision-gate-mcp/src/tools.rs L888-L929]

The builder writes deterministic JSON artifacts for:

- Scenario spec
- Trigger log
- Gate evaluation log
- Decision log
- Packet log
- Submission log
- Tool call log

[F:decision-gate-core/src/runtime/runpack.rs L57-L208]

---

## Artifact Integrity Model

For each artifact, the builder:

- Serializes using JCS
- Computes a file hash
- Adds a file hash entry and artifact record

A root hash is computed over the canonical list of file hashes to guard against
artifact reordering or omission.
[F:decision-gate-core/src/runtime/runpack.rs L415-L463]

---

## Runpack Verification Flow

Verification validates integrity and structural invariants:

- All artifact hashes match the manifest.
- The root hash matches the file-hash list.
- Decision log contains no duplicate decisions per trigger id.
- Anchor policy validation runs when present in the manifest.

[F:decision-gate-core/src/runtime/runpack.rs L320-L549]

The `runpack_verify` tool parses the manifest, reads artifacts from disk, and
returns a structured verification report.
[F:decision-gate-mcp/src/tools.rs L931-L1011]

---

## Filesystem Sink/Reader Safety

Filesystem artifacts are handled by hardened sink/reader implementations:

- Paths must be relative and cannot escape the runpack root.
- Path component and total path length limits are enforced.
- Reads enforce max byte limits and fail closed on violations.

[F:decision-gate-mcp/src/runpack.rs L43-L217]

---

## File-by-File Cross Reference

| Area | File | Notes |
| --- | --- | --- |
| Manifest schema | `decision-gate-core/src/core/runpack.rs` | Manifest fields, integrity, security context. |
| Builder + verifier | `decision-gate-core/src/runtime/runpack.rs` | Artifact writing and verification logic. |
| Tool integration | `decision-gate-mcp/src/tools.rs` | runpack_export/runpack_verify flows. |
| Filesystem IO | `decision-gate-mcp/src/runpack.rs` | Safe artifact sink/reader with path validation. |

