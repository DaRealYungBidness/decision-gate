# Decision Gate

Decision Gate is a deterministic, replayable control plane for gated disclosure.
It evaluates evidence-backed gates, emits auditable decisions, and supports
offline verification via runpacks. It is backend-agnostic and integrates via
explicit interfaces rather than embedding into agent frameworks.

RET stands for **Requirement Evaluation Tree** and refers to the universal
predicate algebra used by the engine.

## Repository Layout

- `decision-gate-core`: deterministic engine, schemas, and runpack tooling
- `decision-gate-broker`: reference sources/sinks and composite dispatcher
- `ret-logic`: universal predicate evaluation engine (RET)
- `examples/`: runnable examples (`minimal`, `file-disclosure`, `llm-scenario`)

## Quick Start

- Run core tests: `cargo test -p decision-gate-core`
- Run broker tests: `cargo test -p decision-gate-broker`
- Run examples:
  - `cargo run -p decision-gate-example-minimal`
  - `cargo run -p decision-gate-example-file-disclosure`
  - `cargo run -p decision-gate-example-llm-scenario`

## Security

Decision Gate assumes hostile inputs and fails closed on missing or invalid
evidence. See `Docs/security/threat_model.md` for the full threat model.
