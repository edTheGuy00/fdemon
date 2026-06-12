//! Service layer for flutter-demon
//!
//! This module provides service traits and implementations that abstract
//! Flutter daemon operations. Both the TUI and future MCP server use these
//! services for consistent behavior.
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────┐     ┌─────────────┐
//! │     TUI     │     │  MCP Server │
//! └──────┬──────┘     └──────┬──────┘
//!        │                   │
//!        └─────────┬─────────┘
//!                  │
//!           ┌──────▼──────┐
//!           │  Services   │
//!           │  (traits)   │
//!           └──────┬──────┘
//!                  │
//!           ┌──────▼──────┐
//!           │ SharedState │
//!           │ (Arc<RwLock>)│
//!           └─────────────┘
//! ```
//!
//! ## Key Components
//!
//! - [`SharedState`]: Thread-safe shared state with event broadcasting
//! - [`FlutterController`]: Hot reload/restart operations
//! - [`LogService`]: Log buffer access and filtering
//! - [`StateService`]: App state and device queries
//! - [`SessionService`]: Session listing, start/stop by id, DevTools URLs
//! - [`ProjectService`]: Project-level operations (`pub get`, `clean`)

pub mod clipboard;
mod flutter_controller;
mod log_service;
mod project_service;
mod session_service;
mod state_service;

#[cfg(test)]
pub use clipboard::MemoryClipboard;
pub use clipboard::{create_clipboard, Clipboard, ClipboardInit, NullClipboard, SystemClipboard};

pub use flutter_controller::{
    CommandSenderController, DaemonFlutterController, FlutterCommand, FlutterController,
    LocalFlutterController, ReloadResult, RestartResult,
};

pub use log_service::{LocalLogService, LogFilter, LogService, SharedLogService};

pub use project_service::{
    CommandOutput, FlutterProjectService, LocalProjectService, ProjectService,
};

pub use session_service::{
    LocalSessionService, SessionService, SessionSnapshot, SharedSessionService,
};

pub use state_service::{
    AppRunState, LocalStateService, ProjectInfo, SharedState, SharedStateService, StateService,
};
