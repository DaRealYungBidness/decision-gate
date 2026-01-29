// sdks/typescript/src/errors.ts
// ============================================================================
// Module: SDK Errors
// Description: Error types for Decision Gate TypeScript client SDK.
// Purpose: Provide structured error reporting for transport and JSON-RPC failures.
// Dependencies: stdlib
// ============================================================================

export class DecisionGateError extends Error {
  public readonly cause?: unknown;

  public constructor(message: string, cause?: unknown) {
    super(message);
    this.name = "DecisionGateError";
    this.cause = cause;
  }
}

export class DecisionGateTransportError extends DecisionGateError {
  public readonly statusCode?: number;
  public readonly body?: string;

  public constructor(message: string, options?: { statusCode?: number; body?: string; cause?: unknown }) {
    super(message, options?.cause);
    this.name = "DecisionGateTransportError";
    this.statusCode = options?.statusCode;
    this.body = options?.body;
  }
}

export class DecisionGateProtocolError extends DecisionGateError {
  public constructor(message: string, cause?: unknown) {
    super(message, cause);
    this.name = "DecisionGateProtocolError";
  }
}

export class DecisionGateRpcError extends DecisionGateError {
  public readonly code: number;
  public readonly data?: Record<string, unknown>;
  public readonly requestId?: string;

  public constructor(code: number, message: string, options?: { data?: Record<string, unknown>; requestId?: string }) {
    super(message);
    this.name = "DecisionGateRpcError";
    this.code = code;
    this.data = options?.data;
    this.requestId = options?.requestId;
  }

  public get kind(): string | undefined {
    const value = this.data?.kind;
    return typeof value === "string" ? value : undefined;
  }

  public get retryable(): boolean | undefined {
    const value = this.data?.retryable;
    return typeof value === "boolean" ? value : undefined;
  }

  public get retryAfterMs(): number | undefined {
    const value = this.data?.retry_after_ms;
    return typeof value === "number" ? value : undefined;
  }
}
