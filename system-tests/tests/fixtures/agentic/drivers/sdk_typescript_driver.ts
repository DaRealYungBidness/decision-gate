// system-tests/tests/fixtures/agentic/drivers/sdk_typescript_driver.ts
// ============================================================================
// Module: Agentic Flow Harness TypeScript SDK Driver
// Description: Executes a scenario pack via the Decision Gate TypeScript SDK.
// ============================================================================

import { DecisionGateClient } from "../../../../../sdks/typescript/src/index.ts";
import * as fs from "node:fs";
import * as path from "node:path";

const HTTP_PLACEHOLDER = "{{HTTP_BASE_URL}}";

function loadJson(filePath: string): unknown {
  return JSON.parse(fs.readFileSync(filePath, "utf-8"));
}

function replacePlaceholder(value: unknown, placeholder: string, replacement: string): unknown {
  if (Array.isArray(value)) {
    return value.map((item) => replacePlaceholder(item, placeholder, replacement));
  }
  if (value && typeof value === "object") {
    const output: Record<string, unknown> = {};
    for (const [key, entry] of Object.entries(value as Record<string, unknown>)) {
      output[key] = replacePlaceholder(entry, placeholder, replacement);
    }
    return output;
  }
  if (typeof value === "string" && value.includes(placeholder)) {
    return value.replaceAll(placeholder, replacement);
  }
  return value;
}

function extractOutcomeKind(status: Record<string, unknown>): string | null {
  const decision = status["last_decision"] as Record<string, unknown> | null;
  if (!decision || typeof decision !== "object") {
    return null;
  }
  const outcome = decision["outcome"] as Record<string, unknown> | null;
  if (!outcome || typeof outcome !== "object") {
    return null;
  }
  const kind = outcome["kind"];
  if (typeof kind === "string") {
    return kind.toLowerCase();
  }
  const keys = Object.keys(outcome);
  if (keys.length !== 1) {
    return null;
  }
  return keys[0].toLowerCase();
}

async function main(): Promise<number> {
  const scenarioDir = process.env.DG_SCENARIO_PACK;
  if (!scenarioDir) {
    throw new Error("missing DG_SCENARIO_PACK");
  }

  const endpoint = process.env.DG_ENDPOINT;
  if (!endpoint) {
    throw new Error("missing DG_ENDPOINT");
  }

  const httpBaseUrl = process.env.DG_HTTP_BASE_URL;
  const runpackDir = process.env.DG_RUNPACK_DIR;
  const token = process.env.DG_TOKEN;

  let spec = loadJson(path.join(scenarioDir, "spec.json"));
  if (httpBaseUrl) {
    spec = replacePlaceholder(spec, HTTP_PLACEHOLDER, httpBaseUrl);
  }

  const runConfig = loadJson(path.join(scenarioDir, "run_config.json")) as Record<string, unknown>;
  const trigger = loadJson(path.join(scenarioDir, "trigger.json"));

  const client = new DecisionGateClient({ endpoint, authToken: token ?? undefined });

  await client.scenario_define({ spec });
  await client.scenario_start({
    scenario_id: runConfig.scenario_id as string,
    run_config: runConfig,
    started_at: { kind: "logical", value: 1 },
    issue_entry_packets: false,
  });
  await client.scenario_trigger({ scenario_id: runConfig.scenario_id as string, trigger });

  const status = await client.scenario_status({
    scenario_id: runConfig.scenario_id as string,
    request: {
      tenant_id: runConfig.tenant_id,
      namespace_id: runConfig.namespace_id,
      run_id: runConfig.run_id,
      requested_at: { kind: "logical", value: 3 },
      correlation_id: null,
    },
  });

  const outputDir = runpackDir ?? fs.mkdtempSync(path.join(process.cwd(), "dg-agentic-runpack-"));
  const exportResult = await client.runpack_export({
    scenario_id: runConfig.scenario_id as string,
    run_id: runConfig.run_id as string,
    tenant_id: runConfig.tenant_id,
    namespace_id: runConfig.namespace_id,
    output_dir: outputDir,
    manifest_name: "manifest.json",
    generated_at: { kind: "logical", value: 10 },
    include_verification: false,
  });

  const rootHash = (exportResult as Record<string, any>)?.manifest?.integrity?.root_hash ?? {};
  const summary = {
    driver: "typescript_sdk",
    scenario_id: runConfig.scenario_id,
    status: (status as Record<string, unknown>).status,
    outcome: extractOutcomeKind(status as Record<string, unknown>),
    runpack_root_hash: rootHash.value,
    runpack_hash_algorithm: rootHash.algorithm,
    runpack_dir: outputDir,
  };

  console.log(JSON.stringify(summary));
  return 0;
}

main()
  .then((code) => process.exit(code))
  .catch((error) => {
    console.log(JSON.stringify({ status: "fatal_error", error: String(error) }));
    process.exit(1);
  });
