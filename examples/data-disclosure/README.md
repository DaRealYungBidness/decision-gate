<!--
Decision Gate Data Disclosure Example README
============================================================================
Document: examples/data-disclosure
Description: Disclosure stage with packet emission after approval.
============================================================================
-->

# Data Disclosure Example

## Overview
Models a disclosure workflow where a gate approval unlocks a stage that emits
packet payloads.

## Run
```bash
cargo run -p decision-gate-example-data-disclosure
```

## Notes
- Uses in-memory evidence and dispatch adapters.
- Demonstrates packet dispatch and stage advancement.
