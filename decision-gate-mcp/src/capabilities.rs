// decision-gate-mcp/src/capabilities.rs
// ============================================================================
// Module: Provider Capability Registry
// Description: Capability metadata registry for providers and predicates.
// Purpose: Validate ScenarioSpec and EvidenceQuery inputs against contracts.
// Dependencies: decision-gate-contract, decision-gate-core, jsonschema
// ============================================================================

//! ## Overview
//! The capability registry validates predicates against the canonical provider
//! contracts. It enforces comparator allow-lists and schema validation to keep
//! authoring deterministic and secure.
//! Security posture: inputs are untrusted; see `Docs/security/threat_model.md`.

// ============================================================================
// SECTION: Imports
// ============================================================================

use std::collections::BTreeMap;
use std::path::Path;

use decision_gate_contract::providers::provider_contracts;
use decision_gate_contract::types::PredicateContract;
use decision_gate_contract::types::ProviderContract;
use decision_gate_core::Comparator;
use decision_gate_core::EvidenceQuery;
use decision_gate_core::ScenarioSpec;
use jsonschema::CompilationOptions;
use jsonschema::Draft;
use jsonschema::JSONSchema;
use serde_json::Value;
use thiserror::Error;

use crate::config::DecisionGateConfig;
use crate::config::ProviderConfig;
use crate::config::ProviderType;

// ============================================================================
// SECTION: Constants
// ============================================================================

/// Maximum size for provider capability contract files (bytes).
const MAX_CAPABILITY_BYTES: usize = 1024 * 1024;
/// Maximum length of a single path component.
const MAX_PATH_COMPONENT_LENGTH: usize = 255;
/// Maximum total path length.
const MAX_TOTAL_PATH_LENGTH: usize = 4096;

// ============================================================================
// SECTION: Errors
// ============================================================================

/// Capability registry errors.
#[derive(Debug, Error)]
pub enum CapabilityError {
    /// Provider not registered in the capability registry.
    #[error("provider not registered: {provider_id}")]
    ProviderMissing {
        /// Missing provider identifier.
        provider_id: String,
    },
    /// Predicate not registered for the provider.
    #[error("predicate not supported: {provider_id}.{predicate}")]
    PredicateMissing {
        /// Provider identifier.
        provider_id: String,
        /// Predicate name.
        predicate: String,
    },
    /// Predicate parameters are required but missing.
    #[error("predicate params required for {provider_id}.{predicate}")]
    ParamsMissing {
        /// Provider identifier.
        provider_id: String,
        /// Predicate name.
        predicate: String,
    },
    /// Predicate parameters failed schema validation.
    #[error("predicate params invalid for {provider_id}.{predicate}: {error}")]
    ParamsInvalid {
        /// Provider identifier.
        provider_id: String,
        /// Predicate name.
        predicate: String,
        /// Validation error details.
        error: String,
    },
    /// Predicate expected value failed schema validation.
    #[error("predicate expected value invalid for {provider_id}.{predicate}: {error}")]
    ExpectedInvalid {
        /// Provider identifier.
        provider_id: String,
        /// Predicate name.
        predicate: String,
        /// Validation error details.
        error: String,
    },
    /// Comparator is not allowed for the predicate.
    #[error("comparator not allowed for {provider_id}.{predicate}: {comparator}")]
    ComparatorNotAllowed {
        /// Provider identifier.
        provider_id: String,
        /// Predicate name.
        predicate: String,
        /// Comparator label.
        comparator: String,
    },
    /// Provider capability contract is missing.
    #[error("provider capability contract missing for {provider_id}")]
    ContractMissing {
        /// Provider identifier.
        provider_id: String,
    },
    /// Provider capability contract cannot be read.
    #[error("provider capability contract read failed: {path}: {error}")]
    ContractRead {
        /// Path to the contract file.
        path: String,
        /// Error details.
        error: String,
    },
    /// Provider capability contract failed to parse.
    #[error("provider capability contract parse failed: {path}: {error}")]
    ContractParse {
        /// Path to the contract file.
        path: String,
        /// Error details.
        error: String,
    },
    /// Provider contract metadata is invalid.
    #[error("provider capability contract invalid: {provider_id}: {error}")]
    ContractInvalid {
        /// Provider identifier.
        provider_id: String,
        /// Error details.
        error: String,
    },
    /// Provider capability contract path is invalid.
    #[error("provider capability contract path invalid: {path}: {error}")]
    ContractPathInvalid {
        /// Path to the contract file.
        path: String,
        /// Error details.
        error: String,
    },
    /// Duplicate provider identifiers were found.
    #[error("duplicate provider id: {provider_id}")]
    DuplicateProvider {
        /// Provider identifier.
        provider_id: String,
    },
    /// Contract schema compilation failed.
    #[error("schema compilation failed for {provider_id}.{predicate}: {error}")]
    SchemaCompile {
        /// Provider identifier.
        provider_id: String,
        /// Predicate name.
        predicate: String,
        /// Error details.
        error: String,
    },
}

impl CapabilityError {
    /// Returns the stable error code for this capability error.
    #[must_use]
    pub const fn code(&self) -> &'static str {
        match self {
            Self::ProviderMissing {
                ..
            } => "provider_missing",
            Self::PredicateMissing {
                ..
            } => "predicate_missing",
            Self::ParamsMissing {
                ..
            } => "params_missing",
            Self::ParamsInvalid {
                ..
            } => "params_invalid",
            Self::ExpectedInvalid {
                ..
            } => "expected_invalid",
            Self::ComparatorNotAllowed {
                ..
            } => "comparator_not_allowed",
            Self::ContractMissing {
                ..
            } => "contract_missing",
            Self::ContractRead {
                ..
            } => "contract_read_failed",
            Self::ContractParse {
                ..
            } => "contract_parse_failed",
            Self::ContractInvalid {
                ..
            } => "contract_invalid",
            Self::ContractPathInvalid {
                ..
            } => "contract_path_invalid",
            Self::DuplicateProvider {
                ..
            } => "duplicate_provider",
            Self::SchemaCompile {
                ..
            } => "schema_compile_failed",
        }
    }
}

// ============================================================================
// SECTION: Capability Registry
// ============================================================================

/// Registry of provider capabilities derived from contracts.
pub struct CapabilityRegistry {
    /// Capability map keyed by provider identifier.
    providers: BTreeMap<String, ProviderCapabilities>,
}

/// Provider capability bundle with compiled predicate schemas.
struct ProviderCapabilities {
    /// Predicate capability map keyed by predicate name.
    predicates: BTreeMap<String, PredicateCapabilities>,
}

/// Predicate capability bundle with compiled schemas.
struct PredicateCapabilities {
    /// Predicate contract metadata.
    contract: PredicateContract,
    /// Compiled schema for predicate parameters.
    params_schema: JSONSchema,
    /// Compiled schema for predicate results.
    result_schema: JSONSchema,
}

impl CapabilityRegistry {
    /// Builds a capability registry from the MCP configuration.
    ///
    /// # Errors
    ///
    /// Returns [`CapabilityError`] when provider capabilities are missing or invalid.
    pub fn from_config(config: &DecisionGateConfig) -> Result<Self, CapabilityError> {
        let builtin_contracts = provider_contracts();
        let mut builtin_index = BTreeMap::new();
        for contract in builtin_contracts {
            builtin_index.insert(contract.provider_id.clone(), contract);
        }

        let mut providers = BTreeMap::new();
        for provider in &config.providers {
            let contract = match provider.provider_type {
                ProviderType::Builtin => builtin_contract_for(provider, &builtin_index)?,
                ProviderType::Mcp => load_external_contract(provider)?,
            };
            let provider_id = contract.provider_id.clone();
            if providers.contains_key(&provider_id) {
                return Err(CapabilityError::DuplicateProvider {
                    provider_id,
                });
            }
            let predicates = compile_predicates(&contract)?;
            providers.insert(
                provider_id,
                ProviderCapabilities {
                    predicates,
                },
            );
        }

        Ok(Self {
            providers,
        })
    }

    /// Validates a scenario spec against provider capabilities.
    ///
    /// # Errors
    ///
    /// Returns [`CapabilityError`] for missing providers, predicates, or schema violations.
    pub fn validate_spec(&self, spec: &ScenarioSpec) -> Result<(), CapabilityError> {
        for predicate in &spec.predicates {
            let provider_id = predicate.query.provider_id.as_str();
            let predicate_name = predicate.query.predicate.as_str();
            let capability = self.lookup_predicate(provider_id, predicate_name)?;
            validate_params(
                provider_id,
                predicate_name,
                predicate.query.params.as_ref(),
                capability.contract.params_required,
                &capability.params_schema,
            )?;
            validate_expected_value(
                provider_id,
                predicate_name,
                predicate.comparator,
                predicate.expected.as_ref(),
                &capability.result_schema,
            )?;
            if !capability.contract.allowed_comparators.contains(&predicate.comparator) {
                return Err(CapabilityError::ComparatorNotAllowed {
                    provider_id: provider_id.to_string(),
                    predicate: predicate_name.to_string(),
                    comparator: comparator_label(predicate.comparator),
                });
            }
        }
        Ok(())
    }

    /// Validates an evidence query against provider capabilities.
    ///
    /// # Errors
    ///
    /// Returns [`CapabilityError`] for missing providers, predicates, or schema violations.
    pub fn validate_query(&self, query: &EvidenceQuery) -> Result<(), CapabilityError> {
        let provider_id = query.provider_id.as_str();
        let predicate_name = query.predicate.as_str();
        let capability = self.lookup_predicate(provider_id, predicate_name)?;
        validate_params(
            provider_id,
            predicate_name,
            query.params.as_ref(),
            capability.contract.params_required,
            &capability.params_schema,
        )
    }

    /// Lists providers and their predicate identifiers.
    #[must_use]
    pub fn list_providers(&self) -> Vec<(String, Vec<String>)> {
        let mut providers = Vec::with_capacity(self.providers.len());
        for (provider_id, capabilities) in &self.providers {
            let predicates = capabilities.predicates.keys().cloned().collect();
            providers.push((provider_id.clone(), predicates));
        }
        providers
    }

    /// Returns the predicate contract for the requested provider.
    ///
    /// # Errors
    ///
    /// Returns [`CapabilityError`] when the provider or predicate is missing.
    pub fn predicate_contract(
        &self,
        provider_id: &str,
        predicate: &str,
    ) -> Result<&PredicateContract, CapabilityError> {
        let capability = self.lookup_predicate(provider_id, predicate)?;
        Ok(&capability.contract)
    }

    /// Locates a predicate capability by provider and predicate name.
    fn lookup_predicate(
        &self,
        provider_id: &str,
        predicate: &str,
    ) -> Result<&PredicateCapabilities, CapabilityError> {
        let provider =
            self.providers.get(provider_id).ok_or_else(|| CapabilityError::ProviderMissing {
                provider_id: provider_id.to_string(),
            })?;
        provider.predicates.get(predicate).ok_or_else(|| CapabilityError::PredicateMissing {
            provider_id: provider_id.to_string(),
            predicate: predicate.to_string(),
        })
    }
}

// ============================================================================
// SECTION: Contract Loading
// ============================================================================

/// Returns the builtin provider contract for a config entry.
fn builtin_contract_for(
    provider: &ProviderConfig,
    builtin: &BTreeMap<String, ProviderContract>,
) -> Result<ProviderContract, CapabilityError> {
    if provider.capabilities_path.is_some() {
        return Err(CapabilityError::ContractInvalid {
            provider_id: provider.name.clone(),
            error: "builtin providers must not specify capabilities_path".to_string(),
        });
    }
    builtin.get(&provider.name).cloned().ok_or_else(|| CapabilityError::ContractMissing {
        provider_id: provider.name.clone(),
    })
}

/// Loads and validates an external provider contract from disk.
fn load_external_contract(provider: &ProviderConfig) -> Result<ProviderContract, CapabilityError> {
    let path =
        provider.capabilities_path.as_ref().ok_or_else(|| CapabilityError::ContractMissing {
            provider_id: provider.name.clone(),
        })?;
    validate_capability_path(path)?;
    let bytes = std::fs::read(path).map_err(|err| CapabilityError::ContractRead {
        path: path.display().to_string(),
        error: err.to_string(),
    })?;
    if bytes.len() > MAX_CAPABILITY_BYTES {
        return Err(CapabilityError::ContractInvalid {
            provider_id: provider.name.clone(),
            error: "capability contract exceeds size limit".to_string(),
        });
    }
    let contract: ProviderContract =
        serde_json::from_slice(&bytes).map_err(|err| CapabilityError::ContractParse {
            path: path.display().to_string(),
            error: err.to_string(),
        })?;
    if contract.provider_id != provider.name {
        return Err(CapabilityError::ContractInvalid {
            provider_id: provider.name.clone(),
            error: format!(
                "provider id mismatch (expected {}, got {})",
                provider.name, contract.provider_id
            ),
        });
    }
    if contract.transport != "mcp" {
        return Err(CapabilityError::ContractInvalid {
            provider_id: provider.name.clone(),
            error: "external providers must declare transport=mcp".to_string(),
        });
    }
    Ok(contract)
}

/// Validates provider contract paths for length limits.
fn validate_capability_path(path: &Path) -> Result<(), CapabilityError> {
    let path_string = path.display().to_string();
    if path_string.len() > MAX_TOTAL_PATH_LENGTH {
        return Err(CapabilityError::ContractPathInvalid {
            path: path_string,
            error: "path exceeds max length".to_string(),
        });
    }
    for component in path.components() {
        let name = component.as_os_str().to_string_lossy();
        if name.len() > MAX_PATH_COMPONENT_LENGTH {
            return Err(CapabilityError::ContractPathInvalid {
                path: path.display().to_string(),
                error: format!("path component too long: {name}"),
            });
        }
    }
    Ok(())
}

// ============================================================================
// SECTION: Predicate Compilation
// ============================================================================

/// Compiles predicate schemas for a provider contract.
fn compile_predicates(
    contract: &ProviderContract,
) -> Result<BTreeMap<String, PredicateCapabilities>, CapabilityError> {
    let mut predicates = BTreeMap::new();
    for predicate in &contract.predicates {
        let params_schema = compile_schema(contract, predicate, &predicate.params_schema)?;
        let result_schema = compile_schema(contract, predicate, &predicate.result_schema)?;
        if predicate.allowed_comparators.is_empty() {
            return Err(CapabilityError::ContractInvalid {
                provider_id: contract.provider_id.clone(),
                error: format!("predicate {} missing allowed_comparators", predicate.name),
            });
        }
        if !is_canonical_comparator_order(&predicate.allowed_comparators) {
            return Err(CapabilityError::ContractInvalid {
                provider_id: contract.provider_id.clone(),
                error: format!("predicate {} comparators not in canonical order", predicate.name),
            });
        }
        predicates.insert(
            predicate.name.clone(),
            PredicateCapabilities {
                contract: predicate.clone(),
                params_schema,
                result_schema,
            },
        );
    }
    Ok(predicates)
}

/// Compiles a JSON schema with Decision Gate defaults.
fn compile_schema(
    provider: &ProviderContract,
    predicate: &PredicateContract,
    schema: &Value,
) -> Result<JSONSchema, CapabilityError> {
    let mut options = CompilationOptions::default();
    options.with_draft(Draft::Draft202012);
    options.compile(schema).map_err(|err| CapabilityError::SchemaCompile {
        provider_id: provider.provider_id.clone(),
        predicate: predicate.name.clone(),
        error: err.to_string(),
    })
}

// ============================================================================
// SECTION: Validation Helpers
// ============================================================================

/// Validates predicate params against schema and required flag.
fn validate_params(
    provider_id: &str,
    predicate: &str,
    params: Option<&Value>,
    params_required: bool,
    schema: &JSONSchema,
) -> Result<(), CapabilityError> {
    if params_required && params.is_none() {
        return Err(CapabilityError::ParamsMissing {
            provider_id: provider_id.to_string(),
            predicate: predicate.to_string(),
        });
    }
    let Some(params) = params else {
        return Ok(());
    };
    validate_schema_value(provider_id, predicate, params, schema).map_err(|error| {
        CapabilityError::ParamsInvalid {
            provider_id: provider_id.to_string(),
            predicate: predicate.to_string(),
            error,
        }
    })
}

/// Validates a JSON value against a compiled schema.
fn validate_schema_value(
    provider_id: &str,
    predicate: &str,
    value: &Value,
    schema: &JSONSchema,
) -> Result<(), String> {
    match schema.validate(value) {
        Ok(()) => Ok(()),
        Err(errors) => {
            let messages: Vec<String> = errors.map(|err| err.to_string()).collect();
            let summary = messages.join("; ");
            Err(format!("{provider_id}.{predicate}: {summary}"))
        }
    }
}

/// Validates expected values against predicate comparators.
fn validate_expected_value(
    provider_id: &str,
    predicate: &str,
    comparator: Comparator,
    expected: Option<&Value>,
    schema: &JSONSchema,
) -> Result<(), CapabilityError> {
    match comparator {
        Comparator::Exists | Comparator::NotExists => {
            if expected.is_some() {
                return Err(CapabilityError::ExpectedInvalid {
                    provider_id: provider_id.to_string(),
                    predicate: predicate.to_string(),
                    error: "expected value must be omitted for exists/not_exists".to_string(),
                });
            }
            Ok(())
        }
        Comparator::InSet => {
            let expected = expected.ok_or_else(|| CapabilityError::ExpectedInvalid {
                provider_id: provider_id.to_string(),
                predicate: predicate.to_string(),
                error: "expected array required for in_set comparator".to_string(),
            })?;
            let Value::Array(values) = expected else {
                return Err(CapabilityError::ExpectedInvalid {
                    provider_id: provider_id.to_string(),
                    predicate: predicate.to_string(),
                    error: "expected array required for in_set comparator".to_string(),
                });
            };
            for value in values {
                validate_schema_value(provider_id, predicate, value, schema).map_err(|error| {
                    CapabilityError::ExpectedInvalid {
                        provider_id: provider_id.to_string(),
                        predicate: predicate.to_string(),
                        error,
                    }
                })?;
            }
            Ok(())
        }
        _ => {
            let expected = expected.ok_or_else(|| CapabilityError::ExpectedInvalid {
                provider_id: provider_id.to_string(),
                predicate: predicate.to_string(),
                error: "expected value required for comparator".to_string(),
            })?;
            validate_schema_value(provider_id, predicate, expected, schema).map_err(|error| {
                CapabilityError::ExpectedInvalid {
                    provider_id: provider_id.to_string(),
                    predicate: predicate.to_string(),
                    error,
                }
            })
        }
    }
}

/// Returns the comparator label used in error messages.
fn comparator_label(comparator: Comparator) -> String {
    match comparator {
        Comparator::Equals => "equals".to_string(),
        Comparator::NotEquals => "not_equals".to_string(),
        Comparator::GreaterThan => "greater_than".to_string(),
        Comparator::GreaterThanOrEqual => "greater_than_or_equal".to_string(),
        Comparator::LessThan => "less_than".to_string(),
        Comparator::LessThanOrEqual => "less_than_or_equal".to_string(),
        Comparator::LexGreaterThan => "lex_greater_than".to_string(),
        Comparator::LexGreaterThanOrEqual => "lex_greater_than_or_equal".to_string(),
        Comparator::LexLessThan => "lex_less_than".to_string(),
        Comparator::LexLessThanOrEqual => "lex_less_than_or_equal".to_string(),
        Comparator::Contains => "contains".to_string(),
        Comparator::InSet => "in_set".to_string(),
        Comparator::DeepEquals => "deep_equals".to_string(),
        Comparator::DeepNotEquals => "deep_not_equals".to_string(),
        Comparator::Exists => "exists".to_string(),
        Comparator::NotExists => "not_exists".to_string(),
    }
}

/// Returns true when comparators follow canonical ordering.
fn is_canonical_comparator_order(comparators: &[Comparator]) -> bool {
    let mut indices = Vec::new();
    for comparator in comparators {
        if let Some(index) = comparator_index(*comparator) {
            indices.push(index);
        }
    }
    indices.windows(2).all(|pair| pair[0] <= pair[1])
}

/// Returns the canonical comparator index.
fn comparator_index(comparator: Comparator) -> Option<usize> {
    comparator_order().iter().position(|candidate| *candidate == comparator)
}

/// Returns the canonical comparator ordering.
const fn comparator_order() -> [Comparator; 16] {
    [
        Comparator::Equals,
        Comparator::NotEquals,
        Comparator::GreaterThan,
        Comparator::GreaterThanOrEqual,
        Comparator::LessThan,
        Comparator::LessThanOrEqual,
        Comparator::LexGreaterThan,
        Comparator::LexGreaterThanOrEqual,
        Comparator::LexLessThan,
        Comparator::LexLessThanOrEqual,
        Comparator::Contains,
        Comparator::InSet,
        Comparator::DeepEquals,
        Comparator::DeepNotEquals,
        Comparator::Exists,
        Comparator::NotExists,
    ]
}
