# AGENTS.md (examples/file-disclosure)

> **Audience:** Agents updating the file-disclosure example.
> **Goal:** Demonstrate broker sources/sinks without adding production logic.

## Standards (Read First)
Before making changes, read and follow:
- `Docs/standards/codebase_engineering_standards.md`
- `Docs/standards/codebase_formatting_standards.md`
- `Docs/standards/doc_formatting_standards.md`

## In scope
- Updating example wiring for current broker APIs.
- Keeping outputs deterministic.

## Out of scope
- Adding new broker capabilities.
- Introducing nondeterministic behavior.

## Testing
```bash
cargo run -p decision-gate-example-file-disclosure
```
