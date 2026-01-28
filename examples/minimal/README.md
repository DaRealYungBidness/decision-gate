<!--
Decision Gate Minimal Example README
============================================================================
Document: examples/minimal
Description: Minimal end-to-end scenario lifecycle example.
Purpose: Demonstrate ControlPlane usage with in-memory adapters.
Dependencies:
  - ../../decision-gate-core/README.md
============================================================================
-->

# Minimal Example

Minimal Decision Gate scenario run using in-memory adapters.

## Table of Contents

- [Overview](#overview)
- [What It Demonstrates](#what-it-demonstrates)
- [Run](#run)
- [Notes](#notes)
- [References](#references)

## Overview

This example constructs a small `ScenarioSpec`, starts a run, advances it with
`scenario_next`, and checks status. It is intended for quick verification and
local experimentation.

## What It Demonstrates

- In-memory `EvidenceProvider`, `Dispatcher`, and `RunStateStore`.
- Basic gate evaluation and stage advancement.
- Entry packet dispatch on stage transitions.

## Run

```bash
cargo run -p decision-gate-example-minimal
```

## Notes

- All evidence is in-memory; no external providers.
- The example is deterministic and uses logical timestamps.

## References

