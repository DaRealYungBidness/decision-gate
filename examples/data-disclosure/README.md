<!--
Decision Gate Data Disclosure Example README
============================================================================
Document: examples/data-disclosure
Description: Disclosure stage with packet emission after approval.
Purpose: Demonstrate packet dispatch and disclosure policy flow.
Dependencies:
  - ../../decision-gate-core/README.md
  - ../../decision-gate-broker/README.md
============================================================================
-->

# Data Disclosure Example

Shows a gate-controlled disclosure stage that emits packet payloads after
approval.

## Table of Contents

- [Overview](#overview)
- [What It Demonstrates](#what-it-demonstrates)
- [Run](#run)
- [Notes](#notes)
- [References](#references)

## Overview

This example models a disclosure workflow where a gate passes and a stage emits
packets for downstream processing.

## What It Demonstrates

- Packet disclosure driven by gate outcomes.
- In-memory evidence and dispatch adapters.
- Stage advancement to terminal states.

## Run

```bash
cargo run -p decision-gate-example-data-disclosure
```

## Notes

- Uses in-memory adapters for clarity.
- Suitable for understanding packet lifecycles.

## References

