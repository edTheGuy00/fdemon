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
pub(super) mod native_logs;
pub(super) mod network;
pub(super) mod performance;
pub(super) mod ready_check;
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
    dap_debug_senders: Arc<Mutex<Vec<tokio::sync::mpsc::Sender<fdemon_dap::adapter::DebugEvent>>>>,
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

        UpdateAction::DiscoverDevices { flutter } => {
            spawn::spawn_device_discovery(msg_tx, flutter);
        }

        UpdateAction::RefreshDevicesBackground { flutter } => {
            // Same as DiscoverDevices but errors are logged only (no UI feedback)
            // This runs when we already have cached devices displayed
            spawn::spawn_device_discovery_background(msg_tx, flutter);
        }

        UpdateAction::DiscoverDevicesAndAutoLaunch { configs, flutter } => {
            spawn::spawn_auto_launch(msg_tx, configs, project_path.to_path_buf(), flutter);
        }

        UpdateAction::SpawnSession {
            session_id,
            device,
            config,
            flutter,
        } => {
            session::spawn_session(
                session_id,
                device,
                config,
                flutter,
                project_path,
                msg_tx,
                session_tasks,
                shutdown_rx,
            );
        }

        UpdateAction::DiscoverEmulators { flutter } => {
            spawn::spawn_emulator_discovery(msg_tx, flutter);
        }

        UpdateAction::LaunchEmulator {
            emulator_id,
            flutter,
        } => {
            spawn::spawn_emulator_launch(msg_tx, emulator_id, flutter);
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
            // is attached. Pass `msg_tx_clone` so that `hotReload`/`hotRestart`
            // custom DAP requests can dispatch through the TEA pipeline
            // (Phase 4, Task 02).
            let factory = Arc::new(crate::handler::dap_backend::VmBackendFactory::new(
                vm_handle_for_dap,
                dap_debug_senders,
                Some(msg_tx_clone.clone()),
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

        // ─────────────────────────────────────────────────────────────────────
        // DAP Debug Event Forwarding (DAP Server Phase 4, Task 03)
        //
        // Forwards translated VM debug events to all connected DAP client
        // adapters.  Runs outside the synchronous TEA `update()` cycle so
        // that the blocking `std::sync::Mutex` lock and `try_send` calls do
        // not stall the main loop (TEA purity).
        //
        // Stale senders (receivers dropped by disconnected clients) are pruned
        // automatically via `retain` + `try_send` returning `Err(Closed)`.
        // A full channel (`Err(Full)`) logs at `warn!` level and retains the
        // sender — a full backlog suggests the client is misbehaving but may
        // recover.
        // ─────────────────────────────────────────────────────────────────────
        UpdateAction::ForwardDapDebugEvents(events) => {
            match dap_debug_senders.lock() {
                Ok(mut senders) => {
                    for ev in &events {
                        senders.retain(|tx| {
                            match tx.try_send(ev.clone()) {
                                Ok(()) => true,
                                Err(tokio::sync::mpsc::error::TrySendError::Full(_)) => {
                                    warn!(
                                        "DAP debug event channel full — event dropped, \
                                         IDE may desync"
                                    );
                                    true // retain: client may recover
                                }
                                Err(tokio::sync::mpsc::error::TrySendError::Closed(_)) => {
                                    false // prune: client disconnected
                                }
                            }
                        });
                    }
                }
                Err(e) => {
                    warn!("dap_debug_senders lock poisoned: {}", e);
                }
            }
        }

        // ─────────────────────────────────────────────────────────────────────
        // Native Platform Log Capture (Phase 1, Task 07)
        // ─────────────────────────────────────────────────────────────────────
        UpdateAction::StartNativeLogCapture {
            session_id,
            platform,
            device_id,
            device_name,
            app_id,
            settings,
            project_path,
            running_source_names,
            running_shared_names,
        } => {
            native_logs::spawn_native_log_capture(
                session_id,
                platform,
                device_id,
                device_name,
                app_id,
                &settings,
                project_path,
                msg_tx.clone(),
                running_source_names,
                running_shared_names,
            );
        }

        // ─────────────────────────────────────────────────────────────────────
        // Pre-App Custom Sources (pre-app-custom-sources Phase 1, Task 06)
        // ─────────────────────────────────────────────────────────────────────
        UpdateAction::SpawnPreAppSources {
            session_id,
            device,
            config,
            settings,
            project_path,
            running_shared_names,
        } => {
            native_logs::spawn_pre_app_sources(
                session_id,
                device,
                config,
                &settings,
                &project_path,
                &msg_tx,
                &running_shared_names,
            );
        }

        // ─────────────────────────────────────────────────────────────────────
        // IDE Config Generation (DAP Server Phase 5, Task 02)
        //
        // Dispatches IDE-specific DAP config generation (launch.json,
        // languages.toml, etc.) in an async task so the TEA loop is not
        // blocked by file I/O.  Per-IDE generator implementations are added
        // incrementally in Tasks 04–08; until then generate_ide_config()
        // returns Ok(None) for all IDEs.
        // ─────────────────────────────────────────────────────────────────────
        UpdateAction::GenerateIdeConfig { port, ide_override } => {
            let project_path = project_path.to_path_buf();
            let msg_tx_clone = msg_tx.clone();
            tokio::spawn(async move {
                // Use the CLI-specified IDE override when provided.  Otherwise
                // detect the parent IDE from the environment (process-name
                // heuristic). We don't carry Settings through UpdateAction to
                // keep the action payload small.
                let ide = ide_override.or_else(crate::config::settings::detect_parent_ide);

                match crate::ide_config::generate_ide_config(ide, port, &project_path) {
                    Ok(Some(result)) => {
                        let action_str = match &result.action {
                            crate::ide_config::ConfigAction::Created => "Created".to_string(),
                            crate::ide_config::ConfigAction::Updated => "Updated".to_string(),
                            crate::ide_config::ConfigAction::Skipped(reason) => {
                                format!("Skipped: {}", reason)
                            }
                        };
                        let ide_name = ide
                            .map(|i| i.display_name().to_string())
                            .unwrap_or_else(|| "Unknown".to_string());
                        let _ = msg_tx_clone
                            .send(Message::DapConfigGenerated {
                                ide_name,
                                path: result.path,
                                action: action_str,
                            })
                            .await;
                    }
                    Ok(None) => {
                        // No IDE detected or IDE doesn't support DAP config.
                        tracing::debug!(
                            "No IDE config generated (no IDE detected or IDE unsupported)"
                        );
                    }
                    Err(e) => {
                        tracing::warn!("Failed to generate IDE DAP config: {}", e);
                    }
                }
            });
        }

        // ── Flutter Version Panel ─────────────────────────────────────────────
        UpdateAction::ScanInstalledSdks { active_sdk_root } => {
            let msg_tx = msg_tx.clone();
            tokio::spawn(async move {
                let result = tokio::task::spawn_blocking(move || {
                    fdemon_daemon::flutter_sdk::scan_installed_versions(active_sdk_root.as_deref())
                })
                .await;

                match result {
                    Ok(versions) => {
                        let _ = msg_tx
                            .send(Message::FlutterVersionScanCompleted { versions })
                            .await;
                    }
                    Err(e) => {
                        let _ = msg_tx
                            .send(Message::FlutterVersionScanFailed {
                                reason: format!("Cache scan failed: {e}"),
                            })
                            .await;
                    }
                }
            });
        }

        UpdateAction::SwitchFlutterVersion {
            version,
            sdk_path: _,
            project_path,
            explicit_sdk_path,
        } => {
            let msg_tx = msg_tx.clone();
            // Clone version before it is moved into the blocking closure so
            // it is still available for the `FlutterVersionSwitchCompleted`
            // message sent after the closure returns.
            let version_for_msg = version.clone();
            tokio::spawn(async move {
                let result = tokio::task::spawn_blocking(move || {
                    switch_flutter_version(&version, &project_path, explicit_sdk_path.as_deref())
                })
                .await;

                match result {
                    Ok(Ok(sdk)) => {
                        // Update global SDK state first so handle_switch_completed
                        // sees the updated resolved_sdk when it refreshes the panel.
                        let _ = msg_tx.send(Message::SdkResolved { sdk }).await;
                        let _ = msg_tx
                            .send(Message::FlutterVersionSwitchCompleted {
                                version: version_for_msg,
                            })
                            .await;
                    }
                    Ok(Err(e)) => {
                        let _ = msg_tx
                            .send(Message::FlutterVersionSwitchFailed {
                                reason: format!("{e}"),
                            })
                            .await;
                    }
                    Err(e) => {
                        let _ = msg_tx
                            .send(Message::FlutterVersionSwitchFailed {
                                reason: format!("Task failed: {e}"),
                            })
                            .await;
                    }
                }
            });
        }

        UpdateAction::ProbeFlutterVersion { executable } => {
            if let Some(executable) = executable {
                let tx = msg_tx.clone();
                tokio::spawn(async move {
                    let result =
                        fdemon_daemon::flutter_sdk::probe_flutter_version(&executable).await;
                    let _ = tx
                        .send(Message::FlutterVersionProbeCompleted {
                            result: result.map_err(|e| e.to_string()),
                        })
                        .await;
                });
            } else {
                tracing::debug!("ProbeFlutterVersion: no resolved SDK executable — skipping probe");
            }
        }

        UpdateAction::RemoveFlutterVersion {
            version,
            path,
            active_sdk_root: _,
        } => {
            let msg_tx = msg_tx.clone();
            tokio::spawn(async move {
                let result =
                    tokio::task::spawn_blocking(move || remove_flutter_version_path(&path)).await;

                match result {
                    Ok(Ok(())) => {
                        let _ = msg_tx
                            .send(Message::FlutterVersionRemoveCompleted {
                                version: version.clone(),
                            })
                            .await;
                    }
                    Ok(Err(e)) => {
                        let _ = msg_tx
                            .send(Message::FlutterVersionRemoveFailed {
                                reason: format!("{e}"),
                            })
                            .await;
                    }
                    Err(e) => {
                        let _ = msg_tx
                            .send(Message::FlutterVersionRemoveFailed {
                                reason: format!("Task failed: {e}"),
                            })
                            .await;
                    }
                }
            });
        }
    }
}

/// Removes a Flutter SDK version directory after verifying it is inside the FVM cache.
///
/// Uses [`fdemon_daemon::flutter_sdk::resolve_fvm_cache_path()`] to determine the
/// canonical FVM cache root (respecting `FVM_CACHE_PATH` env var), then checks that
/// `path` is a descendant of that root before calling `std::fs::remove_dir_all`.
///
/// # Errors
///
/// Returns a config error if:
/// - The FVM cache directory cannot be found (neither `FVM_CACHE_PATH` nor `~/fvm/versions/`
///   exists).
/// - `path` does not start with the resolved FVM cache root.
/// - The directory removal fails.
fn remove_flutter_version_path(path: &std::path::Path) -> fdemon_core::Result<()> {
    // Safety: refuse to remove paths outside the FVM versions cache.
    // This is a defense-in-depth measure beyond the handler's is_active guard.
    // Use resolve_fvm_cache_path() so that FVM_CACHE_PATH env var is respected,
    // matching the same logic the cache scanner uses when discovering versions.
    let fvm_cache = fdemon_daemon::flutter_sdk::resolve_fvm_cache_path().ok_or_else(|| {
        fdemon_core::Error::config(
            "FVM cache directory not found; cannot safely remove version".to_string(),
        )
    })?;
    if !path.starts_with(&fvm_cache) {
        return Err(fdemon_core::Error::config(format!(
            "Refusing to remove path outside FVM cache: {}",
            path.display()
        )));
    }
    std::fs::remove_dir_all(path).map_err(|e| {
        fdemon_core::Error::config(format!("Failed to remove {}: {e}", path.display()))
    })
}

/// Write `.fvmrc` in the project root and re-resolve the Flutter SDK.
///
/// This function performs a **read-merge-write** on `.fvmrc` so that only the
/// `"flutter"` field is updated; all other FVM v3 fields (e.g. `"flavors"`,
/// `"runPubGetOnSdkChanges"`, `"updateVscodeSettings"`) are preserved.
///
/// Behaviour for edge cases:
/// - **File missing**: Creates a new file with `{"flutter": "<version>"}`.
/// - **File exists with extra fields**: Updates only `"flutter"`; other fields
///   are preserved verbatim.
/// - **File is not valid JSON** or is a non-object value (array, string, …):
///   Resets to a clean object containing only `"flutter"`.
/// - **Read error** (e.g. permission denied): Falls back to creating a fresh
///   file (same as "missing").
///
/// After writing, `find_flutter_sdk` is called so that the FVM detector picks
/// up the newly written file and returns an updated `FlutterSdk`.
fn switch_flutter_version(
    version: &str,
    project_path: &std::path::Path,
    explicit_sdk_path: Option<&std::path::Path>,
) -> fdemon_core::Result<fdemon_daemon::FlutterSdk> {
    // 1. Write .fvmrc in project root using a read-merge-write pattern so that
    //    existing FVM configuration fields are not destroyed.
    let fvmrc_path = project_path.join(".fvmrc");

    // Read and parse existing file, or start with an empty JSON object.
    let mut json: serde_json::Value = std::fs::read_to_string(&fvmrc_path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_else(|| serde_json::Value::Object(serde_json::Map::new()));

    // If the existing file was not a JSON object (e.g. corrupted or a bare
    // array/string), reset to an empty object rather than crashing.
    if !json.is_object() {
        json = serde_json::Value::Object(serde_json::Map::new());
    }

    // Set only the flutter field; all other fields are preserved.
    json["flutter"] = serde_json::Value::String(version.to_string());

    let fvmrc_content = serde_json::to_string_pretty(&json)
        .map_err(|e| fdemon_core::Error::config(format!("Failed to serialize .fvmrc: {e}")))?;

    std::fs::write(&fvmrc_path, &fvmrc_content).map_err(|e| {
        fdemon_core::Error::config(format!("Failed to write {}: {e}", fvmrc_path.display()))
    })?;

    tracing::info!("Wrote .fvmrc: {}", fvmrc_content);

    // 2. Re-resolve SDK — the FVM detector now picks up the new .fvmrc
    let sdk = fdemon_daemon::flutter_sdk::find_flutter_sdk(project_path, explicit_sdk_path)?;

    tracing::info!(
        "SDK re-resolved after version switch: {} via {}",
        sdk.version,
        sdk.source
    );
    Ok(sdk)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_task_map_default_is_empty() {
        let map: SessionTaskMap = Arc::new(std::sync::Mutex::new(HashMap::new()));
        assert!(map.lock().unwrap().is_empty());
    }

    #[test]
    fn test_remove_rejects_path_outside_fvm_cache() {
        // A path that is clearly outside any FVM cache directory should be rejected.
        // The function either returns "outside FVM cache" (when a cache dir is found but
        // the path isn't under it) or "not found" (when no FVM cache dir exists at all).
        let result =
            remove_flutter_version_path(std::path::Path::new("/definitely-not-fvm/some-sdk"));
        assert!(result.is_err());
        let err = result.unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("outside FVM cache") || msg.contains("not found"),
            "unexpected error message: {msg}"
        );
    }

    // ── write_fvmrc merge tests ───────────────────────────────────────────────

    /// Helper: call only the .fvmrc write portion of switch_flutter_version,
    /// without attempting to resolve the Flutter SDK (which requires a real
    /// Flutter installation).  This replicates the merge logic verbatim so
    /// tests remain isolated from the file system toolchain.
    fn write_fvmrc_version(
        project_path: &std::path::Path,
        version: &str,
    ) -> fdemon_core::Result<()> {
        let fvmrc_path = project_path.join(".fvmrc");

        let mut json: serde_json::Value = std::fs::read_to_string(&fvmrc_path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_else(|| serde_json::Value::Object(serde_json::Map::new()));

        if !json.is_object() {
            json = serde_json::Value::Object(serde_json::Map::new());
        }

        json["flutter"] = serde_json::Value::String(version.to_string());

        let fvmrc_content = serde_json::to_string_pretty(&json)
            .map_err(|e| fdemon_core::Error::config(format!("Failed to serialize .fvmrc: {e}")))?;

        std::fs::write(&fvmrc_path, &fvmrc_content).map_err(|e| {
            fdemon_core::Error::config(format!("Failed to write {}: {e}", fvmrc_path.display()))
        })
    }

    #[test]
    fn test_switch_version_preserves_fvmrc_fields() {
        let dir = tempfile::tempdir().unwrap();
        let fvmrc = dir.path().join(".fvmrc");

        // Write initial .fvmrc with extra fields
        std::fs::write(
            &fvmrc,
            r#"{"flutter": "3.19.0", "flavors": {"dev": "3.19.0"}, "runPubGetOnSdkChanges": true}"#,
        )
        .unwrap();

        write_fvmrc_version(dir.path(), "3.22.0").unwrap();

        let content: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&fvmrc).unwrap()).unwrap();
        assert_eq!(content["flutter"], "3.22.0");
        assert_eq!(content["flavors"]["dev"], "3.19.0"); // preserved
        assert_eq!(content["runPubGetOnSdkChanges"], true); // preserved
    }

    #[test]
    fn test_switch_version_creates_fvmrc_when_missing() {
        let dir = tempfile::tempdir().unwrap();
        let fvmrc = dir.path().join(".fvmrc");
        assert!(!fvmrc.exists());

        write_fvmrc_version(dir.path(), "3.22.0").unwrap();

        let content: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&fvmrc).unwrap()).unwrap();
        assert_eq!(content["flutter"], "3.22.0");
    }

    #[test]
    fn test_switch_version_handles_corrupted_fvmrc() {
        let dir = tempfile::tempdir().unwrap();
        let fvmrc = dir.path().join(".fvmrc");
        std::fs::write(&fvmrc, "not json at all").unwrap();

        write_fvmrc_version(dir.path(), "3.22.0").unwrap();

        let content: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&fvmrc).unwrap()).unwrap();
        assert_eq!(content["flutter"], "3.22.0");
    }

    #[test]
    fn test_switch_version_handles_non_object_fvmrc() {
        let dir = tempfile::tempdir().unwrap();
        let fvmrc = dir.path().join(".fvmrc");
        // A valid JSON value that is not an object (array)
        std::fs::write(&fvmrc, r#"["3.19.0", "3.22.0"]"#).unwrap();

        write_fvmrc_version(dir.path(), "3.24.0").unwrap();

        let content: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&fvmrc).unwrap()).unwrap();
        assert_eq!(content["flutter"], "3.24.0");
        // Result should be a plain object, not an array
        assert!(content.is_object());
    }

    #[test]
    fn test_switch_version_fvmrc_is_pretty_printed() {
        let dir = tempfile::tempdir().unwrap();

        write_fvmrc_version(dir.path(), "3.22.0").unwrap();

        let raw = std::fs::read_to_string(dir.path().join(".fvmrc")).unwrap();
        // Pretty-printed JSON contains newlines
        assert!(
            raw.contains('\n'),
            "expected pretty-printed JSON, got: {raw}"
        );
    }
}
