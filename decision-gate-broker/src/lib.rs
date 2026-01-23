// decision-gate-broker/src/lib.rs
// ============================================================================
// Module: Decision Gate Broker Library
// Description: Reference sources/sinks and composite dispatcher for Decision Gate.
// Purpose: Resolve external payloads and dispatch disclosures.
// Dependencies: decision-gate-core, reqwest, tokio, url
// ============================================================================

//! ## Overview
//! Decision Gate Broker provides ready-made source and sink implementations
//! plus a composite dispatcher that wires them together.

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
pub use source::InlineSource;
pub use source::Source;
pub use source::SourceError;
pub use source::SourcePayload;

#[cfg(test)]
mod tests {
    //! Test-only lint relaxations for panic-based assertions and debug output.
    #![allow(
        clippy::panic,
        clippy::print_stdout,
        clippy::print_stderr,
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::use_debug,
        clippy::dbg_macro,
        clippy::panic_in_result_fn,
        clippy::unwrap_in_result,
        reason = "Test-only output and panic-based assertions are permitted."
    )]
}
