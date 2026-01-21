# NASA-Grade Engineering Standards

**Status:** Authoritative. This document supersedes prior ad-hoc guidance.
**Scope:** Entire repository (all crates), production code only. Tests are governed by a relaxed variant (see Section 9).

---

## 0. Security Posture & Threat Model

**Status:** Foundational. This section defines the security mindset that governs all engineering decisions.

This codebase assumes deployment in adversarial, high-stakes environments. Every design decision, code path, and API must reflect this reality.

### 0.1 Target Client Posture

We build for the most demanding customers on the planet:

- **Hyperscalers & Frontier AI Labs** (FAANG-grade security review, red teams, bug bounties)
- **Defense & Government** (DoD, FedRAMP, ITAR, classified-adjacent workloads)
- **Regulated Industries** (finance, healthcare, critical infrastructure)

If a feature wouldn't survive a security review at these organizations, it doesn't ship.

### 0.2 Threat Model

Assume adversaries with nation-state-level capabilities:

- **Inputs are hostile** -- Every external input is malicious until validated
- **Boundaries are attack surfaces** -- FFI, serialization, HTTP, IPC are all adversary-controlled
- **Insiders exist** -- Defense-in-depth; no single layer is trusted
- **Time is a weapon** -- Timing attacks, race conditions, and resource exhaustion are in-scope

### 0.2.1 Threat Model Alignment (Required)

`Docs/security/threat_model.md` is the authoritative threat model. Update it when a change introduces or expands:

- Inputs or parsing surfaces
- Trust boundaries or privilege transitions
- New persisted state or data classes
- New authn/authz flows or access control decisions
- New external integrations or outbound data flows
- New resource exhaustion risks or fail-open behavior

If no update is needed, explicitly note `Threat Model Delta: none` with a brief rationale in your change summary or PR notes.

### 0.3 Zero Trust Architecture

**Never trust. Always verify.** This applies at every layer:

| Boundary | Zero Trust Requirement |
|----------|------------------------|
| Network | All requests authenticated; no ambient authority |
| Service-to-service | mTLS or equivalent; no implicit trust between daemons |
| Function calls | Validate inputs even from "trusted" internal callers |
| Data at rest | Crypto envelope validation (integrity/authenticity/encryption + chain hash); assume storage is compromised |
| Data in transit | Encryption mandatory; assume network is hostile |
| User sessions | Short-lived tokens; re-authenticate on privilege escalation |

**Fail closed, never fail open.** When validation fails, deny access. When parsing fails, reject the input. When authorization is ambiguous, deny.

### 0.4 Engineering Implication

LLMs dramatically reduce the cost of doing things *correctly*. There is no longer any excuse for:

- "Expedient" implementations that defer security to "later"
- StackOverflow copy-paste that collapses under adversarial input
- "Happy path only" code that doesn't handle malicious cases
- Implicit trust assumptions that bypass validation

**Build every feature as if it will face adversarial scrutiny on day one.** Because it will.

### 0.5 Security Thinking Checklist

Before any code is considered complete, verify:

- [ ] **Inputs**: What untrusted data enters here? Is it all validated, bounded, and sanitized?
- [ ] **Failure modes**: What happens on error? Does it fail closed (deny) or open (allow)?
- [ ] **Trust boundaries**: Does this cross a trust boundary? Is re-validation performed?
- [ ] **Authorization**: Is access control enforced? Can it be bypassed?
- [ ] **Information leakage**: Do errors reveal internal state to attackers?
- [ ] **Resource exhaustion**: Can an attacker cause DoS via this path?
- [ ] **Audit trail**: Is the operation logged with sufficient context for forensics?
- [ ] **Threat model alignment**: Does this require an update to `Docs/security/threat_model.md`?

---

### 0.6 Config Input Hard Limits (Non-Configurable)

The config runtime enforces hard limits to prevent traversal, parsing abuse, and memory exhaustion.
These are **not** config knobs and therefore do **not** appear in `Docs/configuration/*.md`
or the generated schema outputs. Changes to these values must be documented here to avoid
surprise failures in production.

Current limits enforced by `assetcore-config-runtime/src/lib.rs`:

- `MAX_CONFIG_NESTING_DEPTH = 32` (max TOML/JSON nesting depth)
- `MAX_TOKEN_LENGTH = 16 * 1024` (max token length, bytes)
- `MAX_ENV_VAR_LENGTH = 64 * 1024` (max env var value length, bytes)
- `MAX_CONFIG_VALUE_COUNT = 10_000` (max parsed config values)
- `MAX_CONFIG_FILE_SIZE = 1024 * 1024` (max config file size, bytes)
- `MAX_PATH_COMPONENT_LENGTH = 255` (max single path component length)
- `MAX_TOTAL_PATH_LENGTH = 4096` (max total path length)

Related hard limits for auth token directories are enforced in daemon auth modules and are also
not exposed as config knobs:

- `daemon-read/src/auth/directory/mod.rs`: `MAX_TOKEN_FILE_BYTES`, `MAX_TOKEN_FILES`, `MAX_TOKEN_LEN`
- `daemon-write/src/auth/directory/mod.rs`: `MAX_TOKEN_FILE_BYTES`, `MAX_TOKEN_FILES`, `MAX_TOKEN_LEN`

---

## 1. Integer & ID Policy

This document defines the **single, canonical standard** for:

- Integer conversions (especially narrowing).
- ID types built on `NonZeroU16`, `NonZeroU32`, `NonZeroU64`.
- Index access patterns (`index()` methods).
- Commit log offset handling.

There are no alternatives or options in this document. This is the required behavior for "NASA-grade, hyperscaler-grade" correctness.

---

## 2. Global Invariants

1. **No panics in production code.**

   - Disallowed in all non-test code:

     - `panic!(...)`
     - `unwrap()` / `expect(...)` on `Result`/`Option`
     - `unreachable!()` / `todo!()` / `unimplemented!()`

   - Any such usage must be under `#[cfg(test)]` or in dedicated test modules.

2. **No unchecked narrowing conversions.**

   - Any conversion from a **wider** integer type to a **narrower** one (where truncation or wrap is possible) **must**:

     - Use `TryFrom` / `try_from`, or
     - Use a shared helper that returns a **typed error**.

   - Narrowing conversions must **never** use `as` in production code.

3. **Widening conversions are allowed but must be explicit.**

   - Narrower -> wider (e.g. `u16 -> u32`, `u32 -> u64`, `u16/u32 -> usize` on our targets) is safe.
   - Prefer `From`/`Into`/`T::from(...)` over `as` to avoid teaching LLMs bad habits.

4. **Clippy lints (production):**

   - `clippy::cast_possible_truncation = "deny"`
   - `clippy::unwrap_used          = "deny"`
   - `clippy::expect_used          = "deny"`
   - `panic                        = "deny"`

5. **Typed errors over implicit failure.**

   - Any real failure mode must be surfaced as `Result<_, DomainError>` (e.g. `AssetError::IndexOverflow`, `CommitLogError::OffsetExceedsAddressSpace`).
   - Silent wrap, silent truncation, and implicit panics are forbidden.

### 2.1 Fixed-Point Numeric Policy (Deterministic Geometry)

This policy governs continuous geometry and any fixed-point numeric representation.

- No floats in core runtime, storage, or event payloads. Float conversion is allowed only at SDK/adapter edges.
- Quantization uses round-to-nearest, ties-to-even. Dequantization uses the same rule. Alternative rounding requires a design note.
- Scale limits are explicit: `abs(real) * quantization_inv <= i32::MAX`. Bounds/width/height must fit in i32 after quantization.
- All arithmetic that can overflow i32 uses i64 intermediates with checked casts; overflow yields a typed error.
- No saturation, no wrap, no lossy cast. Fail closed on overflow or out-of-range values.
- Deterministic trig only: no runtime libm in core. Use a checked-in table (Q1.30, millidegree) or a documented deterministic equivalent with test vectors.
- Replay uses stored fixed-point post-state values only; no recompute on replay.

---

### 2.2 Deterministic Replay Checklist

Use this checklist for any event-sourced or replayed state:

- Events carry all post-state fields needed to restore state without recomputation.
- Replay applies setters only (no arithmetic, no diffs, no incremental logic).
- Derived indexes rebuild deterministically from stored post-state in a fixed order.
- No floats, no hashing, no randomized iteration order in replay paths.
- Replaying the same event sequence N times yields identical bytes in storage.
- Schema version changes include explicit migration notes and replay expectations.

---

## 3. Ownership, Borrowing, and Cloning Discipline

The codebase follows a **zero-copy-by-default philosophy**. All contributors and LLM agents must treat ownership, borrowing, and data movement as first-class architectural concerns. In production code, cloning is exceptional and must never be used as a convenience mechanism to satisfy the borrow checker. Any `.clone()`, `.to_owned()`, or similar duplication requires a clear semantic justification (e.g., creating an intentional snapshot, transferring ownership across threads, or isolating mutation boundaries).

The workspace lint posture (`clippy::all = "deny"`) already forbids most classes of accidental copying, including implicit clones, redundant clones, cloning of `Copy` types, and unnecessary `to_owned` allocations. These rules are part of our performance and correctness guarantees: no silent allocation, no hidden data duplication, and no degradation of hot paths through casual cloning.

**Permitted use:** Cloning is allowed when semantically correct and intentionally chosen, such as cheap bitwise copies of `Copy` types, startup-time duplication of configuration objects, or creating a deliberate owned value for async tasks. When cloning is appropriate, it should be explicit and minimally scoped.

**Prohibited use:** Cloning solely to resolve borrow-checker errors, cloning large structures unnecessarily, cloning inside hot loops, cloning for temporary access where borrowing would suffice, or cloning via hidden mechanisms (`to_owned`, `to_string`, implicit iterator cloning) without justification.

This discipline ensures consistent ownership semantics, eliminates accidental performance regressions, and maintains the architectural integrity of the engine's dataflow model. Contributors should assume that any clone introduced in production code will be scrutinized; where duplication is necessary, document the rationale in code comments or API documentation.

---

## 4. Integer Conversion Rules

We distinguish three categories.

### 4.1 Widening (always allowed, prefer `From`)

Examples:

- `u16 -> u32`, `u16 -> u64`
- `u32 -> u64`
- `u16 -> usize`, `u32 -> usize` on 32/64-bit platforms

Policy:

- Use `T::from(x)` or `x.into()` where possible.
- Avoid `as` in production code even if it is technically safe, to avoid reintroducing unsafe patterns via LLMs.

Example (ID index for 32-bit IDs):

```rust
pub const fn index(self) -> usize {
    usize::from(self.0.get().saturating_sub(1))
}
```

### 4.2 Narrowing (must be checked)

Examples:

- `usize -> u32` / `usize -> u16`
- `u64 -> u32` / `u64 -> u16` / `u64 -> usize`

Policy:

- Use `u32::try_from(...)`, `u16::try_from(...)`, `usize::try_from(...)` and map errors to a **domain error** (`IndexOverflow`, `OffsetExceedsAddressSpace`, etc.).
- No `as` casts for these cases in production code.

Example (index -> ID):

```rust
pub fn try_from_index(index: usize) -> Result<Self, CapabilityError> {
    let idx = u32::try_from(index).map_err(|_| CapabilityError::IndexOverflow)?;
    let val = idx.checked_add(1).ok_or(CapabilityError::IndexOverflow)?;
    let nz = NonZeroU32::new(val).ok_or(CapabilityError::IndexOverflow)?;
    Ok(Self(nz))
}
```

### 4.3 Constants

- Primary sizing constants should be declared in the type used for indexing/buffer sizing (usually `usize`).
- Any `u64` representation needed for on-disk formats should be derived via widening:

```rust
pub const FILE_HEADER_SIZE: usize = 32;
pub const FILE_HEADER_SIZE_U64: u64 = FILE_HEADER_SIZE as u64; // widening
```

There must be **no** `u64` constants that are then cast down to `usize` with `as`.

---

## 5. ID Types (NonZero-backed)

We distinguish three families:

1. 16-bit IDs: `NonZeroU16` internally.
2. 32-bit IDs: `NonZeroU32` internally.
3. 64-bit IDs: `NonZeroU64` internally.

Each family has a standard shape.

Default to `fn`. Promote to `const fn` only when you need compile-time use (const/static/const generics) or when you want the stronger contract that the function is pure and const-evaluable to enforce invariants. Do not contort APIs, introduce `unsafe`, or block future evolution just to make a function `const`.

### 5.1 32-bit IDs (e.g., `ClassId`, `ContainerId`, `CapabilityId`, `TargetSetId`)

Canonical pattern (example: `CapabilityId`):

```rust
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd, Ord)]
pub struct CapabilityId(NonZeroU32);

impl CapabilityId {
    /// Creates from already validated non-zero.
    #[must_use]
    pub const fn from_raw_u32(raw: NonZeroU32) -> Self {
        Self(raw)
    }

    /// Creates from raw u32 (e.g. FFI). Zero is rejected.
    pub fn from_raw(v: u32) -> Option<Self> {
        NonZeroU32::new(v).map(Self)
    }

    /// Returns the raw 1-based value.
    #[must_use]
    pub const fn get(self) -> u32 {
        self.0.get()
    }

    /// Returns the 0-based dense index.
    ///
    /// This is infallible and widening (u32 -> usize).
    #[must_use]
    pub const fn index(self) -> usize {
        usize::from(self.0.get().saturating_sub(1))
    }

    /// Creates from a 0-based index (fallible).
    pub fn try_from_index(index: usize) -> Result<Self, CapabilityError> {
        let idx = u32::try_from(index).map_err(|_| CapabilityError::IndexOverflow)?;
        let val = idx.checked_add(1).ok_or(CapabilityError::IndexOverflow)?;
        let nz = NonZeroU32::new(val).ok_or(CapabilityError::IndexOverflow)?;
        Ok(Self(nz))
    }

    /// Test-only helper: panics on overflow.
    #[cfg(test)]
    pub fn from_index(index: usize) -> Self {
        Self::try_from_index(index).expect("CapabilityId index overflow")
    }
}
```

**Rules:**

- `index()` **always returns `usize` and is infallible** for 16/32-bit IDs.
- All narrowing (`usize -> u32`) happens in `try_from_index` and returns a typed error.
- Any panic-based helper (`from_index`) is `#[cfg(test)]` only.

### 5.2 16-bit IDs (e.g., `FieldId`, `FlagId`, `PredId`, `ResourceId`)

Identical shape, with `u16`/`NonZeroU16` and the same invariants:

```rust
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd, Ord)]
pub struct FieldId(NonZeroU16);

impl FieldId {
    #[must_use]
    pub const fn from_raw_u16(raw: NonZeroU16) -> Self {
        Self(raw)
    }

    pub fn from_raw(v: u16) -> Option<Self> {
        NonZeroU16::new(v).map(Self)
    }

    #[must_use]
    pub const fn get(self) -> u16 {
        self.0.get()
    }

    #[must_use]
    pub const fn index(self) -> usize {
        usize::from(self.0.get().saturating_sub(1))
    }

    pub fn try_from_index(index: usize) -> Result<Self, CapabilityError> {
        let idx = u16::try_from(index).map_err(|_| CapabilityError::IndexOverflow)?;
        let val = idx.checked_add(1).ok_or(CapabilityError::IndexOverflow)?;
        let nz = NonZeroU16::new(val).ok_or(CapabilityError::IndexOverflow)?;
        Ok(Self(nz))
    }

    #[cfg(test)]
    pub fn from_index(index: usize) -> Self {
        Self::try_from_index(index).expect("FieldId index overflow")
    }
}
```

### 5.3 64-bit IDs (e.g., `StackId`, `InstId`)

For IDs that use `NonZeroU64` internally, `index()` can no longer be infallible, because `u64 -> usize` is narrowing on 32-bit and potentially on 64-bit given very large ranges.

Canonical pattern:

```rust
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd, Ord)]
pub struct StackId(NonZeroU64);

impl StackId {
    #[must_use]
    pub const fn from_raw_u64(raw: NonZeroU64) -> Self {
        Self(raw)
    }

    pub fn from_raw(v: u64) -> Option<Self> {
        NonZeroU64::new(v).map(Self)
    }

    #[must_use]
    pub const fn get(self) -> u64 {
        self.0.get()
    }

    /// Returns the 0-based dense index if it fits in usize.
    pub fn index(self) -> Result<usize, AssetError> {
        let raw = self.0.get().saturating_sub(1);
        let idx = usize::try_from(raw).map_err(|_| AssetError::IndexOverflow)?;
        Ok(idx)
    }

    pub fn try_from_index(index: usize) -> Result<Self, AssetError> {
        let raw = index_to_nonzero_u64(index)?; // shared helper
        Ok(Self(raw))
    }

    #[cfg(test)]
    pub fn index_unchecked(self) -> usize {
        self.index().expect("StackId index overflow in test")
    }
}
```

**Rules:**

- For 64-bit IDs, `index()` is **fallible** and returns `Result<usize, AssetError>`.
- Any code that needs a bare `usize` must choose explicitly how to handle overflow (propagate, map to a higher-level error, or, in tests only, panic).

---

## 6. Const Schema Helpers (`from_index_const`)

Some schema functions need **compile-time constant IDs** (e.g., `field_hp()`, `flag_dead()`, `capability_id_move()`). These must not introduce panics into production code.

### 6.1 Requirements

- No `expect`/`unwrap`/`panic!` inside `const fn` used in production.
- `from_index_const` and similar APIs are **only** for small, hard-coded indices with trivially verified invariants.

### 6.2 Pattern

For a `NonZeroU32` ID:

```rust
impl CapabilityId {
    /// Creates a [`CapabilityId`] from a compile-time constant index.
    ///
    /// # Safety
    /// Caller must ensure `index + 1` is non-zero and fits in `u32`.
    /// This is intended only for hardcoded schema constants.
    #[doc(hidden)]
    pub const fn from_index_const(index: u32) -> Self {
        // SAFETY: For schema constants we only ever pass small indices (e.g., 0, 1, 2).
        // Thus index + 1 is always in 1..=u32::MAX and cannot be zero.
        let raw = unsafe { NonZeroU32::new_unchecked(index + 1) };
        Self::from_raw_u32(raw)
    }
}
```

For `NonZeroU16` IDs, use the analogous `NonZeroU16::new_unchecked` pattern.

You may alternatively inline this unsafe block directly in the schema functions instead of exposing `from_index_const`. The critical point is:

- **No panics in production.**
- `unsafe` is allowed here, but must be tightly scoped and justified with a `SAFETY` comment.

---

## 7. Commit Log: Offsets and Indexing

Commit log subsystems deal heavily with `u64` offsets (file positions, segment offsets, logical commit log offsets). All conversions to `usize` for buffer indexing or `Vec` indexing must be **checked and explicit**.

### 7.1 Helper for offset -> index

Define a shared helper in the commit-log module:

```rust
#[inline]
fn offset_to_index(offset: u64) -> Result<usize, CommitLogError> {
    usize::try_from(offset).map_err(|_| CommitLogError::OffsetExceedsAddressSpace)
}
```

### 7.2 Usage

Replace **all** `u64 as usize` conversions of offsets in:

- `commitlog/inmemory.rs`
- `commitlog/file/format.rs`
- `commitlog/file/flusher.rs`
- `commitlog/file/reader.rs`
- `commitlog/file/mod.rs`
- `commitlog/mmap/segment.rs`
- `commitlog/mmap/reader.rs`

with:

```rust
let index = offset_to_index(file_offset)?;
```

and propagate `CommitLogError::OffsetExceedsAddressSpace` up the call chain.

This makes the 32-bit limitation explicit and guarantees that we never silently wrap or truncate offsets when moving into `usize` space.

### 7.3 Constants in the commit log

- Header and entry size constants should be declared as `usize` and, if needed, mirrored as `u64` via widening.
- There should be **no** `u64` constants that are later cast down to `usize` with `as`.

---

## 8. Clippy & Lint Posture

Once the above patterns are implemented, the following will be true for production code:

- No `unwrap`, `expect`, or `panic!` anywhere.
- No `as` for narrowing conversions; all such cases go through `try_from` + typed errors or shared helpers.
- `cast_possible_truncation`, `unwrap_used`, `expect_used`, and `panic` can be safely set to `deny`.

Procedurally:

1. Complete the conversion of all ID types to the patterns in Section 5.
2. Complete the commit-log conversion to `offset_to_index(...)` and eliminate all `u64 as usize` in production.
3. Normalize constants to be `usize`-first with widening to `u64` where needed.
4. Then enforce:

   - `cargo clippy --workspace --all-targets --all-features -D warnings`

---

## 9. Tests and Test-Only Helpers

Tests operate under a **relaxed** version of these rules:

- Panics (`unwrap`, `expect`, `panic!`) are **allowed in tests** and in `#[cfg(test)]` helpers, but must not be exposed in production modules.
- It is acceptable to have `_unchecked` or `from_index` helpers under `#[cfg(test)]` that wrap the fallible APIs and panic on failure.

Examples:

```rust
#[cfg(test)]
impl CapabilityId {
    pub fn index_unchecked(self) -> usize {
        self.index() // for 32-bit IDs this is infallible anyway
    }
}

#[cfg(test)]
impl StackId {
    pub fn index_unchecked(self) -> usize {
        self.index().expect("StackId index overflow in test")
    }
}
```

These helpers **must not** be available in non-test builds.

---

## 10. Migration Notes for `capability/src/ids.rs`

The current version of `capability/src/ids.rs` violates this policy in several ways:

- `from_index_const` uses `NonZeroU16::new(...).expect(...)` / `NonZeroU32::new(...).expect(...)` in production.
- `index()` returns `u16`/`u32` instead of `usize`.
- Some conversions are not using the `ContainerId` pattern.

Required changes:

1. Change all `index()` methods to:

   - Return `usize`.
   - Use `usize::from(self.0.get().saturating_sub(1))`.

2. Keep all `try_from_index` methods fallible and returning `CapabilityError::IndexOverflow`, using `u16::try_from` / `u32::try_from` and `NonZeroU16::new` / `NonZeroU32::new`.

3. Replace all `from_index_const` implementations with the `new_unchecked` pattern described in Section 6, or remove them and inline the pattern in schema helpers. There must be **no** `expect` in production code.

4. Add or retain `#[cfg(test)]` panic wrappers only where needed for ergonomic tests.

Once this file is migrated, it should look structurally identical to the `ContainerId` / `ClassId` patterns described above.

---

## 11. Summary

- Production code must be **panic-free** and **free of unchecked narrowing**.
- Fixed-point geometry uses deterministic rounding, checked overflow, and deterministic trig; no floats in core.
- All ID types follow a small set of **canonical shapes**, with:

  - Infallible, widening `index() -> usize` for 16/32-bit IDs.
  - Fallible `index() -> Result<usize, Error>` for 64-bit IDs.
  - Fallible `try_from_index` for all IDs, returning typed overflow errors.

- Commit-log subsystems use a shared `offset_to_index` helper to convert `u64` offsets into `usize` safely and explicitly.
- Const schema IDs use small, well-documented `unsafe` blocks (`new_unchecked`) rather than panics.
- Tests are allowed to use panic-based helpers under `#[cfg(test)]`, but these must not leak into production.

This document is the single point of truth for integer, ID, and fixed-point numeric handling going forward. Any deviation in the codebase should be considered a bug and corrected to match these patterns.
