//! # DAP Stdio Mode Runner
//!
//! Implements the `--dap-stdio` execution path. This runs fdemon as a single-session
//! DAP adapter over stdin/stdout, suitable for IDE integration with Zed, Helix,
//! and nvim-dap.
//!
//! ## What this does NOT do (intentionally)
//!
//! - Does **not** start a Flutter Engine or Flutter process.
//! - Does **not** start the TUI (incompatible with stdio DAP).
//! - Does **not** route `attach` commands to the Dart VM Service.
//!
//! Those concerns are handled in later phases (tasks 03 and 10). This runner
//! only establishes the DAP transport layer over stdin/stdout.
//!
//! ## Stdout Isolation
//!
//! The tracing subscriber is configured by the caller (`main.rs`) to write to
//! stderr. This runner must not write anything to stdout. Any stdout output
//! (including accidental `println!()`) would corrupt the DAP wire protocol.

use tokio::sync::mpsc;

use fdemon_core::prelude::*;

use fdemon_dap::{DapServerEvent, DapService};

/// Run as a DAP adapter over stdin/stdout.
///
/// Starts a single DAP session over the process's stdin/stdout streams,
/// forwards lifecycle events to tracing, and returns when the DAP client
/// disconnects or a fatal error occurs.
///
/// # Returns
///
/// `Ok(())` after the DAP client disconnects cleanly.
/// `Err(_)` on fatal I/O errors.
pub async fn run_dap_stdio() -> Result<()> {
    tracing::info!("Starting DAP stdio session (adapter subprocess mode)");
    tracing::info!("All non-DAP output is directed to stderr");

    let (event_tx, mut event_rx) = mpsc::channel::<DapServerEvent>(16);

    // Start the stdio DAP session (runs until client disconnects or shutdown).
    let handle = DapService::start_stdio(event_tx).await?;

    // Bridge lifecycle events to tracing (no-op: this version doesn't start an Engine).
    // In a future task, these events will be mapped to Engine messages for debug session management.
    let event_task = tokio::spawn(async move {
        while let Some(event) = event_rx.recv().await {
            match event {
                DapServerEvent::ClientConnected { client_id } => {
                    tracing::info!("DAP client connected: {}", client_id);
                }
                DapServerEvent::ClientDisconnected { client_id } => {
                    tracing::info!("DAP client disconnected: {}", client_id);
                    // In stdio mode, a single client owns the process lifetime.
                    // Once it disconnects, there will be no further meaningful
                    // events. Break immediately so the event consumer exits
                    // without waiting for the channel to close naturally.
                    break;
                }
                DapServerEvent::ServerError { reason } => {
                    tracing::error!("DAP server error: {}", reason);
                }
                DapServerEvent::DebugSessionStarted { client_id } => {
                    tracing::info!("DAP debug session started: {}", client_id);
                }
                DapServerEvent::DebugSessionEnded { client_id } => {
                    tracing::info!("DAP debug session ended: {}", client_id);
                }
            }
        }
        tracing::debug!("DAP event channel closed");
    });

    // Wait for the stdio session to complete (client disconnected or shutdown).
    DapService::stop(handle).await;

    // Wait for the event consumer to drain.
    let _ = tokio::time::timeout(std::time::Duration::from_secs(2), event_task).await;

    tracing::info!("DAP stdio session ended");
    Ok(())
}
