<!--
sdks/typescript/README.md
============================================================================
Document: Decision Gate TypeScript SDK
Description: Usage guide for the Decision Gate TypeScript client.
Purpose: Provide quickstart instructions for SDK consumers.
============================================================================
-->

# Decision Gate TypeScript SDK

TypeScript client for Decision Gate MCP JSON-RPC tools.

## Typed + documented models

The SDK ships generated interfaces with field-level documentation derived from
the Decision Gate contract (schemas + tool notes).

## Install (local)

```bash dg-skip dg-reason="local install step" dg-expires=2026-12-31
npm install
npm run build
```

## Usage

```typescript dg-run dg-level=fast dg-server=mcp dg-session=sdk-typescript dg-requires=node,cargo dg-script-root=sdks/typescript
import { DecisionGateClient } from "./src/index.ts";

const endpoint = process.env.DG_ENDPOINT ?? "http://127.0.0.1:8080/rpc";
const token = process.env.DG_TOKEN ?? "token-1";

const client = new DecisionGateClient({
  endpoint,
  authToken: token,
});

const response = await client.scenario_define({
  spec: {
    scenario_id: "example-scenario",
    spec_version: "1",
    namespace_id: 1,
    default_tenant_id: null,
    policies: [],
    conditions: [],
    schemas: [],
    stages: [],
  },
});
console.log(response);
```

## Validation (optional)

Runtime validation helpers are generated alongside the types. Install a JSON
schema validator (Ajv) and call the per-tool helpers.

```bash dg-skip dg-reason="optional validation extra" dg-expires=2026-12-31
npm install ajv
```

```typescript dg-skip dg-reason="requires optional validator" dg-expires=2026-12-31
import { validateScenarioDefineRequestWithAjv } from "./src/index.ts";

await validateScenarioDefineRequestWithAjv({
  spec: {
    scenario_id: "example-scenario",
    spec_version: "1",
    namespace_id: 1,
    default_tenant_id: null,
    policies: [],
    conditions: [],
    schemas: [],
    stages: [],
  },
});
```
