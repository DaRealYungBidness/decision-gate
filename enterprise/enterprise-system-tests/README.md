# Decision Gate Enterprise System Tests

## Overview
Private system tests for enterprise features (tenant isolation, quotas, SSO,
audit exports, and managed-cloud behavior). These tests must mirror production
behavior end-to-end and remain deterministic.

## Quick Start
```bash
# Run the full enterprise system-tests suite (opt-in feature)
cargo test -p enterprise-system-tests --features enterprise-system-tests

# Run with nextest (recommended for CI)
cargo nextest run -p enterprise-system-tests --features enterprise-system-tests

# Run a single test
cargo test -p enterprise-system-tests --features enterprise-system-tests --test tenant_authz \
  -- --exact enterprise_tenant_authz_core_matrix
```

## Test Contract Standards
- No fail-open logic. If a check is required, assert it explicitly.
- No sleeps for correctness; use readiness probes and explicit polling.
- Use production types from `decision-gate-core` and `decision-gate-mcp`.
- Record artifacts for every test (`summary.json`, `summary.md`, `tool_transcript.json`).

## Infrastructure Notes
- Postgres + S3 fixtures use Docker when external endpoints are not provided.
- Set `DECISION_GATE_ENTERPRISE_PG_URL` or `DECISION_GATE_ENTERPRISE_S3_*` to
  target external services.

## Architecture
Enterprise current-state docs (kept in `Docs/architecture/enterprise/` for repo split):
- `Docs/architecture/enterprise/decision_gate_enterprise_system_test_architecture.md`

## Governance and Standards
Follow the repository standards and security posture:

- `Docs/standards/codebase_formatting_standards.md`
- `Docs/standards/codebase_engineering_standards.md`
- `Docs/security/threat_model.md`

If any change affects security posture or trust boundaries, update
`Docs/security/threat_model.md` and the relevant docs in `Docs/architecture/`.

## OSS Boundary
- These tests are private and must not depend on OSS system-tests.
- Keep enterprise tests separate from `system-tests/`.

## Registry and Gaps
- `enterprise/enterprise-system-tests/test_registry.toml` is the authoritative inventory.
- `enterprise/enterprise-system-tests/test_gaps.toml` tracks missing coverage.
- Regenerate coverage docs: `python scripts/coverage_report.py generate`.
