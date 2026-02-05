# Codebase Formatting and Documentation Standards

This document defines universal formatting, structure, and documentation expectations for all source files. It ensures every file is consistent, self-contained, and easy to scan for both humans and automated tools.

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

Each file must begin with a path comment followed by a brief metadata block that establishes context.

### Example

```rust
// crates/decision-gate-core/src/runtime/runpack.rs
// ============================================================================
// Module: Decision Gate Runpack Builder and Verifier
// Description: Builds deterministic runpack artifacts and verifies them offline.
// Purpose: Provide tamper-evident audit bundles for Decision Gate runs.
// Dependencies: decision-gate-core only; no provider or transport logic.
// ============================================================================
```

### Rules

- Always include the relative file path from project root.
- Include one-line summaries for Module name, Description, Purpose, and Dependencies.
- Avoid decorative ASCII art banners except the standardized `// ============================================================================` style.

---

## 2. File Overview Block

Immediately below the header comment, add a file-level documentation block describing the file's conceptual role, invariants, and relationship to other layers.

### Example

```rust
//! ## Overview
//! Builds deterministic runpack artifacts and verifies them offline.
//! Invariants:
//! - Manifest hashes are stable for identical inputs.
//! - Verification fails closed on missing or mismatched artifacts.
//! Security posture: Treat all runpack inputs as untrusted. See Docs/security/threat_model.md.
```

### Rules

- Begin with `//! ## Overview` or equivalent for other languages.
- Explain what the file contributes to the system, not implementation details.
- Mention key invariants or guiding principles.
- For trust-boundary files, add a one-line security posture note referencing `Docs/security/threat_model.md`.

---

## 3. Sectioning and Headings

Every file should use consistent, scannable section banners.

### Example

```rust
// ============================================================================
// SECTION: Runpack Verification
// ============================================================================
```

### Guidelines

- Use `// SECTION:` headings for major conceptual divisions.
- Keep headings capitalized and separated with blank lines.
- Avoid ad-hoc headings like `// ---` or `// ===`.

---

## 4. Function and Type Documentation

### Constructors and Getters

Each `new(...)`, `from_raw(...)`, or similar constructor must:
- Accurately describe its contract.
- Never claim to return `None` unless it actually can.
- Default to `fn`. Promote to `const fn` only when compile-time use is required.

Example:

```rust
impl ScenarioSpec {
    /// Validates the ScenarioSpec invariants.
    ///
    /// # Errors
    /// Returns [`SpecError`] when validation fails.
    pub fn validate(&self) -> Result<(), SpecError> {
        // ...
        Ok(())
    }
}
```

### Invariants and Safety Contracts

Each public struct or enum must explicitly declare its invariants.

Example:

```rust
/// Comparator applied to evidence values.
///
/// # Invariants
/// - Variants are stable for serialization and contract matching.
```

### Conversion Patterns

Uniformly define and document conversion patterns for ID-like types:
- `new(NonZero*) -> Self`
- `from_raw(primitive) -> Option<Self>`
- `get() -> primitive`
- `index() -> usize`
- `try_from_index(usize) -> Result<Self, Error>`

Each pattern should use the same doc comment phrasing across types.

---

## 5. Cross-References and Navigation Links

All cross-references to other functions, types, modules, or layers must use validated backtick link syntax in Rust docs. This is enforced by `rustdoc::broken_intra_doc_links`.

### Enforcement (Rust)

The workspace `Cargo.toml` must include:

```toml
[workspace.lints.rustdoc]
broken_intra_doc_links = "deny"
private_intra_doc_links = "warn"
bare_urls = "warn"
```

### Link Syntax Reference

| What You Are Linking | Syntax | Example |
| --- | --- | --- |
| Type in same module | `[`Type`]` | `[`ScenarioSpec`]` |
| Type in other module | `[`module::Type`]` | `[`core::ScenarioSpec`]` |
| Type from crate root | `[`crate::path::Type`]` | `[`crate::core::ScenarioSpec`]` |
| Method | `[`Type::method`]` | `[`ScenarioSpec::validate`]` |
| Associated function | `[`Type::new`]` | `[`TenantId::new`]` |
| Trait | `[`Trait`]` | `[`EvidenceProvider`]` |
| Trait method | `[`Trait::method`]` | `[`EvidenceProvider::query`]` |
| Module | `[`module`]` | `[`runtime`]` |
| Enum variant | `[`Enum::Variant`]` | `[`Comparator::Equals`]` |
| Function | `[`function`]` | `[`hash_canonical_json`]` |

### Guidelines

When to link:
- Type names in documentation prose.
- Function and method references.
- Module names when discussing architecture.
- Error types in Returns or Errors sections.
- Trait bounds in generic documentation.

When not to link:
- Common words that happen to match type names.
- Types already linked in the same sentence.
- Code examples.
- User-facing strings or messages.

### Examples

Bad:

```rust
//! - Called by: runtime layer after successful validation
//! - Uses: the validation system to check constraints
```

Good:

```rust
//! - Called by: [`ScenarioSpec::validate`] after authoring input normalization
//! - Uses: [`SpecError`] to report validation failures
```

### Validation Commands

```bash
cargo check --workspace
RUSTDOCFLAGS="-D rustdoc::broken-intra-doc-links" cargo doc --no-deps --workspace
cargo doc --workspace 2>&1 | grep "broken intra-doc link"
```

---

## 6. Doctests and Examples

- Prefer runnable examples over `ignore` fences.
- Each public type should have at least one example showing real usage.

Example:

```rust
use decision_gate_core::Comparator;

let comparator = Comparator::Equals;
assert_eq!(comparator, Comparator::Equals);
```

---

## 7. File Index (Optional for Long Files)

For large modules, include a navigable index near the top of the file.

Example:

```rust
//! ## Index
//! - Scenario model: [`ScenarioSpec`], [`StageSpec`], [`ConditionSpec`]
//! - Evidence model: [`EvidenceQuery`], [`EvidenceResult`], [`Comparator`]
//! - Runpack: [`RunpackManifest`], [`RunpackArtifact`]
//! - Decisions: [`DecisionRecord`]
```

---

## 8. Language-Agnostic Adaptations

| Language | Header Comment Syntax | Doc Comment Syntax | Example Marker |
| --- | --- | --- | --- |
| Rust | `//` | `///`, `//!` | Default |
| TypeScript or JS | `//` or `/* ... */` | `/** ... */` | `// #region` allowed |
| Python | `#` | `"""docstring"""` | Section banners via `# ======` |
| C or C++ | `//` | `/** ... */` | Follow Rust style where possible |

All languages should preserve the same semantic layout: path comment -> metadata -> overview -> sectioning -> consistent docs.

---

## 9. Imports and Dependency Declarations

- Group imports by category: std -> external crates -> internal modules.
- Use blank lines between groups.
- Prefer absolute `crate::` imports in foundational files.

Example:

```rust
use core::num::NonZeroU32;
use serde::{Deserialize, Serialize};
use crate::core::hashing::hash_canonical_json;
```

---

## 10. Constants and Globals

- Prefix each constant block with a short comment on intent.
- Group related constants into one block.

Example:

```rust
// ============================================================================
// CONSTANTS: Runpack manifest versioning
// ============================================================================
pub const RUNPACK_MANIFEST_VERSION: u32 = 1;
```

---

## 11. Summary of Key Rules

1. Every file starts with path comment plus metadata block.
2. Always include an Overview docblock explaining purpose and invariants.
3. For trust-boundary files, include a security posture note referencing `Docs/security/threat_model.md`.
4. Use consistent `// SECTION:` banners.
5. Fix all doc and signature mismatches.
6. Use validated backtick links for all cross-references in Rust docs.
7. Prefer runnable doctests to ignored examples.
8. Include Index headers for long foundational files.
9. Keep naming, structure, and conversion patterns uniform across modules.
10. Imports grouped: std -> extern -> crate.
11. One-line invariant docs for every public struct or enum.
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
- name: Validate documentation
  run: cargo doc --no-deps --workspace
  env:
    RUSTDOCFLAGS: "-D rustdoc::broken-intra-doc-links"
```

---

## 13. Code Quality Standards (Clippy)

This project uses Clippy to enforce Rust code quality standards beyond what the compiler checks.

### Lint Categories

Enforced as errors:
- `clippy::correctness`
- `clippy::suspicious`
- `clippy::perf`

Enforced as warnings:
- `clippy::style`
- `clippy::complexity`
- `clippy::pedantic`

### Test-Specific Allowances

Test code has relaxed standards. The following lints may be allowed inside `#[cfg(test)]` modules:
- `clippy::unwrap_used`
- `clippy::expect_used`
- `clippy::panic`
- `clippy::similar_names`

Any `#[allow]` must include a comment explaining the rationale.

### Production Lint Locks

The following lints are never allowed in non-test code:
- `clippy::unwrap_used`
- `clippy::expect_used`
- `clippy::panic`
- `clippy::cast_possible_truncation`

---

## 14. Numeric Conversions and Narrowing Policy

This project enforces strict checked conversions for all narrowing numeric casts.

### Forbidden in Production Code

```rust
let x = large_value as u32;        // Silent truncation
value.unwrap();                    // May panic
#[allow(clippy::cast_possible_truncation)]
```

### Required in Production Code

```rust
let x = u32::try_from(large_value)
    .map_err(|_| DomainError::IndexOverflow)?;
```

`DomainError` is a placeholder for the crate's typed error enum.

Every boundary crossing must return a typed error instead of truncating.

---

## 15. Optional Enhancements (Future-proofing)

- `#[serde(transparent)]` for all `#[repr(transparent)]` structs.
- Auto-generated indexes via build script for large foundational files.
- CI lint check enforcing presence of the path comment.

---

## 16. Character Set and Encoding

All source files, documentation, and comments must use ASCII-compatible characters only.

### Rules

- No emojis.
- Avoid non-ASCII punctuation like smart quotes and em dashes.
- UTF-8 encoding is required, but content should remain within the ASCII subset.
- Exceptions are allowed only for string literals that intentionally exercise non-ASCII behavior.

---

### Purpose of This Standard

This style guide ensures every file in the repository:
- Can be understood in isolation.
- Embeds context and purpose explicitly.
- Provides validated navigation through compiler-checked cross-references.
- Encourages consistency and predictability.
- Supports security-first development by making code reviewable and auditable.

See `Docs/standards/codebase_engineering_standards.md` for the security posture and determinism requirements.
