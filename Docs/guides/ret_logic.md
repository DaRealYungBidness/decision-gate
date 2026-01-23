<!--
Docs/guides/ret_logic.md
============================================================================
Document: Requirement Evaluation Trees
Description: Requirement algebra used for Decision Gate gates.
Purpose: Explain RET structure, operators, and tri-state evaluation.
Dependencies:
  - ret-logic/README.md
  - decision-gate-core runtime gate evaluation
============================================================================
-->

# Requirement Evaluation Trees (RET)

## Overview
RET stands for Requirement Evaluation Tree. It is a deterministic boolean
algebra that describes how predicates combine to decide whether a gate passes.
Decision Gate uses RET to keep gate logic explicit, auditable, and replayable.

## How Decision Gate Uses RET
Every gate in a ScenarioSpec has a `requirement` field that contains a RET
expression. Predicates are defined in the ScenarioSpec and referenced by key.
Decision Gate evaluates the tree to determine whether the gate is satisfied.

## Core Operators
RET supports universal boolean operators:

- `And`: All children must pass.
- `Or`: Any child may pass.
- `Not`: Invert a child requirement.
- `RequireGroup`: At least N of M children must pass.
- `Predicate`: A leaf node referencing a predicate key.

## Tri-State Outcomes
Decision Gate evaluates predicates into tri-state outcomes:

- `true`: Evidence satisfies the predicate and comparator.
- `false`: Evidence contradicts the predicate.
- `unknown`: Evidence is missing or cannot be evaluated.

Gate evaluation treats unknown outcomes as not passing. This keeps the system
fail-closed and deterministic under partial evidence.

## Decision Gate Logic Mode
Decision Gate evaluates RETs using strong Kleene logic (tri-state). In practice:

- `And`: `false` dominates; all `true` yields `true`; otherwise `unknown`.
- `Or`: `true` dominates; all `false` yields `false`; otherwise `unknown`.
- `Not`: `unknown` stays `unknown`.
- `RequireGroup`: `true` when satisfied; `false` when even unknowns cannot reach `min`; otherwise `unknown`.

Unknown outcomes keep the run in a hold state unless you explicitly branch on
`unknown` in `advance_to`.

## Branching on Gate Outcomes
`advance_to` supports a branch mode that routes on gate outcomes:

```json
{
  "advance_to": {
    "kind": "branch",
    "branches": [
      { "gate_id": "env_gate", "outcome": "true", "next_stage_id": "ship" },
      { "gate_id": "env_gate", "outcome": "unknown", "next_stage_id": "hold" }
    ],
    "default": "deny"
  }
}
```

The first matching branch wins. If no branch matches and `default` is null,
Decision Gate fails the evaluation with a gate resolution error.

## Example: Simple Gate

```json
{
  "gate_id": "env_gate",
  "requirement": {
    "And": [
      { "Predicate": "env_is_prod" },
      { "Predicate": "after_freeze" }
    ]
  }
}
```

## Example: Groups and Not

```json
{
  "gate_id": "release_gate",
  "requirement": {
    "RequireGroup": {
      "min": 2,
      "reqs": [
        { "Predicate": "env_is_prod" },
        { "Predicate": "audit_signed" },
        { "Not": { "Predicate": "blocklist_hit" } }
      ]
    }
  }
}
```

## Authoring Tips
- Keep predicate keys stable and descriptive.
- Use `RequireGroup` for quorum-style checks.
- Prefer smaller trees with focused predicates for easier audits.
- Validate RET structure during ScenarioSpec registration to fail fast.
