// decision-gate-provider-sdk/typescript/test/index.test.js
// ============================================================================
// Module: TypeScript Evidence Provider Template Tests
// Description: Smoke tests for JSON-RPC framing and tool responses.
// Purpose: Validate end-to-end framing with the compiled Node provider.
// Dependencies: Node.js standard library (assert, child_process, test, url, util).
// ============================================================================

/**
 * ## Overview
 * These tests spawn the compiled provider to verify Content-Length framing and
 * the `tools/list` response shape without modifying the template API surface.
 */

import assert from "node:assert/strict";
import { execFile } from "node:child_process";
import { fileURLToPath } from "node:url";
import test from "node:test";
import { promisify } from "node:util";

// ============================================================================
// SECTION: Helpers
// ============================================================================

const execFileAsync = promisify(execFile);
const providerPath = fileURLToPath(new URL("../dist/index.js", import.meta.url));
const headerSeparator = Buffer.from("\r\n\r\n", "utf8");

function buildFrame(payload) {
  const body = Buffer.from(JSON.stringify(payload), "utf8");
  const header = Buffer.from(`Content-Length: ${body.length}\r\n\r\n`, "utf8");
  return Buffer.concat([header, body]);
}

function parseFrame(output) {
  const headerEnd = output.indexOf(headerSeparator);
  if (headerEnd === -1) {
    throw new Error("missing Content-Length header");
  }

  const headerText = output.slice(0, headerEnd).toString("utf8");
  const match = /Content-Length:\s*(\d+)/i.exec(headerText);
  if (!match) {
    throw new Error("invalid Content-Length header");
  }

  const contentLength = Number.parseInt(match[1], 10);
  const bodyStart = headerEnd + headerSeparator.length;
  const body = output.slice(bodyStart, bodyStart + contentLength);
  return JSON.parse(body.toString("utf8"));
}

// ============================================================================
// SECTION: Tests
// ============================================================================

test("tools/list returns evidence_query tool", async () => {
  const request = buildFrame({ jsonrpc: "2.0", id: 1, method: "tools/list" });
  const { stdout } = await execFileAsync(process.execPath, [providerPath], {
    input: request,
    timeout: 2000,
    maxBuffer: 1024 * 1024,
  });

  const response = parseFrame(stdout);
  assert.equal(response.jsonrpc, "2.0");
  assert.equal(response.id, 1);
  assert.ok(Array.isArray(response.result?.tools));
  assert.ok(response.result.tools.some((tool) => tool.name === "evidence_query"));
});

test("tools/call returns evidence result with lane and error fields", async () => {
  const request = buildFrame({
    jsonrpc: "2.0",
    id: 2,
    method: "tools/call",
    params: {
      name: "evidence_query",
      arguments: {
        query: { provider_id: "custom", predicate: "echo", params: { value: "ok" } },
        context: {
          tenant_id: 1,
          namespace_id: 1,
          run_id: "run-1",
          scenario_id: "scenario-1",
          stage_id: "stage-1",
          trigger_id: "trigger-1",
          trigger_time: { kind: "unix_millis", value: 0 },
          correlation_id: null,
        },
      },
    },
  });

  const { stdout } = await execFileAsync(process.execPath, [providerPath], {
    input: request,
    timeout: 2000,
    maxBuffer: 1024 * 1024,
  });

  const response = parseFrame(stdout);
  const json = response.result?.content?.[0]?.json;
  assert.equal(response.jsonrpc, "2.0");
  assert.equal(response.id, 2);
  assert.equal(json?.lane, "verified");
  assert.equal(json?.error, null);
  assert.deepEqual(json?.value, { kind: "json", value: "ok" });
});
