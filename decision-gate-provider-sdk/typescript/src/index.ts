// decision-gate-provider-sdk/typescript/src/index.ts
// ============================================================================
// Module: TypeScript Evidence Provider Template
// Description: Minimal MCP stdio server for Decision Gate evidence queries.
// Purpose: Provide a starter implementation for `evidence_query` providers.
// Dependencies: Node.js stdio, JSON parsing utilities.
// ============================================================================

/**
 * ## Overview
 * This template implements the MCP `tools/list` and `tools/call` handlers over
 * stdio. It parses Content-Length framed JSON-RPC messages and replies with a
 * JSON EvidenceResult. Security posture: inputs are untrusted and must be
 * validated; see Docs/security/threat_model.md.
 */

// ============================================================================
// SECTION: Types
// ============================================================================

type JsonRpcId = number | string | null;

interface JsonRpcRequest {
  jsonrpc: string;
  id: JsonRpcId;
  method: string;
  params?: unknown;
}

interface JsonRpcError {
  code: number;
  message: string;
}

interface JsonRpcResponse {
  jsonrpc: "2.0";
  id: JsonRpcId;
  result?: unknown;
  error?: JsonRpcError;
}

interface ToolCallParams {
  name: string;
  arguments: {
    query: EvidenceQuery;
    context: EvidenceContext;
  };
}

interface EvidenceQuery {
  provider_id: string;
  predicate: string;
  params?: Record<string, unknown>;
}

interface EvidenceContext {
  tenant_id: number;
  namespace_id: number;
  run_id: string;
  scenario_id: string;
  stage_id: string;
  trigger_id: string;
  trigger_time: { kind: "unix_millis" | "logical"; value: number };
  correlation_id?: string | null;
}

type EvidenceValue =
  | { kind: "json"; value: unknown }
  | { kind: "bytes"; value: number[] };

interface EvidenceResult {
  value: EvidenceValue | null;
  lane: "verified" | "asserted";
  error: EvidenceProviderError | null;
  evidence_hash: unknown | null;
  evidence_ref: unknown | null;
  evidence_anchor: unknown | null;
  signature: unknown | null;
  content_type: string | null;
}

interface EvidenceProviderError {
  code: string;
  message: string;
  details: unknown | null;
}

// ============================================================================
// SECTION: Tool Metadata
// ============================================================================

const TOOL_LIST_RESULT = {
  tools: [
    {
      name: "evidence_query",
      description: "Resolve a Decision Gate evidence query.",
      input_schema: { type: "object" },
    },
  ],
};

// ============================================================================
// SECTION: Framing Limits
// ============================================================================

const HEADER_SEPARATOR = "\r\n\r\n";
const MAX_HEADER_BYTES = 8 * 1024;
const MAX_BODY_BYTES = 1024 * 1024;

// ============================================================================
// SECTION: Stream State
// ============================================================================

let buffer = Buffer.alloc(0);
let discardBytes = 0;
let stopped = false;

// ============================================================================
// SECTION: Stream Processing
// ============================================================================

process.stdin.on("data", (chunk) => {
  if (stopped) {
    return;
  }
  if (discardBytes > 0) {
    const toDiscard = Math.min(discardBytes, chunk.length);
    discardBytes -= toDiscard;
    chunk = chunk.slice(toDiscard);
    if (chunk.length === 0) {
      return;
    }
  }
  if (buffer.length + chunk.length > MAX_HEADER_BYTES + MAX_BODY_BYTES) {
    writeFrame(buildErrorResponse(null, -32600, "frame too large"));
    buffer = Buffer.alloc(0);
    stopServer();
    return;
  }
  buffer = Buffer.concat([buffer, chunk]);
  processBuffer();
});

// ============================================================================
// SECTION: JSON-RPC Handling
// ============================================================================

function processBuffer(): void {
  while (true) {
    const headerEnd = buffer.indexOf(HEADER_SEPARATOR);
    if (headerEnd === -1) {
      if (buffer.length > MAX_HEADER_BYTES) {
        writeFrame(buildErrorResponse(null, -32600, "headers too large"));
        buffer = Buffer.alloc(0);
        stopServer();
      }
      return;
    }

    if (headerEnd > MAX_HEADER_BYTES) {
      writeFrame(buildErrorResponse(null, -32600, "headers too large"));
      buffer = Buffer.alloc(0);
      stopServer();
      return;
    }

    const headerText = buffer.slice(0, headerEnd).toString("utf8");
    const contentLength = parseContentLength(headerText);
    if (contentLength === null) {
      writeFrame(buildErrorResponse(null, -32600, "missing Content-Length"));
      buffer = Buffer.alloc(0);
      stopServer();
      return;
    }

    if (contentLength <= 0) {
      writeFrame(buildErrorResponse(null, -32600, "invalid Content-Length"));
      buffer = Buffer.alloc(0);
      stopServer();
      return;
    }

    if (contentLength > MAX_BODY_BYTES) {
      writeFrame(buildErrorResponse(null, -32600, "payload too large"));
      const bodyStart = headerEnd + HEADER_SEPARATOR.length;
      const available = buffer.length - bodyStart;
      if (available >= contentLength) {
        buffer = buffer.slice(bodyStart + contentLength);
        continue;
      }
      discardBytes = contentLength - available;
      buffer = Buffer.alloc(0);
      return;
    }

    const totalLength = headerEnd + HEADER_SEPARATOR.length + contentLength;
    if (buffer.length < totalLength) {
      return;
    }

    const body = buffer.slice(headerEnd + HEADER_SEPARATOR.length, totalLength);
    buffer = buffer.slice(totalLength);

    const request = parseRequest(body);
    if (!request) {
      writeFrame(buildErrorResponse(null, -32700, "invalid json"));
      continue;
    }
    const response = handleRequest(request);
    writeFrame(response);
  }
}

function parseContentLength(header: string): number | null {
  const lines = header.split(/\r?\n/);
  for (const line of lines) {
    const match = /^Content-Length:\s*(\d+)$/i.exec(line.trim());
    if (match) {
      return Number.parseInt(match[1], 10);
    }
  }
  return null;
}

function parseRequest(body: Buffer): JsonRpcRequest | null {
  try {
    const text = body.toString("utf8");
    const parsed = JSON.parse(text) as JsonRpcRequest;
    return parsed;
  } catch {
    return null;
  }
}

function handleRequest(request: JsonRpcRequest): JsonRpcResponse {
  if (request.jsonrpc !== "2.0") {
    return buildErrorResponse(request.id, -32600, "invalid json-rpc version");
  }

  switch (request.method) {
    case "tools/list":
      return {
        jsonrpc: "2.0",
        id: request.id,
        result: TOOL_LIST_RESULT,
      };
    case "tools/call":
      return handleToolCall(request);
    default:
      return buildErrorResponse(request.id, -32601, "method not found");
  }
}

function handleToolCall(request: JsonRpcRequest): JsonRpcResponse {
  const params = request.params as ToolCallParams | undefined;
  if (!params || params.name !== "evidence_query") {
    return buildErrorResponse(request.id, -32602, "invalid tool params");
  }

  const query = params.arguments?.query;
  const context = params.arguments?.context;
  if (!query || !context) {
    return buildErrorResponse(request.id, -32602, "missing query or context");
  }

  const result = handleEvidenceQuery(query, context);
  if ("error" in result) {
    return buildErrorResponse(request.id, -32000, result.error);
  }

  return {
    jsonrpc: "2.0",
    id: request.id,
    result: {
      content: [
        {
          type: "json",
          json: result,
        },
      ],
    },
  };
}

// ============================================================================
// SECTION: Evidence Logic
// ============================================================================

function handleEvidenceQuery(
  query: EvidenceQuery,
  _context: EvidenceContext,
): EvidenceResult | { error: string } {
  const value = query.params?.value;
  if (typeof value === "undefined") {
    return { error: "params.value is required" };
  }

  return {
    value: { kind: "json", value },
    lane: "verified",
    error: null,
    evidence_hash: null,
    evidence_ref: null,
    evidence_anchor: null,
    signature: null,
    content_type: "application/json",
  };
}

// ============================================================================
// SECTION: Framing Output
// ============================================================================

function buildErrorResponse(id: JsonRpcId, code: number, message: string): JsonRpcResponse {
  return {
    jsonrpc: "2.0",
    id,
    error: { code, message },
  };
}

function writeFrame(response: JsonRpcResponse): void {
  const payload = Buffer.from(JSON.stringify(response), "utf8");
  const header = `Content-Length: ${payload.length}\r\n\r\n`;
  process.stdout.write(header);
  process.stdout.write(payload);
}

function stopServer(): void {
  if (stopped) {
    return;
  }
  stopped = true;
  process.stdin.removeAllListeners("data");
  process.stdin.pause();
}
