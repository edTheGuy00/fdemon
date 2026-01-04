//! Message processing with session event routing
//!
//! Handles TEA message processing and routes JSON-RPC responses
//! to the appropriate RequestTracker for both legacy single-session
//! and multi-session modes.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use tokio::sync::{mpsc, watch, Mutex};

use crate::app::handler::Task;
use crate::app::message::Message;
use crate::app::session::SessionId;
use crate::app::state::AppState;
use crate::app::{handler, UpdateAction};
use crate::core::DaemonEvent;
use crate::daemon::{protocol, CommandSender, DaemonMessage};

use super::actions::handle_action;

/// Process a message through the TEA update function
pub fn process_message(
    state: &mut AppState,
    message: Message,
    msg_tx: &mpsc::Sender<Message>,
    cmd_sender: &Arc<Mutex<Option<CommandSender>>>,
    session_tasks: &Arc<Mutex<HashMap<SessionId, tokio::task::JoinHandle<()>>>>,
    shutdown_rx: &watch::Receiver<bool>,
    project_path: &Path,
) {
    // Route responses from Message::Daemon events (legacy single-session mode)
    route_legacy_daemon_response(&message, cmd_sender);

    // Route responses from Message::SessionDaemon events (multi-session mode)
    route_session_daemon_response(&message, state);

    // Process message through TEA update loop
    let mut msg = Some(message);
    while let Some(m) = msg {
        let result = handler::update(state, m);

        // Handle any action
        if let Some(action) = result.action {
            let session_cmd_sender = get_session_cmd_sender(&action, state);

            handle_action(
                action,
                msg_tx.clone(),
                cmd_sender.clone(),
                session_cmd_sender,
                session_tasks.clone(),
                shutdown_rx.clone(),
                project_path,
            );
        }

        // Continue with follow-up message
        msg = result.message;
    }
}

/// Route JSON-RPC responses for legacy daemon events
fn route_legacy_daemon_response(message: &Message, cmd_sender: &Arc<Mutex<Option<CommandSender>>>) {
    if let Message::Daemon(DaemonEvent::Stdout(ref line)) = message {
        if let Some(json) = protocol::strip_brackets(line) {
            if let Some(DaemonMessage::Response { id, result, error }) = DaemonMessage::parse(json)
            {
                // Try to get the command sender for response routing
                if let Ok(guard) = cmd_sender.try_lock() {
                    if let Some(ref sender) = *guard {
                        if let Some(id_num) = id.as_u64() {
                            let tracker = sender.tracker().clone();
                            tokio::spawn(async move {
                                tracker.handle_response(id_num, result, error).await;
                            });
                        }
                    }
                }
            }
        }
    }
}

/// Route JSON-RPC responses for multi-session daemon events
fn route_session_daemon_response(message: &Message, state: &AppState) {
    if let Message::SessionDaemon {
        session_id,
        event: DaemonEvent::Stdout(ref line),
    } = message
    {
        if let Some(json) = protocol::strip_brackets(line) {
            if let Some(DaemonMessage::Response { id, result, error }) = DaemonMessage::parse(json)
            {
                // Use session-specific cmd_sender for response routing
                if let Some(handle) = state.session_manager.get(*session_id) {
                    if let Some(ref sender) = handle.cmd_sender {
                        if let Some(id_num) = id.as_u64() {
                            let tracker = sender.tracker().clone();
                            tokio::spawn(async move {
                                tracker.handle_response(id_num, result, error).await;
                            });
                        }
                    }
                }
            }
        }
    }
}

/// Get session-specific command sender for SpawnTask actions
fn get_session_cmd_sender(action: &UpdateAction, state: &AppState) -> Option<CommandSender> {
    if let UpdateAction::SpawnTask(task) = action {
        let session_id = match task {
            Task::Reload { session_id, .. } => *session_id,
            Task::Restart { session_id, .. } => *session_id,
            Task::Stop { session_id, .. } => *session_id,
        };
        // Look up session-specific cmd_sender (session_id 0 means legacy mode)
        if session_id > 0 {
            return state
                .session_manager
                .get(session_id)
                .and_then(|h| h.cmd_sender.clone());
        }
    }
    None
}
