# Engineering Standards

**Status:** Authoritative. This document supersedes prior ad-hoc guidance.
**Scope:** Entire repository, production code. Tests follow the relaxed rules in Section 9.

---

## 0. Security Posture and Threat Model

This repository assumes adversarial, high-stakes deployment. Every design and code path must be defensible under hostile input. Assume advanced, well-resourced adversaries with sophisticated tooling and long-horizon persistence.

### 0.1 Target Posture

We build for the strictest environments:
- Hyperscalers and frontier labs.
- Defense and government.
- Regulated industries.

If a feature would not survive a security review in those environments, it does not ship.

### 0.2 Threat Model Alignment

`Docs/security/threat_model.md` is authoritative. Update it when a change introduces or expands:
- New input or parsing surface.
- New trust boundary or privilege transition.
- New persisted data class.
- New authn or authz flow.
- New external integration or outbound data flow.
- New resource exhaustion risk or any fail-open behavior.

If no update is needed, record `Threat Model Delta: none` in the change summary.

### 0.3 Zero Trust Requirements

- All external inputs are hostile until validated.
- All boundaries are attack surfaces, including serialization, file I/O, IPC, and HTTP.
- Insiders and compromised storage are in-scope.
- Fail closed. Ambiguity or parse errors deny, never allow.

### 0.4 Hard Limits

Any hard limit enforced in code must be documented where it is enforced and referenced from the threat model. Do not hide hard limits behind configuration defaults.

---

## 1. Determinism and Runpack Integrity

Decision Gate is a deterministic control plane. The same inputs must yield the same outputs.

### 1.1 Determinism Contract

- Canonical ScenarioSpec hashing is the source of truth for spec identity.
- Given the same ScenarioSpec and the same EvidenceResult inputs, evaluations produce identical DecisionRecord outputs.
- Runpack manifests and artifact hashes are stable across platforms for the same inputs.

### 1.2 Prohibited Sources of Nondeterminism

- Wall clock time in evaluation paths.
- RNG or randomized ordering without an explicit, deterministic seed and ordering step.
- HashMap or Set iteration without ordering when results are serialized or hashed.
- Floating point arithmetic in deterministic core evaluation or hashing paths without a determinism note and tests.

### 1.3 Runpack Requirements

- Runpacks are tamper-evident bundles. Every artifact is hashed, and the manifest captures those hashes.
- Evidence anchors are recorded and verified, not recomputed during verification.
- Verification must fail closed on any missing, unreadable, or mismatched artifact.

### 1.4 Replay and Verification

- Offline verification must reproduce the same decisions or fail closed.
- Replay uses stored post-state data, never live recomputation from external sources.
- Any change that could alter canonical hashing or manifest layout requires versioned migration notes.

---

## 2. Architecture Boundaries

Respect the Decision Gate boundaries. If a change crosses a boundary, document it and update architecture docs.

- `decision-gate-core` owns evaluation, evidence anchoring, decision records, and runpack generation.
- `ret-logic` remains domain-agnostic and must not depend on Decision Gate state or provider semantics.
- `decision-gate-mcp` is transport, serialization, and tool routing only. No policy logic.
- Provider crates only acquire evidence and return EvidenceResult. Disclosure policy belongs to core.
- `decision-gate-contract` is the single source of truth for schema and tool contracts.
- SDKs implement client convenience and validation. They must not encode policy or core rules.
- Stores treat persisted state as untrusted input and must verify integrity before use.
- CLI and broker orchestrate calls. They do not own core policy.

---

## 3. Input Validation and Fail-Closed Defaults

- All evidence, configuration, runpack artifacts, and MCP payloads are untrusted.
- Validate shape, bounds, and sizes before use.
- Missing or invalid evidence yields `unknown` or `deny`, not `allow`.
- Parsing and verification errors must not fall back to permissive behavior.

---

## 4. Error Handling and Panic Policy

- No panics in production code. `panic!`, `unwrap`, `expect`, `todo`, `unimplemented`, and `unreachable` are test-only.
- All real failure modes must surface as typed errors.
- Avoid `as` for narrowing conversions. Use `try_from` and map to domain errors.

---

## 5. Numeric Conversions and ID Patterns

Decision Gate frequently crosses boundaries such as JSON, MCP, and storage. Numeric conversion must be explicit and safe.

### 5.1 Narrowing Conversions

- Any conversion that can truncate must use `TryFrom` or an equivalent checked helper.
- All overflow cases return a typed error.

### 5.2 Example Patterns

Example 32-bit ID wrapper:

```rust
// DomainError is a placeholder for the crate's typed error enum.
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Id32(NonZeroU32);

impl Id32 {
    pub const fn from_raw_u32(raw: NonZeroU32) -> Self {
        Self(raw)
    }

    pub fn from_raw(v: u32) -> Option<Self> {
        NonZeroU32::new(v).map(Self)
    }

    pub const fn get(self) -> u32 {
        self.0.get()
    }

    pub const fn index(self) -> usize {
        usize::from(self.0.get().saturating_sub(1))
    }

    pub fn try_from_index(index: usize) -> Result<Self, DomainError> {
        let idx = u32::try_from(index).map_err(|_| DomainError::IndexOverflow)?;
        let val = idx.checked_add(1).ok_or(DomainError::IndexOverflow)?;
        let nz = NonZeroU32::new(val).ok_or(DomainError::IndexOverflow)?;
        Ok(Self(nz))
    }
}
```

Example 64-bit ID wrapper:

```rust
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Id64(NonZeroU64);

impl Id64 {
    pub const fn from_raw_u64(raw: NonZeroU64) -> Self {
        Self(raw)
    }

    pub fn from_raw(v: u64) -> Option<Self> {
        NonZeroU64::new(v).map(Self)
    }

    pub const fn get(self) -> u64 {
        self.0.get()
    }

    pub fn index(self) -> Result<usize, DomainError> {
        let raw = self.0.get().saturating_sub(1);
        usize::try_from(raw).map_err(|_| DomainError::IndexOverflow)
    }

    pub fn try_from_index(index: usize) -> Result<Self, DomainError> {
        let idx = u64::try_from(index).map_err(|_| DomainError::IndexOverflow)?;
        let val = idx.checked_add(1).ok_or(DomainError::IndexOverflow)?;
        let nz = NonZeroU64::new(val).ok_or(DomainError::IndexOverflow)?;
        Ok(Self(nz))
    }
}
```

---

## 6. Ownership and Cloning Discipline

- Prefer borrowing and zero-copy paths in hot code.
- Cloning is allowed only when it has a clear semantic reason.
- Cloning to satisfy the borrow checker is not acceptable.

---

## 7. Lints and Tooling

- Use the workspace lint settings in `Cargo.toml`.
- Do not weaken lints in new crates or modules.
- Any `#[allow]` must be narrowly scoped with a rationale comment.

---

## 8. Tests and Validation

Changes must be compatible with the standard toolchain and test suite:
- `cargo +nightly fmt --all`.
- `cargo clippy --all-targets --all-features -- -D warnings`.
- `cargo nextest run`.
- `cargo nextest run -p system-tests --features system-tests`.

---

## 9. Test-Only Relaxations

Tests may use panics and unwraps when it improves clarity. Production code must not.

---

## 10. Summary

These standards are non-negotiable for Decision Gate:
- Determinism and runpack integrity are first-class requirements.
- All inputs are hostile; fail closed.
- No unchecked narrowing conversions, no production panics.
- Boundaries are enforced. Policy lives in core, transport remains transport.
