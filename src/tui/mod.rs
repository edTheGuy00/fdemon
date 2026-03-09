//! TUI mode launcher with DAP server integration
//!
//! Wraps `fdemon_tui::run_with_project` with pre-flight configuration for
//! the `--dap-port` CLI flag, which is not yet handled by the library's own
//! runner (that wiring is in Task 06).

pub mod runner;
