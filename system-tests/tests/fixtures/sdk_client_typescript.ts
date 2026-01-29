// system-tests/tests/fixtures/sdk_client_typescript.ts
// ============================================================================
// Module: SDK Client Test Script (TypeScript)
// Description: Exercises the Decision Gate TypeScript SDK against MCP HTTP.
// Purpose: Validate SDK transport + tool invocations in system-tests.
// Dependencies: Decision Gate SDK, fetch API
// ============================================================================

import { DecisionGateClient, DecisionGateTransportError } from "../../sdks/typescript/src/index.ts";

function loadEnvJson(name: string): unknown {
  const value = process.env[name];
  if (!value) {
    throw new Error(`missing env ${name}`);
  }
  return JSON.parse(value);
}

async function main(): Promise<number> {
  const endpoint = process.env.DG_ENDPOINT;
  if (!endpoint) {
    throw new Error("missing DG_ENDPOINT");
  }
  const token = process.env.DG_TOKEN;
  const expectFailure = process.env.DG_EXPECT_FAILURE === "1";

  const spec = loadEnvJson("DG_SCENARIO_SPEC");
  const runConfig = loadEnvJson("DG_RUN_CONFIG") as Record<string, unknown>;
  const startedAt = loadEnvJson("DG_STARTED_AT");

  const client = new DecisionGateClient({
    endpoint,
    authToken: token,
  });

  if (expectFailure) {
    try {
      await client.scenario_define({ spec });
    } catch (error) {
      if (error instanceof DecisionGateTransportError) {
        console.log(JSON.stringify({ status: "expected_failure", error: String(error) }));
        return 0;
      }
      console.log(JSON.stringify({ status: "unexpected_error", error: String(error) }));
      return 1;
    }
    console.log(JSON.stringify({ status: "unexpected_success" }));
    return 1;
  }

  const define = await client.scenario_define({ spec });
  const start = await client.scenario_start({
    scenario_id: runConfig.scenario_id,
    run_config: runConfig,
    started_at: startedAt,
    issue_entry_packets: false,
  });
  const status = await client.scenario_status({
    scenario_id: runConfig.scenario_id,
    request: {
      tenant_id: runConfig.tenant_id,
      namespace_id: runConfig.namespace_id,
      run_id: runConfig.run_id,
      requested_at: { kind: "logical", value: 2 },
      correlation_id: null,
    },
  });
  console.log(JSON.stringify({ define, start, status }));
  return 0;
}

main()
  .then((code) => {
    process.exit(code);
  })
  .catch((error) => {
    console.log(JSON.stringify({ status: "fatal_error", error: String(error) }));
    process.exit(1);
  });
