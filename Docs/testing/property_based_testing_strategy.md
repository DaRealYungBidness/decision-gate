# Property-Based Testing Strategy

## Purpose
Property-based tests complement unit and system tests by exercising broad input
spaces. They are used to detect panics, invariant violations, and unexpected
behavior across comparators and providers.

## Scope
- decision-gate-core: comparator invariants and panic-free evaluation
- decision-gate-providers: URL parsing and JSONPath/path handling

## Tooling
- proptest (default cases: 256)
- Environment controls:
  - PROPTEST_CASES (increase for CI or hardening runs)
  - PROPTEST_SEED (reproducible failures)

## Design Principles
- Keep generators bounded (depth and size) to avoid performance regressions.
- Ensure all failures are actionable (clear assertions, minimal shrink).
- Prefer deterministic outcomes for numeric comparisons.
- Validate fail-closed semantics (errors or Unknown, never silent pass).

## Execution
- Run all property tests:
  - cargo test proptest_
- Increase coverage for hardening:
  - PROPTEST_CASES=5000 cargo test proptest_

## Maintenance
- When adding a new comparator or provider check, add a property test that:
  - Exercises the new input space
  - Confirms no panics and correct invariant behavior
- Record any discovered seeds in regression tests.
