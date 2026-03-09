//! TUI runner wrapper with DAP server startup support.
//!
//! This module wraps [`fdemon_tui::run_with_project_and_dap`] which applies
//! any CLI-level overrides (such as `--dap-port`) and evaluates the
//! `should_auto_start_dap()` decision before entering the main event loop.
//!
//! ## DAP startup in TUI mode
//!
//! 1. `--dap-port` CLI flag sets `dap.enabled = true` and overrides the port.
//! 2. `should_auto_start_dap()` is evaluated: returns true when `dap.enabled`
//!    or when an IDE is detected and `auto_start_in_ide = true`.
//! 3. If true, `Message::StartDapServer` is sent before the event loop starts.
//! 4. Engine shutdown stops the DAP server if it is running.

use std::path::Path;

use fdemon_core::prelude::*;

/// Run the TUI application with a Flutter project and optional DAP configuration.
///
/// # Arguments
///
/// * `project_path` — Path to the Flutter project directory.
/// * `dap_port` — If `Some(port)`, overrides `settings.dap.port` and forces
///   `settings.dap.enabled = true`. Use `0` for an OS-assigned ephemeral port.
/// * `dap_config` — If `Some(ide)`, stores the CLI-provided IDE override on
///   `AppState` so `handle_started()` can pass it to `GenerateIdeConfig`,
///   bypassing environment-based IDE detection.
pub async fn run_with_project_and_dap(
    project_path: &Path,
    dap_port: Option<u16>,
    dap_config: Option<fdemon_app::config::ParentIde>,
) -> Result<()> {
    // Delegate to the TUI runner which handles DAP port override and auto-start.
    fdemon_tui::run_with_project_and_dap(project_path, dap_port, dap_config).await
}
