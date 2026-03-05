//! Action handlers: UpdateAction dispatch and background task spawning

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};

use tokio::sync::{mpsc, watch};
use tracing::warn;

use crate::handler::Task;
use crate::message::Message;
use crate::session::SessionId;
use crate::UpdateAction;
use fdemon_daemon::{vm_service::VmRequestHandle, CommandSender, ToolAvailability};
use fdemon_dap::{DapServerEvent, DapServerHandle, DapService};

use super::spawn;

pub(super) mod session;

pub(super) mod inspector;
pub(super) mod network;
pub(super) mod performance;
pub(super) mod vm_service;

/// Convenience type alias for session task tracking
pub type SessionTaskMap = Arc<std::sync::Mutex<HashMap<SessionId, tokio::task::JoinHandle<()>>>>;

/// Convenience type alias for the shared DAP server handle slot.
///
/// The Engine stores the running `DapServerHandle` here so that
/// `handle_action` can deposit it (on `SpawnDapServer`) or withdraw it
/// (on `StopDapServer`) without taking ownership of the Engine.
pub type DapHandleSlot = Arc<Mutex<Option<DapServerHandle>>>;

/// Channel capacity for DAP server events (connect/disconnect/error notifications).
const DAP_EVENT_CHANNEL_CAPACITY: usize = 32;

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
    dap_server_handle: DapHandleSlot,
    vm_handle_for_dap: Arc<Mutex<Option<VmRequestHandle>>>,
) {
    match action {
        UpdateAction::SpawnTask(task) => {
            // Spawn async task for command execution using session-specific sender
            tokio::spawn(async move {
                session::execute_task(task, msg_tx, session_cmd_sender).await;
            });
        }

        UpdateAction::ReloadAllSessions { sessions: _ } => {
            // Spawn reload tasks for each session
            for (session_id, app_id, sender) in session_senders {
                let msg_tx_clone = msg_tx.clone();
                let task = Task::Reload { session_id, app_id };
                tokio::spawn(async move {
                    session::execute_task(task, msg_tx_clone, Some(sender)).await;
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
            match session_tasks.lock() {
                Ok(mut guard) => {
                    guard.insert(session_id, handle);
                }
                Err(e) => {
                    warn!(
                        "ConnectVmService: could not track VM task for session {} \
                         (poisoned lock): {}",
                        session_id, e
                    );
                }
            }
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
                network::spawn_clear_http_profile(session_id, handle);
            } else {
                tracing::debug!(
                    "ClearHttpProfile for session {} — no VM handle (VM disconnected), skipping",
                    session_id
                );
            }
        }

        // ─────────────────────────────────────────────────────────────────────
        // Debug RPC Actions (DAP Server Phase 1, Task 05)
        //
        // These variants are defined now to satisfy the exhaustive match but are
        // not dispatched to async executors until Phase 2 (DAP server wiring).
        // Reaching these arms in the current build is unexpected; log at warn.
        // ─────────────────────────────────────────────────────────────────────
        UpdateAction::PauseIsolate {
            session_id,
            vm_handle: _,
            isolate_id: _,
        } => {
            tracing::warn!(
                "PauseIsolate action for session {} — DAP executor not yet wired (Phase 2)",
                session_id
            );
        }

        UpdateAction::ResumeIsolate {
            session_id,
            vm_handle: _,
            isolate_id: _,
            step: _,
        } => {
            tracing::warn!(
                "ResumeIsolate action for session {} — DAP executor not yet wired (Phase 2)",
                session_id
            );
        }

        UpdateAction::AddBreakpoint {
            session_id,
            vm_handle: _,
            isolate_id: _,
            script_uri: _,
            line: _,
            column: _,
        } => {
            tracing::warn!(
                "AddBreakpoint action for session {} — DAP executor not yet wired (Phase 2)",
                session_id
            );
        }

        UpdateAction::RemoveBreakpoint {
            session_id,
            vm_handle: _,
            isolate_id: _,
            breakpoint_id: _,
        } => {
            tracing::warn!(
                "RemoveBreakpoint action for session {} — DAP executor not yet wired (Phase 2)",
                session_id
            );
        }

        UpdateAction::SetIsolatePauseMode {
            session_id,
            vm_handle: _,
            isolate_id: _,
            mode: _,
        } => {
            tracing::warn!(
                "SetIsolatePauseMode action for session {} — DAP executor not yet wired (Phase 2)",
                session_id
            );
        }

        // ─────────────────────────────────────────────────────────────────────
        // DAP Server Actions (DAP Server Phase 2, Task 05)
        // ─────────────────────────────────────────────────────────────────────
        UpdateAction::SpawnDapServer { port, bind_addr } => {
            let msg_tx_clone = msg_tx.clone();
            let handle_slot = dap_server_handle.clone();
            // Construct a factory from the current VM handle slot so each
            // accepted DAP client gets a real backend when a Flutter session
            // is attached.
            let factory = Arc::new(crate::handler::dap_backend::VmBackendFactory::new(
                vm_handle_for_dap,
            ));
            tokio::spawn(async move {
                // Create the event channel: DapServerEvent → Message bridge
                let (event_tx, mut event_rx) =
                    tokio::sync::mpsc::channel::<DapServerEvent>(DAP_EVENT_CHANNEL_CAPACITY);

                // Keep a copy of bind_addr for logging after the move below.
                let bind_addr_log = bind_addr.clone();

                // Start the TCP server with the backend factory.
                match DapService::start_tcp_with_factory(port, bind_addr, event_tx, factory).await {
                    Ok(server_handle) => {
                        let actual_port = server_handle.port();

                        // Deposit the handle into the shared slot so Engine::shutdown()
                        // can stop it, and StopDapServer can retrieve it.
                        match handle_slot.lock() {
                            Ok(mut guard) => {
                                *guard = Some(server_handle);
                            }
                            Err(e) => {
                                warn!("DAP handle slot poisoned after start: {}", e);
                            }
                        }

                        // Notify the TEA loop that the server is up
                        let _ = msg_tx_clone
                            .send(Message::DapServerStarted { port: actual_port })
                            .await;

                        // Log DAP connection info so IDE users can find the port.
                        // In TUI mode the port is shown in the status bar;
                        // in headless mode the tracing subscriber forwards to
                        // stderr, making this visible in the terminal.
                        tracing::info!(
                            port = actual_port,
                            bind_addr = %bind_addr_log,
                            "DAP server listening on {}:{}",
                            bind_addr_log, actual_port
                        );
                        tracing::info!(
                            "Connect with: Zed (port {} in .zed/debug.json), \
                             Helix (:debug-remote {}:{}), nvim (port {} in dap.adapters)",
                            actual_port,
                            bind_addr_log,
                            actual_port,
                            actual_port
                        );

                        // Bridge DapServerEvent → Message
                        // Runs until the server stops (event_rx closes) or Engine channel drops.
                        while let Some(event) = event_rx.recv().await {
                            let msg = match event {
                                DapServerEvent::ClientConnected { client_id } => {
                                    Message::DapClientConnected { client_id }
                                }
                                DapServerEvent::ClientDisconnected { client_id } => {
                                    Message::DapClientDisconnected { client_id }
                                }
                                DapServerEvent::ServerError { reason } => {
                                    Message::DapServerFailed { reason }
                                }
                                // Debug session lifecycle events — logged but not yet
                                // mapped to specific Message variants. The DapStatus
                                // already tracks connected clients; these events provide
                                // finer-grained state for future UI indicators.
                                DapServerEvent::DebugSessionStarted { client_id } => {
                                    tracing::info!("DAP debug session started: {}", client_id);
                                    continue;
                                }
                                DapServerEvent::DebugSessionEnded { client_id } => {
                                    tracing::info!("DAP debug session ended: {}", client_id);
                                    continue;
                                }
                            };
                            if msg_tx_clone.send(msg).await.is_err() {
                                // Engine channel closed — Engine is shutting down.
                                break;
                            }
                        }
                    }
                    Err(e) => {
                        // Bind failed — report back to TEA loop
                        let _ = msg_tx_clone
                            .send(Message::DapServerFailed {
                                reason: e.to_string(),
                            })
                            .await;
                    }
                }
            });
        }

        UpdateAction::StopDapServer => {
            let handle_slot = dap_server_handle.clone();
            let msg_tx_clone = msg_tx.clone();
            tokio::spawn(async move {
                let maybe_handle = match handle_slot.lock() {
                    Ok(mut guard) => guard.take(),
                    Err(e) => {
                        warn!("DAP handle slot poisoned on StopDapServer: {}", e);
                        None
                    }
                };
                if let Some(handle) = maybe_handle {
                    DapService::stop(handle).await;
                    let _ = msg_tx_clone.send(Message::DapServerStopped).await;
                } else {
                    tracing::debug!("StopDapServer: no running DAP server to stop");
                }
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_task_map_default_is_empty() {
        let map: SessionTaskMap = Arc::new(std::sync::Mutex::new(HashMap::new()));
        assert!(map.lock().unwrap().is_empty());
    }
}
