//! # fdemon-dap — Debug Adapter Protocol Implementation
//!
//! Provides the DAP protocol types, Content-Length framing codec, and server
//! infrastructure for embedding a DAP server inside fdemon, enabling IDE
//! debugger integration (VS Code, IntelliJ, Neovim, etc.).
//!
//! ## Layer Boundary
//!
//! This crate depends on `fdemon-core` (domain types). It does **not** depend
//! on `fdemon-app`, `fdemon-tui`, or `fdemon-daemon`. The `fdemon-daemon`
//! dependency will be added in a future phase when the VM Service bridge for
//! debug operations is implemented.
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

pub mod protocol;
pub mod server;
pub mod service;

// Re-export all protocol types at the crate root for ergonomic usage.
pub use protocol::{
    read_message, write_message, Capabilities, DapEvent, DapMessage, DapRequest, DapResponse,
    InitializeRequestArguments, MAX_MESSAGE_SIZE,
};

// Re-export server types at the crate root for ergonomic usage.
pub use server::{
    DapClientSession, DapServerConfig, DapServerEvent, DapServerHandle, SessionState,
};

// Re-export DapService for ergonomic usage.
pub use service::DapService;
