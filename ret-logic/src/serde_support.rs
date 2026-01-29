// ret-logic/src/serde_support.rs
// ============================================================================
// Module: Requirement Serde Support
// Description: Serde helpers for requirement serialization and validation.
// Purpose: Provide error models, configuration, and tree validation helpers.
// Dependencies: serde::{Deserialize, Serialize}, std::fmt
// ============================================================================

//! ## Overview
//! Strongly typed serde helpers give deterministic serialization/deserialization
//! outcomes while exposing consistent validation errors for requirement structures.
//! Security posture: deserialized requirements are untrusted; validate and fail
//! closed per `Docs/security/threat_model.md`.

// ============================================================================
// SECTION: Imports
// ============================================================================

use std::fmt;

use serde::Deserialize;
use serde::Serialize;

use crate::requirement::Requirement;

// ============================================================================
// SECTION: Serde Errors
// ============================================================================

/// Error types that can occur during requirement serialization/deserialization
///
/// # Invariants
/// - None. Variants capture structured validation failures.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SerdeError {
    /// Invalid requirement structure
    InvalidStructure(String),

    /// Missing required field
    MissingField(String),

    /// Invalid field value
    InvalidValue {
        /// Name of the offending field
        field: String,
        /// Value that failed validation
        value: String,
        /// Expected description for the field
        expected: String,
    },

    /// Circular reference detected
    CircularReference,

    /// Requirement tree too deep
    TooDeep {
        /// Maximum supported tree depth
        max_depth: usize,
        /// Depth encountered during validation
        actual_depth: usize,
    },

    /// Invalid group configuration
    InvalidGroup {
        /// Minimum required items in the group
        min: u8,
        /// Total items provided
        total: usize,
    },
}

// ============================================================================
// SECTION: Display Implementation
// ============================================================================

impl fmt::Display for SerdeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidStructure(msg) => {
                write!(f, "Invalid requirement structure: {msg}")
            }
            Self::MissingField(field) => write!(f, "Missing required field: {field}"),
            Self::InvalidValue {
                field,
                value,
                expected,
            } => {
                write!(f, "Invalid value for field '{field}': got '{value}', expected {expected}")
            }
            Self::CircularReference => {
                write!(f, "Circular reference detected in requirement tree")
            }
            Self::TooDeep {
                max_depth,
                actual_depth,
            } => {
                write!(f, "Requirement tree too deep: {actual_depth} levels (max {max_depth})")
            }
            Self::InvalidGroup {
                min,
                total,
            } => {
                write!(f, "Invalid group requirement: min {min} exceeds total {total}")
            }
        }
    }
}

impl std::error::Error for SerdeError {}

// ============================================================================
// SECTION: Serde Configuration
// ============================================================================

/// Configuration for requirement serialization/deserialization
///
/// # Invariants
/// - No invariants are enforced; callers should choose safe bounds.
#[derive(Debug, Clone)]
pub struct SerdeConfig {
    /// Maximum allowed depth for requirement trees
    pub max_depth: usize,

    /// Whether to validate requirement trees during deserialization
    pub validate_on_deserialize: bool,

    /// Whether to allow empty And/Or requirements
    pub allow_empty_logical: bool,
}

// ============================================================================
// SECTION: Configuration Defaults
// ============================================================================

impl Default for SerdeConfig {
    fn default() -> Self {
        Self {
            max_depth: 32,
            validate_on_deserialize: true,
            allow_empty_logical: true,
        }
    }
}

// ============================================================================
// SECTION: Requirement Validator
// ============================================================================

/// Validator for requirement trees
///
/// # Invariants
/// - Uses the stored [`SerdeConfig`] for all validation decisions.
#[derive(Debug)]
pub struct RequirementValidator {
    /// Validation configuration for structure limits.
    config: SerdeConfig,
}

// ============================================================================
// SECTION: Validation Methods
// ============================================================================

impl RequirementValidator {
    /// Creates a new validator with the given configuration
    #[must_use]
    pub const fn new(config: SerdeConfig) -> Self {
        Self {
            config,
        }
    }

    /// Creates a validator with default configuration
    #[must_use]
    pub fn with_defaults() -> Self {
        Self {
            config: SerdeConfig::default(),
        }
    }

    /// Validates a requirement tree
    ///
    /// This performs structural validation only - domain-specific predicate
    /// validation is handled by the domain during compilation or execution.
    ///
    /// # Arguments
    /// * `requirement` - The requirement tree to validate
    ///
    /// # Returns
    /// `Ok(())` if structurally valid, `Err(SerdeError)` if invalid
    ///
    /// # Errors
    /// Returns [`SerdeError`] when the requirement violates structural limits.
    pub fn validate<P>(&self, requirement: &Requirement<P>) -> Result<(), SerdeError> {
        self.validate_depth(requirement, 0)?;
        self.validate_structure(requirement)?;
        Ok(())
    }

    /// Validates the depth of a requirement tree
    fn validate_depth<P>(
        &self,
        requirement: &Requirement<P>,
        current_depth: usize,
    ) -> Result<(), SerdeError> {
        if current_depth > self.config.max_depth {
            return Err(SerdeError::TooDeep {
                max_depth: self.config.max_depth,
                actual_depth: current_depth,
            });
        }

        match requirement {
            Requirement::And(reqs) | Requirement::Or(reqs) => {
                for req in reqs {
                    self.validate_depth(req, current_depth + 1)?;
                }
            }
            Requirement::RequireGroup {
                reqs, ..
            } => {
                for req in reqs {
                    self.validate_depth(req, current_depth + 1)?;
                }
            }
            Requirement::Not(req) => {
                self.validate_depth(req, current_depth + 1)?;
            }
            Requirement::Predicate(_) => {
                // Predicates are leaf nodes - no further depth
            }
        }

        Ok(())
    }

    /// Validates the logical structure of a requirement tree
    fn validate_structure<P>(&self, requirement: &Requirement<P>) -> Result<(), SerdeError> {
        match requirement {
            Requirement::And(reqs) => {
                if !self.config.allow_empty_logical && reqs.is_empty() {
                    return Err(SerdeError::InvalidStructure(
                        "Empty And requirement not allowed".to_string(),
                    ));
                }
                for req in reqs {
                    self.validate_structure(req)?;
                }
            }

            Requirement::Or(reqs) => {
                if !self.config.allow_empty_logical && reqs.is_empty() {
                    return Err(SerdeError::InvalidStructure(
                        "Empty Or requirement not allowed".to_string(),
                    ));
                }
                for req in reqs {
                    self.validate_structure(req)?;
                }
            }

            Requirement::RequireGroup {
                min,
                reqs,
            } => {
                let min_required = usize::from(*min);
                if min_required > reqs.len() {
                    return Err(SerdeError::InvalidGroup {
                        min: *min,
                        total: reqs.len(),
                    });
                }
                if *min == 0 && !reqs.is_empty() {
                    return Err(SerdeError::InvalidStructure(
                        "RequireGroup with min=0 should be empty or use And instead".to_string(),
                    ));
                }
                for req in reqs {
                    self.validate_structure(req)?;
                }
            }

            Requirement::Not(req) => {
                self.validate_structure(req)?;
            }

            Requirement::Predicate(_) => {
                // Predicates are validated by the domain during compilation
            }
        }

        Ok(())
    }
}

/// Helper for serializing requirements with validation
///
/// # Invariants
/// - Uses the stored [`RequirementValidator`] for structural checks.
#[derive(Debug)]
pub struct RequirementSerializer {
    /// Validator used to enforce structural limits.
    validator: RequirementValidator,
}

impl RequirementSerializer {
    /// Creates a new serializer with the given configuration
    #[must_use]
    pub const fn new(config: SerdeConfig) -> Self {
        Self {
            validator: RequirementValidator::new(config),
        }
    }

    /// Creates a serializer with default configuration
    #[must_use]
    pub fn with_defaults() -> Self {
        Self {
            validator: RequirementValidator::with_defaults(),
        }
    }

    /// Serializes a requirement to RON format with validation
    ///
    /// This is the primary serialization method for the authoring layer.
    /// RON (Rusty Object Notation) provides a human-readable format that
    /// designers can edit directly.
    ///
    /// # Arguments
    /// * `requirement` - The requirement to serialize
    ///
    /// # Returns
    /// RON string representation of the requirement
    ///
    /// # Errors
    /// Returns [`SerdeError`] if validation fails or serialization fails.
    pub fn to_ron<P>(&self, requirement: &Requirement<P>) -> Result<String, SerdeError>
    where
        P: Serialize,
    {
        if self.validator.config.validate_on_deserialize {
            self.validator.validate(requirement)?;
        }

        ron::ser::to_string_pretty(requirement, ron::ser::PrettyConfig::default())
            .map_err(|e| SerdeError::InvalidStructure(e.to_string()))
    }

    /// Deserializes a requirement from RON format with validation
    ///
    /// This is the primary deserialization method for loading authored requirements.
    /// The resulting requirement can be compiled into a Plan for execution.
    ///
    /// # Arguments
    /// * `ron_str` - RON string to deserialize
    ///
    /// # Returns
    /// Deserialized and validated requirement ready for compilation
    ///
    /// # Errors
    /// Returns [`SerdeError`] if parsing fails or validation fails.
    pub fn from_ron<P>(&self, ron_str: &str) -> Result<Requirement<P>, SerdeError>
    where
        P: for<'de> Deserialize<'de>,
    {
        let requirement: Requirement<P> =
            ron::from_str(ron_str).map_err(|e| SerdeError::InvalidStructure(e.to_string()))?;

        if self.validator.config.validate_on_deserialize {
            self.validator.validate(&requirement)?;
        }

        Ok(requirement)
    }

    /// Serializes a requirement to JSON format with validation
    ///
    /// JSON serialization is provided for compatibility with web tools
    /// and external integrations. RON is preferred for human authoring.
    ///
    /// # Arguments
    /// * `requirement` - The requirement to serialize
    ///
    /// # Returns
    /// JSON string representation of the requirement
    ///
    /// # Errors
    /// Returns [`SerdeError`] if validation fails or serialization fails.
    pub fn to_json<P>(&self, requirement: &Requirement<P>) -> Result<String, SerdeError>
    where
        P: Serialize,
    {
        if self.validator.config.validate_on_deserialize {
            self.validator.validate(requirement)?;
        }

        serde_json::to_string_pretty(requirement)
            .map_err(|e| SerdeError::InvalidStructure(e.to_string()))
    }

    /// Deserializes a requirement from JSON format with validation
    ///
    /// # Arguments
    /// * `json_str` - JSON string to deserialize
    ///
    /// # Returns
    /// Deserialized and validated requirement
    ///
    /// # Errors
    /// Returns [`SerdeError`] if parsing fails or validation fails.
    pub fn from_json<P>(&self, json_str: &str) -> Result<Requirement<P>, SerdeError>
    where
        P: for<'de> Deserialize<'de>,
    {
        let requirement: Requirement<P> = serde_json::from_str(json_str)
            .map_err(|e| SerdeError::InvalidStructure(e.to_string()))?;

        if self.validator.config.validate_on_deserialize {
            self.validator.validate(&requirement)?;
        }

        Ok(requirement)
    }

    /// Validates a requirement without serialization
    ///
    /// Useful for checking authored requirements before saving or compilation.
    ///
    /// # Arguments
    /// * `requirement` - The requirement to validate
    ///
    /// # Returns
    /// `Ok(())` if valid, `Err(SerdeError)` with details if invalid
    ///
    /// # Errors
    /// Returns [`SerdeError`] when the requirement violates structural limits.
    pub fn validate<P>(&self, requirement: &Requirement<P>) -> Result<(), SerdeError> {
        self.validator.validate(requirement)
    }
}

impl Default for RequirementSerializer {
    fn default() -> Self {
        Self::with_defaults()
    }
}

/// Convenience functions for serialization without explicit serializer
///
/// These functions use default configuration and are suitable for most use cases.
/// For custom validation rules, create a `RequirementSerializer` explicitly.
pub mod convenience {
    use super::Deserialize;
    use super::Requirement;
    use super::RequirementSerializer;
    use super::RequirementValidator;
    use super::SerdeError;
    use super::Serialize;

    /// Serialize requirement to RON with default configuration
    ///
    /// This is the most common serialization method for saving authored requirements.
    ///
    /// # Errors
    /// Returns [`SerdeError`] if serialization fails or validation fails.
    pub fn to_ron<P: Serialize>(requirement: &Requirement<P>) -> Result<String, SerdeError> {
        RequirementSerializer::default().to_ron(requirement)
    }

    /// Deserialize requirement from RON with default configuration
    ///
    /// This is the most common deserialization method for loading authored requirements.
    ///
    /// # Errors
    /// Returns [`SerdeError`] if parsing fails or validation fails.
    pub fn from_ron<P: for<'de> Deserialize<'de>>(
        ron_str: &str,
    ) -> Result<Requirement<P>, SerdeError> {
        RequirementSerializer::default().from_ron(ron_str)
    }

    /// Serialize requirement to JSON with default configuration
    ///
    /// # Errors
    /// Returns [`SerdeError`] if serialization fails or validation fails.
    pub fn to_json<P: Serialize>(requirement: &Requirement<P>) -> Result<String, SerdeError> {
        RequirementSerializer::default().to_json(requirement)
    }

    /// Deserialize requirement from JSON with default configuration
    ///
    /// # Errors
    /// Returns [`SerdeError`] if parsing fails or validation fails.
    pub fn from_json<P: for<'de> Deserialize<'de>>(
        ron_str: &str,
    ) -> Result<Requirement<P>, SerdeError> {
        RequirementSerializer::default().from_json(ron_str)
    }

    /// Validate a requirement with default configuration
    ///
    /// Performs structural validation only. Domain-specific predicate validation
    /// happens during compilation or execution.
    ///
    /// # Errors
    /// Returns [`SerdeError`] when the requirement violates structural limits.
    pub fn validate<P>(requirement: &Requirement<P>) -> Result<(), SerdeError> {
        RequirementValidator::with_defaults().validate(requirement)
    }

    /// Quick validation check that returns a boolean
    ///
    /// Useful for simple validity checks where error details aren't needed.
    pub fn is_valid<P>(requirement: &Requirement<P>) -> bool {
        validate(requirement).is_ok()
    }
}

/// Utilities for working with RON requirement files
pub mod ron_utils {
    use std::error::Error;
    use std::fmt;
    use std::fs;
    use std::io::Read;
    use std::path::Path;

    use super::Deserialize;
    use super::Requirement;
    use super::Serialize;
    use super::convenience;

    /// Maximum allowed RON file size in bytes.
    const MAX_RON_FILE_BYTES: usize = 1024 * 1024;
    /// Maximum allowed RON file size as u64 for I/O limits.
    const MAX_RON_FILE_BYTES_U64: u64 = 1024 * 1024;

    /// Errors emitted while loading RON requirement files.
    #[derive(Debug)]
    enum RonFileError {
        /// File exceeds configured size limit.
        FileTooLarge {
            /// Maximum allowed bytes.
            max_bytes: usize,
            /// Actual file size in bytes.
            actual_bytes: usize,
        },
    }

    impl fmt::Display for RonFileError {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            match self {
                Self::FileTooLarge {
                    max_bytes,
                    actual_bytes,
                } => {
                    write!(f, "RON file exceeds size limit: {actual_bytes} bytes (max {max_bytes})")
                }
            }
        }
    }

    impl Error for RonFileError {}

    /// Reads a file into a string while enforcing a size cap.
    fn read_to_string_with_limit(path: impl AsRef<Path>) -> Result<String, Box<dyn Error>> {
        let file = fs::File::open(path)?;
        let mut contents = String::new();
        let mut limited = file.take(MAX_RON_FILE_BYTES_U64 + 1);
        limited.read_to_string(&mut contents)?;

        if contents.len() > MAX_RON_FILE_BYTES {
            return Err(Box::new(RonFileError::FileTooLarge {
                max_bytes: MAX_RON_FILE_BYTES,
                actual_bytes: contents.len(),
            }));
        }

        Ok(contents)
    }

    /// Load a requirement from a RON file
    ///
    /// # Arguments
    /// * `path` - Path to the RON file
    ///
    /// # Returns
    /// Loaded and validated requirement
    ///
    /// # Errors
    /// Returns an error if file IO fails, parsing/validation fails, or the file
    /// exceeds `MAX_RON_FILE_BYTES`.
    pub fn load_from_file<P>(
        path: impl AsRef<Path>,
    ) -> Result<Requirement<P>, Box<dyn std::error::Error>>
    where
        P: for<'de> Deserialize<'de>,
    {
        let content = read_to_string_with_limit(path)?;
        let requirement = convenience::from_ron(&content)?;
        Ok(requirement)
    }

    /// Save a requirement to a RON file
    ///
    /// # Arguments
    /// * `requirement` - The requirement to save
    /// * `path` - Path where to save the file
    ///
    /// # Errors
    /// Returns an error if serialization fails or the file cannot be written.
    pub fn save_to_file<P>(
        requirement: &Requirement<P>,
        path: impl AsRef<Path>,
    ) -> Result<(), Box<dyn std::error::Error>>
    where
        P: Serialize,
    {
        let content = convenience::to_ron(requirement)?;
        fs::write(path, content)?;
        Ok(())
    }

    /// Validate a RON file without loading into memory
    ///
    /// Useful for checking file validity during development or as part of
    /// asset validation pipelines.
    ///
    /// # Arguments
    /// * `path` - Path to the RON file to validate
    ///
    /// # Returns
    /// `Ok(())` if file is valid, error with details if invalid
    ///
    /// # Errors
    /// Returns an error if file IO fails, parsing/validation fails, or the file
    /// exceeds `MAX_RON_FILE_BYTES`.
    pub fn validate_file<P>(path: impl AsRef<Path>) -> Result<(), Box<dyn std::error::Error>>
    where
        P: for<'de> Deserialize<'de>,
    {
        let content = read_to_string_with_limit(path)?;
        convenience::validate::<P>(&convenience::from_ron(&content)?)?;
        Ok(())
    }
}
