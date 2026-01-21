// decision-gate-core/src/core/identifiers.rs
// ============================================================================
// Module: Decision Gate Identifiers
// Description: Canonical opaque identifiers for Decision Gate specifications and runs.
// Purpose: Provide strongly typed, serializable IDs with stable string forms.
// Dependencies: serde
// ============================================================================

//! ## Overview
//! This module defines the canonical string-based identifiers used throughout
//! Decision Gate. Identifiers are opaque and serialize as strings. Validation is handled
//! at scenario or runtime boundaries rather than within these simple wrappers.

// ============================================================================
// SECTION: Imports
// ============================================================================

use std::fmt;

use serde::Deserialize;
use serde::Serialize;

// ============================================================================
// SECTION: Identifier Types
// ============================================================================

/// Tenant identifier scoped to Decision Gate runs.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TenantId(String);

impl TenantId {
    /// Creates a new tenant identifier.
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

impl fmt::Display for TenantId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl From<&str> for TenantId {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl From<String> for TenantId {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

/// Scenario identifier for a scenario specification.
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

/// Predicate identifier referenced in requirements.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PredicateKey(String);

impl PredicateKey {
    /// Creates a new predicate key.
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

impl fmt::Display for PredicateKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl From<&str> for PredicateKey {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl From<String> for PredicateKey {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

/// Trigger identifier used for idempotency.
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

/// Policy identifier for disclosure or authorization policies.
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
