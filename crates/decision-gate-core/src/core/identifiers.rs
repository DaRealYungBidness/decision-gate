// crates/decision-gate-core/src/core/identifiers.rs
// ============================================================================
// Module: Decision Gate Identifiers
// Description: Canonical opaque identifiers for Decision Gate specifications and runs.
// Purpose: Provide strongly typed, serializable identifiers with stable wire forms.
// Dependencies: serde
// ============================================================================

//! ## Overview
//! This module defines the canonical identifiers used throughout
//! Decision Gate. Identifiers are opaque and serialize as numbers or strings
//! on the wire. Numeric identifiers enforce non-zero, 1-based invariants at
//! construction boundaries.

// ============================================================================
// SECTION: Imports
// ============================================================================

use std::fmt;
use std::num::NonZeroU64;

use serde::Deserialize;
use serde::Serialize;

// ============================================================================
// SECTION: Identifier Types
// ============================================================================

/// Tenant identifier scoped to Decision Gate runs.
///
/// # Invariants
/// - Always >= 1 (non-zero, 1-based).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TenantId(NonZeroU64);

impl TenantId {
    /// Creates a new tenant identifier from a non-zero value.
    #[must_use]
    pub const fn new(id: NonZeroU64) -> Self {
        Self(id)
    }

    /// Creates a tenant identifier from a raw value (returns `None` if zero).
    #[must_use]
    pub fn from_raw(raw: u64) -> Option<Self> {
        NonZeroU64::new(raw).map(Self)
    }

    /// Returns the raw identifier value (always >= 1).
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0.get()
    }
}

impl fmt::Display for TenantId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.get().fmt(f)
    }
}

/// Namespace identifier scoped within a tenant.
///
/// # Invariants
/// - Always >= 1 (non-zero, 1-based).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct NamespaceId(NonZeroU64);

impl NamespaceId {
    /// Creates a new namespace identifier from a non-zero value.
    #[must_use]
    pub const fn new(id: NonZeroU64) -> Self {
        Self(id)
    }

    /// Creates a namespace identifier from a raw value (returns `None` if zero).
    #[must_use]
    pub fn from_raw(raw: u64) -> Option<Self> {
        NonZeroU64::new(raw).map(Self)
    }

    /// Returns the raw identifier value (always >= 1).
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0.get()
    }
}

impl fmt::Display for NamespaceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.get().fmt(f)
    }
}

/// Scenario identifier for a scenario specification.
///
/// # Invariants
/// - Opaque UTF-8 string; no normalization or validation is applied by this type.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ScenarioId(String);

impl ScenarioId {
    /// Creates a new scenario identifier.
    #[must_use]
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// Returns the identifier as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ScenarioId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl From<&str> for ScenarioId {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl From<String> for ScenarioId {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

/// Scenario specification version identifier.
///
/// # Invariants
/// - Opaque UTF-8 string; no normalization or validation is applied by this type.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SpecVersion(String);

impl SpecVersion {
    /// Creates a new scenario specification version.
    #[must_use]
    pub fn new(version: impl Into<String>) -> Self {
        Self(version.into())
    }

    /// Returns the version as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for SpecVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl From<&str> for SpecVersion {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl From<String> for SpecVersion {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

/// Run identifier scoped to a tenant.
///
/// # Invariants
/// - Opaque UTF-8 string; no normalization or validation is applied by this type.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RunId(String);

impl RunId {
    /// Creates a new run identifier.
    #[must_use]
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// Returns the identifier as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for RunId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl From<&str> for RunId {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl From<String> for RunId {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

/// Stage identifier within a scenario specification.
///
/// # Invariants
/// - Opaque UTF-8 string; no normalization or validation is applied by this type.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct StageId(String);

impl StageId {
    /// Creates a new stage identifier.
    #[must_use]
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// Returns the identifier as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for StageId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl From<&str> for StageId {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl From<String> for StageId {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

/// Packet identifier within a scenario specification.
///
/// # Invariants
/// - Opaque UTF-8 string; no normalization or validation is applied by this type.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PacketId(String);

impl PacketId {
    /// Creates a new packet identifier.
    #[must_use]
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// Returns the identifier as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for PacketId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl From<&str> for PacketId {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl From<String> for PacketId {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

/// Gate identifier within a scenario specification.
///
/// # Invariants
/// - Opaque UTF-8 string; no normalization or validation is applied by this type.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct GateId(String);

impl GateId {
    /// Creates a new gate identifier.
    #[must_use]
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// Returns the identifier as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for GateId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl From<&str> for GateId {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl From<String> for GateId {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

/// Condition identifier referenced in requirements.
///
/// # Invariants
/// - Opaque UTF-8 string; no normalization or validation is applied by this type.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ConditionId(String);

impl ConditionId {
    /// Creates a new condition identifier.
    #[must_use]
    pub fn new(key: impl Into<String>) -> Self {
        Self(key.into())
    }

    /// Returns the key as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ConditionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl From<&str> for ConditionId {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl From<String> for ConditionId {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

/// Evidence provider identifier used by evidence queries.
///
/// # Invariants
/// - Opaque UTF-8 string; no normalization or validation is applied by this type.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ProviderId(String);

impl ProviderId {
    /// Creates a new provider identifier.
    #[must_use]
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// Returns the identifier as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ProviderId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl From<&str> for ProviderId {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl From<String> for ProviderId {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

/// Trigger identifier used for idempotency.
///
/// # Invariants
/// - Opaque UTF-8 string; no normalization or validation is applied by this type.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TriggerId(String);

impl TriggerId {
    /// Creates a new trigger identifier.
    #[must_use]
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// Returns the identifier as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for TriggerId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl From<&str> for TriggerId {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl From<String> for TriggerId {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

/// Decision identifier for logged control-plane decisions.
///
/// # Invariants
/// - Opaque UTF-8 string; no normalization or validation is applied by this type.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct DecisionId(String);

impl DecisionId {
    /// Creates a new decision identifier.
    #[must_use]
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// Returns the identifier as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for DecisionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl From<&str> for DecisionId {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl From<String> for DecisionId {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

/// Correlation identifier used across triggers, decisions, and dispatch.
///
/// # Invariants
/// - Opaque UTF-8 string; no normalization or validation is applied by this type.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct CorrelationId(String);

impl CorrelationId {
    /// Creates a new correlation identifier.
    #[must_use]
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// Returns the identifier as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for CorrelationId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl From<&str> for CorrelationId {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl From<String> for CorrelationId {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

/// Schema identifier for packet schemas.
///
/// # Invariants
/// - Opaque UTF-8 string; no normalization or validation is applied by this type.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SchemaId(String);

impl SchemaId {
    /// Creates a new schema identifier.
    #[must_use]
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// Returns the identifier as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for SchemaId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl From<&str> for SchemaId {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl From<String> for SchemaId {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

/// Data shape schema identifier.
///
/// # Invariants
/// - Opaque UTF-8 string; no normalization or validation is applied by this type.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct DataShapeId(String);

impl DataShapeId {
    /// Creates a new data shape identifier.
    #[must_use]
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// Returns the identifier as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for DataShapeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl From<&str> for DataShapeId {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl From<String> for DataShapeId {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

/// Data shape schema version identifier.
///
/// # Invariants
/// - Opaque UTF-8 string; no normalization or validation is applied by this type.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct DataShapeVersion(String);

impl DataShapeVersion {
    /// Creates a new schema version identifier.
    #[must_use]
    pub fn new(version: impl Into<String>) -> Self {
        Self(version.into())
    }

    /// Returns the version as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for DataShapeVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl From<&str> for DataShapeVersion {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl From<String> for DataShapeVersion {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

/// Policy identifier for disclosure or authorization policies.
///
/// # Invariants
/// - Opaque UTF-8 string; no normalization or validation is applied by this type.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PolicyId(String);

impl PolicyId {
    /// Creates a new policy identifier.
    #[must_use]
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// Returns the identifier as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for PolicyId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl From<&str> for PolicyId {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl From<String> for PolicyId {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}
