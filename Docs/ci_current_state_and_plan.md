# Decision Gate CI Current State and Plan

## Current State (2026-01-29)

- There is no formal CI configuration checked into the repo (no .github/workflows or similar).
- Local CI-style entrypoints live under scripts/ and cover generation drift checks, Rust tests,
  packaging dry runs, adapter smoke tests, and system-test orchestration.
- System-tests are registry driven. Coverage and infrastructure docs are generated from the registry.
- There are no long-running fuzzing harnesses in place yet; current "fuzz" coverage is deterministic
  system-tests only.

## Existing CI-Capable Scripts

| Script | Purpose | Typical usage | Notes |
| --- | --- | --- | --- |
| `scripts/generate_all.sh` | Contract + SDK generation pipeline | `scripts/generate_all.sh --check` | Verifies generated artifacts match committed outputs. |
| `scripts/verify_all.sh` | Primary local CI gate | `scripts/verify_all.sh` | Runs generation check + `cargo test --workspace --exclude system-tests`; optional system tests, packaging, adapter tests. |
| `scripts/package_dry_run.sh` | Packaging verification | `scripts/package_dry_run.sh --all` | Builds and installs Python + TypeScript SDKs without publishing. |
| `scripts/adapter_tests.sh` | Adapter smoke tests | `scripts/adapter_tests.sh --all` | Spawns a local MCP server (unless endpoint provided) and runs framework examples in a venv. |
| `scripts/test_runner.py` | System-test runner | `python scripts/test_runner.py --priority P0` | Registry-driven, supports parallelism, timeouts, and artifact capture. |
| `scripts/coverage_report.py` | Coverage and infra docs | `python scripts/coverage_report.py generate` | Writes `Docs/testing/decision_gate_test_coverage.md` and `Docs/testing/test_infrastructure_guide.md`. |
| `scripts/gap_tracker.py` | Coverage gap management | `python scripts/gap_tracker.py list` | Lists, closes, or generates task prompts for system-test gaps. |

## Recommended Formal CI Flow

The goal is to wire existing scripts into a formal CI pipeline while keeping the local
entrypoints as the source of truth. Suggested split:

### PR Gate (Fast, Required)

- **Format + lint:** `cargo +nightly fmt --all -- --check` and `cargo clippy --all-targets --all-features -D warnings`.
- **Generation drift:** `scripts/generate_all.sh --check`.
- **Unit tests:** `cargo test --workspace --exclude system-tests`.

### Extended Validation (Main Branch or Nightly)

- **System tests P0:** `python scripts/test_runner.py --priority P0`.
- **System tests P1/P2:** nightly or scheduled runs.
- **Packaging dry run:** `scripts/package_dry_run.sh --all` (requires Python + Node).
- **Adapter tests:** `scripts/adapter_tests.sh --all` (requires external deps; likely nightly).
- **Coverage docs regeneration:** `python scripts/coverage_report.py generate` and ensure no diff.

### Release Pipeline (Future)

- Build and sign artifacts for Rust crates and SDKs.
- Publish to PyPI and npm once the packaging dry runs are green.
- Tag releases only after generator drift checks and system tests pass.

## CI Policy Goals

- Treat warnings as errors in CI (Clippy with `-D warnings`).
- Enforce generator drift checks on every PR.
- Keep system-test artifacts as CI outputs (manifest + per-test logs).
- Align CI behavior with `scripts/verify_all.sh` to keep local and CI gates consistent.

## Gaps to Close

- Add an actual CI config (GitHub Actions, GitLab CI, or similar) that invokes the steps above.
- Add a matrix for Linux + Windows where deterministic output matters.
- Add a scheduled job for P1/P2 system tests and adapter checks.
- Config docs are now generated from the canonical config crate via the contract CLI,
  and drift is enforced in generation checks.
- Decide whether and where fuzzing adds real value (avoid test theater). This needs active evaluation
  of the codebase to identify high-leverage fuzz targets before committing to long-running fuzz jobs.
