<!--
Docs/guides/predicate_authoring.md
============================================================================
Document: Predicate Authoring Cookbook
Description: Practical guidance for writing ScenarioSpec predicates.
Purpose: Help authors define predicates that are correct, deterministic, and auditable.
Dependencies:
  - Docs/generated/decision-gate/providers.json
  - Docs/generated/decision-gate/schemas/scenario.schema.json
  - decision-gate-core/src/runtime/comparator.rs
============================================================================
-->

# Predicate Authoring Cookbook

## Overview
Predicates bind a Requirement Evaluation Tree (RET) leaf to an evidence query.
Every predicate must be deterministic, schema-valid, and auditable. This guide
shows how to author predicates that are precise, stable, and easy to verify.

## Anatomy of a PredicateSpec
Predicate specs live in the ScenarioSpec `predicates` array. Each entry includes:

- `predicate`: stable identifier used by gates.
- `query`: evidence provider + predicate + params payload.
- `comparator`: comparison operator applied to evidence.
- `expected`: value compared against evidence output (optional, but required for
  most comparators).
- `policy_tags`: policy labels applied to predicate evaluation.

Note: `query.predicate` is the provider check name from the provider contract.
The top-level `predicate` key is the ScenarioSpec predicate identifier.

Minimal example:

```json
{
  "predicate": "env_is_prod",
  "query": {
    "provider_id": "env",
    "predicate": "get",
    "params": { "key": "DEPLOY_ENV" }
  },
  "comparator": "equals",
  "expected": "production",
  "policy_tags": []
}
```

## Comparator Semantics (Tri-State)
Decision Gate evaluates predicates into tri-state outcomes:

- `true`: evidence satisfies the comparator.
- `false`: evidence contradicts the comparator.
- `unknown`: evidence is missing or cannot be evaluated.

Key rules from the runtime comparator:

- `exists` / `not_exists` only test whether `evidence.value` is present.
- All other comparators return `unknown` if `expected` is missing or if the
  evidence value cannot be compared.
- Non-numeric values with numeric comparators yield `unknown`.

Gates fail closed: a gate only passes when the requirement evaluates to `true`.
Unknown outcomes do not advance the run.

## Comparator Quick Reference
- `equals` / `not_equals`: JSON values or byte arrays; byte comparisons require an array of 0-255 integers.
- `greater_than` / `greater_than_or_equal` / `less_than` / `less_than_or_equal`: JSON numbers only.
- `contains`: string contains substring, or array contains all elements in the expected array.
- `in_set`: expected must be an array; evidence matches when equal to any element.
- `exists` / `not_exists`: ignore `expected` and only check for `evidence.value` presence.

If `expected` is missing or mismatched with the evidence type, the comparator
returns `unknown`.

## Avoiding Unknown Outcomes
- Always supply `expected` for every comparator except `exists`/`not_exists`.
- Match `expected` types to the provider result schema.
- Use provider examples from `providers.json` to sanity-check your payloads.

## Provider Patterns
Use `Docs/generated/decision-gate/providers.json` to confirm:

- Supported predicates.
- Param schemas and required fields.
- Allowed comparators.
- Example params/results.

### time provider
Uses trigger timestamps supplied by the caller (no wall-clock reads).

```json
{
  "predicate": "after_freeze",
  "query": {
    "provider_id": "time",
    "predicate": "after",
    "params": { "timestamp": 1710000000000 }
  },
  "comparator": "equals",
  "expected": true,
  "policy_tags": []
}
```

### env provider
Reads environment variables with allow/deny policy and size limits.

```json
{
  "predicate": "deploy_env",
  "query": {
    "provider_id": "env",
    "predicate": "get",
    "params": { "key": "DEPLOY_ENV" }
  },
  "comparator": "in_set",
  "expected": ["staging", "production"],
  "policy_tags": []
}
```

### json provider
Reads JSON or YAML files and evaluates JSONPath queries.

```json
{
  "predicate": "config_version",
  "query": {
    "provider_id": "json",
    "predicate": "path",
    "params": { "file": "/etc/config.json", "jsonpath": "$.version" }
  },
  "comparator": "equals",
  "expected": "1.2.3",
  "policy_tags": []
}
```

### http provider
Issues bounded HTTP GET requests and returns status or body hashes.

```json
{
  "predicate": "health_ok",
  "query": {
    "provider_id": "http",
    "predicate": "status",
    "params": { "url": "https://api.example.com/health" }
  },
  "comparator": "equals",
  "expected": 200,
  "policy_tags": []
}
```

## Evidence Disclosure Guidance
Predicates may return raw values, hashes, anchors, or references depending on
policy and provider settings. Treat raw values as sensitive and ensure:

- `decision-gate.toml` evidence policies allow raw output only when required.
- Providers opt in to raw disclosure when policy demands it.
- Anchors and hashes are preserved for audit and replay.

## Checklist
- Confirm the predicate exists in `providers.json`.
- Match params and result schema types precisely.
- Use only allowed comparators for the predicate.
- Provide `expected` for all comparators except `exists`/`not_exists`.
- Keep predicate keys stable and descriptive.
