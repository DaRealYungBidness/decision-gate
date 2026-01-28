<!--
_migration_backup/enterprise/decision-gate-enterprise/README.md
============================================================================
Document: Decision Gate Enterprise (Archive)
Description: Archived snapshot of enterprise-only extensions.
Purpose: Preserve migration context without contaminating OSS docs.
============================================================================
-->

# Decision Gate Enterprise (Archive)

This directory is an archived snapshot used during repo migration. It is not
maintained in the OSS repository.

## Table of Contents

- [Status](#status)
- [OSS Boundary](#oss-boundary)
- [References](#references)

## Status

- Archived snapshot only; content may be stale.
- Authoritative enterprise code and docs live in the private Asset Core repo.

## OSS Boundary

- OSS crates must not depend on enterprise crates.
- Enterprise behavior must extend via explicit seams, not forks.

## References

