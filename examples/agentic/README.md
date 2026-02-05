# Agentic Scenario Packs (Mirrored)

These scenario packs are mirrored from the deterministic harness fixtures under:

- `system-tests/tests/fixtures/agentic/`

They exist here so developers can discover and inspect the canonical scenarios
without digging through system-tests. The system-tests fixtures are the source
of truth; any updates should be made there and then mirrored here.

Each scenario directory contains:
- `spec.json` — scenario definition
- `run_config.json` — run configuration
- `trigger.json` — deterministic trigger event
- `fixtures/` — evidence inputs (JSON files, env overrides)
