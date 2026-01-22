# Decision Gate Built-in Providers

This document summarizes built-in providers. Full schemas are in `providers.json`.

## time

Deterministic predicates derived from the trigger timestamp supplied by the caller.

Predicates:
- now: Return the trigger timestamp as a JSON number.
- after: Return true if trigger time is after the threshold.
- before: Return true if trigger time is before the threshold.

Notes:
- Deterministic: no wall-clock reads, only trigger timestamps.
- Supports unix_millis and logical trigger timestamps.

## env

Reads process environment variables with allow/deny policy and size limits.

Predicates:
- get: Fetch an environment variable by key.

Notes:
- Returns null when a key is missing or blocked by policy.
- Size limits apply to both key and value.

## json

Reads JSON or YAML files and evaluates JSONPath queries against them.

Predicates:
- path: Select values via JSONPath from a JSON/YAML file.

Notes:
- File access is constrained by root policy and size limits.
- JSONPath is optional; omitted means the full document.

## http

Issues bounded HTTP GET requests and returns status codes or body hashes.

Predicates:
- status: Return HTTP status code for a URL.
- body_hash: Return a hash of the response body.

Notes:
- Scheme and host allowlists are enforced by configuration.
- Responses are size-limited and hashed deterministically.

