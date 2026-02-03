# AGENTS.md (examples/llm-scenario)

> **Audience:** Agents updating the LLM scenario example.
> **Goal:** Demonstrate callback dispatch without hiding core semantics.

## Standards (Read First)
Before making changes, read and follow:
- `Docs/standards/codebase_engineering_standards.md`
- `Docs/standards/codebase_formatting_standards.md`
- `Docs/standards/doc_formatting_standards.md`

## In scope
- Updating example wiring for broker APIs.
- Clarifying the simulated LLM flow.

## Out of scope
- Adding real LLM integrations or secrets.

## Testing
```bash
cargo run -p decision-gate-example-llm-scenario
```
