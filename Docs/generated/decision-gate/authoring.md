# Decision Gate Authoring Formats

Decision Gate accepts ScenarioSpec authoring input in JSON or RON. JSON is the canonical format used for hashing, schemas, and runpacks. RON exists only as a human-friendly authoring layer and must be normalized to canonical JSON before execution.

## Canonical JSON

- Canonical JSON uses RFC 8785 (JCS) for deterministic ordering.
- ScenarioSpec hashes are computed over canonical JSON bytes.
- Canonical JSON is emitted by `decision-gate authoring normalize`.

## Supported Inputs

- JSON: canonical format for storage, hashing, and validation.
- RON: authoring-only format normalized to canonical JSON.
- YAML: not supported by default (add only with explicit requirement).

## Normalization Pipeline

1. Parse JSON or RON into a structured value.
2. Validate against `schemas/scenario.schema.json`.
3. Run ScenarioSpec semantic validation (IDs, predicates, gates).
4. Canonicalize to JSON (RFC 8785).
5. Compute the canonical spec hash.

## CLI Usage

Validate RON authoring input:

```bash
decision-gate authoring validate --input examples/scenario.ron --format ron
```

Normalize to canonical JSON:

```bash
decision-gate authoring normalize --input examples/scenario.ron --format ron \
  --output examples/scenario.json
```

## References

- `examples/scenario.ron`: authoring example in RON.
- `examples/scenario.json`: canonical JSON output.
- `schemas/scenario.schema.json`: JSON Schema for ScenarioSpec.
