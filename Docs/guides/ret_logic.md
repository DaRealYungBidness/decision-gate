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

## At a Glance

**What:** Boolean algebra for gates, with unknown handling (tri-state logic)
**Why:** Make gate logic explicit, auditable, and deterministic - no hidden rules
**Who:** Developers and operators authoring complex gate requirements
**Prerequisites:** Basic understanding of conditions (see [condition_authoring.md](condition_authoring.md))

---

## Why RET?

**Problem:** How do you combine multiple evidence checks into a single gate decision?

**Example scenario:**
"I want to deploy to production if:
- Environment is 'production' AND
- Tests passed AND
- Coverage is above 85% AND
- At least 2 of 3 reviewers approved"

**Without RET:** You'd have to write custom code for each gate, which is:
- Not auditable (logic is hidden in code)
- Not deterministic (code can change between runs)
- Hard to verify offline (can't replay without re-executing code)

**With RET:** You express the logic as a tree structure:
```json
{
  "requirement": {
    "And": [
      { "Condition": "env_is_prod" },
      { "Condition": "tests_ok" },
      { "Condition": "coverage_ok" },
      {
        "RequireGroup": {
          "min": 2,
          "reqs": [
            { "Condition": "alice_approved" },
            { "Condition": "bob_approved" },
            { "Condition": "carol_approved" }
          ]
        }
      }
    ]
  }
}
```

**Benefits:**
- **Explicit:** Logic is visible in the scenario spec
- **Auditable:** Every evaluation is traced in the runpack
- **Deterministic:** Same evidence -> same outcome
- **Replayable:** Can verify offline without re-querying providers

> [Security]: Explicit gate logic prevents hidden backdoors. All logic is declared in the scenario spec and traced in the runpack for audit.

---

## Mental Model: RET Evaluation Tree

Here's how a requirement tree is evaluated:

```
RET EVALUATION TREE (simplified)

Gate Requirement (tree structure)
  And
  |-- Pred(A) -> true
  |-- Pred(B) -> unknown
  |-- Not(C) -> false
  `-- RequireGroup (min: 2)
      |-- Pred(D) -> true
      |-- Pred(E) -> true
      `-- Pred(F) -> false

Strong Kleene Logic: And(true, unknown, true, true) -> unknown
(gate holds)
```

**Evaluation order:**
1. Leaf conditions evaluate to tri-state (true/false/unknown)
2. Operator nodes combine child outcomes via tri-state logic
3. Root node outcome determines gate result

---

## Tri-State Outcomes

RET uses **tri-state logic** (not just true/false):

- **`true`**: Gate passes (all requirements satisfied)
- **`false`**: Gate fails (requirements contradicted)
- **`unknown`**: Gate holds (requirements inconclusive)

**Why tri-state?**
Gates **fail closed**: a gate only passes when the requirement evaluates to `true`. `unknown` outcomes prevent gates from passing until evidence is complete.

**Example:**
```
Gate: And(tests_ok, coverage_ok)
Conditions:
- tests_ok: true (tests passed)
- coverage_ok: unknown (coverage report missing)

Outcome: unknown (gate holds until coverage is available)
```

---

## Core Operators

### And

**Semantics:** All children must be `true`

**Truth table (2 operands):**
| Left | Right | Result |
|------|-------|--------|
| true | true | true |
| true | false | false |
| true | unknown | unknown |
| false | (any) | false |
| unknown | true | unknown |
| unknown | unknown | unknown |

**Example:**
```json
{
  "requirement": {
    "And": [
      { "Condition": "tests_ok" },
      { "Condition": "coverage_ok" }
    ]
  }
}
```

**Use case:** Both tests and coverage must pass

**Behavior:**
- All `true` -> `true` (gate passes)
- Any `false` -> `false` (gate fails)
- Otherwise -> `unknown` (gate holds)

---

### Or

**Semantics:** Any child may be `true`

**Truth table (2 operands):**
| Left | Right | Result |
|------|-------|--------|
| true | (any) | true |
| false | false | false |
| false | unknown | unknown |
| unknown | false | unknown |
| unknown | unknown | unknown |

**Example:**
```json
{
  "requirement": {
    "Or": [
      { "Condition": "manual_override" },
      { "Condition": "tests_ok" }
    ]
  }
}
```

**Use case:** Either manual override OR automated tests must pass

**Behavior:**
- Any `true` -> `true` (gate passes)
- All `false` -> `false` (gate fails)
- Otherwise -> `unknown` (gate holds)

---

### Not

**Semantics:** Invert child outcome

**Truth table:**
| Input | Result |
|-------|--------|
| true | false |
| false | true |
| unknown | unknown |

**Example:**
```json
{
  "requirement": {
    "And": [
      { "Condition": "tests_ok" },
      { "Not": { "Condition": "blocklist_hit" } }
    ]
  }
}
```

**Use case:** Tests must pass AND blocklist must NOT be hit

**Behavior:**
- `true` -> `false`
- `false` -> `true`
- `unknown` -> `unknown` (fail-closed: can't confirm absence)

---

### RequireGroup (Quorum)

**Semantics:** At least N of M children must be `true`

**Parameters:**
- `min`: Minimum number of `true` outcomes required
- `reqs`: Array of child requirements

**Example:**
```json
{
  "requirement": {
    "RequireGroup": {
      "min": 2,
      "reqs": [
        { "Condition": "alice_approved" },
        { "Condition": "bob_approved" },
        { "Condition": "carol_approved" }
      ]
    }
  }
}
```

**Use case:** At least 2 of 3 reviewers must approve

**Behavior:**
- Count `true` outcomes
- If count >= `min` -> `true` (quorum reached)
- If count + unknowns < `min` -> `false` (quorum impossible)
- Otherwise -> `unknown` (quorum pending)

**Truth table examples:**
| Outcomes | min | Result | Reason |
|----------|-----|--------|---------|
| [true, true, false] | 2 | **true** | 2 true >= min (quorum reached) |
| [true, unknown, unknown] | 2 | **unknown** | 1 true, can't reach min yet |
| [true, false, false] | 2 | **false** | 1 true, max possible is 1 < min |
| [true, true, unknown] | 2 | **true** | 2 true >= min (already met) |
| [false, false, false] | 2 | **false** | 0 true, impossible |

> [Developer]: See [ret-logic crate](../../ret-logic/README.md) for implementation. RequireGroup counts true/false independently (unknown is neither).

---

### Condition (Leaf Node)

**Semantics:** Reference a condition by key

**Example:**
```json
{
  "requirement": { "Condition": "tests_ok" }
}
```

**Use case:** Simple gate with single condition

**Behavior:**
- Evaluates to the condition's tri-state outcome
- Condition must exist in `ScenarioSpec.conditions`

---

## Tri-State Propagation Rules

How `unknown` outcomes propagate through operators:

### And Propagation

| Operands | Result | Reason |
|----------|--------|---------|
| `And(true, true, true)` | **true** | All requirements satisfied |
| `And(true, false, true)` | **false** | One fails -> And fails |
| `And(true, unknown, true)` | **unknown** | Can't confirm all true yet |
| `And(false, unknown)` | **false** | One fails (short-circuit) |
| `And(unknown, unknown)` | **unknown** | Pending evidence |

**Rule:** `false` dominates; all `true` yields `true`; otherwise `unknown`

---

### Or Propagation

| Operands | Result | Reason |
|----------|--------|---------|
| `Or(false, false, false)` | **false** | All requirements failed |
| `Or(true, false, false)` | **true** | One succeeds -> Or succeeds |
| `Or(false, unknown, false)` | **unknown** | Can't confirm all false yet |
| `Or(true, unknown)` | **true** | One succeeds (short-circuit) |
| `Or(unknown, unknown)` | **unknown** | Pending evidence |

**Rule:** `true` dominates; all `false` yields `false`; otherwise `unknown`

---

### RequireGroup Propagation

| Outcomes | min | true count | unknown count | Result |
|----------|-----|-----------|---------------|---------|
| [T, T, F] | 2 | 2 | 0 | **true** (min reached) |
| [T, U, U] | 2 | 1 | 2 | **unknown** (max 3, need 2) |
| [T, F, F] | 2 | 1 | 0 | **false** (max 1 < min) |
| [U, U, U] | 2 | 0 | 3 | **unknown** (max 3, need 2) |
| [F, F, F] | 2 | 0 | 0 | **false** (impossible) |

**Rule:**
- If `true_count >= min` -> `true` (quorum reached)
- If `true_count + unknown_count < min` -> `false` (quorum impossible)
- Otherwise -> `unknown` (quorum pending)

> [LLM Agent]: When RequireGroup returns `unknown`, you need more evidence. Check which conditions are unknown and work to satisfy them.

---

## Practical Use Cases

### Simple Gate: Both Conditions

**Scenario:** Deploy if tests passed AND coverage is above 85%

```json
{
  "gate_id": "quality_gate",
  "requirement": {
    "And": [
      { "Condition": "tests_ok" },
      { "Condition": "coverage_ok" }
    ]
  }
}
```

---

### Quorum Gate: 2 of 3 Reviewers

**Scenario:** Merge PR if at least 2 of 3 reviewers approved

```json
{
  "gate_id": "review_gate",
  "requirement": {
    "RequireGroup": {
      "min": 2,
      "reqs": [
        { "Condition": "alice_approved" },
        { "Condition": "bob_approved" },
        { "Condition": "carol_approved" }
      ]
    }
  }
}
```

---

### Exclusion Gate: NOT Blocklisted

**Scenario:** Deploy if NOT blocklisted

```json
{
  "gate_id": "blocklist_gate",
  "requirement": {
    "Not": { "Condition": "blocklist_hit" }
  }
}
```

---

### Complex Gate: (A AND B) OR C

**Scenario:** Deploy if (tests passed AND coverage OK) OR manual override

```json
{
  "gate_id": "deploy_gate",
  "requirement": {
    "Or": [
      {
        "And": [
          { "Condition": "tests_ok" },
          { "Condition": "coverage_ok" }
        ]
      },
      { "Condition": "manual_override" }
    ]
  }
}
```

---

## Branching on Gate Outcomes

You can route to different stages based on gate outcomes using `advance_to.branch`:

```json
{
  "advance_to": {
    "kind": "branch",
    "branches": [
      { "gate_id": "env_gate", "outcome": "true", "next_stage_id": "ship" },
      { "gate_id": "env_gate", "outcome": "unknown", "next_stage_id": "hold" },
      { "gate_id": "env_gate", "outcome": "false", "next_stage_id": "deny" }
    ],
    "default": null
  }
}
```

**How it works:**
1. Evaluate gate `env_gate`
2. Check outcome against branches (top to bottom)
3. First matching branch wins
4. If no match and `default` is null -> error

**Use cases:**
- **True:** Advance to production deployment
- **Unknown:** Hold for manual review
- **False:** Reject and notify

> [Security]: Use branching to implement fail-safe fallbacks. For example, route `unknown` to manual review instead of auto-advancing.

---

## Logic Mode: Strong Kleene

Decision Gate uses **Strong Kleene logic** (tri-state):

**Key properties:**
- `And(true, unknown)` -> `unknown` (can't confirm all true)
- `Or(false, unknown)` -> `unknown` (can't confirm all false)
- `Not(unknown)` -> `unknown` (can't invert uncertainty)

**Alternative (not used):** Bochvar logic (any unknown -> unknown)

**Why Kleene?**
- More intuitive for partial evidence
- Short-circuits when possible (`And(false, unknown)` -> `false`)
- Balances fail-closed with usability

> [Developer]: See [ret-logic/src/lib.rs](../../ret-logic/src/lib.rs) for the evaluation algorithm.

---

## Use Cases

**Primary:** Complex gates requiring boolean combinations (And, Or, quorum)
**Secondary:** Simple gates with single conditions (Condition node only)
**Anti-pattern:** Don't nest RETs too deeply - prefer focused conditions and flat trees

---

## Troubleshooting

### Problem: Gate Stuck in `unknown`

**Symptoms:**
Gate never passes, always returns `unknown`

**Cause:** One or more conditions are evaluating to `unknown`

**Solution:**
1. Check gate trace to see which conditions are `unknown`
2. Fix the underlying condition issues (see [condition_authoring.md](condition_authoring.md))
3. Common causes:
   - Provider error (e.g., file missing for json provider)
   - JSONPath not found (tool output mismatch)
   - Type mismatch for a type-sensitive comparator

---

### Problem: RequireGroup Never Passes

**Symptoms:**
RequireGroup always returns `false` or `unknown`

**Cause:** `min` is too high, or too many conditions are failing

**Solution:**
1. Check `min` value vs number of conditions
2. Verify condition outcomes in gate trace
3. Ensure at least `min` conditions can be `true` simultaneously

**Example:**
```json
// BAD: min is 3, but only 2 conditions
{
  "RequireGroup": {
    "min": 3,
    "reqs": [
      { "Condition": "a" },
      { "Condition": "b" }
    ]
  }
}

// GOOD: min <= number of conditions
{
  "RequireGroup": {
    "min": 2,
    "reqs": [
      { "Condition": "a" },
      { "Condition": "b" },
      { "Condition": "c" }
    ]
  }
}
```

---

### Problem: Branch Doesn't Match

**Symptoms:**
Gate evaluation error: "No matching branch"

**Cause:** Gate outcome doesn't match any branch, and `default` is null

**Solution:**
1. Add branches for all possible outcomes (true/false/unknown)
2. OR set `default` to a fallback stage
3. Example:
```json
{
  "branches": [
    { "gate_id": "gate1", "outcome": "true", "next_stage_id": "ship" },
    { "gate_id": "gate1", "outcome": "false", "next_stage_id": "deny" }
  ],
  "default": "hold"  // Fallback for unknown
}
```

---

## Authoring Tips

**1. Keep condition keys stable and descriptive**
- Use `tests_ok` not `pred1`
- Keys are referenced in runpacks for audit

**2. Use RequireGroup for quorum-style checks**
- Example: "2 of 3 reviewers", "3 of 5 datacenter checks"
- Alternative: Multiple And conditions (but less flexible)

**3. Prefer smaller trees with focused conditions**
- Easier to audit and understand
- Easier to debug when gates fail

**4. Validate RET structure during scenario definition**
- Decision Gate validates RETs at `scenario_define` time
- Fails fast if structure is invalid (e.g., referencing non-existent conditions)

**5. Use branching for fail-safe fallbacks**
- Route `unknown` to manual review
- Route `false` to alert/deny
- Route `true` to advance

---

## Cross-Reference Learning Paths

**New User Path:**
[getting_started.md](getting_started.md) -> [condition_authoring.md](condition_authoring.md) -> **THIS GUIDE** -> [integration_patterns.md](integration_patterns.md)

**Advanced Logic Path:**
**THIS GUIDE** -> [evidence_flow_and_execution_model.md](evidence_flow_and_execution_model.md) -> Understand how RETs fit into the evaluation pipeline

**Security Path:**
**THIS GUIDE** -> [security_guide.md](security_guide.md) -> Learn how explicit logic prevents backdoors

---

## Glossary

**And:** Operator requiring all children to be `true`.

**Gate:** Decision point in a scenario, evaluated via RET against evidence.

**Or:** Operator requiring any child to be `true`.

**Not:** Operator inverting child outcome (`true` <-> `false`).

**Condition:** Evidence check definition: query + comparator + expected value.

**RequireGroup:** Quorum operator requiring at least N of M children to be `true`.

**RET:** Requirement Evaluation Tree, boolean algebra (And/Or/Not/RequireGroup) for gates.

**TriState:** Evaluation outcome: true (pass), false (fail), or unknown (hold).

**Strong Kleene Logic:** Tri-state logic mode where `And(true, unknown)` -> `unknown`.
