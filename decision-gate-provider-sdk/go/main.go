// decision-gate-provider-sdk/go/main.go
// ============================================================================
// Module: Go Evidence Provider Template
// Description: Minimal MCP stdio server for Decision Gate evidence queries.
// Purpose: Provide a starter implementation for `evidence_query` providers.
// Dependencies: Go standard library (bufio, encoding/json, fmt, io, os, strconv, strings).
// ============================================================================

// ## Overview
// This template implements the MCP `tools/list` and `tools/call` handlers over
// stdio. It parses Content-Length framed JSON-RPC messages and replies with a
// JSON EvidenceResult. Security posture: inputs are untrusted and must be
// validated; see Docs/security/threat_model.md.
package main

import (
	"bufio"
	"encoding/json"
	"fmt"
	"io"
	"os"
	"strconv"
	"strings"
)

// ============================================================================
// SECTION: Limits and Constants
// ============================================================================

const maxHeaderBytes = 8 * 1024
const maxBodyBytes = 1024 * 1024

// ============================================================================
// SECTION: JSON-RPC Types
// ============================================================================

type jsonRpcRequest struct {
	JSONRPC string          `json:"jsonrpc"`
	ID      any             `json:"id"`
	Method  string          `json:"method"`
	Params  json.RawMessage `json:"params,omitempty"`
}

type jsonRpcError struct {
	Code    int    `json:"code"`
	Message string `json:"message"`
}

type jsonRpcResponse struct {
	JSONRPC string       `json:"jsonrpc"`
	ID      any          `json:"id"`
	Result  any          `json:"result,omitempty"`
	Error   *jsonRpcError `json:"error,omitempty"`
}

type toolListResult struct {
	Tools []toolDefinition `json:"tools"`
}

type toolDefinition struct {
	Name        string         `json:"name"`
	Description string         `json:"description"`
	InputSchema map[string]any `json:"input_schema"`
}

type toolCallParams struct {
	Name      string `json:"name"`
	Arguments struct {
		Query   evidenceQuery   `json:"query"`
		Context evidenceContext `json:"context"`
	} `json:"arguments"`
}

type evidenceQuery struct {
	ProviderID string         `json:"provider_id"`
	CheckID    string         `json:"check_id"`
	Params     map[string]any `json:"params,omitempty"`
}

type evidenceContext struct {
	TenantID      uint64 `json:"tenant_id"`
	NamespaceID   uint64 `json:"namespace_id"`
	RunID         string `json:"run_id"`
	ScenarioID    string `json:"scenario_id"`
	StageID       string `json:"stage_id"`
	TriggerID     string `json:"trigger_id"`
	TriggerTime   any    `json:"trigger_time"`
	CorrelationID any    `json:"correlation_id"`
}

type evidenceValue struct {
	Kind  string `json:"kind"`
	Value any    `json:"value"`
}

type evidenceResult struct {
	Value          *evidenceValue `json:"value"`
	Lane           string         `json:"lane"`
	Error          any            `json:"error"`
	EvidenceHash   any            `json:"evidence_hash"`
	EvidenceRef    any            `json:"evidence_ref"`
	EvidenceAnchor any            `json:"evidence_anchor"`
	Signature      any            `json:"signature"`
	ContentType    string         `json:"content_type"`
}

type frameError struct {
	message string
	fatal   bool
}

func (err frameError) Error() string {
	return err.message
}

// ============================================================================
// SECTION: Entry Point
// ============================================================================

func main() {
	reader := bufio.NewReader(os.Stdin)
	writer := bufio.NewWriter(os.Stdout)

	for {
		payload, err := readFrame(reader)
		if err == io.EOF {
			return
		}
		if err != nil {
			fatal := false
			if frameErr, ok := err.(frameError); ok {
				fatal = frameErr.fatal
			}
			writeFrame(writer, buildErrorResponse(nil, -32600, err.Error()))
			if fatal {
				return
			}
			continue
		}

		var request jsonRpcRequest
		if err := json.Unmarshal(payload, &request); err != nil {
			writeFrame(writer, buildErrorResponse(nil, -32700, "invalid json"))
			continue
		}

		response := handleRequest(request)
		writeFrame(writer, response)
	}
}

// ============================================================================
// SECTION: JSON-RPC Handling
// ============================================================================

func handleRequest(request jsonRpcRequest) jsonRpcResponse {
	if request.JSONRPC != "2.0" {
		return buildErrorResponse(request.ID, -32600, "invalid json-rpc version")
	}

	switch request.Method {
	case "tools/list":
		return jsonRpcResponse{
			JSONRPC: "2.0",
			ID:      request.ID,
			Result: toolListResult{
				Tools: []toolDefinition{
					{
						Name:        "evidence_query",
						Description: "Resolve a Decision Gate evidence query.",
						InputSchema: map[string]any{"type": "object"},
					},
				},
			},
		}
	case "tools/call":
		return handleToolCall(request)
	default:
		return buildErrorResponse(request.ID, -32601, "method not found")
	}
}

func handleToolCall(request jsonRpcRequest) jsonRpcResponse {
	var params toolCallParams
	if err := json.Unmarshal(request.Params, &params); err != nil {
		return buildErrorResponse(request.ID, -32602, "invalid tool params")
	}
	if params.Name != "evidence_query" {
		return buildErrorResponse(request.ID, -32602, "invalid tool params")
	}

	result, err := handleEvidenceQuery(params.Arguments.Query, params.Arguments.Context)
	if err != nil {
		return buildErrorResponse(request.ID, -32000, err.Error())
	}

	return jsonRpcResponse{
		JSONRPC: "2.0",
		ID:      request.ID,
		Result: map[string]any{
			"content": []map[string]any{
				{
					"type": "json",
					"json": result,
				},
			},
		},
	}
}

// ============================================================================
// SECTION: Evidence Logic
// ============================================================================

func handleEvidenceQuery(query evidenceQuery, _ evidenceContext) (evidenceResult, error) {
	if query.Params == nil {
		return evidenceResult{}, fmt.Errorf("params.value is required")
	}
	value, ok := query.Params["value"]
	if !ok {
		return evidenceResult{}, fmt.Errorf("params.value is required")
	}
	if valueStr, ok := value.(string); ok && valueStr == "error" {
		return evidenceResult{}, fmt.Errorf("forced error")
	}

	return evidenceResult{
		Value:          &evidenceValue{Kind: "json", Value: value},
		Lane:           "verified",
		Error:          nil,
		EvidenceHash:   nil,
		EvidenceRef:    nil,
		EvidenceAnchor: nil,
		Signature:      nil,
		ContentType:    "application/json",
	}, nil
}

func buildErrorResponse(id any, code int, message string) jsonRpcResponse {
	return jsonRpcResponse{
		JSONRPC: "2.0",
		ID:      id,
		Error: &jsonRpcError{
			Code:    code,
			Message: message,
		},
	}
}

// ============================================================================
// SECTION: Framing
// ============================================================================

func readFrame(reader *bufio.Reader) ([]byte, error) {
	contentLength := -1
	headerBytes := 0
	for {
		line, err := reader.ReadString('\n')
		if err != nil {
			return nil, err
		}
		headerBytes += len(line)
		if headerBytes > maxHeaderBytes {
			return nil, frameError{message: "headers too large", fatal: true}
		}
		line = strings.TrimRight(line, "\r\n")
		if line == "" {
			break
		}
		name, value, ok := strings.Cut(line, ":")
		if ok && strings.EqualFold(strings.TrimSpace(name), "content-length") {
			parsed, err := strconv.Atoi(strings.TrimSpace(value))
			if err != nil {
				return nil, frameError{message: "invalid content length", fatal: true}
			}
			contentLength = parsed
		}
	}
	if contentLength < 0 {
		return nil, frameError{message: "missing content length", fatal: true}
	}
	if contentLength <= 0 {
		return nil, frameError{message: "invalid content length", fatal: true}
	}
	if contentLength > maxBodyBytes {
		if err := discardPayload(reader, contentLength); err != nil {
			return nil, err
		}
		return nil, frameError{message: "payload too large", fatal: false}
	}
	payload := make([]byte, contentLength)
	if _, err := io.ReadFull(reader, payload); err != nil {
		return nil, err
	}
	return payload, nil
}

func discardPayload(reader *bufio.Reader, contentLength int) error {
	if _, err := io.CopyN(io.Discard, reader, int64(contentLength)); err != nil {
		return frameError{message: "unexpected eof", fatal: true}
	}
	return nil
}

func writeFrame(writer *bufio.Writer, response jsonRpcResponse) {
	payload, err := json.Marshal(response)
	if err != nil {
		return
	}
	header := fmt.Sprintf("Content-Length: %d\r\n\r\n", len(payload))
	_, _ = writer.WriteString(header)
	_, _ = writer.Write(payload)
	_ = writer.Flush()
}
