// crates/decision-gate-core/src/core/time.rs
// ============================================================================
// Module: Decision Gate Time Model
// Description: Canonical timestamp representations for triggers and logs.
// Purpose: Provide deterministic, replayable time values across Decision Gate records.
// Dependencies: serde
// ============================================================================

//! ## Overview
//! Decision Gate uses explicit time values embedded in triggers and logs to keep replay
//! deterministic. The core engine never reads wall-clock time directly; hosts
//! must supply timestamps via triggers or runtime helpers.

// ============================================================================
// SECTION: Imports
// ============================================================================

use serde::Deserialize;
use serde::Serialize;

// ============================================================================
// SECTION: Time Values
// ============================================================================

/// Canonical timestamp used in Decision Gate logs and trigger records.
///
/// # Invariants
/// - Values are explicitly provided by callers; the core never reads wall-clock time.
/// - No validation is performed; monotonicity is a caller responsibility.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum Timestamp {
    /// Unix epoch milliseconds.
    UnixMillis(i64),
    /// Monotonic logical time value.
    Logical(u64),
}

impl Timestamp {
    /// Returns the timestamp as unix milliseconds when available.
    #[must_use]
    pub const fn as_unix_millis(&self) -> Option<i64> {
        match self {
            Self::UnixMillis(value) => Some(*value),
            Self::Logical(_) => None,
        }
    }

    /// Returns the timestamp as logical time when available.
    #[must_use]
    pub const fn as_logical(&self) -> Option<u64> {
        match self {
            Self::UnixMillis(_) => None,
            Self::Logical(value) => Some(*value),
        }
    }
}
