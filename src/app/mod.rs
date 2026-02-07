//! Application layer - state management and orchestration

pub mod actions;
pub mod confirm_dialog;
pub mod editor;
pub mod handler;
pub mod hyperlinks;
pub mod log_view_state;
pub mod message;
pub mod new_session_dialog;
pub mod process;
pub mod session;
pub mod session_manager;
pub mod settings_items;
pub mod signals;
pub mod spawn;
pub mod state;

// Re-export handler types for event loop integration
pub use handler::{Task, UpdateAction, UpdateResult};

// Re-export session types
pub use session::{Session, SessionHandle, SessionId};
pub use session_manager::{SessionManager, MAX_SESSIONS};

use std::path::PathBuf;

use crate::common::prelude::*;
use crate::tui;

/// Main application entry point
///
/// Parses command-line arguments and runs the TUI with the specified
/// Flutter project (or current directory if not specified).
pub async fn run() -> Result<()> {
    // Initialize error handling
    color_eyre::install().map_err(|e| Error::terminal(e.to_string()))?;

    // Initialize logging (to file, since TUI owns stdout)
    crate::common::logging::init()?;

    info!("═══════════════════════════════════════════════════════");
    info!("Flutter Demon starting");
    info!("═══════════════════════════════════════════════════════");

    // Get project path from args or current directory
    let project_path = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());

    info!("Project path: {}", project_path.display());

    // Run the TUI with Flutter project
    let result = tui::run_with_project(&project_path).await;

    if let Err(ref e) = result {
        error!("Application error: {:?}", e);
    }

    info!("Flutter Demon exiting");
    result
}

/// Main application entry point with a specific project path
pub async fn run_with_project(project_path: &std::path::Path) -> Result<()> {
    // Initialize error handling
    color_eyre::install().map_err(|e| Error::terminal(e.to_string()))?;

    // Initialize logging
    crate::common::logging::init()?;

    info!("═══════════════════════════════════════════════════════");
    info!("Flutter Demon starting");
    info!("Project: {}", project_path.display());
    info!("═══════════════════════════════════════════════════════");

    // Run the TUI application with Flutter
    let result = tui::run_with_project(project_path).await;

    if let Err(ref e) = result {
        error!("Application error: {:?}", e);
    }

    info!("Flutter Demon exiting");
    result
}
