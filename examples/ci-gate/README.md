<!--
Decision Gate CI Gate Example README
============================================================================
Document: examples/ci-gate
Description: CI status + approval gating example.
Purpose: Demonstrate evidence predicates for CI/CD checks.
Dependencies:
  - ../../decision-gate-core/README.md
============================================================================
-->

# CI Gate Example

Models a CI gate requiring both a passing status and a minimum number of
approvals before advancing.

## Table of Contents

- [Overview](#overview)
- [What It Demonstrates](#what-it-demonstrates)
- [Run](#run)
- [Notes](#notes)
- [References](#references)

## Overview

This example simulates CI status and approval counts in memory and evaluates
predicates using the ControlPlane.

## What It Demonstrates

- Evidence predicates for CI status and approval count.
- Gate evaluation based on multiple predicates.
- Deterministic disclosures with in-memory dispatch.

## Run

```bash
cargo run -p decision-gate-example-ci-gate
```

## Notes

- Evidence is simulated in memory; no external provider required.
- Useful as a reference for CI/CD gating patterns.

## References

