// decision-gate-provider-sdk/go/main_test.go
// ============================================================================
// Module: Go Evidence Provider Template Tests
// Description: Unit tests for framing and request handling helpers.
// Purpose: Validate Content-Length parsing and fail-closed behavior.
// Dependencies: Go standard library (bufio, errors, strings, testing).
// ============================================================================

// ## Overview
// These tests exercise the framing parser to ensure valid requests are accepted
// and malformed frames are rejected with explicit errors.

package main

import (
	"bufio"
	"errors"
	"strings"
	"testing"
)

// ============================================================================
// SECTION: Framing Tests
// ============================================================================

func TestReadFrameParsesContentLength(t *testing.T) {
	frame := "Content-Length: 5\r\n\r\nhello"
	reader := bufio.NewReader(strings.NewReader(frame))

	payload, err := readFrame(reader)
	if err != nil {
		t.Fatalf("expected nil error, got %v", err)
	}
	if string(payload) != "hello" {
		t.Fatalf("expected payload 'hello', got %q", string(payload))
	}
}

func TestReadFrameParsesLowercaseContentLength(t *testing.T) {
	frame := "content-length: 5\r\n\r\nhello"
	reader := bufio.NewReader(strings.NewReader(frame))

	payload, err := readFrame(reader)
	if err != nil {
		t.Fatalf("expected nil error, got %v", err)
	}
	if string(payload) != "hello" {
		t.Fatalf("expected payload 'hello', got %q", string(payload))
	}
}

func TestReadFrameRejectsMissingContentLength(t *testing.T) {
	frame := "X-Test: 1\r\n\r\nhello"
	reader := bufio.NewReader(strings.NewReader(frame))

	_, err := readFrame(reader)
	if err == nil {
		t.Fatal("expected error, got nil")
	}
	var frameErr frameError
	if !errors.As(err, &frameErr) {
		t.Fatalf("expected frameError, got %T", err)
	}
	if !frameErr.fatal {
		t.Fatalf("expected fatal error, got %v", frameErr.fatal)
	}
}

// ============================================================================
// SECTION: Evidence Tests
// ============================================================================

func TestHandleEvidenceQueryMissingValueReturnsError(t *testing.T) {
	result := handleEvidenceQuery(evidenceQuery{Params: map[string]any{}}, evidenceContext{})

	if result.Error == nil {
		t.Fatal("expected error, got nil")
	}
	if result.Error.Code != "invalid_params" {
		t.Fatalf("expected invalid_params, got %q", result.Error.Code)
	}
	if result.Value != nil {
		t.Fatalf("expected nil value, got %v", result.Value)
	}
	if result.ContentType != nil {
		t.Fatalf("expected nil content_type, got %v", *result.ContentType)
	}
}
