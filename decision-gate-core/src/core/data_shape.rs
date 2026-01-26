// decision-gate-core/src/core/data_shape.rs
// ============================================================================
// Module: Data Shape Registry Types
// Description: Canonical identifiers and records for asserted data shapes.
// Purpose: Provide shared types for schema registry and precheck evaluation.
// Dependencies: crate::core::{identifiers, time}, serde, serde_json
// ============================================================================

//! ## Overview
//! Data shapes describe asserted evidence payloads. They are registry-scoped by
//! tenant and namespace and are versioned and immutable once registered.

use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;

use crate::core::identifiers::DataShapeId;
use crate::core::identifiers::DataShapeVersion;
use crate::core::identifiers::NamespaceId;
use crate::core::identifiers::TenantId;
use crate::core::time::Timestamp;

/// Reference to a data shape schema.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DataShapeRef {
    /// Data shape identifier.
    pub schema_id: DataShapeId,
    /// Data shape version identifier.
    pub version: DataShapeVersion,
}

/// Data shape registry record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DataShapeRecord {
    /// Tenant identifier.
    pub tenant_id: TenantId,
    /// Namespace identifier.
    pub namespace_id: NamespaceId,
    /// Data shape identifier.
    pub schema_id: DataShapeId,
    /// Data shape version identifier.
    pub version: DataShapeVersion,
    /// JSON Schema payload for the data shape.
    pub schema: Value,
    /// Optional description of the data shape.
    pub description: Option<String>,
    /// Timestamp recorded when the schema was created.
    pub created_at: Timestamp,
    /// Optional signing metadata for registry records.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signing: Option<DataShapeSignature>,
}

/// Optional schema signing metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DataShapeSignature {
    /// Signing key identifier.
    pub key_id: String,
    /// Signature string (base64 or provider-defined encoding).
    pub signature: String,
    /// Optional signature algorithm label.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub algorithm: Option<String>,
}

/// Page of data shapes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DataShapePage {
    /// Data shape records in the page.
    pub items: Vec<DataShapeRecord>,
    /// Optional pagination token for the next page.
    pub next_token: Option<String>,
}
