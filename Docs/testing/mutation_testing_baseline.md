# Mutation Testing Baseline

## Tool
- cargo-mutants (recommended for Rust mutation testing)

## Baseline Targets
- decision-gate-core (comparator, control plane runtime)
- decision-gate-providers (http, json, env providers)
- decision-gate-store-sqlite (run state store integrity)

## Suggested Commands
- cargo mutants --package decision-gate-core -- --lib
- cargo mutants --package decision-gate-providers -- --lib
- cargo mutants --package decision-gate-store-sqlite -- --lib

## Initial Baseline Expectations
- Comparator and hashing modules should exceed 75% mutant kill rate
- Provider policy modules should exceed 70% mutant kill rate
- Store integrity modules should exceed 75% mutant kill rate

## Maintenance
- Record baseline results in CI logs
- For any mutant that survives, add a focused unit test or justify exclusion
