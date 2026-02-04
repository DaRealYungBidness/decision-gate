// system-tests/src/config/env_tests.rs
// ============================================================================
// Module: System Test Env Unit Tests
// Description: Unit coverage for strict environment parsing in system-tests.
// Purpose: Ensure configuration parsing fails closed on invalid inputs.
// Dependencies: std
// ============================================================================

//! ## Overview
//! Unit coverage for strict environment parsing in system-tests.
//! Purpose: Ensure configuration parsing fails closed on invalid inputs.
//! Invariants:
//! - Environment parsing rejects invalid or empty values.
//! - Tests restore environment state after each run.

use std::sync::Mutex;
use std::sync::OnceLock;
use std::time::Duration;

use super::SystemTestConfig;
use super::SystemTestEnv;

fn env_lock() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(())).lock().expect("env lock poisoned")
}

struct EnvGuard {
    entries: Vec<(&'static str, Option<String>)>,
}

impl EnvGuard {
    fn new(names: &[&'static str]) -> Self {
        let entries = names.iter().map(|name| (*name, std::env::var(*name).ok())).collect();
        Self {
            entries,
        }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        for (name, value) in self.entries.drain(..) {
            match value {
                Some(value) => std::env::set_var(name, value),
                None => std::env::remove_var(name),
            }
        }
    }
}

fn env_names() -> [&'static str; 5] {
    [
        SystemTestEnv::RunRoot.as_str(),
        SystemTestEnv::HttpBind.as_str(),
        SystemTestEnv::ProviderUrl.as_str(),
        SystemTestEnv::TimeoutSeconds.as_str(),
        SystemTestEnv::AllowOverwrite.as_str(),
    ]
}

#[test]
fn timeout_rejects_invalid_values() {
    let _lock = env_lock();
    let _guard = EnvGuard::new(&env_names());

    std::env::set_var(SystemTestEnv::TimeoutSeconds.as_str(), "0");
    assert!(SystemTestConfig::load().is_err());

    std::env::set_var(SystemTestEnv::TimeoutSeconds.as_str(), "not-a-number");
    assert!(SystemTestConfig::load().is_err());

    std::env::set_var(SystemTestEnv::TimeoutSeconds.as_str(), "   ");
    assert!(SystemTestConfig::load().is_err());
}

#[test]
fn timeout_accepts_positive_values() {
    let _lock = env_lock();
    let _guard = EnvGuard::new(&env_names());

    std::env::set_var(SystemTestEnv::TimeoutSeconds.as_str(), "5");
    let config = SystemTestConfig::load().expect("config should load");
    assert_eq!(config.timeout, Some(Duration::from_secs(5)));
}

#[test]
fn allow_overwrite_parses_bool_values() {
    let _lock = env_lock();
    let _guard = EnvGuard::new(&env_names());

    std::env::set_var(SystemTestEnv::AllowOverwrite.as_str(), "1");
    let config = SystemTestConfig::load().expect("config should load");
    assert!(config.allow_overwrite);

    std::env::set_var(SystemTestEnv::AllowOverwrite.as_str(), "false");
    let config = SystemTestConfig::load().expect("config should load");
    assert!(!config.allow_overwrite);
}

#[test]
fn allow_overwrite_rejects_invalid_values() {
    let _lock = env_lock();
    let _guard = EnvGuard::new(&env_names());

    std::env::set_var(SystemTestEnv::AllowOverwrite.as_str(), "maybe");
    assert!(SystemTestConfig::load().is_err());
}

#[test]
fn empty_values_fail_closed() {
    let _lock = env_lock();
    let _guard = EnvGuard::new(&env_names());

    std::env::set_var(SystemTestEnv::RunRoot.as_str(), "");
    assert!(SystemTestConfig::load().is_err());
}
