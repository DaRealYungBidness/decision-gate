# Decision Gate Built-in Providers

This document summarizes built-in providers. Full schemas are in `providers.json`.

## time

Deterministic checks derived from the trigger timestamp supplied by the caller.

**Provider contract**

- Name: Time Provider
- Transport: builtin

**Notes**

- Deterministic: no wall-clock reads, only trigger timestamps.
- Supports unix_millis and logical trigger timestamps.

### Configuration schema

Config fields:

- `allow_logical` (optional): Allow logical trigger timestamps in comparisons. Default: true.

```json
{
  "additionalProperties": false,
  "properties": {
    "allow_logical": {
      "default": true,
      "description": "Allow logical trigger timestamps in comparisons.",
      "type": "boolean"
    }
  },
  "type": "object"
}
```

### Checks

#### now

Return the trigger timestamp as a JSON number.

- Determinism: time_dependent
- Params required: no
- Allowed comparators: equals, not_equals, greater_than, greater_than_or_equal, less_than, less_than_or_equal, in_set, exists, not_exists
- Anchor types: trigger_time_unix_millis, trigger_time_logical
- Content types: application/json

Params fields:

_No fields._

Params schema:
```json
{
  "additionalProperties": false,
  "description": "No parameters required.",
  "properties": {},
  "type": "object"
}
```
Result schema:
```json
{
  "type": "integer"
}
```
Examples:

Return trigger time.

Params:
```json
{}
```
Result:
```json
1710000000000
```

#### after

Return true if trigger time is after the threshold.

- Determinism: time_dependent
- Params required: yes
- Allowed comparators: equals, not_equals, in_set, exists, not_exists
- Anchor types: trigger_time_unix_millis, trigger_time_logical
- Content types: application/json

Params fields:

- `timestamp` (required): Unix millis number or RFC3339 timestamp string.

Params schema:
```json
{
  "additionalProperties": false,
  "properties": {
    "timestamp": {
      "description": "Unix millis number or RFC3339 timestamp string.",
      "oneOf": [
        {
          "type": "integer"
        },
        {
          "type": "string"
        }
      ]
    }
  },
  "required": [
    "timestamp"
  ],
  "type": "object"
}
```
Result schema:
```json
{
  "type": "boolean"
}
```
Examples:

Trigger time after threshold.

Params:
```json
{
  "timestamp": 1710000000000
}
```
Result:
```json
true
```

#### before

Return true if trigger time is before the threshold.

- Determinism: time_dependent
- Params required: yes
- Allowed comparators: equals, not_equals, in_set, exists, not_exists
- Anchor types: trigger_time_unix_millis, trigger_time_logical
- Content types: application/json

Params fields:

- `timestamp` (required): Unix millis number or RFC3339 timestamp string.

Params schema:
```json
{
  "additionalProperties": false,
  "properties": {
    "timestamp": {
      "description": "Unix millis number or RFC3339 timestamp string.",
      "oneOf": [
        {
          "type": "integer"
        },
        {
          "type": "string"
        }
      ]
    }
  },
  "required": [
    "timestamp"
  ],
  "type": "object"
}
```
Result schema:
```json
{
  "type": "boolean"
}
```
Examples:

Trigger time before threshold.

Params:
```json
{
  "timestamp": "2024-01-01T00:00:00Z"
}
```
Result:
```json
false
```

## env

Reads process environment variables with allow/deny policy and size limits.

**Provider contract**

- Name: Environment Provider
- Transport: builtin

**Notes**

- Returns null when a key is missing or blocked by policy.
- Size limits apply to both key and value.

### Configuration schema

Config fields:

- `allowlist` (optional): Optional allowlist of environment keys.
- `denylist` (optional): Explicit denylist of environment keys. Default: [].
- `max_key_bytes` (optional): Maximum bytes allowed for an environment key. Default: 255.
- `max_value_bytes` (optional): Maximum bytes allowed for an environment value. Default: 65536.
- `overrides` (optional): Optional deterministic override map for env lookups.

```json
{
  "additionalProperties": false,
  "properties": {
    "allowlist": {
      "description": "Optional allowlist of environment keys.",
      "items": {
        "type": "string"
      },
      "type": "array"
    },
    "denylist": {
      "default": [],
      "description": "Explicit denylist of environment keys.",
      "items": {
        "type": "string"
      },
      "type": "array"
    },
    "max_key_bytes": {
      "default": 255,
      "description": "Maximum bytes allowed for an environment key.",
      "minimum": 0,
      "type": "integer"
    },
    "max_value_bytes": {
      "default": 65536,
      "description": "Maximum bytes allowed for an environment value.",
      "minimum": 0,
      "type": "integer"
    },
    "overrides": {
      "additionalProperties": {
        "type": "string"
      },
      "description": "Optional deterministic override map for env lookups.",
      "type": "object"
    }
  },
  "type": "object"
}
```

### Checks

#### get

Fetch an environment variable by key.

- Determinism: external
- Params required: yes
- Allowed comparators: equals, not_equals, contains, in_set, exists, not_exists
- Anchor types: env
- Content types: text/plain

Params fields:

- `key` (required): Environment variable key.

Params schema:
```json
{
  "additionalProperties": false,
  "properties": {
    "key": {
      "description": "Environment variable key.",
      "type": "string"
    }
  },
  "required": [
    "key"
  ],
  "type": "object"
}
```
Result schema:
```json
{
  "oneOf": [
    {
      "type": "string"
    },
    {
      "type": "null"
    }
  ]
}
```
Examples:

Read DEPLOY_ENV.

Params:
```json
{
  "key": "DEPLOY_ENV"
}
```
Result:
```json
"production"
```

## json

Reads JSON or YAML files and evaluates JSONPath queries against them.

**Provider contract**

- Name: JSON Provider
- Transport: builtin

**Notes**

- File access is constrained by the configured root and size limits.
- File paths must be root-relative; absolute paths are rejected.
- JSONPath is optional; omitted means the full document.
- Missing JSONPath yields a null value with error metadata (jsonpath_not_found).

### Configuration schema

Config fields:

- `allow_yaml` (optional): Allow YAML parsing for .yaml/.yml files. Default: true.
- `max_bytes` (optional): Maximum file size in bytes. Default: 1048576.
- `root` (required): Root directory for file resolution (required).
- `root_id` (required): Stable identifier for the configured root (required).

```json
{
  "additionalProperties": false,
  "properties": {
    "allow_yaml": {
      "default": true,
      "description": "Allow YAML parsing for .yaml/.yml files.",
      "type": "boolean"
    },
    "max_bytes": {
      "default": 1048576,
      "description": "Maximum file size in bytes.",
      "minimum": 0,
      "type": "integer"
    },
    "root": {
      "description": "Root directory for file resolution (required).",
      "type": "string"
    },
    "root_id": {
      "description": "Stable identifier for the configured root (required).",
      "pattern": "^[a-z0-9][a-z0-9_-]{0,63}$",
      "type": "string"
    }
  },
  "required": [
    "root",
    "root_id"
  ],
  "type": "object"
}
```

### Checks

#### path

Select values via JSONPath from a JSON/YAML file.

- Determinism: external
- Params required: yes
- Allowed comparators: equals, not_equals, greater_than, greater_than_or_equal, less_than, less_than_or_equal, lex_greater_than, lex_greater_than_or_equal, lex_less_than, lex_less_than_or_equal, contains, in_set, deep_equals, deep_not_equals, exists, not_exists
- Anchor types: file_path_rooted
- Content types: application/json, application/yaml

Params fields:

- `file` (required): Path to a JSON or YAML file.
- `jsonpath` (optional): Optional JSONPath selector.

Params schema:
```json
{
  "additionalProperties": false,
  "properties": {
    "file": {
      "description": "Path to a JSON or YAML file.",
      "type": "string"
    },
    "jsonpath": {
      "description": "Optional JSONPath selector.",
      "type": "string"
    }
  },
  "required": [
    "file"
  ],
  "type": "object"
}
```
Result schema:
```json
{
  "description": "JSONPath result value (dynamic JSON type).",
  "x-decision-gate": {
    "dynamic_type": true
  }
}
```
Examples:

Example 1: Read version from config.json (root-relative path).

Params:
```json
{
  "file": "config.json",
  "jsonpath": "$.version"
}
```
Result:
```json
"1.2.3"
```
Example 2: Return full document when jsonpath is omitted.

Params:
```json
{
  "file": "config.json"
}
```
Result:
```json
{
  "version": "1.2.3"
}
```

## http

Issues bounded HTTP GET requests and returns status codes or body hashes.

**Provider contract**

- Name: HTTP Provider
- Transport: builtin

**Notes**

- Scheme and host allowlists are enforced with DNS resolution pinning per request.
- Private/link-local destinations are blocked by default unless explicitly enabled.
- Responses are size-limited and hashed deterministically.

### Configuration schema

Config fields:

- `allow_http` (optional): Allow cleartext http:// URLs. Default: false.
- `allow_private_networks` (optional): Allow private/link-local/loopback destination addresses. Default: false.
- `allowed_hosts` (optional): Optional allowlist of hostnames.
- `hash_algorithm` (optional): Hash algorithm used for body_hash responses. Default: "sha256".
- `max_response_bytes` (optional): Maximum response size in bytes. Default: 1048576.
- `timeout_ms` (optional): Request timeout in milliseconds. Default: 5000.
- `user_agent` (optional): User agent string for outbound requests. Default: "decision-gate/0.1".

```json
{
  "additionalProperties": false,
  "properties": {
    "allow_http": {
      "default": false,
      "description": "Allow cleartext http:// URLs.",
      "type": "boolean"
    },
    "allow_private_networks": {
      "default": false,
      "description": "Allow private/link-local/loopback destination addresses.",
      "type": "boolean"
    },
    "allowed_hosts": {
      "description": "Optional allowlist of hostnames.",
      "items": {
        "type": "string"
      },
      "type": "array"
    },
    "hash_algorithm": {
      "default": "sha256",
      "description": "Hash algorithm used for body_hash responses.",
      "enum": [
        "sha256"
      ],
      "type": "string"
    },
    "max_response_bytes": {
      "default": 1048576,
      "description": "Maximum response size in bytes.",
      "minimum": 0,
      "type": "integer"
    },
    "timeout_ms": {
      "default": 5000,
      "description": "Request timeout in milliseconds.",
      "minimum": 0,
      "type": "integer"
    },
    "user_agent": {
      "default": "decision-gate/0.1",
      "description": "User agent string for outbound requests.",
      "type": "string"
    }
  },
  "type": "object"
}
```

### Checks

#### status

Return HTTP status code for a URL.

- Determinism: external
- Params required: yes
- Allowed comparators: equals, not_equals, greater_than, greater_than_or_equal, less_than, less_than_or_equal, in_set, exists, not_exists
- Anchor types: url
- Content types: application/json

Params fields:

- `url` (required): URL to query.

Params schema:
```json
{
  "additionalProperties": false,
  "properties": {
    "url": {
      "description": "URL to query.",
      "type": "string"
    }
  },
  "required": [
    "url"
  ],
  "type": "object"
}
```
Result schema:
```json
{
  "type": "integer"
}
```
Examples:

Fetch status for a health endpoint.

Params:
```json
{
  "url": "https://api.example.com/health"
}
```
Result:
```json
200
```

#### body_hash

Return a hash of the response body.

- Determinism: external
- Params required: yes
- Allowed comparators: exists, not_exists
- Anchor types: url
- Content types: application/json

Params fields:

- `url` (required): URL to query.

Params schema:
```json
{
  "additionalProperties": false,
  "properties": {
    "url": {
      "description": "URL to query.",
      "type": "string"
    }
  },
  "required": [
    "url"
  ],
  "type": "object"
}
```
Result schema:
```json
{
  "additionalProperties": false,
  "properties": {
    "algorithm": {
      "enum": [
        "sha256"
      ],
      "type": "string"
    },
    "value": {
      "description": "Lowercase hex digest.",
      "type": "string"
    }
  },
  "required": [
    "algorithm",
    "value"
  ],
  "type": "object"
}
```
Examples:

Hash the body of a health endpoint.

Params:
```json
{
  "url": "https://api.example.com/health"
}
```
Result:
```json
{
  "algorithm": "sha256",
  "value": "7b4d0d3d16c8f85f67ad79b0870a2c9f1e88924c4cbb4ed4bb7f5c6a1d1b7f9a"
}
```

