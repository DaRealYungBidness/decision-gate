# AGENTS.md (examples/agent-loop)

> **Audience:** Agents updating the agent-loop example.
> **Goal:** Keep the loop deterministic and aligned with core evaluation.

## Standards (Read First)
Before making changes, read and follow:
- `Docs/standards/codebase_engineering_standards.md`
- `Docs/standards/codebase_formatting_standards.md`
- `Docs/standards/doc_formatting_standards.md`

## In scope
- Updating conditions and flow for current APIs.
- Improving clarity of staged progression.

## Out of scope
- Adding external dependencies or side effects.

## Testing
```bash
cargo run -p decision-gate-example-agent-loop
```
