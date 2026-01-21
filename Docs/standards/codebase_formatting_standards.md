# Codebase Formatting & Documentation Standards

This document defines **universal formatting, structure, and documentation expectations** for all source files across the codebase. It ensures every file—regardless of language or purpose—is consistent, self-contained, and easily navigable by both humans and LLMs.

## Table of Contents

1. [File Header](#1-file-header)
2. [File Overview Block](#2-file-overview-block)
3. [Sectioning and Headings](#3-sectioning-and-headings)
4. [Function and Type Documentation](#4-function-and-type-documentation)
5. [Cross-References and Navigation Links](#5-cross-references-and-navigation-links)
6. [Doctests and Examples](#6-doctests-and-examples)
7. [File Index (Optional for Long Files)](#7-file-index-optional-for-long-files)
8. [Language-Agnostic Adaptations](#8-language-agnostic-adaptations)
9. [Imports and Dependency Declarations](#9-imports-and-dependency-declarations)
10. [Constants and Globals](#10-constants-and-globals)
11. [Summary of Key Rules](#11-summary-of-key-rules)
12. [Enforcement and CI Integration](#12-enforcement-and-ci-integration)
13. [Code Quality Standards (Clippy)](#13-code-quality-standards-clippy)
14. [Numeric Conversions and Narrowing Policy](#14-numeric-conversions-and-narrowing-policy)
15. [Optional Enhancements (Future-proofing)](#15-optional-enhancements-future-proofing)
16. [Character Set and Encoding](#16-character-set-and-encoding)

---

## 1. File Header

Each file must begin with a **path comment** followed by a brief metadata block that establishes context.

### Example

```rust
// assetcore-types/src/types.rs
// ============================================================================
// Module: Core Type Definitions
// Description: Foundational opaque identifiers, shapes, and metadata types.
// Purpose: Define universally shared structural primitives used throughout the system.
// Dependencies: None (self-contained)
// ============================================================================
```

### Rules

- Always include **relative file path** (from project root). This ensures LLMs and readers know exactly where the file belongs.
- Include **one-line summaries** for:
  - **Module name** – what the file conceptually represents.
  - **Description** – what the file defines.
  - **Purpose** – why it exists (the motivation or responsibility).
  - **Dependencies** – if the file intentionally avoids or requires other modules.
- Avoid decorative ASCII art banners except the standardized `// ============================================================================` style for clear sectioning.

---

## 2. File Overview Block

Immediately below the header comment, add a **file-level documentation block** describing the file's conceptual role, invariants, and relationship to other layers.

### Example

```rust
//! ## Overview
//! This module defines the canonical structural types used across all subsystems.
//! Each type enforces strict invariants such as:
//! - 1-based identifiers (no zero values)
//! - Sentinel-free design using `Option<NonZero*>`
//! - Clear FFI mappings for all boundary conversions
//!
//! These types form the foundation for serialization, ECS integration, and API schemas.
```

### Rules

- Begin with `//! ## Overview` or equivalent for other languages (`/** ... */` for JS/TS, `///` for Go, etc.).
- Explain **what this file contributes to the system**, not implementation details.
- Mention **key invariants** or guiding principles (e.g., sentinel-free, stateless, thread-safe, etc.).
- For foundational files, mention **relationship to higher layers** ("Used by World Engine serialization and ECS storage").
- For files on trust boundaries or handling untrusted inputs, add a one-line **Security posture** note that references `Docs/security/threat_model.md`.

---

## 3. Sectioning and Headings

Every file should use **consistent, scannable section banners**.

### Example

```rust
// ============================================================================
// SECTION: Identifiers (u32-based)
// ============================================================================
```

### Guidelines

- Use `// SECTION:` headings for major conceptual divisions.
- Keep all headings capitalized and clearly separated with blank lines.
- Avoid ad-hoc headings like `// ---` or `// ===` unless continuing a major banner block.
- Each section should ideally represent a logical grouping (e.g., "Core Types," "Helpers," "FFI Mappings").

---

## 4. Function and Type Documentation

### Constructors and Getters

Each `new(...)`, `from_raw(...)`, or similar constructor must:

- Accurately describe **its contract** (e.g., whether zero is allowed or not).
- Never claim to return `None` unless it actually can.
- Default to `fn`. Promote to `const fn` only when you need compile-time use or when you want the stronger contract that the function is pure and const-evaluable; do not add `unsafe` or contort APIs just to force `const`.

**Example (Correct):**

```rust
impl ClassId {
    /// Creates a [`ClassId`] from a known non-zero value.
    ///
    /// Use [`ClassId::from_raw`] when starting from a `u32` that may be zero.
    pub const fn new(id: NonZeroU32) -> Self { Self(id) }
}
```

### Invariants and Safety Contracts

Each struct or enum must explicitly declare its invariants.

**Example:**

```rust
/// Represents a 1-based identifier for class entities.
///
/// # Invariants
/// - Always `>= 1`.
/// - `0` is reserved for FFI sentinel and never valid in Rust.
/// - Safe for cross-thread usage; no interior mutability.
```

### Conversion Patterns

Uniformly define:

- `new(NonZero*) -> Self`
- `from_raw(primitive) -> Option<Self>`
- `get() -> primitive`
- `index() -> usize`
- `from_index(usize) -> Self`

Each pattern should use the **same doc comment phrasing** across all types to reinforce predictability.

---

## 5. Cross-References and Navigation Links

**CRITICAL:** All cross-references to other functions, types, modules, or layers **must** use validated backtick link syntax. This is enforced at the workspace level via Rust's built-in `rustdoc::broken_intra_doc_links` lint.

### Enforcement (Rust)

The workspace `Cargo.toml` must include:

```toml
[workspace.lints.rustdoc]
broken_intra_doc_links = "deny"          # Error on broken [`Type`] links
private_intra_doc_links = "warn"         # Warn when linking to private items
bare_urls = "warn"                       # Prefer [text](url) over raw URLs
```

This makes broken documentation links a **compile-time error** across all workspace crates.

### Link Syntax Reference

| What You're Linking  | Syntax                  | Example                        |
| -------------------- | ----------------------- | ------------------------------ |
| Type in same module  | `[`Type`]`              | `[`Runtime`]`                  |
| Type in other module | `[`module::Type`]`      | `[`events::Observer`]`         |
| Type from crate root | `[`crate::path::Type`]` | `[`crate::types::ClassId`]`    |
| Method               | `[`Type::method`]`      | `[`Runtime::commit`]`          |
| Associated function  | `[`Type::new`]`         | `[`Runtime::new`]`             |
| Trait                | `[`Trait`]`             | `[`TimeProvider`]`             |
| Trait method         | `[`Trait::method`]`     | `[`Observer::on_event`]`       |
| Module               | `[`module`]`            | `[`transaction`]`              |
| Enum variant         | `[`Enum::Variant`]`     | `[`AssetError::InvalidStack`]` |
| Function             | `[`function`]`          | `[`create_stack`]`             |

### Guidelines

**When to link:**

- Type names in documentation prose
- Function and method references
- Module names when discussing architecture
- Error types in Returns/Errors sections
- Trait bounds in generic documentation

**When NOT to link:**

- Common words that happen to match type names ("new", "get", "result")
- Types already linked in the same sentence
- Code examples (they're validated separately)
- User-facing strings or messages

**Path clarity:**

- Use **simple paths** for same-module items: `[`Type`]`
- Use **qualified paths** for other modules: `[`crate::module::Type`]`
- Use **full paths** when ambiguous: `[`crate::runtime::core::Transaction`]`

### Examples

**Bad (natural language, no validation):**

```rust
//! - **Called by:** Runtime layer after successful transaction commit
//! - **Uses:** The validation system to check constraints
//! - **Returns to:** Transaction coordinator
```

**Good (validated links):**

```rust
//! - **Called by:** [`Runtime::commit`] after successful transaction commit
//! - **Uses:** [`ValidationSystem::check_constraints`] to verify invariants
//! - **Returns to:** [`TransactionCoordinator::finalize`]
```

**Bad (over-linking):**

```rust
/// Creates a [`new`] [`Runtime`] using the given [`TimeProvider`] and returns a [`Result`].
```

**Good (appropriate linking):**

```rust
/// Creates a new [`Runtime`] using the given [`TimeProvider`].
```

**Bad (ambiguous path):**

```rust
/// Uses [`Transaction`] for state management
// Error: Multiple types named Transaction
```

**Good (qualified path):**

```rust
/// Uses [`crate::runtime::Transaction`] for state management
```

### Validation Commands

```bash
# Check for broken links
cargo check --workspace

# Generate documentation (strict mode)
RUSTDOCFLAGS="-D rustdoc::broken-intra-doc-links" cargo doc --no-deps --workspace

# Find all broken links
cargo doc --workspace 2>&1 | grep "broken intra-doc link"
```

### Fixing Broken Links

When the compiler reports a broken link:

1. **Find the correct path:**

   ```bash
   rg "^pub struct TypeName" --type rust
   rg "^pub fn method_name" --type rust
   ```

2. **Update with full path if needed:**

   ```rust
   // Before: [`Runtime::commit`] (broken)
   // After:  [`crate::runtime::Runtime::commit`] (fixed)
   ```

3. **Verify the fix:**
   ```bash
   cargo check
   ```

### Benefits

- **IDE integration:** Ctrl+Click / Cmd+Click jumps to definition
- **Compile-time validation:** Broken links fail the build
- **Refactoring safety:** Compiler catches stale references
- **LLM navigation:** Explicit structure for code understanding
- **Documentation quality:** Links never go stale

### Language-Specific Adaptations

**Rust:** Enforced via `rustdoc` lints (built-in)
**TypeScript/JavaScript:** Use JSDoc `{@link Type}` syntax where tooling supports it
**Python:** Use Sphinx cross-references (`:class:\`ClassName\``, `:func:\`function_name\``)
**Other languages:** Follow ecosystem conventions, document in comments if no tooling exists

---

## 6. Doctests and Examples

- Prefer **runnable examples** (` ```rust ` blocks) over `ignore` fences.
- Each public type should have at least one example showing real usage.

**Example:**

````rust
/// ```
/// use crate::types::{ItemShape, Rotation};
/// let shape = ItemShape::new(NonZeroU32::new(1).unwrap(), NonZeroU32::new(3).unwrap());
/// let dims = shape.effective_dimensions(Rotation::Clockwise90);
/// assert_eq!((dims.0.get(), dims.1.get()), (3, 1));
/// ```
````

---

## 7. File Index (Optional for Long Files)

At the end of the file header or start of doc comment, include a navigable index for large modules.

**Example:**

```rust
//! ## Index
//! - Opaque Identifiers (u32): [`ClassId`], [`ContainerId`], [`Owner`]
//! - Opaque Identifiers (u64): [`StackId`], [`InstId`]
//! - Structural Types: [`ItemShape`], [`Rotation`], [`ItemPlacement`]
//! - Transaction Types: [`TxId`], [`TxMeta`], [`EventMeta`]
```

### Benefits

- LLMs and humans can jump directly to a section.
- Reduces re-parsing cost for multi-hundred-line files.

---

## 8. Language-Agnostic Adaptations

| Language            | Header Comment Syntax | Doc Comment Syntax | Example Marker                   |
| ------------------- | --------------------- | ------------------ | -------------------------------- |
| **Rust**            | `//`                  | `///`, `//!`       | ✅ Default                       |
| **TypeScript / JS** | `//` or `/* ... */`   | `/** ... */`       | `// #region` allowed for blocks  |
| **Python**          | `#`                   | `"""docstring"""`  | Section banners via `# ======`   |
| **C / C++**         | `//`                  | `/** ... */`       | Follow Rust style where possible |

All languages should preserve the **same semantic layout**: path comment → metadata → overview → sectioning → consistent docs.

---

## 9. Imports and Dependency Declarations

- Always group imports by category: std → external crates → internal modules.
- Use **blank lines** between groups.
- For Rust, prefer absolute `crate::` imports in foundational files.

**Example:**

```rust
use core::num::NonZeroU32;
use serde::{Serialize, Deserialize};
use crate::ffi::StackKey;
```

---

## 10. Constants and Globals

- Prefix each constant block with a short comment on intent.
- Prefer grouping related constants into one block.

**Example:**

```rust
// ============================================================================
// CONSTANTS: System-wide limits and defaults
// ============================================================================
/// Maximum nesting depth allowed for recursive containers.
pub const MAX_NESTING_DEPTH: usize = 8;
```

---

## 11. Summary of Key Rules

1. Every file starts with **path comment + metadata block.**
2. Always include an **Overview docblock** explaining purpose and invariants.
3. For trust-boundary files, include a **Security posture** note referencing `Docs/security/threat_model.md`.
4. Use consistent `// SECTION:` banners.
5. Fix all **doc/signature mismatches** (e.g., `new` vs. `from_raw`).
6. **REQUIRED: Use validated backtick links** for all cross-references (e.g., [`Type`], [`function`])—enforced by workspace lints.
7. Prefer **runnable doctests** to ignored examples.
8. Include **Index** headers for long foundational files.
9. Keep **naming, structure, and conversion patterns uniform** across modules.
10. Imports grouped: std → extern → crate.
11. One-liner invariant docs for every public struct or enum.
12. Avoid duplication of meaning; prioritize clarity and searchability.

---

## 12. Enforcement and CI Integration

### Workspace Lints (Rust)

Required in workspace `Cargo.toml`:

```toml
[workspace.lints.rustdoc]
broken_intra_doc_links = "deny"
private_intra_doc_links = "warn"
bare_urls = "warn"

[workspace.lints.rust]
missing_docs = "warn"
```

### CI Configuration

```yaml
# .github/workflows/docs.yml
- name: Validate documentation
  run: cargo doc --no-deps --workspace
  env:
    RUSTDOCFLAGS: "-D rustdoc::broken-intra-doc-links"
```

This ensures:

- No code can be merged with broken documentation links
- Documentation quality is maintained automatically
- Link accuracy is validated on every commit

---

## 13. Code Quality Standards (Clippy)

This project uses **Clippy** to enforce Rust code quality standards beyond what the compiler checks. Clippy lints catch potential bugs, performance issues, and style inconsistencies.

### Lint Categories

#### Enforced (Error Level)

These lints cause build failures and must be fixed:

- **`clippy::correctness`** - Actual bugs and logic errors (always deny)
- **`clippy::suspicious`** - Code that is likely wrong or misleading (deny)
- **`clippy::perf`** - Performance issues (unnecessary clones, inefficient patterns)

#### Warning Level

These should be addressed but won't block builds:

- **`clippy::style`** - Idiomatic Rust style preferences
- **`clippy::complexity`** - Overly complex code that could be simplified
- **`clippy::pedantic`** - Additional style opinions (enable selectively)

### Test-Specific Allowances

Test code has different quality standards. The following lints are **allowed in test modules** via `#![allow(...)]` at the module level:

```rust
#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    #![allow(clippy::expect_used)]
    #![allow(clippy::panic)]
    #![allow(clippy::similar_names)]  // a/b naming in test helpers
}
```

**Rationale:**

- Tests can panic/unwrap (failures are caught by test harness)
- Test clarity often trumps production-level error handling
- Similar variable names (e.g., `slot_a`, `slot_b`) are acceptable in test fixtures

### Production Lint Locks (Non-Negotiable)

The following lints are **never** allowed in non-test code:

- `clippy::all`
- `clippy::unwrap_used`
- `clippy::expect_used`
- `clippy::panic`
- `clippy::cast_possible_truncation`
- `clippy::restriction`

**Enforcement:**

- Use `#![cfg_attr(not(test), forbid(...))]` at crate roots for the lint list above.
- CI must fail if any non-test `#[allow(clippy::...)]` violates this policy.

### Critical Lints to Address

#### 1. `cast_possible_truncation` (HIGH PRIORITY)

**Issue:** Silent data corruption from unsafe numeric casts

```rust
// BAD: Truncates on 64-bit systems without error
let index: usize = 5_000_000_000;
let id = (index as u32);  // Silently wraps to wrong value

// GOOD: Returns error on overflow
let id = u32::try_from(index)?;
```

#### 2. `use_self` (LOW PRIORITY)

**Issue:** Refactoring safety and consistency

```rust
// Before
impl ClassId {
    pub fn new(id: NonZeroU32) -> ClassId { ... }
}

// After (preferred)
impl ClassId {
    pub fn new(id: NonZeroU32) -> Self { ... }
}
```

**Impact:** Zero runtime cost, improves maintainability

### Handling Violations

When Clippy flags code, choose one of:

#### Option 1: Fix the Issue (Preferred)

Address the underlying problem:

```rust
// Before
let value = (index as u32);

// After
let value = u32::try_from(index)
    .map_err(|_| AssetError::IndexOverflow)?;
```

#### Option 2: Justify with `#[allow]`

If the lint is a false positive or intentional, suppress it with a comment explaining why:

```rust
// Sorting comparator uses conventional a/b naming
#[allow(clippy::similar_names)]
fn compare(a: &Item, b: &Item) -> Ordering { ... }
```

**Rules for `#[allow]`:**

- Always include a comment explaining **why** the allow is justified
- Use the narrowest scope (function > module > crate)
- Prefer fixing over allowing unless there's a good reason
- `clippy::too_many_lines` may be allowed when a single linear function preserves auditability and ordered state updates

#### Option 3: Never Allow (Correctness Lints)

Some lints should **never** be suppressed in production code:

- `clippy::correctness` - These are bugs, not style choices
- `clippy::mem_forget` - Memory safety issues
- `clippy::cast_ptr_alignment` - Undefined behavior

### CI/CD Integration

```yaml
# .github/workflows/ci.yml
- name: Run Clippy
  run: cargo clippy --all-targets --all-features -- -D warnings
```

This ensures:

- All clippy warnings are treated as errors in CI
- No code can be merged with clippy violations
- Production code maintains consistent quality standards

### Running Clippy Locally

```bash
# Check all code (including tests)
cargo clippy --all-targets --all-features

# Treat warnings as errors (CI mode)
cargo clippy --all-targets --all-features -- -D warnings

# Fix auto-fixable issues
cargo clippy --fix --all-targets --all-features
```

### Workspace Configuration

Add to workspace `Cargo.toml`:

```toml
[workspace.lints.clippy]
correctness = "deny"
suspicious = "deny"
perf = "warn"
style = "warn"

# Specific high-priority lints
cast_possible_truncation = "warn"
unwrap_used = "warn"  # Production code only
expect_used = "warn"  # Production code only
```

### Summary

- **Always fix** correctness and suspicious lints
- **Prefer fixing** over `#[allow]` for all production code
- **Document** all `#[allow]` directives with explanatory comments
- **Test code** has relaxed standards (panic/unwrap allowed)
- **CI enforces** clippy checks on all commits

---

## 14. Numeric Conversions and Narrowing Policy

This project enforces **strict checked conversions** for all narrowing numeric casts. Silent truncation is a critical bug class that must be prevented at the language level.

### Global Policy

#### Forbidden in Production Code

These patterns are **never allowed** in production code:

```rust
// ❌ FORBIDDEN
let x = large_value as u32;        // Silent truncation
let y = index as u16;               // Silent truncation
let z = i64_value as i32;           // Silent truncation

// ❌ FORBIDDEN
value.unwrap()                      // May panic
value.expect("msg")                 // May panic
panic!("overflow")                  // Always panics
assert!(condition, "msg")           // May panic

// ❌ FORBIDDEN
#[allow(clippy::cast_possible_truncation)]  // Hides bugs
```

**Zero tolerance:** No narrowing `as`-casts, no panics, no suppressions in production code.

#### Required in Production Code

All narrowing conversions must be **explicit, checked, and return typed errors**:

```rust
// ✅ REQUIRED
let x = u32::try_from(large_value)
    .map_err(|_| AssetError::IndexOverflow)?;

// ✅ REQUIRED
let id = ClassId::try_from_index(index)?;

// ✅ REQUIRED
result.ok_or(AssetError::InvalidInput)?
```

#### Test Code Exceptions

Only under `#[cfg(test)]`:

```rust
#[cfg(test)]
mod tests {
    #[test]
    fn test_something() {
        let id = ClassId::from_index(42);  // OK in tests
        value.unwrap();                     // OK in tests
        assert!(condition, "msg");          // OK in tests
    }
}
```

### Required Conversions for ID Types

Every ID type (`ClassId`, `ContainerId`, `Owner`, `StackId`, `InstId`) must provide:

#### 1. Checked Constructor (Primary)

```rust
/// Creates an ID from a usize index (1-based).
///
/// Returns [`AssetError::IndexOverflow`] if index exceeds `u32::MAX - 1`.
pub fn try_from_index(index: usize) -> Result<Self, AssetError> {
    let idx = u32::try_from(index)
        .map_err(|_| AssetError::IndexOverflow)?;
    let nz = NonZeroU32::new(idx.saturating_add(1))
        .ok_or(AssetError::IndexOverflow)?;
    Ok(Self(nz))
}
```

**This is the canonical production pathway for `usize → Id` conversions.**

#### 2. Raw Constructor (For Known-Valid Values)

```rust
/// Creates an ID from a raw non-zero u32.
///
/// Use this only when you have a validated NonZeroU32 value.
pub const fn from_raw_u32(raw: NonZeroU32) -> Self {
    Self(raw)
}
```

#### 3. FFI Constructor (Optional)

```rust
/// Creates an ID from a raw u32 (returns None if zero).
///
/// Use at FFI boundaries where zero is a sentinel value.
pub const fn from_raw(raw: u32) -> Option<Self> {
    match NonZeroU32::new(raw) {
        Some(nz) => Some(Self(nz)),
        None => None,
    }
}
```

### Boundary Conversion Rules

At FFI, serialization, or external API boundaries:

```rust
// ❌ WRONG - Silent truncation
fn serialize_count(count: usize) -> u32 {
    count as u32
}

// ✅ CORRECT - Explicit error
fn serialize_count(count: usize) -> Result<u32, SerializationError> {
    u32::try_from(count)
        .map_err(|_| SerializationError::CountOverflow)
}
```

**Every boundary crossing must return a typed error instead of truncating.**

### Pattern Examples

#### Converting usize to u32

```rust
// ❌ WRONG
let index_u32 = index as u32;

// ✅ CORRECT
let index_u32 = u32::try_from(index)
    .map_err(|_| AssetError::IndexOverflow)?;
```

#### Converting i64 to i32

```rust
// ❌ WRONG
let timestamp = large_timestamp as i32;

// ✅ CORRECT
let timestamp = i32::try_from(large_timestamp)
    .map_err(|_| AssetError::TimestampOverflow)?;
```

#### Array Indexing

```rust
// ❌ WRONG
fn get_item(&self, id: ClassId) -> &Item {
    &self.items[id.index() as usize]
}

// ✅ CORRECT
fn get_item(&self, id: ClassId) -> Result<&Item, AssetError> {
    let idx = id.index();  // Already usize, no conversion needed
    self.items.get(idx).ok_or(AssetError::InvalidId)
}
```

### Testing Requirements

For **every** ID type and conversion function, tests must include:

#### 1. Maximum Valid Value

```rust
#[test]
fn test_max_valid_index() {
    let max_valid = (u32::MAX - 1) as usize;
    let id = ClassId::try_from_index(max_valid).unwrap();
    assert_eq!(id.index(), max_valid);
}
```

#### 2. Overflow Detection

```rust
#[test]
fn test_index_overflow() {
    let overflow = (u32::MAX as usize) + 1;
    assert!(matches!(
        ClassId::try_from_index(overflow),
        Err(AssetError::IndexOverflow)
    ));
}
```

#### 3. Large Value on 64-bit

```rust
#[cfg(target_pointer_width = "64")]
#[test]
fn test_large_index_overflow() {
    let huge: usize = 5_000_000_000;
    assert!(ClassId::try_from_index(huge).is_err());
}
```

#### 4. Round-Trip Property

```rust
#[test]
fn test_index_roundtrip() {
    for index in [0, 1, 100, 1000, u32::MAX as usize - 1] {
        let id = ClassId::try_from_index(index).unwrap();
        assert_eq!(id.index(), index);
    }
}
```

### Rationale

#### Why Zero Tolerance?

Silent truncation is a **critical bug class**:

- **Data corruption** - Wraps to wrong values without error
- **Security risk** - May bypass bounds checks
- **Hard to debug** - Only manifests on 64-bit with large datasets
- **Type system failure** - Undermines Rust's safety guarantees

#### Why Not Just Document "Safe" Casts?

```rust
// Even "obviously safe" casts can fail
let len = vec.len();  // usize on 64-bit
let len_u32 = len as u32;  // May truncate if vec > 4GB!
```

Platform differences make "safe" casts unreliable. Explicit checks are the only guarantee.

#### Performance Impact

**Zero** - `try_from` is a zero-cost abstraction:

- Inlined at optimization levels
- Same assembly as unchecked cast when bounds are proven
- Additional check is often optimized away

### Workspace Enforcement

Workspace lint configuration:

```toml
[workspace.lints.clippy]
cast_possible_truncation = "deny"  # No narrowing as-casts
unwrap_used = "deny"                # No unwrap in production
expect_used = "deny"                # No expect in production
panic = "deny"                      # No panic in production
```

### Summary

- **No `as`-casts** for narrowing conversions in production code
- **All conversions** through `try_from` with typed errors
- **No panics** in production (unwrap/expect/panic! forbidden)
- **Comprehensive tests** for overflow detection
- **Workspace lints** enforce policy automatically

---

## 15. Optional Enhancements (Future-proofing)

- `#[serde(transparent)]` for all `#[repr(transparent)]` structs.
- Auto-generated Indexes via build script for large foundational files.
- CI lint check enforcing presence of `// src/...` path comment.

---

## 16. Character Set and Encoding

All source files, documentation, and comments must use **simple universal characters (ASCII-compatible) only**.

### Rules

- **No Emojis**: Do not use emojis in comments, documentation, or code (e.g., `✅`, `❌`, `🚀`).
- **No Non-ASCII Characters**: Avoid special symbols like `—` (em dash), `’` (smart quotes), or mathematical symbols not on a standard keyboard.
- **Encoding**: All files must be UTF-8, but content should remain within the ASCII subset where possible.
- **Exceptions**: String literals that specifically require non-ASCII characters for testing or localization purposes (if applicable).

### Rationale

- **Universal Compatibility**: Ensures code renders correctly in all editors, terminals, and diff tools.
- **Avoids Mojibake**: Prevents encoding issues where characters appear as garbage (e.g., `[square]`).
- **Professionalism**: Maintains a clean, professional codebase appearance.
- **Tooling Safety**: Reduces risk of tools or scripts misinterpreting multi-byte characters.

---

### Purpose of This Standard

This style guide ensures every file in the repository:

- Can be **understood in isolation** by humans and LLMs.
- Embeds its **context** and **purpose** explicitly.
- Provides **validated navigation** through compiler-checked cross-references.
- Encourages **consistency and predictability**, reducing hallucination and redundant explanation.
- Forms a **unified code narrative**—each file reads like a page in a system manual.
- Supports **security-first development** by making code reviewable, auditable, and resistant to "clever" shortcuts that introduce vulnerabilities. See [Docs/standards/codebase_engineering_standards.md](Docs/standards/codebase_engineering_standards.md) section 0 for threat model.

---

> **TL;DR:** Every file must explain where it lives, what it does, why it exists, and how it fits—before doing anything else. All cross-references must be validated via compiler-checked links.
