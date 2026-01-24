<!--
Decision Gate Agent Loop Example README
============================================================================
Document: examples/agent-loop
Description: Multi-step predicate satisfaction example.
============================================================================
-->

# Agent Loop Example

## Overview
Simulates an agent loop where predicates are satisfied over time, exercising
multi-step gate progression.

## Run
```bash
cargo run -p decision-gate-example-agent-loop
```

## Notes
- Uses in-memory providers.
- Demonstrates staged `scenario_next` evaluation.
