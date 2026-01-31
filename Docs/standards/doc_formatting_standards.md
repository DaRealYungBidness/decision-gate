<!--
Docs/standards/doc_formatting_standards.md
============================================================================
Document: Decision Gate Documentation Formatting Standards
Description: Lightweight, enforceable formatting rules for Markdown docs.
Purpose: Keep docs readable without over-constraining prose or layout.
============================================================================
Last Updated: 2026-01-31 (UTC)
============================================================================
-->

# Documentation Formatting Standards

## Goal
These rules keep docs readable and consistent while avoiding aggressive
reflows or stylistic bikeshedding. We enforce structural correctness only.

## Scope
Applies to:
- `Docs/**`
- `README.md`

## Enforced (via markdownlint)
- Valid Markdown syntax.
- Consistent fenced code blocks (no broken fences).
- Lists render correctly (no malformed list structures).
- No trailing whitespace.

## Not Enforced (by design)
- Line length limits.
- Blank line requirements around headings, lists, or fences.
- Heading level preferences for emphasis vs heading tokens.

## Cross-References
Use the file/line cross-reference format below. Tooling linkifies these into
clickable GitHub links for maximum usability.

- Example: `[F:decision-gate-mcp/src/auth.rs L217-L296](decision-gate-mcp/src/auth.rs#L217-L296)`

## Authoring Notes
- Prefer explicit language identifiers on code fences when practical.
- Keep headings meaningful and stable; avoid churn unless semantics change.
- If a doc is generated, keep it under `Docs/generated/` and avoid manual edits.
