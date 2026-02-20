//! Message processing with session event routing
//!
//! Handles TEA message processing and routes JSON-RPC responses
//! to the appropriate RequestTracker for multi-session mode.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use tokio::sync::{mpsc, watch};

use crate::handler::Task;
use crate::message::Message;
use crate::session::SessionId;
use crate::state::AppState;
use crate::{handler, UpdateAction};
use fdemon_core::{DaemonEvent, DaemonMessage};
use fdemon_daemon::{parse_daemon_message, CommandSender};

use super::actions::handle_action;

/// Process a message through the TEA update function
pub fn process_message(
    state: &mut AppState,
    message: Message,
    msg_tx: &mpsc::Sender<Message>,
    session_tasks: &Arc<std::sync::Mutex<HashMap<SessionId, tokio::task::JoinHandle<()>>>>,
    shutdown_rx: &watch::Receiver<bool>,
    project_path: &Path,
) {
    // Route JSON-RPC responses from SessionDaemon events to RequestTracker
    route_session_daemon_response(&message, state);

    // Process message through TEA update loop
    let mut msg = Some(message);
    while let Some(m) = msg {
        let result = handler::update(state, m);

        // Handle any action
        if let Some(action) = result.action {
            // For ReloadAllSessions, collect cmd_senders for all sessions
            let session_senders = get_session_cmd_senders_for_action(&action, state);
            let session_cmd_sender = get_session_cmd_sender(&action, state);

            // Capture the pre-hydration action for defense-in-depth failure
            // reporting. If hydration discards a FetchWidgetTree or
            // FetchLayoutData action (e.g. VM disconnected between handler and
            // hydration), we send a failure message so the loading spinner is
            // cleared.
            let pre_hydration_action = action.clone();

            // Hydrate actions that carry an optional VmRequestHandle with the
            // actual handle from the session. The handlers only return session_id;
            // we need the handle from AppState here before dispatching.
            let action = hydrate_start_performance_monitoring(action, state);
            let action = action.and_then(|a| hydrate_fetch_widget_tree(a, state));
            let action = action.and_then(|a| hydrate_fetch_layout_data(a, state));
            let action = action.and_then(|a| hydrate_toggle_overlay(a, state));
            let action = action.and_then(|a| hydrate_dispose_devtools_groups(a, state));

            if let Some(action) = action {
                handle_action(
                    action,
                    msg_tx.clone(),
                    session_cmd_sender,
                    session_senders,
                    session_tasks.clone(),
                    shutdown_rx.clone(),
                    project_path,
                    state.tool_availability.clone(),
                );
            } else {
                // Hydration discarded the action. Send a failure message for
                // fetch actions so the loading spinner is not stuck forever.
                match &pre_hydration_action {
                    UpdateAction::FetchWidgetTree { session_id, .. } => {
                        let _ = msg_tx.try_send(Message::WidgetTreeFetchFailed {
                            session_id: *session_id,
                            error: "VM Service handle unavailable".to_string(),
                        });
                    }
                    UpdateAction::FetchLayoutData { session_id, .. } => {
                        let _ = msg_tx.try_send(Message::LayoutDataFetchFailed {
                            session_id: *session_id,
                            error: "VM Service handle unavailable".to_string(),
                        });
                    }
                    _ => {}
                }
            }
        }

        // Continue with follow-up message
        msg = result.message;
    }
}

/// Hydrate `StartPerformanceMonitoring` with the `VmRequestHandle` from the
/// session, returning `None` if the handle is unavailable (e.g. the VM has not
/// yet connected or has already disconnected) — in that case the action is
/// silently discarded.
///
/// All other action variants are returned unchanged.
fn hydrate_start_performance_monitoring(
    action: UpdateAction,
    state: &AppState,
) -> Option<UpdateAction> {
    if let UpdateAction::StartPerformanceMonitoring {
        session_id,
        handle,
        performance_refresh_ms,
    } = action
    {
        if handle.is_some() {
            // Already hydrated (shouldn't happen in normal flow, but safe).
            return Some(UpdateAction::StartPerformanceMonitoring {
                session_id,
                handle,
                performance_refresh_ms,
            });
        }
        // Extract the VM request handle from the session. If unavailable,
        // discard the action — there is nothing to poll yet.
        let vm_handle = state
            .session_manager
            .get(session_id)
            .and_then(|h| h.vm_request_handle.clone())?;
        return Some(UpdateAction::StartPerformanceMonitoring {
            session_id,
            handle: Some(vm_handle),
            performance_refresh_ms,
        });
    }
    Some(action)
}

/// Hydrate `FetchWidgetTree` with the `VmRequestHandle` from the session.
///
/// Returns `None` (discards the action) if the session has no active VM
/// connection, since there is nothing to query without one.
/// All other action variants are returned unchanged.
fn hydrate_fetch_widget_tree(action: UpdateAction, state: &AppState) -> Option<UpdateAction> {
    if let UpdateAction::FetchWidgetTree {
        session_id,
        vm_handle,
        tree_max_depth,
    } = action
    {
        if vm_handle.is_some() {
            return Some(UpdateAction::FetchWidgetTree {
                session_id,
                vm_handle,
                tree_max_depth,
            });
        }
        let handle = state
            .session_manager
            .get(session_id)
            .and_then(|h| h.vm_request_handle.clone())?;
        return Some(UpdateAction::FetchWidgetTree {
            session_id,
            vm_handle: Some(handle),
            tree_max_depth,
        });
    }
    Some(action)
}

/// Hydrate `FetchLayoutData` with the `VmRequestHandle` from the session.
///
/// Returns `None` (discards the action) if the session has no active VM
/// connection, since there is nothing to query without one.
/// All other action variants are returned unchanged.
fn hydrate_fetch_layout_data(action: UpdateAction, state: &AppState) -> Option<UpdateAction> {
    if let UpdateAction::FetchLayoutData {
        session_id,
        node_id,
        vm_handle,
    } = action
    {
        if vm_handle.is_some() {
            return Some(UpdateAction::FetchLayoutData {
                session_id,
                node_id,
                vm_handle,
            });
        }
        let handle = state
            .session_manager
            .get(session_id)
            .and_then(|h| h.vm_request_handle.clone())?;
        return Some(UpdateAction::FetchLayoutData {
            session_id,
            node_id,
            vm_handle: Some(handle),
        });
    }
    Some(action)
}

/// Hydrate `ToggleOverlay` with the `VmRequestHandle` from the session.
///
/// Returns `None` (discards the action) if the session has no active VM
/// connection. All other action variants are returned unchanged.
fn hydrate_toggle_overlay(action: UpdateAction, state: &AppState) -> Option<UpdateAction> {
    if let UpdateAction::ToggleOverlay {
        session_id,
        extension,
        vm_handle,
    } = action
    {
        if vm_handle.is_some() {
            return Some(UpdateAction::ToggleOverlay {
                session_id,
                extension,
                vm_handle,
            });
        }
        let handle = state
            .session_manager
            .get(session_id)
            .and_then(|h| h.vm_request_handle.clone())?;
        return Some(UpdateAction::ToggleOverlay {
            session_id,
            extension,
            vm_handle: Some(handle),
        });
    }
    Some(action)
}

/// Hydrate `DisposeDevToolsGroups` with the `VmRequestHandle` from the session.
///
/// Unlike the fetch hydration functions, this one does **not** return `None`
/// when the handle is unavailable. If the VM is not connected there is nothing
/// to dispose, so the action is silently discarded by returning `None`.
/// All other action variants are returned unchanged.
fn hydrate_dispose_devtools_groups(action: UpdateAction, state: &AppState) -> Option<UpdateAction> {
    if let UpdateAction::DisposeDevToolsGroups {
        session_id,
        vm_handle,
    } = action
    {
        if vm_handle.is_some() {
            // Already hydrated.
            return Some(UpdateAction::DisposeDevToolsGroups {
                session_id,
                vm_handle,
            });
        }
        // If no VM handle is available (VM disconnected or not yet connected),
        // silently discard — there is nothing to dispose.
        let handle = state
            .session_manager
            .get(session_id)
            .and_then(|h| h.vm_request_handle.clone())?;
        return Some(UpdateAction::DisposeDevToolsGroups {
            session_id,
            vm_handle: Some(handle),
        });
    }
    Some(action)
}

/// Route JSON-RPC responses for multi-session daemon events
fn route_session_daemon_response(message: &Message, state: &AppState) {
    if let Message::SessionDaemon {
        session_id,
        event: DaemonEvent::Stdout(ref line),
    } = message
    {
        if let Some(DaemonMessage::Response { id, result, error }) = parse_daemon_message(line) {
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

/// Get session-specific command sender for SpawnTask actions
fn get_session_cmd_sender(action: &UpdateAction, state: &AppState) -> Option<CommandSender> {
    if let UpdateAction::SpawnTask(task) = action {
        let session_id = match task {
            Task::Reload { session_id, .. } => *session_id,
            Task::Restart { session_id, .. } => *session_id,
            Task::Stop { session_id, .. } => *session_id,
        };
        return state
            .session_manager
            .get(session_id)
            .and_then(|h| h.cmd_sender.clone());
    }
    None
}

/// Get command senders for all sessions in ReloadAllSessions action
fn get_session_cmd_senders_for_action(
    action: &UpdateAction,
    state: &AppState,
) -> Vec<(SessionId, String, CommandSender)> {
    if let UpdateAction::ReloadAllSessions { sessions } = action {
        sessions
            .iter()
            .filter_map(|(session_id, app_id)| {
                state
                    .session_manager
                    .get(*session_id)
                    .and_then(|h| h.cmd_sender.clone())
                    .map(|sender| (*session_id, app_id.clone(), sender))
            })
            .collect()
    } else {
        Vec::new()
    }
}
