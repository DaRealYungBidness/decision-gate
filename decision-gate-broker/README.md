<!--
Decision Gate Broker README
============================================================================
Document: decision-gate-broker
Description: Reference payload sources and disclosure sinks.
Purpose: Resolve external packet payloads and dispatch disclosures.
Dependencies:
  - ../../README.md (Decision Gate overview)
  - ../decision-gate-core/README.md
  - ../../Docs/security/threat_model.md
============================================================================
-->

# decision-gate-broker

Reference implementations for payload sources and sinks. The broker resolves
external packet payloads (file/http/inline) and dispatches disclosures to a
configured sink, implementing the `Dispatcher` trait from
`decision-gate-core`.

## Table of Contents

- [Overview](#overview)
- [Architecture](#architecture)
- [Sources](#sources)
- [Sinks](#sinks)
- [CompositeBroker](#compositebroker)
- [Usage Examples](#usage-examples)
- [Security Model](#security-model)
- [Testing](#testing)
- [References](#references)

## Overview

The broker is optional. Use it when you want reference implementations for:

- Resolving `PacketPayload::External` content references.
- Dispatching packets to logging, channels, or callbacks.

It does not execute arbitrary code and fails closed on invalid or oversized
payloads.

## Architecture

```mermaid
flowchart LR
  CP[ControlPlane] --> Broker[CompositeBroker]
  Broker --> Source[Source
  (inline/file/http)]
  Broker --> Sink[Sink
  (log/channel/callback)]
```

## Sources

Sources resolve `ContentRef` URIs into raw payload bytes.

### InlineSource

- Supports: `inline:`, `inline+json:`, `inline+bytes:`
- Payloads are base64-encoded in the URI.

### FileSource

- Supports: `file://` URIs.
- Optional root directory; when set, paths must resolve under the root.

### HttpSource

- Supports: `http://` and `https://` URIs.
- Rejects redirects and non-2xx responses.
- Enforces a 30s request timeout and max payload size.

## Sinks

Sinks deliver disclosure packets to external systems.

### LogSink

Writes packet details to a `Write` target (stdout, stderr, or a file).

### ChannelSink

Sends packets to a Tokio `mpsc` channel for async processing.

### CallbackSink

Invokes a user callback for each dispatched packet.

## CompositeBroker

`CompositeBroker` routes payload resolution by URI scheme and dispatches using
one configured sink.

```rust
use decision_gate_broker::{CompositeBroker, FileSource, InlineSource, LogSink};

let broker = CompositeBroker::builder()
    .source("file", FileSource::new("/workspace"))
    .source("inline", InlineSource::new())
    .sink(LogSink::new(std::io::stdout()))
    .build()?;
```

Schemes are matched directly. For `inline+json:` and `inline+bytes:` URIs,
register `inline` as the base scheme.

## Usage Examples

Resolve a file-backed payload and log disclosures:

```rust
use decision_gate_broker::{CompositeBroker, FileSource, LogSink};

let broker = CompositeBroker::builder()
    .source("file", FileSource::new("/tmp"))
    .sink(LogSink::new(std::io::stdout()))
    .build()?;
```

Custom sink with callback:

```rust
use decision_gate_broker::{CallbackSink, CompositeBroker, InlineSource};

let broker = CompositeBroker::builder()
    .source("inline", InlineSource::new())
    .sink(CallbackSink::new(|msg| {
        // Deliver to external system
        send_to_webhook(msg.target, msg.payload)?;
        Ok(())
    }))
    .build()?;
```

## Security Model

- **Size limits**: payloads are capped (2 MiB) to prevent resource exhaustion.
- **Scheme allowlist**: only registered schemes are resolved.
- **No redirects**: HTTP sources reject redirects.
- **Fail closed**: invalid URIs or fetch errors abort dispatch.

See `../../Docs/security/threat_model.md` for system-level assumptions.

## Testing

```bash
cargo test -p decision-gate-broker
```

## References

Paleface Swiss, & Stick To Your Guns. (2025). _Instrument of War_ [Audio recording]. YouTube. https://www.youtube.com/watch?v=5FTa4GJP5mc

Stick To Your Guns. (2017). _Married to the Noise_ [Audio recording]. YouTube. https://www.youtube.com/watch?v=OQqZVRn1mWM

Stick To Your Guns. (2017). _Delinelle_ [Audio recording]. YouTube. https://www.youtube.com/watch?v=v4GzHovFi8w
