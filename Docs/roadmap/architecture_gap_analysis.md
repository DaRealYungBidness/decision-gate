# Decision Gate Architecture & Ecosystem Design

*Reference Document - January 2026*

---

## Part 1: Vision & Rationale

### Why Decision Gate Exists

Decision Gate solves a problem that existing tools don't address: **auditable, deterministic control over when and what data is disclosed**.

The AI/LLM ecosystem has mature solutions for:
- Calling LLMs (Anthropic SDK, OpenAI SDK)
- Building agents (LangChain, CrewAI, AutoGPT)
- Tool execution (MCP, function calling)

But **none of these answer**:
- "Should this data be released right now?"
- "What evidence justifies this disclosure?"
- "Can I prove what was disclosed, when, and why?"
- "Can I replay this decision offline for audit?"

**Decision Gate's unique value**: It's a deterministic, replayable decision engine for controlled disclosure. It doesn't replace agent frameworks—it adds a missing layer of governance.

### Why a Separate Repository

Decision Gate currently lives in the AssetCore monorepo. This was convenient for development but creates problems:

1. **Coupling perception**: Decision Gate appears tied to AssetCore, but it's actually backend-agnostic
2. **Adoption friction**: Users who want Decision Gate must clone the entire AssetCore repo
3. **Versioning confusion**: Decision Gate releases are tangled with AssetCore releases
4. **Ecosystem signal**: A standalone repo signals "this is a general-purpose tool"

**Decision**: Move `ret-logic` and `decision-gate-core` to a new standalone repository.

### Design Philosophy

**1. Backend-agnostic core**
Decision Gate core has no opinion about where data comes from or where it goes. It defines traits (`Dispatcher`, `EvidenceProvider`, `PolicyDecider`) that users implement for their environment.

**2. Batteries-included broker**
While the core is abstract, we provide `decision-gate-broker` with reference implementations so users can get started immediately without writing plumbing code.

**3. Ecosystem-first**
The broker is designed to make Decision Gate trivially adoptable:
- Common sources work out of the box (files, HTTP)
- Common sinks work out of the box (channels, callbacks)
- Custom integrations (AssetCore, databases) are thin adapters

**4. Determinism is non-negotiable**
Every design decision preserves bit-for-bit reproducibility. Decision Gate's value depends on being able to replay any decision offline with identical results.

---

## Part 2: Repository Structure

### New Repository Layout

```
decision-gate/
├── README.md                 # What Decision Gate is, why it exists, quick start
├── SECURITY.md               # Fail-closed guarantees, threat model
├── Cargo.toml                # Workspace root
│
├── ret-logic/                # MOVED from AssetCore (no changes)
│   ├── Cargo.toml
│   └── src/
│       └── lib.rs            # Tri-state algebra, pure logic
│
├── decision-gate-core/       # MOVED from AssetCore (no changes)
│   ├── Cargo.toml
│   └── src/
│       ├── core/             # Types: DecisionRecord, PacketEnvelope, etc.
│       ├── runtime/          # ControlPlane engine
│       └── interfaces/       # Trait definitions (Dispatcher, etc.)
│
├── decision-gate-broker/     # NEW - reference implementations
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs            # Re-exports, composite broker
│       ├── source/           # Payload source implementations
│       │   ├── mod.rs
│       │   ├── file.rs       # FileSource: load from local paths
│       │   ├── http.rs       # HttpSource: fetch from URLs
│       │   └── inline.rs     # InlineSource: pass-through for embedded payloads
│       ├── sink/             # Delivery sink implementations
│       │   ├── mod.rs
│       │   ├── channel.rs    # ChannelSink: push to async channel
│       │   ├── callback.rs   # CallbackSink: invoke user function
│       │   └── log.rs        # LogSink: write to audit log only
│       └── broker.rs         # CompositeBroker: wires sources + sinks
│
└── examples/
    ├── minimal/              # Simplest possible scenario
    ├── file-disclosure/      # File-based disclosure workflow
    └── llm-scenario/         # Full LLM integration example
```

### Crate Responsibilities

| Crate | Purpose | Dependencies | New Code? |
|-------|---------|--------------|-----------|
| `ret-logic` | Tri-state boolean algebra | None (pure) | No - move as-is |
| `decision-gate-core` | Control plane engine | `ret-logic` | No - move as-is |
| `decision-gate-broker` | Reference implementations | `decision-gate-core`, `tokio` | **Yes - all new** |

### What Stays in AssetCore

AssetCore doesn't need any Decision Gate-specific code changes. If you want to use AssetCore as a data source:

```rust
// ~50 lines of glue code, could be in your application or a tiny crate
impl Source for AssetCoreSource {
    fn fetch(&self, content_ref: &ContentRef) -> Result<SourcePayload, SourceError> {
        // HTTP call to existing read daemon endpoint
        // Parse response into SourcePayload
    }
}
```

This is intentionally thin—AssetCore's HTTP API already exists.

---

## Part 3: The Broker Layer

### What is a Broker?

A broker bridges Decision Gate's abstract authorization decisions to concrete data movement. It has two jobs:

1. **Source resolution**: Given a `content_ref` string in a PacketEnvelope, fetch the actual bytes
2. **Sink delivery**: Given a payload and target, deliver it and return a receipt

```
┌─────────────────────────────────────────────────────────────┐
│                    BROKER ARCHITECTURE                       │
├─────────────────────────────────────────────────────────────┤
│                                                              │
│  PacketEnvelope ──┬──▶ Source ──▶ Payload                   │
│  (from Decision Gate)       │       │                                  │
│                   │       ▼                                  │
│                   └──▶ Sink ──▶ DispatchReceipt             │
│                          │      (back to Decision Gate)                │
│                          ▼                                   │
│                       Target                                 │
│                   (agent, channel, webhook)                  │
│                                                              │
└─────────────────────────────────────────────────────────────┘
```

### Why Brokers Exist

**Problem**: Decision Gate core defines `Dispatcher` as a trait, but users need to implement it themselves. This creates adoption friction—even a simple "load file and return it" requires writing boilerplate.

**Solution**: `decision-gate-broker` provides ready-made implementations:

```rust
// Without decision-gate-broker: user writes ~100 lines of plumbing
// With decision-gate-broker: user writes ~5 lines of configuration

let broker = CompositeBroker::builder()
    .source("file", FileSource::new("/data"))
    .source("http", HttpSource::new())
    .sink(ChannelSink::new(tx))
    .build();

let engine = ControlPlane::new(spec, broker, ...);
```

### Source Implementations

| Source | `content_ref` format | Behavior |
|--------|---------------------|----------|
| `FileSource` | `file:///path/to/doc.pdf` | Read from local filesystem |
| `HttpSource` | `https://api.example.com/data` | HTTP GET, return body |
| `InlineSource` | (none - payload already embedded) | Pass-through for `PacketPayload::Json/Bytes` |

**Extensibility**: Users implement `Source` trait for custom backends:
```rust
pub trait Source: Send + Sync {
    fn fetch(&self, content_ref: &ContentRef) -> Result<SourcePayload, SourceError>;
}
```

### Sink Implementations

| Sink | Behavior | Use Case |
|------|----------|----------|
| `ChannelSink` | Push to `tokio::sync::mpsc` | Async processing, MCP integration |
| `CallbackSink` | Invoke user-provided `Fn` | Synchronous handlers |
| `LogSink` | Write to audit log, don't deliver | Dry-run, compliance logging |

**Extensibility**: Users implement `Sink` trait for custom delivery:
```rust
pub trait Sink: Send + Sync {
    fn deliver(&self, target: &DispatchTarget, payload: &Payload) -> Result<DispatchReceipt, SinkError>;
}
```

### Composite Broker

The `CompositeBroker` wires sources and sinks together and implements `Dispatcher`:

```rust
impl Dispatcher for CompositeBroker {
    fn dispatch(
        &self,
        target: &DispatchTarget,
        envelope: &PacketEnvelope,
        payload: &PacketPayload,
    ) -> Result<DispatchReceipt, DispatchError> {
        // 1. Resolve payload (if External, fetch from source)
        let resolved = match payload {
            PacketPayload::External { content_ref } => {
                let resolved = self.resolve_source(&content_ref.uri)?.fetch(content_ref)?;
                Payload {
                    envelope: envelope.clone(),
                    body: PayloadBody::Bytes(resolved.bytes),
                }
            }
            PacketPayload::Json { value } => Payload {
                envelope: envelope.clone(),
                body: PayloadBody::Json(value.clone()),
            },
            PacketPayload::Bytes { bytes } => Payload {
                envelope: envelope.clone(),
                body: PayloadBody::Bytes(bytes.clone()),
            },
        };

        // 2. Deliver via sink
        self.sink.deliver(target, &resolved)
    }
}
```

---

## Part 4: Ecosystem Integration

### How Users Adopt Decision Gate

**Level 1: Core only** (maximum control)
```toml
[dependencies]
decision-gate-core = "0.1"
```
User implements all traits themselves. Full control, more code.

**Level 2: Core + Broker** (recommended)
```toml
[dependencies]
decision-gate-core = "0.1"
decision-gate-broker = "0.1"
```
User configures broker with provided sources/sinks. Minimal code, works immediately.

**Level 3: Full example** (learning/prototyping)
Clone the repo and run an example:
```bash
cargo run --example llm-scenario
```

### Integration with LLM Frameworks

Decision Gate doesn't replace LLM frameworks—it sits alongside them:

```
┌─────────────────────────────────────────────────────────────┐
│                    USER'S APPLICATION                        │
├─────────────────────────────────────────────────────────────┤
│                                                              │
│   ┌─────────────┐     ┌─────────────┐     ┌─────────────┐  │
│   │             │     │             │     │             │   │
│   │  LangChain  │ ◀──▶│     Decision Gate     │◀───▶│   Broker    │   │
│   │  / CrewAI   │     │  (control)  │     │  (delivery) │   │
│   │  / Custom   │     │             │     │             │   │
│   │             │     │             │     │             │   │
│   └─────────────┘     └─────────────┘     └─────────────┘  │
│         │                   │                   │           │
│         ▼                   ▼                   ▼           │
│   ┌─────────────┐     ┌─────────────┐     ┌─────────────┐  │
│   │   LLM API   │     │   Runpack   │     │   Sources   │  │
│   │  (Anthropic,│     │   (audit)   │     │(files, APIs)│  │
│   │   OpenAI)   │     │             │     │             │   │
│   └─────────────┘     └─────────────┘     └─────────────┘  │
│                                                              │
└─────────────────────────────────────────────────────────────┘
```

**Typical integration pattern**:
1. Agent framework handles conversation and tool routing
2. When a tool needs controlled data, it calls `scenario_next()`
3. Decision Gate evaluates gates, issues PacketEnvelope
4. Broker fetches payload and delivers to agent context
5. Agent framework continues with the disclosed data

### MCP Integration (Future)

MCP (Model Context Protocol) is a natural fit for Decision Gate:
- MCP tools can wrap `scenario_status`, `scenario_next`, `scenario_submit`
- Broker's `ChannelSink` can feed MCP tool responses
- This could be a separate `decision-gate-mcp` crate or an example

---

## Part 5: Gap Analysis (Current State)

### Feature Completeness

| # | Requirement | Status |
|---|-------------|--------|
| 1 | Deterministic engine codepath | ✅ PRESENT |
| 2 | Decision artifact with correlation ID | ✅ PRESENT |
| 3 | Evaluation trace (tri-state) | ✅ PRESENT |
| 4 | Evidence anchoring | ✅ PRESENT |
| 5 | Safe summaries | ⚠️ PARTIAL (no leak tests) |
| 6 | Runpack captures full audit | ✅ PRESENT |
| 7 | Offline verifier | ✅ PRESENT |
| 8 | Async story (holds/idempotence) | ✅ PRESENT |
| 9 | Adversarial input tests | ⚠️ PARTIAL (minimal coverage) |
| 10 | No bypass surfaces | ✅ PRESENT |

**Summary**: 8/10 Present, 2/10 Partial

### Determinism Risk Register

| Risk | Mitigation | Status |
|------|------------|--------|
| Map iteration order | BTreeMap | ✅ Mitigated |
| Float formatting | RFC 8785 | ✅ Mitigated |
| Timestamps | Input-controlled | ✅ Mitigated |
| Random IDs | Caller-provided | ✅ Mitigated |
| JSON key order | RFC 8785 | ✅ Mitigated |

**Verdict**: No unmitigated risks.

---

## Part 6: Remaining Work

### Phase 1: Repository Setup (No code changes)
1. Create new `decision-gate` repository
2. Move `ret-logic` crate (copy, preserve history if desired)
3. Move `decision-gate-core` crate (copy, preserve history if desired)
4. Set up CI/CD, README, SECURITY.md

### Phase 2: Broker Implementation (New code)
1. Define `Source` and `Sink` traits
2. Implement `FileSource`, `HttpSource`, `InlineSource`
3. Implement `ChannelSink`, `CallbackSink`, `LogSink`
4. Implement `CompositeBroker`
5. Write tests for each component

### Phase 3: Examples
1. `minimal`: Simplest possible scenario demonstrating Decision Gate flow
2. `file-disclosure`: Multi-stage workflow with file-based payloads
3. `llm-scenario`: Full integration with an LLM (could use Claude API)

### Phase 4: Hardening (Optional, industrial-grade)
1. Add adversarial input tests
2. Add SafeSummary leak verification tests
3. Expand SECURITY.md with threat model details

### Phase 5: Ecosystem Outreach
1. Publish to crates.io
2. Write blog post / announcement
3. Create integration examples for popular frameworks
