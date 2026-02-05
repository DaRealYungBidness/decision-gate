<!--
Decision Gate LLM Scenario Example README
============================================================================
Document: examples/llm-scenario
Description: Disclosure flow into a callback sink (LLM simulation).
Purpose: Demonstrate packet dispatch into a custom callback target.
Dependencies:
  - ../../crates/decision-gate-core/README.md
  - ../../crates/decision-gate-broker/README.md
============================================================================
-->

# LLM Scenario Example

Simulates an LLM-style disclosure flow by dispatching packets to a callback
sink.

## Table of Contents

- [Overview](#overview)
- [What It Demonstrates](#what-it-demonstrates)
- [Run](#run)
- [Notes](#notes)
- [References](#references)

## Overview

This example uses the broker's `CallbackSink` to capture packet payloads as if
they were prompts or model submissions.

## What It Demonstrates

- Callback-based disclosure dispatch.
- Controlled disclosure flow driven by gate outcomes.
- Packet metadata and receipt logging.

## Run

```bash
cargo run -p decision-gate-example-llm-scenario
```

## Notes

- Intended for integration modeling, not production use.

## References

