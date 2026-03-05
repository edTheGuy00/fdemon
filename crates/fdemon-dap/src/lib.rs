//! # fdemon-dap — Debug Adapter Protocol Implementation
//!
//! Provides the DAP protocol types, Content-Length framing codec, server
//! infrastructure, and adapter core for embedding a DAP server inside fdemon,
//! enabling IDE debugger integration (VS Code, IntelliJ, Neovim, etc.).
//!
//! ## Layer Boundary
//!
//! This crate depends on `fdemon-core` (domain types). It does **not** depend
//! on `fdemon-app`, `fdemon-tui`, or `fdemon-daemon`. The VM Service bridge is
//! provided by `fdemon-app` via the [`adapter::DebugBackend`] trait.
//!
//! ## Public API
//!
//! ### Protocol Types (`protocol::types`)
//! - [`DapMessage`] — Top-level tagged enum (`request` | `response` | `event`)
//! - [`DapRequest`] — Client → Server request
//! - [`DapResponse`] — Server → Client response
//! - [`DapEvent`] — Server → Client unsolicited event
//! - [`Capabilities`] — Server capabilities for the initialize handshake
//! - [`InitializeRequestArguments`] — Client capabilities from initialize request
//!
//! ### Codec (`protocol::codec`)
//! - [`read_message`] — Read a Content-Length framed DAP message from an async reader
//! - [`write_message`] — Write a Content-Length framed DAP message to an async writer
//!
//! ### Adapter (`adapter`)
//! - [`adapter::DapAdapter`] — Core bridge between DAP and VM Service
//! - [`adapter::DebugBackend`] — Trait implemented by the Engine integration layer
//! - [`adapter::DebugEvent`] — Debug events forwarded from VM Service to adapter
//! - [`adapter::ThreadMap`] — Isolate ID ↔ DAP thread ID mapping
//! - [`adapter::VariableStore`] — Variable reference allocator (invalidated on resume)
//! - [`adapter::FrameStore`] — Frame ID allocator (invalidated on resume)
//! - [`adapter::BreakpointState`] — Breakpoint tracking state
//! - [`adapter::log_level_to_category`] — Map log level string to DAP output category

pub mod adapter;
pub mod protocol;
pub mod server;
pub mod service;
pub mod transport;

// Re-export all protocol types at the crate root for ergonomic usage.
pub use protocol::{
    read_message, write_message, Capabilities, DapEvent, DapMessage, DapRequest, DapResponse,
    InitializeRequestArguments, MAX_MESSAGE_SIZE,
};

// Re-export server types at the crate root for ergonomic usage.
pub use server::{
    DapClientSession, DapServerConfig, DapServerEvent, DapServerHandle, NoopBackend, SessionState,
};

// Re-export DapService for ergonomic usage.
pub use service::DapService;

// Re-export transport types for ergonomic usage.
pub use transport::TransportMode;
