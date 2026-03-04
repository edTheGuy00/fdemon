//! # DAP Protocol Module
//!
//! Provides the wire-level types and framing codec for the Debug Adapter
//! Protocol (DAP). All communication between a DAP client (e.g., VS Code)
//! and the fdemon DAP adapter flows through this module.
//!
//! ## Sub-modules
//!
//! - [`types`] — DAP message types: `DapMessage`, `DapRequest`, `DapResponse`,
//!   `DapEvent`, `Capabilities`, `InitializeRequestArguments`.
//! - [`codec`] — Content-Length framed async reader/writer: `read_message`,
//!   `write_message`.

pub mod codec;
pub mod types;

// Re-export everything at the protocol module level for ergonomic usage.
pub use codec::{read_message, write_message, MAX_MESSAGE_SIZE};
pub use types::{
    Capabilities, DapEvent, DapMessage, DapRequest, DapResponse, InitializeRequestArguments,
};
