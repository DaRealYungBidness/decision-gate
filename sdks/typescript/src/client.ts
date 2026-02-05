// sdks/typescript/src/client.ts
// ============================================================================
// Module: Decision Gate Client
// Description: HTTP JSON-RPC client for Decision Gate MCP server.
// Purpose: Provide authenticated, structured access to Decision Gate tools.
// Dependencies: fetch API, generated SDK surface
// ============================================================================

import { GeneratedDecisionGateClient } from "./_generated.js";
import {
  DecisionGateProtocolError,
  DecisionGateRpcError,
  DecisionGateTransportError,
} from "./errors.js";

export type FetchLike = (input: RequestInfo, init?: RequestInit) => Promise<Response>;

export interface DecisionGateClientOptions {
  endpoint?: string;
  authToken?: string;
  timeoutMs?: number;
  headers?: Record<string, string>;
  fetch?: FetchLike;
  userAgent?: string;
}

const DEFAULT_ENDPOINT = "http://127.0.0.1:8080/rpc";
const DEFAULT_TIMEOUT_MS = 10_000;

export class DecisionGateClient extends GeneratedDecisionGateClient {
  private readonly endpoint: string;
  private readonly authToken?: string;
  private readonly timeoutMs: number;
  private readonly headers: Record<string, string>;
  private readonly fetcher: FetchLike;
  private readonly userAgent?: string;
  private requestId = 0;

  public constructor(options: DecisionGateClientOptions = {}) {
    super();
    this.endpoint = options.endpoint ?? DEFAULT_ENDPOINT;
    this.authToken = options.authToken;
    this.timeoutMs = options.timeoutMs ?? DEFAULT_TIMEOUT_MS;
    this.headers = { ...(options.headers ?? {}) };
    const fallbackFetch = typeof fetch === "function" ? fetch : undefined;
    this.fetcher = options.fetch ?? fallbackFetch ?? (() => {
      throw new DecisionGateTransportError("fetch is not available; provide a fetch implementation");
    });
    this.userAgent = options.userAgent ?? "decision-gate-typescript-sdk/0.1.0";
  }

  protected async callTool<T>(name: string, arguments_: object): Promise<T> {
    const payload = {
      jsonrpc: "2.0",
      id: this.nextRequestId(),
      method: "tools/call",
      params: {
        name,
        arguments: arguments_,
      },
    };

    const headers: Record<string, string> = {
      "Content-Type": "application/json",
      ...this.headers,
    };
    if (this.userAgent) {
      headers["User-Agent"] = this.userAgent;
    }
    if (this.authToken) {
      headers["Authorization"] = `Bearer ${this.authToken}`;
    }

    const controller = new AbortController();
    const timeoutId = setTimeout(() => controller.abort(), this.timeoutMs);

    let response: Response;
    let bodyText: string;
    try {
      response = await this.fetcher(this.endpoint, {
        method: "POST",
        headers,
        body: JSON.stringify(payload),
        signal: controller.signal,
      });
      bodyText = await response.text();
    } catch (error) {
      clearTimeout(timeoutId);
      throw new DecisionGateTransportError("Decision Gate transport error", { cause: error });
    } finally {
      clearTimeout(timeoutId);
    }

    if (!response.ok) {
      throw new DecisionGateTransportError(`HTTP ${response.status} from Decision Gate`, {
        statusCode: response.status,
        body: bodyText,
      });
    }

    let parsed: unknown;
    try {
      parsed = JSON.parse(bodyText);
    } catch (error) {
      throw new DecisionGateProtocolError("invalid JSON-RPC response", error);
    }

    if (!parsed || typeof parsed !== "object") {
      throw new DecisionGateProtocolError("invalid JSON-RPC response shape");
    }
    const payloadObj = parsed as Record<string, unknown>;
    if (payloadObj.error) {
      throw this.rpcErrorFromPayload(payloadObj);
    }

    const result = payloadObj.result;
    if (!result || typeof result !== "object") {
      throw new DecisionGateProtocolError("missing JSON-RPC result");
    }
    const content = (result as Record<string, unknown>).content;
    if (!Array.isArray(content) || content.length === 0) {
      throw new DecisionGateProtocolError("missing JSON-RPC content");
    }
    const first = content[0];
    if (!first || typeof first !== "object") {
      throw new DecisionGateProtocolError("invalid JSON-RPC content item");
    }
    const firstObj = first as Record<string, unknown>;
    if (firstObj.type !== "json") {
      throw new DecisionGateProtocolError("unsupported JSON-RPC content type");
    }
    if (!("json" in firstObj)) {
      throw new DecisionGateProtocolError("missing JSON payload in content item");
    }
    return firstObj.json as T;
  }

  private nextRequestId(): number {
    this.requestId += 1;
    return this.requestId;
  }

  private rpcErrorFromPayload(payload: Record<string, unknown>): DecisionGateRpcError {
    const error = payload.error;
    if (!error || typeof error !== "object") {
      throw new DecisionGateProtocolError("invalid JSON-RPC error shape");
    }
    const errorObj = error as Record<string, unknown>;
    const code = errorObj.code;
    const message = errorObj.message;
    if (typeof code !== "number") {
      throw new DecisionGateProtocolError("invalid JSON-RPC error code");
    }
    if (typeof message !== "string") {
      throw new DecisionGateProtocolError("invalid JSON-RPC error message");
    }
    const data = errorObj.data;
    const requestId = payload.id;
    return new DecisionGateRpcError(code, message, {
      data: typeof data === "object" && data !== null ? (data as Record<string, unknown>) : undefined,
      requestId: requestId !== undefined ? String(requestId) : undefined,
    });
  }
}
