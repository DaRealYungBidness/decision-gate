# AGENTS.md (examples/data-disclosure)

> **Audience:** Agents updating the data-disclosure example.
> **Goal:** Demonstrate packet dispatch and gating with minimal complexity.

## Standards (Read First)
Before making changes, read and follow:
- `Docs/standards/codebase_engineering_standards.md`
- `Docs/standards/codebase_formatting_standards.md`
- `Docs/standards/doc_formatting_standards.md`

## In scope
- Keeping example aligned with current APIs.
- Clear, deterministic outputs.

## Out of scope
- Adding production integration code.

## Testing
```bash
cargo run -p decision-gate-example-data-disclosure
```
