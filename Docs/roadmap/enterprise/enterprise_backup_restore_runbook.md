<!--
Docs/roadmap/enterprise/enterprise_backup_restore_runbook.md
============================================================================
Document: Decision Gate Enterprise Backup + Restore Runbook
Description: Minimal, actionable backup/restore guidance for Phase-1 storage.
Purpose: Provide operators with hardened defaults for Postgres + runpack S3.
Dependencies:
  - enterprise/decision-gate-store-enterprise/src/postgres_store.rs
  - enterprise/decision-gate-store-enterprise/src/s3_runpack_store.rs
============================================================================
Last Updated: 2026-01-27 (UTC)
============================================================================
-->

# Decision Gate Enterprise Backup + Restore Runbook

## Scope
Phase-1 managed deployments using:
- Postgres for run state + schema registry.
- S3-compatible object storage for runpacks.

## Postgres Backups (Run State + Schema Registry)
Use both periodic base backups and WAL archiving for point-in-time recovery.

### Baseline backup (daily)
1) Ensure the DB user has `pg_read_all_data` and `pg_read_all_settings`.
2) Run:
   - `pg_dump --format=custom --file=dg-backup.dump --no-owner --no-privileges <db>`
3) Store the dump in encrypted object storage (S3 SSE-KMS).

### WAL archiving (continuous)
1) Enable WAL archiving on the Postgres instance.
2) Ship WAL segments to a separate bucket with immutable retention.
3) Validate that WAL upload latency stays within RPO.

### Restore (point-in-time)
1) Restore the latest base backup to a new instance.
2) Replay WAL to the target timestamp.
3) Run integrity checks:
   - Sample run states and verify stored hash matches payload.
   - Sample schema records and verify schema hash matches payload.

## S3 Runpack Backups
Runpacks are stored as tar archives with SHA-256 metadata.

### Storage hardening
- Enable versioning on the bucket.
- Enable object lock (WORM) for compliance tiers.
- Enforce SSE-KMS and restrict IAM principals to write-only from the app.

### Restore
1) Locate the runpack object by tenant/namespace/run id prefix.
2) Restore the object version (if needed).
3) The runpack store verifies SHA-256 metadata before extraction.

## Validation Checklist
- Postgres restore passes schema integrity verification (hash checks).
- Runpack retrieval verifies SHA-256 metadata.
- Audit logs are retained separately (hash-chained logs are append-only).

## Incident Readiness
- Document the last successful backup time and WAL lag.
- Keep a tested runbook for restoring to a staging environment monthly.
