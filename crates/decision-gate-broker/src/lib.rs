// crates/decision-gate-broker/src/lib.rs
// ============================================================================
// Module: Decision Gate Broker Library
// Description: Reference sources/sinks and composite dispatcher for Decision Gate.
// Purpose: Resolve external payloads and dispatch disclosures.
// Dependencies: decision-gate-core, reqwest, tokio, url
// ============================================================================

//! ## Overview
//! Decision Gate Broker provides ready-made [`Source`] and [`Sink`] implementations
//! plus the [`CompositeBroker`] dispatcher that wires them together.
//! Invariants:
//! - Payload hashes are validated against envelope or content reference hashes.
//! - Source payloads are capped at [`MAX_SOURCE_BYTES`].
//! - Sinks return receipts only on successful delivery.
//!
//! Security posture: resolves untrusted content references and dispatch targets;
//! see `Docs/security/threat_model.md`.

// ============================================================================
// SECTION: Modules
// ============================================================================

pub mod broker;
pub mod payload;
pub mod sink;
pub mod source;

// ============================================================================
// SECTION: Re-Exports
// ============================================================================

pub use broker::BrokerError;
pub use broker::CompositeBroker;
pub use broker::CompositeBrokerBuilder;
pub use payload::Payload;
pub use payload::PayloadBody;
pub use sink::CallbackSink;
pub use sink::ChannelSink;
pub use sink::DispatchMessage;
pub use sink::LogSink;
pub use sink::Sink;
pub use sink::SinkError;
pub use source::FileSource;
pub use source::HttpSource;
pub use source::HttpSourcePolicy;
pub use source::InlineSource;
pub use source::MAX_SOURCE_BYTES;
pub use source::Source;
pub use source::SourceError;
pub use source::SourcePayload;

#[cfg(test)]
mod tests;
