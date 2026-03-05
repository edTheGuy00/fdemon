//! # DAP Stdio Runner
//!
//! Entry point for `--dap-stdio` mode, which runs fdemon as a DAP adapter
//! subprocess communicating over stdin/stdout.
//!
//! This module is only active when the `--dap-stdio` CLI flag is passed.
//! In this mode:
//! - The TUI is not started (it requires terminal raw mode which conflicts with DAP stdio).
//! - All tracing output goes to stderr (stdout is reserved for the DAP wire protocol).
//! - The process exits when the DAP client disconnects.

pub mod runner;
