// crates/decision-gate-providers/src/registry.rs
// ============================================================================
// Module: Provider Registry
// Description: Registry for built-in and external evidence providers.
// Purpose: Route evidence queries by provider identifier with policy checks.
// Dependencies: decision-gate-core
// ============================================================================

//! ## Overview
//! The provider registry resolves evidence queries by provider identifier and
//! enforces allowlist and denylist policies. It implements the core
//! [`decision_gate_core::EvidenceProvider`] interface for seamless integration
//! with the control plane engine.
//! Security posture: evidence inputs are untrusted; see `Docs/security/threat_model.md`.

// ============================================================================
// SECTION: Imports
// ============================================================================

use std::collections::BTreeMap;
use std::collections::BTreeSet;

use decision_gate_core::EvidenceContext;
use decision_gate_core::EvidenceError;
use decision_gate_core::EvidenceProvider;
use decision_gate_core::EvidenceQuery;
use decision_gate_core::EvidenceResult;
use decision_gate_core::ProviderMissingError;
use decision_gate_core::ScenarioSpec;

use crate::EnvProvider;
use crate::EnvProviderConfig;
use crate::HttpProvider;
use crate::HttpProviderConfig;
use crate::JsonProvider;
use crate::JsonProviderConfig;
use crate::TimeProvider;
use crate::TimeProviderConfig;

// ============================================================================
// SECTION: Built-in Config
// ============================================================================

/// Configuration bundle for built-in providers.
///
/// # Invariants
/// - `json` must include a valid root and `root_id`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuiltinProviderConfigs {
    /// Configuration for the time provider.
    pub time: TimeProviderConfig,
    /// Configuration for the env provider.
    pub env: EnvProviderConfig,
    /// Configuration for the json provider.
    pub json: JsonProviderConfig,
    /// Configuration for the http provider.
    pub http: HttpProviderConfig,
}

impl BuiltinProviderConfigs {
    /// Creates configs with defaults except for the required JSON provider config.
    #[must_use]
    pub fn new(json: JsonProviderConfig) -> Self {
        Self {
            time: TimeProviderConfig::default(),
            env: EnvProviderConfig::default(),
            json,
            http: HttpProviderConfig::default(),
        }
    }
}

// ============================================================================
// SECTION: Access Policy
// ============================================================================

/// Access policy controlling which providers may be queried.
///
/// # Invariants
/// - `denylist` overrides `allowlist` when both are present.
/// - If `allowlist` is `None`, all providers are allowed unless denied.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderAccessPolicy {
    /// Optional allowlist of provider identifiers.
    pub allowlist: Option<BTreeSet<String>>,
    /// Explicit denylist of provider identifiers.
    pub denylist: BTreeSet<String>,
}

impl ProviderAccessPolicy {
    /// Returns a policy that permits all providers.
    #[must_use]
    pub const fn allow_all() -> Self {
        Self {
            allowlist: None,
            denylist: BTreeSet::new(),
        }
    }

    /// Returns true when the provider is allowed by policy.
    #[must_use]
    pub fn is_allowed(&self, provider_id: &str) -> bool {
        if self.denylist.contains(provider_id) {
            return false;
        }
        if let Some(allowlist) = &self.allowlist {
            return allowlist.contains(provider_id);
        }
        true
    }
}

impl Default for ProviderAccessPolicy {
    fn default() -> Self {
        Self::allow_all()
    }
}

// ============================================================================
// SECTION: Provider Registry
// ============================================================================

/// Evidence provider registry with policy enforcement.
///
/// # Invariants
/// - Provider identifiers are unique within the registry.
/// - Access policy is enforced on every query.
/// - Registered providers are `Send + Sync` and stored behind trait objects.
pub struct ProviderRegistry {
    /// Provider implementations keyed by provider identifier.
    providers: BTreeMap<String, Box<dyn EvidenceProvider + Send + Sync>>,
    /// Access control policy for provider usage.
    policy: ProviderAccessPolicy,
}

impl ProviderRegistry {
    /// Creates a new registry with the provided policy.
    #[must_use]
    pub fn new(policy: ProviderAccessPolicy) -> Self {
        Self {
            providers: BTreeMap::new(),
            policy,
        }
    }

    /// Creates a registry with built-in providers registered.
    ///
    /// # Errors
    ///
    /// Returns [`EvidenceError`] when provider initialization fails.
    pub fn with_builtin_providers(configs: BuiltinProviderConfigs) -> Result<Self, EvidenceError> {
        let mut registry = Self::new(ProviderAccessPolicy::default());
        registry.register_builtin_providers(configs)?;
        Ok(registry)
    }

    /// Registers a new provider under the given identifier.
    ///
    /// # Errors
    ///
    /// Returns [`EvidenceError`] when the provider identifier is already registered.
    pub fn register_provider(
        &mut self,
        provider_id: impl Into<String>,
        provider: impl EvidenceProvider + Send + Sync + 'static,
    ) -> Result<(), EvidenceError> {
        let provider_id = provider_id.into();
        if self.providers.contains_key(&provider_id) {
            return Err(EvidenceError::Provider(format!(
                "provider already registered: {provider_id}"
            )));
        }
        self.providers.insert(provider_id, Box::new(provider));
        Ok(())
    }

    /// Registers built-in providers with default configuration.
    ///
    /// # Errors
    ///
    /// Returns [`EvidenceError`] when provider initialization fails.
    pub fn register_builtin_providers(
        &mut self,
        configs: BuiltinProviderConfigs,
    ) -> Result<(), EvidenceError> {
        self.register_provider("time", TimeProvider::new(configs.time))?;
        self.register_provider("env", EnvProvider::new(configs.env))?;
        self.register_provider("json", JsonProvider::new(configs.json)?)?;
        let http = HttpProvider::new(configs.http)?;
        self.register_provider("http", http)?;
        Ok(())
    }

    /// Returns the configured policy.
    #[must_use]
    pub const fn policy(&self) -> &ProviderAccessPolicy {
        &self.policy
    }
}

impl EvidenceProvider for ProviderRegistry {
    fn query(
        &self,
        query: &EvidenceQuery,
        ctx: &EvidenceContext,
    ) -> Result<EvidenceResult, EvidenceError> {
        let provider_id = query.provider_id.as_str();
        if !self.policy.is_allowed(provider_id) {
            return Err(EvidenceError::Provider(format!(
                "provider blocked by policy: {provider_id}"
            )));
        }
        let Some(provider) = self.providers.get(provider_id) else {
            return Err(EvidenceError::Provider(format!("provider not registered: {provider_id}")));
        };
        provider.query(query, ctx)
    }

    fn validate_providers(&self, spec: &ScenarioSpec) -> Result<(), ProviderMissingError> {
        let mut missing = BTreeSet::new();
        let mut required = BTreeSet::new();
        let mut blocked_by_policy = false;

        for condition in &spec.conditions {
            let provider_id = condition.query.provider_id.as_str();
            let check_id = condition.query.check_id.as_str();
            let exists = self.providers.contains_key(provider_id);
            let allowed = self.policy.is_allowed(provider_id);
            if !exists || !allowed {
                missing.insert(provider_id.to_string());
                required.insert(check_id.to_string());
                if exists && !allowed {
                    blocked_by_policy = true;
                }
            }
        }

        if missing.is_empty() {
            return Ok(());
        }

        Err(ProviderMissingError {
            missing_providers: missing.into_iter().collect(),
            required_capabilities: required.into_iter().collect(),
            blocked_by_policy,
        })
    }
}
