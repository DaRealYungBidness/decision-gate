# Scripts

This directory hosts repo tooling, grouped by workflow. Common entry points live
in `scripts/bootstrap/` and `scripts/ci/`.

## Layout

- `scripts/bootstrap/`: onboarding and quickstart flows (bash/PowerShell).
- `scripts/ci/`: CI-style gates, packaging checks, SBOMs, and release gating.
- `scripts/docs/`: documentation verification utilities.
- `scripts/system_tests/`: system-test runner, coverage report, gap tracker.
- `scripts/adapters/`: adapter conformance, roundtrip, and type checks.
- `scripts/agentic/`: agentic flow harness bootstrap and runner.
- `scripts/container/`: container image build helpers.

## Common Flows

- Quick smoke test: `scripts/bootstrap/quickstart.sh` or `scripts/bootstrap/quickstart.ps1`.
- Full local gate: `scripts/ci/verify_all.sh`.
- Generation drift check: `scripts/ci/generate_all.sh --check`.
- Python format check: `scripts/ci/verify_all.sh --python-format`.
- Adapter tests: `scripts/adapters/adapter_tests.sh --all`.
- System tests (P0): `python scripts/system_tests/test_runner.py --priority P0`.
- Docs verification (fast): `python scripts/docs/docs_verify.py --run --level=fast`.
