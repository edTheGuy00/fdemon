//! # fdemon-app - Application State and Orchestration
//!
//! Implements the TEA (The Elm Architecture) pattern, the Engine abstraction,
//! configuration loading, service traits, and file watching.
//!
//! Depends on [`fdemon_core`] and [`fdemon_daemon`].
//!
//! ## Architecture
//!
//! The crate is organized around the **Engine** -- the shared orchestration core
//! used by both TUI and headless runners:
//!
//! ```text
//! Runner (TUI/Headless)
//!     │
//!     ▼
//!   Engine
//!     ├── AppState (TEA Model)
//!     ├── Message Channel
//!     ├── Session Tasks
//!     ├── File Watcher
//!     ├── SharedState (Service Layer)
//!     └── Event Broadcasting
//! ```
//!
//! ## Public API
//!
//! ### Engine (`engine`)
//! - [`Engine`] - Shared orchestration core
//! - [`EngineEvent`] - Domain events for external consumers
//! - [`EnginePlugin`] - Extension trait for plugin hooks
//!
//! ### TEA Pattern
//! - [`AppState`] - Complete application state (the Model)
//! - [`Message`] - All possible events/actions
//! - [`UpdateAction`] - Side effects from message processing
//! - [`UpdateResult`] - Return type from the update function
//!
//! ### Sessions
//! - [`Session`] - Per-device session state
//! - [`SessionHandle`] - Session + process + command sender
//! - [`SessionManager`] - Multi-session coordination
//!
//! ### Services (Extension Point)
//! - [`services::FlutterController`] - Reload/restart/stop
//! - [`services::LogService`] - Log buffer access
//! - [`services::StateService`] - App state access
//!
//! ### Configuration
//! - [`config::Settings`] - Global settings from `.fdemon/config.toml`
//! - [`config::LaunchConfig`] - Launch configuration
//!
//! ## Extension Points
//!
//! Two mechanisms for extending the Engine:
//!
//! 1. **Event subscription** via [`Engine::subscribe()`] -- async broadcast channel
//! 2. **Plugin trait** via [`EnginePlugin`] -- synchronous lifecycle callbacks

pub(crate) mod actions;
pub mod config;
pub mod confirm_dialog;
pub mod editor;
pub mod engine;
pub mod engine_event;
pub mod handler;
pub mod hyperlinks;
pub(crate) mod input_key;
pub mod log_view_state;
pub mod message;
pub mod new_session_dialog;
pub mod plugin;
pub(crate) mod process;
pub mod services;
pub mod session;
pub mod session_manager;
pub mod settings_items;
pub(crate) mod signals;
pub mod spawn;
pub mod state;
pub mod watcher;

// Re-export primary types
pub use engine::Engine;
pub use engine_event::EngineEvent;
pub use handler::{update, Task, UpdateAction, UpdateResult};
pub use message::{DebugOverlayKind, Message};
pub use plugin::EnginePlugin;
pub use session::{Session, SessionHandle, SessionId};
pub use session_manager::{SessionManager, MAX_SESSIONS};
pub use state::{AppState, DevToolsError, DevToolsPanel, DevToolsViewState, InspectorState};

// Re-export action types used by TUI for startup
pub use actions::SessionTaskMap;

// Re-export input key type used by TUI for event conversion
pub use input_key::InputKey;

/// Re-exported from `fdemon-daemon` for crates that depend on `fdemon-app`
/// but not `fdemon-daemon` directly (e.g., `fdemon-tui`).
pub use fdemon_daemon::{AndroidAvd, Device, IosSimulator, SimulatorState, ToolAvailability};
