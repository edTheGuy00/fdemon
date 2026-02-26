//! Action handlers: UpdateAction dispatch and background task spawning

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use tokio::sync::{mpsc, watch};
use tracing::warn;

use crate::handler::Task;
use crate::message::Message;
use crate::session::SessionId;
use crate::UpdateAction;
use fdemon_daemon::{CommandSender, ToolAvailability};

use super::spawn;

pub mod session;
pub use session::execute_task;

pub(super) mod inspector;
pub(super) mod network;
pub(super) mod performance;
pub(super) mod vm_service;

/// Convenience type alias for session task tracking
pub type SessionTaskMap = Arc<std::sync::Mutex<HashMap<SessionId, tokio::task::JoinHandle<()>>>>;

/// Execute an action by spawning a background task
#[allow(clippy::too_many_arguments)]
pub fn handle_action(
    action: UpdateAction,
    msg_tx: mpsc::Sender<Message>,
    session_cmd_sender: Option<CommandSender>,
    session_senders: Vec<(SessionId, String, CommandSender)>,
    session_tasks: SessionTaskMap,
    shutdown_rx: watch::Receiver<bool>,
    project_path: &Path,
    tool_availability: ToolAvailability,
) {
    match action {
        UpdateAction::SpawnTask(task) => {
            // Spawn async task for command execution using session-specific sender
            tokio::spawn(async move {
                execute_task(task, msg_tx, session_cmd_sender).await;
            });
        }

        UpdateAction::ReloadAllSessions { sessions: _ } => {
            // Spawn reload tasks for each session
            for (session_id, app_id, sender) in session_senders {
                let msg_tx_clone = msg_tx.clone();
                let task = Task::Reload { session_id, app_id };
                tokio::spawn(async move {
                    execute_task(task, msg_tx_clone, Some(sender)).await;
                });
            }
        }

        UpdateAction::DiscoverDevices => {
            spawn::spawn_device_discovery(msg_tx);
        }

        UpdateAction::RefreshDevicesBackground => {
            // Same as DiscoverDevices but errors are logged only (no UI feedback)
            // This runs when we already have cached devices displayed
            spawn::spawn_device_discovery_background(msg_tx);
        }

        UpdateAction::DiscoverDevicesAndAutoLaunch { configs } => {
            spawn::spawn_auto_launch(msg_tx, configs, project_path.to_path_buf());
        }

        UpdateAction::SpawnSession {
            session_id,
            device,
            config,
        } => {
            session::spawn_session(
                session_id,
                device,
                config,
                project_path,
                msg_tx,
                session_tasks,
                shutdown_rx,
            );
        }

        UpdateAction::DiscoverEmulators => {
            spawn::spawn_emulator_discovery(msg_tx);
        }

        UpdateAction::LaunchEmulator { emulator_id } => {
            spawn::spawn_emulator_launch(msg_tx, emulator_id);
        }

        UpdateAction::LaunchIOSSimulator => {
            spawn::spawn_ios_simulator_launch(msg_tx);
        }

        UpdateAction::CheckToolAvailability => {
            spawn::spawn_tool_availability_check(msg_tx);
        }

        UpdateAction::DiscoverBootableDevices => {
            spawn::spawn_bootable_device_discovery(msg_tx, tool_availability);
        }

        UpdateAction::BootDevice {
            device_id,
            platform,
        } => {
            spawn::spawn_device_boot(msg_tx, device_id, platform, tool_availability);
        }

        UpdateAction::AutoSaveConfig { configs } => {
            // Clone data for async task
            let project_path = project_path.to_path_buf();
            let tx = msg_tx.clone();

            // Spawn async save task to avoid blocking UI
            tokio::spawn(async move {
                match crate::config::writer::save_fdemon_configs(&project_path, &configs) {
                    Ok(()) => {
                        tracing::debug!("Config auto-saved successfully");
                        let _ = tx.send(Message::NewSessionDialogConfigSaved).await;
                    }
                    Err(e) => {
                        tracing::error!("Config auto-save failed: {}", e);
                        let _ = tx
                            .send(Message::NewSessionDialogConfigSaveFailed {
                                error: e.to_string(),
                            })
                            .await;
                    }
                }
            });
        }

        UpdateAction::LaunchFlutterSession {
            device: _,
            mode: _,
            flavor: _,
            dart_defines: _,
            config_name: _,
        } => {
            // NOTE: This action is no longer used - handle_launch now creates
            // the session and returns SpawnSession directly.
            // Kept for backward compatibility, but this branch should never execute.
            tracing::warn!("LaunchFlutterSession action reached - this should not happen");
        }

        UpdateAction::DiscoverEntryPoints { project_path } => {
            spawn::spawn_entry_point_discovery(msg_tx, project_path);
        }

        UpdateAction::ConnectVmService { session_id, ws_uri } => {
            let handle = vm_service::spawn_vm_service_connection(session_id, ws_uri, msg_tx);
            session_tasks.lock().unwrap().insert(session_id, handle);
        }

        UpdateAction::StartPerformanceMonitoring {
            session_id,
            handle,
            performance_refresh_ms,
            allocation_profile_interval_ms,
        } => {
            // `handle` is guaranteed to be Some here because process.rs
            // discards actions where it couldn't hydrate the handle.
            if let Some(vm_handle) = handle {
                performance::spawn_performance_polling(
                    session_id,
                    vm_handle,
                    msg_tx,
                    performance_refresh_ms,
                    allocation_profile_interval_ms,
                );
            } else {
                warn!(
                    "StartPerformanceMonitoring reached handle_action with no VmRequestHandle \
                     for session {} — skipping",
                    session_id
                );
            }
        }

        // ─────────────────────────────────────────────────────────
        // DevTools Actions (Phase 4, Task 02)
        // ─────────────────────────────────────────────────────────
        UpdateAction::FetchWidgetTree {
            session_id,
            vm_handle,
            tree_max_depth,
            fetch_timeout_secs,
        } => {
            if let Some(handle) = vm_handle {
                inspector::spawn_fetch_widget_tree(
                    session_id,
                    handle,
                    msg_tx,
                    tree_max_depth,
                    fetch_timeout_secs,
                );
            } else {
                warn!(
                    "FetchWidgetTree reached handle_action with no VmRequestHandle \
                     for session {} — skipping",
                    session_id
                );
            }
        }

        UpdateAction::FetchLayoutData {
            session_id,
            node_id,
            vm_handle,
        } => {
            if let Some(handle) = vm_handle {
                inspector::spawn_fetch_layout_data(session_id, node_id, handle, msg_tx);
            } else {
                warn!(
                    "FetchLayoutData reached handle_action with no VmRequestHandle \
                     for session {} — skipping",
                    session_id
                );
            }
        }

        UpdateAction::ToggleOverlay {
            session_id,
            extension,
            vm_handle,
        } => {
            if let Some(handle) = vm_handle {
                inspector::spawn_toggle_overlay(session_id, extension, handle, msg_tx);
            } else {
                warn!(
                    "ToggleOverlay reached handle_action with no VmRequestHandle \
                     for session {} — skipping",
                    session_id
                );
            }
        }

        // ─────────────────────────────────────────────────────────
        // DevTools Group Disposal (Phase 4, Task 07)
        // ─────────────────────────────────────────────────────────
        UpdateAction::DisposeDevToolsGroups {
            session_id,
            vm_handle,
        } => {
            if let Some(handle) = vm_handle {
                inspector::spawn_dispose_devtools_groups(session_id, handle);
            } else {
                tracing::debug!(
                    "DisposeDevToolsGroups reached handle_action with no VmRequestHandle \
                     for session {} — skipping",
                    session_id
                );
            }
        }

        // ─────────────────────────────────────────────────────────
        // DevTools Browser Launch (Phase 4, Task 03)
        // ─────────────────────────────────────────────────────────
        UpdateAction::OpenBrowserDevTools { url, browser } => {
            tokio::spawn(async move {
                if let Err(e) = network::open_url_in_browser(&url, &browser) {
                    tracing::error!("Failed to open browser DevTools: {e}");
                }
            });
        }

        // ─────────────────────────────────────────────────────────
        // Network Monitoring (Phase 4, Task 05)
        // ─────────────────────────────────────────────────────────
        UpdateAction::StartNetworkMonitoring {
            session_id,
            handle,
            poll_interval_ms,
        } => {
            // `handle` is guaranteed to be Some here because process.rs
            // discards actions where it couldn't hydrate the handle.
            if let Some(vm_handle) = handle {
                network::spawn_network_monitoring(session_id, vm_handle, msg_tx, poll_interval_ms);
            } else {
                warn!(
                    "StartNetworkMonitoring reached handle_action with no VmRequestHandle \
                     for session {} — skipping",
                    session_id
                );
            }
        }

        UpdateAction::FetchHttpRequestDetail {
            session_id,
            request_id,
            vm_handle,
        } => {
            if let Some(handle) = vm_handle {
                network::spawn_fetch_http_request_detail(session_id, request_id, handle, msg_tx);
            } else {
                warn!(
                    "FetchHttpRequestDetail reached handle_action with no VmRequestHandle \
                     for session {} — skipping",
                    session_id
                );
            }
        }

        UpdateAction::ClearHttpProfile {
            session_id,
            vm_handle,
        } => {
            if let Some(handle) = vm_handle {
                network::spawn_clear_http_profile(session_id, handle, msg_tx);
            } else {
                tracing::debug!(
                    "ClearHttpProfile for session {} — no VM handle (VM disconnected), skipping",
                    session_id
                );
            }
        }
    }
}
