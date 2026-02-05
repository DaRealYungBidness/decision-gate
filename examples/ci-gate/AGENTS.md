# AGENTS.md (examples/ci-gate)

> **Audience:** Agents updating the CI gate example.
> **Goal:** Demonstrate CI-style gating without external integrations.

## Standards (Read First)
Before making changes, read and follow:
- `Docs/standards/codebase_engineering_standards.md`
- `Docs/standards/codebase_formatting_standards.md`
- `Docs/standards/doc_formatting_standards.md`

## In scope
- Updating conditions and thresholds for current APIs.
- Clarifying RequireGroup usage.

## Out of scope
- Adding real CI integrations or secrets.

## Testing
```bash
cargo run -p decision-gate-example-ci-gate
```
