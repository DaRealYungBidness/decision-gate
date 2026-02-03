# Agent Investigation Guide

**Audience:** LLM agents and copilots performing targeted review or refinement passes on specific folders or crates.
**Goal:** Find and fix meaningful issues (bugs, drift, design violations, unnecessary complexity) without random refactors or architecture changes. Assume advanced, well-resourced adversaries when evaluating security posture.

This guide defines how an agent should think and operate during an investigation pass, once it has been pointed at a specific folder and given:
- `Docs/standards/codebase_formatting_standards.md`.
- `Docs/standards/codebase_engineering_standards.md`.
- `Docs/security/threat_model.md`.
- `AGENTS.md` for the crate, or root if present.
- This guide.

The intent is to make each pass focused, safe, and high-leverage, not to rewrite everything.

---

## 1. Scope and Non-Goals

### 1.1 In Scope

During an investigation pass, you may:
- Identify and fix correctness issues, misused APIs, and broken edge cases.
- Fix violations of documented architecture or boundary rules.
- Replace stringly-typed or raw numeric values with existing enums or newtypes where appropriate.
- Remove obvious performance pitfalls on known hot paths.
- Fix documentation drift where comments no longer match behavior.
- Fix formatting or structure that violates `Docs/standards/codebase_formatting_standards.md`.
- Add or tighten tests that lock in existing behavior.
- Apply small, localized refactors that improve clarity or safety without changing semantics.

### 1.2 Out of Scope

You must not:
- Redesign or move core abstractions unless explicitly instructed.
- Change public APIs in ways that break downstream tools or providers unless a bug fix requires it.
- Introduce new dependencies, crates, or large subsystems.
- Perform sweeping renames across many files.
- Perform purely aesthetic rewrites of working code.

If you believe something should be redesigned, document it as a recommendation, but do not implement the redesign in this pass.

---

## 2. Standard Workflow for Each Investigation

### Step 0 - Orient Yourself

1. Read the root `AGENTS.md` if present.
2. Read the crate `AGENTS.md` if present.
3. Read `Docs/standards/codebase_formatting_standards.md`.
4. Read `Docs/standards/codebase_engineering_standards.md`.
5. Read `Docs/security/threat_model.md` sections relevant to the target area.
6. Read `Docs/guides/security_guide.md` when working on MCP, providers, or storage.
7. Skim the relevant README files.
8. Identify subsystems in play and which files are hot path versus peripheral.

Do not edit until you have this context.

### Step 1 - Build a Mental Map

1. List the key modules or files and what each one does.
2. Identify entry points, core data types, and invariants noted in docs or comments.

### Step 2 - Run Static Checks (Mentally or Explicitly)

Assume all changes must pass:
- `cargo +nightly fmt --all`.
- `cargo clippy --all-targets --all-features -- -D warnings`.
- `cargo nextest run`.
- `cargo nextest run -p system-tests --features system-tests`.

Do not introduce anything that would obviously violate these commands.

### Step 3 - Scan for Issue Classes

Work with a targeted checklist. Look for high-signal issues that conflict with:
- Architecture docs.
- Formatting standards.
- Performance posture.
- Type safety expectations.

Also scan for debt triggers that warrant a fix or a follow-up note.

### Step 4 - Propose Minimal, High-Value Changes

For each issue you decide to fix:
- Prefer the smallest coherent change that resolves the problem.
- Keep behavior identical unless you are clearly fixing a bug.
- Co-locate or update tests.
- Update any obviously stale docs or comments in the same file.

Avoid multi-concern changes in a single diff.

### Step 5 - Summarize Findings

At the end of the pass, produce a short summary with:
- High-level scope.
- Fixed issues and rationale.
- Flagged issues and suggested follow-up.

---

## 2.1 Debt Triggers and Small Wins

Debt triggers to fix or flag:
- Repeated TODO or FIXME markers in the same module.
- Flaky tests or brittle fixtures that require manual retries.
- Manual recovery steps or repair scripts.
- Ad-hoc error enums where a shared error type exists.
- Unbounded inputs or missing size limits in evidence parsing or provider I/O.
- Duplicate logic for the same schema or contract in multiple crates.
- Debug-only flags or environment toggles required for normal use.

Small wins allowed:
- Improve error messages with actionable context.
- Tighten input bounds with clear failure modes.
- Remove dead parameters or unused fields when local and not part of a public contract.
- Add tiny helper functions to reduce local boilerplate.
- Fix obvious documentation drift.

---

## 3. Architecture and Boundary Checks

Your first responsibility is to ensure the code respects the documented Decision Gate boundaries.

Questions to ask:
- Does `decision-gate-core` own gate evaluation, evidence anchoring, decision records, and runpack artifacts?
- Does `ret-logic` remain domain-agnostic with no provider semantics or disclosure policy?
- Is `decision-gate-mcp` transport and tool routing only?
- Do providers only acquire evidence and return EvidenceResult data?
- Are schema or contract changes centralized in `decision-gate-contract` with regenerated outputs under `Docs/generated/decision-gate`?
- Is persistent storage treated as untrusted input with hash verification and fail-closed behavior?

When to change:
- Change directly if the fix is local and low-risk.

When to flag:
- Flag when fixing requires touching multiple crates or changing public contracts.

---

## 4. Determinism, Replay, and Runpack Integrity

Decision Gate's value comes from deterministic evaluation and offline verification. Check for:
- Nondeterminism in evaluation or artifact generation.
- Run state mutations that are not replayable or idempotent.
- Runpack artifacts or hashes that vary across runs for identical inputs.
- Evidence anchors that are recomputed instead of recorded and verified.
- Changes that would break offline verification without a clear migration path.

If a change affects determinism, add or update determinism and replay tests.

---

## 5. Evidence, Disclosure, and Provider Safety

- Treat all evidence as hostile input and validate shape, bounds, and sizes.
- Fail closed on missing or invalid evidence; do not default allow.
- Respect disclosure policy. Raw evidence_query values are denied by default and must not leak via logs or errors.
- Signature verification must be strict when configured.
- Avoid provider-specific logic in core. Keep provider semantics in provider crates.

---

## 6. Type and Modeling Hygiene

Look for:
- Repeated string literals encoding a finite state or mode.
- Raw integer IDs where a newtype already exists.
- Ad-hoc maps keyed by String where an enum or typed key would be better.
- Structs exposing loosely typed fields where a stronger type exists.

You may refactor to enums or newtypes when:
- The value set is finite and stable.
- The usage is internal to the crate or module.
- The change is local and mechanical.

Do not introduce new enums or newtypes when:
- The data is bound to an external protocol or schema that requires a String.
- The change would require a schema migration or public API change without approval.

---

## 7. Performance and Allocation Hygiene

In hot paths:
- Remove unnecessary clones in tight loops.
- Avoid needless String allocations in evaluation paths.
- Avoid maps with String keys in per-evaluation logic.

In non-hot paths:
- Prefer clarity over micro-optimizations.

---

## 8. Documentation, Comments, and Formatting

For each public type or function:
- Confirm doc comments reflect the actual signature and behavior.
- Fix parameter name drift or stale invariants.
- Update references to renamed types or modules.

For each file you touch:
- Ensure it has the header, overview block, and section banners required by `Docs/standards/codebase_formatting_standards.md`.

---

## 9. How to Decide: Fix vs Flag

Fix now when:
- The change is local and small.
- It is a clear correctness or policy violation.
- The impact is well understood and testable.

Flag for follow-up when:
- The issue spans multiple crates.
- The fix requires architectural decisions.
- You are unsure about external contract impact.

---

## 10. Output Expectations for an Investigation Pass

Your output should include:
1. Scope description (crate or folder, major modules).
2. Summary of changes (what you modified).
3. Issue inventory with fixed issues and flagged issues.
4. Threat model delta (or `Threat Model Delta: none`).
5. Confidence statement for non-trivial changes.

---

## 11. Quick Checklist

Before you start:
- Read root and crate `AGENTS.md`.
- Read `Docs/standards/codebase_formatting_standards.md`.
- Read `Docs/standards/codebase_engineering_standards.md`.
- Read relevant sections of `Docs/security/threat_model.md`.
- Skim relevant README files.

During investigation:
- Respect control-plane boundaries.
- Keep `ret-logic` domain-agnostic.
- Keep MCP transport-only; no policy logic.
- Fail closed on missing or invalid evidence.
- Avoid allocations and clones on hot paths unless justified.
- Keep public contracts stable unless clearly fixing a bug.
- Update docs and comments when they drift.
- Add or update tests for any behavior change.
- Default to `fn`; promote to `const fn` only when needed.
