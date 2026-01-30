<!--
Decision Gate Agent Loop Example README
============================================================================
Document: examples/agent-loop
Description: Multi-step condition satisfaction example.
Purpose: Demonstrate staged gate progression across multiple triggers.
Dependencies:
  - ../../decision-gate-core/README.md
============================================================================
-->

# Agent Loop Example

Simulates an agent loop where conditions are satisfied over time.

## Table of Contents

- [Overview](#overview)
- [What It Demonstrates](#what-it-demonstrates)
- [Run](#run)
- [Notes](#notes)
- [References](#references)

## Overview

This example updates in-memory evidence between `scenario_next` calls to show
how a run advances only after all gate requirements are met.

## What It Demonstrates

- Multi-step gate satisfaction across triggers.
- In-memory evidence provider updated between steps.
- Deterministic progression with logical timestamps.

## Run

```bash
cargo run -p decision-gate-example-agent-loop
```

## Notes

- Evidence is modeled as atomic flags updated between steps.
- Intended as a control-plane walkthrough, not an integration test.

## References
