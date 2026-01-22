# Gate Duality: Barrier vs Target

This note captures the "dual" reading of gate logic in Decision Gate. The same
mathematical object can be read in two different ways:

1) Barrier (control plane): a gate blocks disclosure and stage advancement.
2) Target (agent goal): a gate specifies what "done" means, so an agent can
   plan actions that make the gate evaluate to True.

The math is identical; the primary motivation and mental model differ.

## Minimal formalization

Let:
- S be the space of world states (or state snapshots).
- E be evidence available from external systems.
- G be a gate: G : S x E -> {T, F, U} (tri-state).
- A stage i has a gate set Gi = {g1, g2, ...}.
- A gate set evaluates by conjunction: Gi(s, e) = T iff all gk(s, e) = T.

Decision Gate uses a deterministic evaluator over G with recorded evidence and
tri-state logic (Kleene/Bochvar).

## View 1: Gate as barrier (precondition)

Barrier view treats a gate set as a precondition for advancement or disclosure.

Definition (barrier): a stage i is eligible to advance when Gi(s, e) = T.

This aligns with a control-plane reading:
- Stage i is the current disclosure boundary.
- Gate outcomes decide if the boundary may be crossed.
- If Gi(s, e) != T, the engine holds and returns a safe summary.

In short: "We release data only if the predicates are satisfied."

## View 2: Gate as target (postcondition)

Target view treats the same gate set as a success criterion for a task.

Definition (target set): Ti = { (s, e) | Gi(s, e) = T }.

An agent can plan actions that move the system toward Ti. In this view:
- The gate set is a declarative goal specification.
- The agent derives tasks from predicates (or from their descriptions).
- The agent checks progress by re-evaluating Gi.

In short: "We must make these predicates true to be done."

## Same math, different direction

Barrier view is about control: "Do not advance unless the predicate holds."
Target view is about intention: "Make the predicate hold."

This is a duality of interpretation, not of structure:
- Barrier: predicate as a guard (precondition).
- Target: predicate as a goal (postcondition).

Both are expressions of the same logical requirement tree.

## Predicate algebra grounding

Each predicate pk is defined by:
- A query qk into some system (DB, log, time, receipt, etc.).
- A comparator ck (equals, exists, greater-than, etc.).
- An expected value vk (optional).

This yields a truth value pk(s, e) in {T, F, U}.

The requirement tree (RET) composes these predicates into gate outcomes.
This is the formal "plan language" you can evaluate against arbitrary systems.

## Different mental models

Barrier (control plane):
- Security and disclosure boundary.
- Evidence-backed policy enforcement.
- Deterministic auditing and replay.

Target (agent goal):
- Success condition and completion proof.
- Task list derivation from predicates.
- Test-oracle for "did we do the work?"

Contractual reading (shared):
- "If the system claims success, the gate must evaluate to True."
- "If the gate evaluates to True, the system may advance."

## Design implications

- Evidence providers define what the predicates mean in the real world.
- Safe summaries report unmet gate identifiers without leaking evidence.
- The core engine remains agnostic; the semantics live in predicates and their
  evidence queries.
- If you want "task text," attach descriptions to gate ids or provide sidecar
  mappings; the math remains the same.

## Mapping to the codebase

- ret-logic: requirement algebra and tri-state evaluation.
- decision-gate-core: evidence queries, comparators, gate evaluation, run state,
  and disclosure decisions.
- decision-gate-broker: resolves payloads and dispatches them, but does not
  decide whether the gates are satisfied.

---

# LLM Integration and Market Context

This section extends the mathematical framework to address how Decision Gate
relates to LLM-based agents and the broader market for agent orchestration.

## The LLM Planning Landscape

Modern LLM agents use various approaches to multi-step task execution:

1. **Scratchpad reasoning**: The model writes out steps, executes them, and
   self-assesses completion. Examples: chain-of-thought prompting, ReAct.

2. **Graph-based workflows**: Explicit DAGs with conditional edges. The model
   executes nodes; edges are typically code-defined conditions. Examples:
   LangGraph, DSPy.

3. **Goal decomposition**: The model recursively breaks goals into subgoals,
   executing leaf tasks. Examples: AutoGPT, BabyAGI.

4. **Output validation**: Post-generation checks verify output structure or
   content. Examples: Guardrails AI, Outlines, Instructor.

Each approach has tradeoffs. Scratchpad reasoning is flexible but unreliable.
Graph workflows are structured but require upfront definition. Goal decomposition
is autonomous but hard to constrain. Output validation catches errors but only
after generation.

## The Self-Assessment Problem

A fundamental issue with LLM-driven task execution: the model is both executor
and judge. When an LLM says "I completed step 3," there is no external
verification that step 3 is actually complete.

This matters because:

1. **LLMs hallucinate completeness.** Benchmarks show models claiming task
   completion when objectives are unmet. The model's confidence is not
   correlated with actual success.

2. **Context window limits state tracking.** As tasks grow complex, models lose
   track of what was planned vs executed. Early steps may be "forgotten" or
   incorrectly recalled.

3. **Self-assessment is not auditable.** Regulated domains require proof of
   execution, not self-reports. "The model said it did it" is insufficient
   evidence for compliance.

Scratchpad reasoning keeps the plan inside the model's context. This is
convenient but unreliable. The plan exists only in the model's representation
of the world, which may diverge from actual system state.

## External Verification: The Decision Gate Model

Decision Gate addresses self-assessment by externalizing verification:

```
Scratchpad:       LLM plans → LLM executes → LLM self-assesses
Decision Gate:    LLM acts → External system evaluates evidence → Gate result
```

Key properties:

1. **Evidence-backed evaluation.** Predicates query external systems (databases,
   APIs, filesystems, CI pipelines). The truth source is the system, not the
   model's claim.

2. **Tri-state logic.** Unknown means "evidence insufficient." The system holds
   rather than forcing a binary decision. This prevents premature advancement.

3. **Deterministic replay.** Run state captures all evidence snapshots and gate
   evaluations. The same inputs always produce the same outputs. Auditors can
   verify decisions post-hoc.

4. **Separation of concerns.** The LLM can focus on acting; the gate system
   handles verification. This reduces cognitive load and error modes.

## Where Formal Gates Provide Value

Not all tasks benefit from formal verification. The value proposition is
strongest when:

| Condition                          | Scratchpad | Formal Gates |
|------------------------------------|------------|--------------|
| Low stakes, exploratory work       | Sufficient | Overkill     |
| High stakes, consequential output  | Risky      | Valuable     |
| Compliance or audit requirements   | Inadequate | Essential    |
| Multi-step with external deps      | Fragile    | Robust       |
| Long-running or resumable tasks    | Lossy      | Durable      |

Target domains include:
- Financial services (audit trails, compliance gates)
- Healthcare (approval workflows, evidence logging)
- Legal (document review stages, sign-off requirements)
- DevOps (deployment gates, rollback conditions)
- Enterprise automation (approval chains, escalation logic)

## Market Positioning

Decision Gate occupies a specific niche:

```
             Flexibility
                 ↑
                 |  Scratchpad / ReAct
                 |
                 |      LangGraph
                 |
                 |          Decision Gate
                 |
                 |              Formal Verification
                 +------------------------→ Rigor
```

Compared to alternatives:

- **vs Scratchpad**: More structured, externally verified, auditable. Less
  flexible, requires predicate definitions.

- **vs LangGraph**: Declarative logic vs imperative code. Evidence-backed vs
  code-defined conditions. Tri-state vs binary.

- **vs Guardrails AI**: Pre-disclosure control vs post-generation validation.
  Stage-gated information vs output checking.

- **vs Temporal.io**: Similar durability model, but LLM/agent-oriented with
  tri-state logic and disclosure semantics.

The value proposition: "Formal verification for agent workflows, with
evidence-backed gates and auditable decision trails."

## LLM-Authored Gates

An interesting pattern: LLMs can author their own gate requirements using the
ret-logic DSL:

```
all(data_fetched, analysis_complete, at_least(2, peer_reviewed, manager_approved, qa_passed))
```

This inverts typical guardrails. Instead of external constraints imposed on the
model, the model expresses its own preconditions in a formal, verifiable
language.

Workflow:
1. LLM receives task
2. LLM generates requirement tree expressing success conditions
3. LLM takes actions toward satisfying the requirement
4. External system evaluates gate against evidence
5. True → advance | Unknown → hold | False → fail

The model is writing its own tests as it works. The tests are then evaluated
externally, not self-assessed.

This preserves the flexibility of model-driven planning while adding the rigor
of external verification.

## Integration Patterns

Three primary integration modes:

1. **Library**: Embed ret-logic and decision-gate-core in application code.
   Rust-native, zero-overhead for Rust applications. Python bindings for ML
   workflows.

2. **Sidecar/Daemon**: Run decision-gate as a service. REST/gRPC API for gate
   evaluation, evidence queries, and run state management. Suits platform teams.

3. **Agent SDK Plugin**: Integrate with existing agent frameworks (LangChain,
   Semantic Kernel). Provides gate evaluation as a tool the agent can call.

Evidence connectors are pluggable. Common targets:
- Databases (Postgres, SQLite, Redis)
- APIs (REST, GraphQL, webhooks)
- CI/CD (GitHub Actions, GitLab CI)
- File systems (local, S3)
- Time/scheduling

## Open Questions

1. What is the right developer experience for defining predicates? Pure code,
   DSL, or visual builder?

2. How granular should evidence snapshots be? Per-predicate, per-gate, or
   per-stage?

3. Can gate definitions be learned or suggested from task descriptions?

4. What is the minimal viable integration for early adopters?
