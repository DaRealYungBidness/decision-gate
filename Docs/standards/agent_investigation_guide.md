# Agent Investigation Guide

> **Audience:** LLM agents and copilots performing **targeted review / refinement passes** on specific folders or crates.  
> **Goal:** Find and fix meaningful issues (bugs, drift, design violations, unnecessary complexity) **without** random refactors or architecture changes.

This guide defines how an agent should **think and operate** during an investigation pass, once it has been pointed at a specific folder (its “area of responsibility”) and given:

- `Docs/standards/codebase_formatting_standards.md`
- `Docs/standards/codebase_engineering_standards.md`
- `Docs/security/threat_model.md`
- `AGENTS.md` (crate-specific; root if present)
- This guide.

The intent is to make each pass **focused, safe, and high-leverage**, not to “rewrite everything”.

---

## 1. Scope and Non-Goals

### 1.1 In Scope

During an investigation pass, you **may**:

- Identify and fix:
  - Clear correctness issues (logic errors, misused APIs, broken edge cases).
  - Violations of documented architecture / boundary rules.
  - Type-safety gaps: using ad-hoc `String`s or `u32`s where a dedicated type/enum already exists.
  - Obvious performance pitfalls on known hot paths (unnecessary clones, needless allocations, string comparisons in tight loops).
  - Documentation drift: comments or docstrings that no longer match the implementation.
  - Formatting / structure that clearly violates `Docs/standards/codebase_formatting_standards.md`.

- Add or tighten:
  - Tests that lock in existing behavior.
  - Small, localized refactors that improve clarity or safety **without** changing semantics.

### 1.2 Out of Scope (for a single investigation pass)

You **must not**:

- Redesign or move core abstractions (control plane boundaries, ScenarioSpec/EvidenceQuery schema, runpack/manifest format, provider protocol, contract generation) unless explicitly instructed.
- Change public APIs in ways that would break downstream tools or providers, unless a bug fix absolutely requires it.
- Introduce new dependencies, crates, or large subsystems.
- Perform sweeping renames across many files.
- Perform purely aesthetic rewrites of working code.

If you believe something **should** be redesigned, document it as a **recommendation** (see §9), but do not implement the redesign in this pass.

---

## 2. Standard Workflow for Each Investigation

Whenever you are given an area of responsibility (usually a crate or a sub-folder), follow this sequence.

### Step 0 – Orient Yourself

1. Read:
   - Root `AGENTS.md` (if present).
   - The crate’s own `AGENTS.md` (if present).
   - `Docs/standards/codebase_formatting_standards.md`.
   - `Docs/standards/codebase_engineering_standards.md`.
   - `Docs/security/threat_model.md` (at least the boundary/entry-point sections relevant to the area).
   - `Docs/guides/security_guide.md` when working on MCP, providers, or storage.
2. Skim the relevant README(s) for that crate/module.
3. Identify:
   - Which subsystem(s) are in play (control plane/core, ret-logic, MCP transport, providers, contract/codegen, store, CLI, broker, system tests).
   - Which files are **hot path** vs peripheral (as described in AGENTS and/or README).

Do **not** start editing until you have this context.

### Step 1 – Build a Mental Map of the Folder

For the folder you were assigned:

1. List the key modules / files and what each one does.
2. Note:
   - Entry points (public APIs, handlers, commands).
   - Core data types and enums.
   - Key invariants mentioned in docs or comments.

You may write a short internal note or comment block (e.g., in the PR description) summarizing this map, but do not add noisy in-code commentary unless it’s clarifying and permanent.

### Step 2 – Run Static Checks (Mentally or Explicitly)

Before or during edits:

- Respect the workspace lints in `Cargo.toml` (no `unsafe`, no unuseds, missing docs, clippy rules, etc.).
- Assume that **all changes must pass**:
  - `cargo +nightly fmt --all`
  - `cargo clippy --all-targets --all-features -- -D warnings`
  - `cargo nextest run` (unit/integration default tier)
  - `cargo nextest run -p system-tests --features system-tests` (OSS system-tests)

You do not need to script these commands, but you must avoid introducing anything that would obviously violate them.

### Step 3 – Scan for Issue Classes

Work through the code with a **targeted checklist** (see §3–§8). Do not “optimize everything”. Instead, look for **high-signal issues** that clearly conflict with:

- Architecture docs
- Formatting standards
- Performance posture
- Type-safety expectations

In addition, scan for **debt triggers** that warrant a fix or a follow-up note (see §2.1).

### Step 4 – Propose Minimal, High-Value Changes

For each issue you decide to fix:

1. Prefer the **smallest coherent change** that resolves the problem.
2. Keep **behavior identical** unless you are clearly fixing a bug.
3. Co-locate or update tests.
4. Update any obviously stale docs or comments in the same file.

Avoid multi-concern changes in a single diff (e.g., don’t mix API changes, formatting cleanups, and incidental renames).

### Step 5 – Summarize Findings

At the end of the pass, produce a short summary (for PR body or review notes) with:

- A high-level overview of what you inspected.
- A bulleted list of:
  - **Fixed** issues (with brief rationale).
  - **Flagged** issues (that you did not change but that deserve human review or a future pass).

---

## 2.1 Debt Triggers and Small Wins

These are low-friction indicators that tech debt or QoL friction is building. If you see them, either fix them locally or flag them explicitly in the summary.

### Debt Triggers (Fix or Flag)

- Repeated TODOs/FIXMEs in the same module (especially around parsing, validation, or storage).
- Flaky tests or brittle fixtures that require manual retries.
- Repeated manual recovery steps (repair scripts, hand-edited state, “run this twice” notes).
- Ad-hoc error enums or stringly-typed errors when a common error type exists.
- Unbounded inputs or missing size limits in evidence parsing or provider I/O.
- Duplicate logic for the same schema/contract in multiple crates.
- Debug-only flags or environment toggles that are effectively required for normal use.

### Small Wins Allowed (Local, Non-Breaking)

- Improve error messages to include actionable context (ids, bounds, expected values).
- Tighten input bounds or validation with clear failure modes.
- Remove dead parameters or unused fields when local and not part of a public contract.
- Add tiny helper functions to reduce repeated boilerplate in a single module.
- Fix obvious docs drift (parameter names, invariants, examples).

## 3. Architecture and Boundary Checks

Your first responsibility is to ensure the code respects the **documented control-plane boundaries**.

### 3.1 Questions to Ask

For each file / module:

- Does `decision-gate-core` own gate evaluation, evidence anchoring, decision records, and runpack artifacts?
- Does `ret-logic` stay domain-agnostic (no provider semantics, no disclosure policy, no Decision Gate state)?
- Is `decision-gate-mcp` limited to transport/serialization and tool routing (no policy logic or hidden state)?
- Do providers only acquire evidence and return `EvidenceResult` data, leaving disclosure policy to the core?
- Are schema or contract changes centralized in `decision-gate-contract` with regenerated outputs under `Docs/generated/decision-gate`?
- Is persistent storage treated as untrusted input with hash verification and fail-closed behavior?

### 3.2 When to Change vs Flag

- **Change directly** if:
  - A module is trivially doing work that clearly belongs to another subsystem, and moving it is local and low-risk.
  - An obvious invariant check is missing in the core decision/evidence path, and the fix is straightforward.

- **Flag for human review** if:
  - Fixing the issue would require touching multiple crates or changing public contracts.
  - You are unsure how a change would impact external providers or tooling.

Document flagged items in the summary under a **“Boundary / architecture concerns”** heading.

---

## 4. Determinism, Replay, and Runpack Integrity

Decision Gate’s value comes from deterministic evaluation and offline verification. Check for:

- Nondeterminism in evaluation or artifact generation (RNGs, wall-clock time, unordered map iteration without ordering).
- Run state mutations that are not replayable or idempotent.
- Runpack artifacts or hashes that can vary across runs for identical inputs.
- Evidence anchors that are recomputed instead of recorded and verified.
- Changes that would break offline verification without a clear migration path.

If a change affects determinism, add or update determinism and replay tests.

---

## 5. Evidence, Disclosure, and Provider Safety

- Treat all evidence as hostile input; validate shape, bounds, and sizes.
- Fail closed on missing or invalid evidence; do not “default allow”.
- Respect disclosure policy: raw `evidence_query` values are denied by default and must not leak via logs or errors.
- Signature verification (when configured) must be strict; missing or untrusted signatures hold/deny.
- Avoid provider-specific logic in the core; keep provider semantics inside the provider crates.

---

## 6. Type & Modeling Hygiene

This is where issues like “strings instead of enums” or “raw integers instead of newtypes” live.

### 6.1 What to Look For

- Repeated string literals that obviously encode a finite state or mode.
- Raw `u32`/`u64` IDs used in places where a newtype already exists elsewhere in the codebase.
- Ad-hoc maps keyed by `String` in internal logic where an enum or typed key would be better.
- Structs exposing loosely typed fields where a stronger type exists.

### 6.2 When to Introduce/Use Enums or Newtypes

You **may** refactor to enums or newtypes when all of the following are true:

1. The set of values is clearly finite and stable.
2. The usage is internal to the crate or module (i.e., not part of a published external schema or protocol).
3. The change can be kept **local and mechanical**:
   - Replace string/int usages with enum/newtype.
   - Update pattern matches and tests accordingly.
   - No cross-crate architectural changes required.

You **must not** introduce new enums/newtypes when:

- The data is directly bound to an external protocol (MCP tools, provider protocol, JSON/RON schema) where `String` is an explicit part of the contract.
- The change would require simultaneous schema migrations or public API changes without explicit approval.

In those cases, prefer to:

- Keep the boundary type as `String`.
- Introduce an internal enum/newtype that you convert to/from at the boundary.

### 6.3 Practical Rules

- Prefer **existing** enums and newtypes over creating new ones. Search the repo before inventing a new type.
- If you add a new type:
  - Follow naming, constructor, and doc patterns from `Docs/standards/codebase_formatting_standards.md`.
  - Document invariants explicitly.
- If you change schemas or contract types, update `decision-gate-contract` and regenerate outputs in `Docs/generated/decision-gate`.
- Default to `fn`. Promote to `const fn` only for compile-time use or when you want a strict purity/const-evaluable contract; do not contort APIs just to force `const`.

---

## 7. Performance & Allocation Hygiene

The project has clear performance ambitions. Your job is to spot **violations that matter**, not to micro-optimize everything.

### 7.1 What to Look For

In hot paths (as defined by `AGENTS.md`/README):

- Unnecessary `.clone()` calls inside tight loops.
- Unnecessary `String` allocations in evaluation paths.
- Maps or sets with string keys in per-evaluation code.
- Temporary allocations that can be avoided by reusing buffers or taking references.

In non-hot paths (CLI, setup code, provider I/O):

- Sanity-check clones and allocations, but prioritize **clarity** over micro-optimizations.

### 7.2 When It Is Safe to Remove a Clone

You may remove or avoid a `.clone()` if:

1. The value can be borrowed for the required lifetime without tangling lifetimes or making code significantly harder to read.
2. You are not breaking ownership expectations (e.g., moving out of something still used later).
3. The change does not force major structural changes like introducing `Arc`/`Rc` or complex lifetimes.

If removing `.clone()` makes the code materially harder to understand, consider **leaving it** and simply flagging it as a potential optimization for a targeted perf pass.

---

## 8. Documentation, Comments, and Formatting

Drift between documentation and code is a common source of confusion, especially for LLMs.

### 8.1 Documentation Drift

For each public type/function:

- Check if the doc comments reflect the actual signature and behavior.
- Fix:
  - Changed parameter names or semantics that are not reflected in docs.
  - Invariants that no longer hold.
  - References to types or functions that no longer exist (update or remove).

Use validated intra-doc links as required by `Docs/standards/codebase_formatting_standards.md` and workspace lints.

### 8.2 File Headers and Sections

For each file you touch:

- Ensure it has:
  - Path comment and metadata block.
  - `## Overview` docblock explaining purpose and invariants.
  - `// SECTION:` banners for major parts.
- Do not introduce new ad-hoc section styles; follow the established standard.

### 8.3 Tests and Examples

- When you change behavior or fix a bug, add or update tests.
- When you adjust a modeling choice (e.g., new enum), update examples and doctests to match.
- Do not add tests that rely on unspecified behavior or internals; test via the public/intentional API.

---

## 9. How to Decide: Fix vs Flag

Every potential issue should be triaged into one of three categories.

### 9.1 Fix Now (Small, Local, Obvious)

Fix immediately when:

- The change is local to a file or small set of files.
- It is a clear correctness bug or clear violation of a documented rule.
- The behavioral impact is well-understood and covered by tests.
- The change does not alter public APIs in a breaking way.

Examples:

- Incorrect bounds check.
- Decision records missing a required field.
- Comment claims idempotence but a replay function is not setter-only, and a small local fix makes it so.
- Unnecessary clone in a clearly hot loop which can be trivially avoided.

### 9.2 Flag for Follow-Up

Do not change, but document under a “Follow-up recommendations” heading, when:

- The problem spans multiple crates or subsystems.
- Fixing it would require architectural decisions (e.g., changes to provider protocol or runpack format).
- You are unsure whether a behavior is contractual for external callers.

Include:

- File(s) and symbol(s) involved.
- A short description of the concern.
- A rough suggestion of what a future change might do.

### 9.3 Ignore for Now

Explicitly **ignore** (i.e., do not even flag) when:

- The code is stable and consistent with documented design, even if you would have written it differently.
- The change would be purely stylistic and high-churn.
- The benefit is marginal and would add complexity.

---

## 10. Output Expectations for an Investigation Pass

Whether you are creating a PR or just a report, your output should include:

1. **Scope description**
   - Which crate/folder did you inspect?
   - Which major modules/files?

2. **Summary of changes**
   - Short bullet list of what you actually modified (code, tests, docs).

3. **Issue inventory**
   - **Fixed issues**
     - Bullet list of individual fixes with 1–2 sentences of rationale.
   - **Flagged issues**
     - Bullet list of items you chose not to change, with suggested next steps.

4. **Threat model delta**
   - Note `Threat Model Delta: none` or describe the change and where it was updated.

5. **Confidence**
   - For any non-trivial change, briefly state your confidence level (e.g., “high: behavior identical, test coverage updated”).

This structure ensures human reviewers can quickly understand what you did and why, and lets future agents build on your work.

---

## 11. Quick Checklist (TL;DR for Agents)

Before you start:

- [ ] Read root + crate `AGENTS.md` (if present).
- [ ] Read `Docs/standards/codebase_formatting_standards.md`.
- [ ] Read `Docs/standards/codebase_engineering_standards.md`.
- [ ] Read `Docs/security/threat_model.md` (relevant boundary sections).
- [ ] Skim relevant README(s) and `Docs/guides/security_guide.md` if working on MCP/providers/storage.

During investigation:

- [ ] Respect control-plane boundaries (core vs ret-logic vs MCP vs providers).
- [ ] Keep `ret-logic` domain-agnostic.
- [ ] Keep MCP transport-only; no policy logic.
- [ ] Fail closed on missing/invalid evidence.
- [ ] Avoid allocations/clones on hot paths unless justified.
- [ ] Keep public contracts stable unless clearly fixing a bug.
- [ ] Update or fix docs/comments when they drift from code.
- [ ] Add/update tests for any behavioral change.
- [ ] Default to `fn`; promote to `const fn` only when needed.
- [ ] Update `Docs/security/threat_model.md` if inputs, boundaries, or security posture change (or note `Threat Model Delta: none`).

After investigation:

- [ ] Summarize what you inspected.
- [ ] List fixes with rationale.
- [ ] List flagged items for human review or future passes.
