<!--
Decision Gate File Disclosure Example README
============================================================================
Document: examples/file-disclosure
Description: File-backed payload disclosure using broker sources/sinks.
Purpose: Demonstrate external payload resolution via the broker.
Dependencies:
  - ../../crates/decision-gate-core/README.md
  - ../../crates/decision-gate-broker/README.md
============================================================================
-->

# File Disclosure Example

Demonstrates file-backed payload disclosure using the broker's `FileSource` and
`LogSink` implementations.

## Table of Contents

- [Overview](#overview)
- [What It Demonstrates](#what-it-demonstrates)
- [Run](#run)
- [Notes](#notes)
- [References](#references)

## Overview

This example builds a scenario that emits a packet with an external file
reference. The broker resolves the file and logs the disclosure.

## What It Demonstrates

- `PacketPayload::External` with `file://` URIs.
- Broker source resolution and sink dispatch.
- Deterministic packet hashing and receipts.

## Run

```bash
cargo run -p decision-gate-example-file-disclosure
```

## Notes

- Uses a temporary directory for the file payload.
- Intended as a wiring example for source/sink resolution.

## References

