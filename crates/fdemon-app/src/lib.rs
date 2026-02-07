//! fdemon-app - Application state and orchestration for Flutter Demon
//!
//! This crate implements the TEA (The Elm Architecture) pattern for state management,
//! the Engine abstraction for shared orchestration, configuration loading, service
//! traits, and file watching.

pub mod actions;
pub mod config;
pub mod confirm_dialog;
pub mod editor;
pub mod engine;
pub mod engine_event;
pub mod handler;
pub mod hyperlinks;
pub mod input_key;
pub mod log_view_state;
pub mod message;
pub mod new_session_dialog;
pub mod process;
pub mod services;
pub mod session;
pub mod session_manager;
pub mod settings_items;
pub mod signals;
pub mod spawn;
pub mod state;
pub mod watcher;

// Re-export primary types
pub use engine::Engine;
pub use engine_event::EngineEvent;
pub use handler::{Task, UpdateAction, UpdateResult};
pub use message::Message;
pub use session::{Session, SessionHandle, SessionId};
pub use session_manager::{SessionManager, MAX_SESSIONS};
pub use state::AppState;

// Re-export daemon types for TUI
pub use fdemon_daemon::{AndroidAvd, Device, IosSimulator, SimulatorState, ToolAvailability};
