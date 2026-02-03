# Decision Gate CI Current State and Plan

## Current State (2026-02-03)

- There is one CI workflow in the repo: `.github/workflows/golden_runpack_cross_os.yml` runs a
  cross-OS golden runpack test on Linux and Windows for pushes and PRs.
- Local CI-style entrypoints live under `scripts/` and cover generation drift checks, Rust tests,
  packaging dry runs, adapter smoke tests, and system-test orchestration.
- System-tests are registry driven. Coverage and infrastructure docs are generated from the registry.
- There are no scheduled (nightly) CI jobs yet.
- `cargo-deny` is configured (`deny.toml`) but is not wired into CI yet.
- There are no long-running fuzzing harnesses in place yet; current "fuzz" coverage is deterministic
  system-tests only.

## Why This Plan Is "World-Class"

World-class here means: an outside reviewer can see intentional, repeatable, evidence-driven gates
that favor correctness and supply-chain hygiene over convenience.

- **Deterministic and auditable**: generator drift checks and pinned toolchains reduce surprises.
- **Strict quality gates**: warnings are treated as errors; linting is mandatory.
- **System confidence**: required system tests on `main` prove integration health.
- **Supply-chain discipline**: `cargo-deny` catches license/security/source problems early.
- **Deliberate releases**: tags trigger publishing; `main` is continuous integration, not auto-release.

## Decisions (2026-02-03)

- **PR gate (required)** runs: fmt, clippy, generator drift check, unit tests, and `cargo-deny`.
- **Main gate (required)** runs **P0 + P1** system tests.
- **P2 system tests** are **manual** (workflow dispatch); no scheduled/nightly jobs yet.
- **Cross-OS golden runpack** remains a separate, required workflow.
- **Releases are tag-driven** only (e.g., `v0.1.0`), not on every merge to `main`.
- **Docker**: build on PR; build + push on release tags.
- **Multi-arch images**: build `linux/amd64` and `linux/arm64`; smoke-test `amd64` only and document
  the arm64 testing limitation.

## Existing CI-Capable Scripts

| Script                       | Purpose                            | Typical usage                                 | Notes                                                                                                                     |
| ---------------------------- | ---------------------------------- | --------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------- |
| `scripts/generate_all.sh`    | Contract + SDK generation pipeline | `scripts/generate_all.sh --check`             | Verifies generated artifacts match committed outputs.                                                                     |
| `scripts/verify_all.sh`      | Primary local CI gate              | `scripts/verify_all.sh`                       | Runs generation check + `cargo test --workspace --exclude system-tests`; optional system tests, packaging, adapter tests. |
| `scripts/package_dry_run.sh` | Packaging verification             | `scripts/package_dry_run.sh --all`            | Builds and installs Python + TypeScript SDKs without publishing.                                                          |
| `scripts/adapter_tests.sh`   | Adapter smoke tests                | `scripts/adapter_tests.sh --all`              | Spawns a local MCP server (unless endpoint provided) and runs framework examples in a venv.                               |
| `scripts/test_runner.py`     | System-test runner                 | `python scripts/test_runner.py --priority P0` | Registry-driven, supports parallelism, timeouts, and artifact capture.                                                    |
| `scripts/coverage_report.py` | Coverage and infra docs            | `python scripts/coverage_report.py generate`  | Writes `Docs/testing/decision_gate_test_coverage.md` and `Docs/testing/test_infrastructure_guide.md`.                     |
| `scripts/gap_tracker.py`     | Coverage gap management            | `python scripts/gap_tracker.py list`          | Lists, closes, or generates task prompts for system-test gaps.                                                            |

## Formal CI Flow (Planned)

### PR Gate (Fast, Required)

- **Format:** `cargo +nightly fmt --all -- --check`
- **Lint:** `cargo clippy --all-targets --all-features -D warnings`
- **Supply-chain policy:** `cargo deny check`
- **Generation drift:** `scripts/generate_all.sh --check`
- **Unit tests:** `cargo test --workspace --exclude system-tests`
- **Cross-OS gate:** keep `golden_runpack_cross_os` as a dedicated required workflow

### Main Gate (Required)

- **System tests P0:** `python scripts/test_runner.py --priority P0`
- **System tests P1:** `python scripts/test_runner.py --priority P1`

### Manual Heavy Gate (On Demand)

- **System tests P2:** `python scripts/test_runner.py --priority P2`

### Release Pipeline (Tag-Driven)

- Triggered by pushing a version tag (e.g., `v0.1.0`).
- Re-run main-level checks or require the last main run to be green.
- Build and publish artifacts (Rust crates, SDKs) once the packaging dry-runs are green.
- Build and push Docker images (multi-arch).

## Docker + Multi-Arch Guidance

- **PRs:** build only (no push) to validate Dockerfile integrity.
- **Release tags:** build + push with buildx for `linux/amd64` and `linux/arm64`.
- **Smoke tests:** run on `amd64` only; explicitly document that arm64 is built in CI but not fully
  integration-tested yet.

## Cross-Architecture Reality

Local development currently targets Windows. CI provides Linux/Windows runners, and buildx handles
arm64 builds without local ARM hardware. The repo should clearly document architecture guarantees:

- **Tested:** `amd64` (Linux/Windows), `golden_runpack_cross_os` parity tests
- **Built but not fully tested:** `arm64` (CI buildx)

## CI Policy Goals

- Treat warnings as errors in CI (Clippy with `-D warnings`).
- Enforce generator drift checks on every PR.
- Require supply-chain policy checks (`cargo-deny`).
- Require system-test P0 + P1 on `main`.
- Keep system-test artifacts as CI outputs (manifest + per-test logs).
- Align CI behavior with `scripts/verify_all.sh` to keep local and CI gates consistent.

## Optional Fast Linkers (Local Opt-In)

Rust build times can improve with faster linkers (clang + lld on Linux/macOS, or lld-link on
Windows), but OSS repos should avoid forcing a toolchain download on contributors. Recommended
approach is **opt-in** via local configuration:

- **Local only:** Document optional settings in developer docs; do not require them in CI.
- **Common patterns:** `.cargo/config.toml` or environment variables such as
  `RUSTFLAGS="-C linker=clang -C link-arg=-fuse-ld=lld"` on Linux/macOS, or
  `RUSTFLAGS="-C linker=lld-link"` on Windows.
- **Graceful fallback:** Builds should succeed without these settings; CI should not depend on them.

## Implementation Map (Planned)

- Keep: `.github/workflows/golden_runpack_cross_os.yml` (cross-OS required check)
- Add: `.github/workflows/ci_pr.yml` (PR gate)
- Add: `.github/workflows/ci_main.yml` (main gate: P0 + P1)
- Add: `.github/workflows/ci_manual.yml` (manual P2 runs)
- Add: `.github/workflows/release.yml` (tag-driven releases)

## Deferred Work (Explicit)

- Scheduled/nightly CI runs for P2, adapter tests, and packaging checks.
- Extended system-test coverage and additional adapters in CI.
- Fuzzing: only add once high-leverage fuzz targets are identified.
- SBOM/provenance enhancements (e.g., attestations, signed releases).
