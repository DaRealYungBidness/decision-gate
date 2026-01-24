<!--
Decision Gate CI Gate Example README
============================================================================
Document: examples/ci-gate
Description: CI status + approval gating example.
============================================================================
-->

# CI Gate Example

## Overview
Models a CI gate requiring both a passing CI status and a minimum number of
approvals before advancing.

## Run
```bash
cargo run -p decision-gate-example-ci-gate
```

## Notes
- Uses in-memory signals to simulate CI status and approvals.
- Demonstrates RequireGroup semantics.
