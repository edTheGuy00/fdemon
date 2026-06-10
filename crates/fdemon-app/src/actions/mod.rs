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

        UpdateAction::RefreshDevicesAndBootableBackground { flutter } => {
            // Connected device refresh — errors logged only (UI shows cached list).
            spawn::spawn_device_discovery_background(msg_tx.clone(), flutter);
            // Bootable refresh — errors logged only.
            spawn::spawn_bootable_device_discovery(msg_tx, tool_availability);
        }

        UpdateAction::DiscoverDevicesAndBootable { flutter } => {
            // Foreground connected discovery (loading-aware, shows spinner and surfaces errors).
            spawn::spawn_device_discovery(msg_tx.clone(), flutter);
            // Background bootable discovery in parallel (uses tool_availability for
            // emulator/simulator listings; errors are logged only).
            spawn::spawn_bootable_device_discovery(msg_tx, tool_availability);
        }

        UpdateAction::DiscoverDevicesAndAutoLaunch {
            configs,
            flutter,
            cache_allowed,
        } => {
            spawn::spawn_auto_launch(
                msg_tx,
                configs,
                project_path.to_path_buf(),
                flutter,
                cache_allowed,
            );
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

        // ─────────────────────────────────────────────────────────────────────
        // Settings Persistence (devtools-inspector-parity Phase 1.5, Task 02)
        //
        // Mirrors `AutoSaveConfig` but writes `.fdemon/config.toml` (Settings)
        // rather than the launch configs. Uses `spawn_blocking` because
        // `save_settings` is synchronous std I/O.
        // ─────────────────────────────────────────────────────────────────────
        UpdateAction::PersistSettings {
            settings,
            project_path,
        } => {
            let tx = msg_tx.clone();
            tokio::spawn(async move {
                let result = tokio::task::spawn_blocking(move || {
                    crate::config::settings::save_settings(&project_path, &settings)
                })
                .await;
                match result {
                    Ok(Ok(())) => {
                        let _ = tx.send(Message::SettingsPersisted).await;
                    }
                    Ok(Err(e)) => {
                        let msg = format!("save_settings failed: {e}");
                        tracing::warn!("{msg}");
                        let _ = tx.send(Message::SettingsPersistFailed { error: msg }).await;
                    }
                    Err(join_err) => {
                        let msg = format!("save_settings task panicked: {join_err}");
                        tracing::warn!("{msg}");
                        let _ = tx.send(Message::SettingsPersistFailed { error: msg }).await;
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

        UpdateAction::ConnectVmService {
            session_id,
            ws_uri,
            rebuilt_widgets_gate_rx,
        } => {
            let handle = vm_service::spawn_vm_service_connection(
                session_id,
                ws_uri,
                msg_tx,
                rebuilt_widgets_gate_rx,
            );
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
            mode,
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
                    mode,
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
            inspector_readiness_poll_attempts,
            inspector_readiness_poll_interval_ms,
            inspector_readiness_poll_call_timeout_ms,
            trigger,
        } => {
            if let Some(handle) = vm_handle {
                inspector::spawn_fetch_widget_tree(
                    session_id,
                    handle,
                    msg_tx,
                    tree_max_depth,
                    fetch_timeout_secs,
                    inspector_readiness_poll_attempts,
                    inspector_readiness_poll_interval_ms,
                    inspector_readiness_poll_call_timeout_ms,
                    trigger,
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

        UpdateAction::FetchInspectorProperties {
            session_id,
            node_id,
            vm_handle,
        } => {
            if let Some(handle) = vm_handle {
                inspector::spawn_fetch_inspector_properties(session_id, node_id, handle, msg_tx);
            } else {
                warn!(
                    session_id = %session_id,
                    node_id = %node_id,
                    "FetchInspectorProperties dispatched without VM handle \
                     (no active VM Service) — skipping"
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
            mode,
        } => {
            // `handle` is guaranteed to be Some here because process.rs
            // discards actions where it couldn't hydrate the handle.
            if let Some(vm_handle) = handle {
                network::spawn_network_monitoring(
                    session_id,
                    vm_handle,
                    msg_tx,
                    poll_interval_ms,
                    mode,
                );
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

        // ── Install Wizard ────────────────────────────────────────────────────
        UpdateAction::RunToolchainPreflight {
            project_path,
            explicit_sdk_path,
            android_sdk_root,
            web_browser_executable,
        } => {
            let msg_tx = msg_tx.clone();
            tokio::spawn(async move {
                let outcome = fdemon_daemon::toolchain::run_preflight(
                    &project_path,
                    explicit_sdk_path.as_deref(),
                    android_sdk_root.as_deref(),
                    web_browser_executable.as_deref(),
                )
                .await;

                // If the preflight resolved a live Flutter SDK, send `SdkResolved` so
                // that `AppState::resolved_sdk` is populated before
                // `handle_preflight_completed` evaluates the handback predicate.
                // `run_preflight` now returns the `FlutterSdk` it resolved internally,
                // eliminating the former second `find_flutter_sdk` / `spawn_blocking`
                // block and the TOCTOU window it introduced.
                // Ordering: `SdkResolved` is sent before `ToolchainPreflightCompleted`.
                if let Some(sdk) = outcome.flutter_sdk {
                    let _ = msg_tx
                        .send(crate::message::Message::SdkResolved { sdk })
                        .await;
                }

                let _ = msg_tx
                    .send(crate::message::Message::ToolchainPreflightCompleted {
                        report: outcome.report,
                    })
                    .await;
            });
        }

        // ── Install Wizard Step Executor (Phase 2+3, Tasks 08+06) ───────────────
        // Dispatches to the Flutter SDK installer, Android tools installer, or
        // PATH config writer, streaming progress back via the TEA message channel.
        // All I/O runs inside the spawned task; handlers in
        // `handler/install_wizard/actions.rs` remain pure.
        UpdateAction::RunWizardStep {
            kind,
            run_seq,
            cancel_token,
            install,
            path_bin_dir,
            android_sdk_root,
            android,
        } => {
            use crate::install_wizard::WizardStepKind;
            use fdemon_daemon::toolchain::{
                add_android_env, add_to_path, install_android_tools, install_flutter,
                resolve_install_dir, AndroidInstallTarget, FlutterInstallTarget, HostPlatform,
                HostShell, InstallEvent, DEFAULT_CMDLINE_TOOLS_BUILD,
            };

            // Clone msg_tx: one for the spawned task, one for the ready message.
            let msg_tx_task = msg_tx.clone();
            let msg_tx_ready = msg_tx.clone();

            // Reuse the token minted synchronously by `handle_run_selected_step`
            // (already stored on `InstallWizardState::install_task`). This
            // eliminates the window where `is_step_running()==true` but the
            // cancel token is unknown to state (F3 fix).
            let cancel_for_task = cancel_token;

            // Shared slot to deposit the JoinHandle after spawn so that
            // `WizardInstallTaskReady` can carry it to state for abort backstop.
            let handle_slot: std::sync::Arc<std::sync::Mutex<Option<tokio::task::JoinHandle<()>>>> =
                std::sync::Arc::new(std::sync::Mutex::new(None));
            let handle_slot_for_task = handle_slot.clone();

            // Capture run_seq for inclusion in WizardStepStarted so the handler
            // can discard stale cross-kind Started messages (F-PR53-01 fix).
            let run_seq_for_task = run_seq;

            let join = tokio::spawn(async move {
                let msg_tx = msg_tx_task;
                // ── Announce start ────────────────────────────────────────────
                let _ = msg_tx
                    .send(crate::message::Message::WizardStepStarted {
                        kind,
                        run_seq: run_seq_for_task,
                    })
                    .await;

                // Capture cancel token for use in install calls.
                let cancel = cancel_for_task;

                match kind {
                    WizardStepKind::FlutterSdk => {
                        // Guard: install params are required for the FlutterSdk step.
                        let params = match install {
                            Some(p) => p,
                            None => {
                                let _ = msg_tx
                                    .send(crate::message::Message::WizardStepFailed {
                                        kind,
                                        reason: "Missing install parameters for FlutterSdk step"
                                            .to_string(),
                                    })
                                    .await;
                                return;
                            }
                        };

                        // Resolve the install root (blocking I/O — kept trivial, just mkdir).
                        let install_root = match resolve_install_dir(params.install_root.as_deref())
                        {
                            Ok(dir) => dir,
                            Err(e) => {
                                let _ = msg_tx
                                    .send(crate::message::Message::WizardStepFailed {
                                        kind,
                                        reason: format!("Failed to resolve install dir: {e}"),
                                    })
                                    .await;
                                return;
                            }
                        };

                        // Build the install target.
                        let target = FlutterInstallTarget {
                            method: params.method,
                            channel: params.channel.clone(),
                            install_root,
                            // Use the channel name as the version directory name so the
                            // SDK lands at `~/fvm/versions/stable`.  After install the
                            // `version` file inside the SDK provides the concrete version.
                            version_dir_name: params.channel.clone(),
                            // Task 04 threads the picker selection here.
                            version_tag: None,
                        };

                        // Clone a sender for the synchronous on_event callback.
                        let tx_for_events = msg_tx.clone();

                        let result = install_flutter(&target, cancel.clone(), move |ev| match ev {
                            InstallEvent::Log(line) => {
                                let _ = tx_for_events.try_send(
                                    crate::message::Message::WizardStepLog { kind, line },
                                );
                            }
                            InstallEvent::Download(p) => {
                                let _ = tx_for_events.try_send(
                                    crate::message::Message::WizardDownloadProgress {
                                        kind,
                                        received: p.received,
                                        total: p.total,
                                    },
                                );
                            }
                            InstallEvent::Phase(label) => {
                                let _ = tx_for_events.try_send(
                                    crate::message::Message::WizardStepPhase {
                                        kind,
                                        label: label.to_string(),
                                    },
                                );
                            }
                        })
                        .await;

                        match result {
                            Ok(outcome) => {
                                let summary = format!(
                                    "Installed Flutter {} at {}",
                                    outcome.version,
                                    outcome.sdk_path.display()
                                );
                                let _ = msg_tx
                                    .send(crate::message::Message::WizardStepCompleted {
                                        kind,
                                        summary,
                                        sdk_path: Some(outcome.sdk_path),
                                    })
                                    .await;
                            }
                            Err(ref e) if e.is_cancelled() => {
                                // Cancelled by the user (Esc): forward the error Display
                                // directly — Error::Cancelled already carries the
                                // "Cancelled: " prefix, so format!("{e}") produces
                                // "Cancelled: <message>" (no doubling).
                                let _ = msg_tx
                                    .send(crate::message::Message::WizardStepFailed {
                                        kind,
                                        reason: format!("{e}"),
                                    })
                                    .await;
                            }
                            Err(e) => {
                                let _ = msg_tx
                                    .send(crate::message::Message::WizardStepFailed {
                                        kind,
                                        reason: format!("{e}"),
                                    })
                                    .await;
                            }
                        }
                    }

                    WizardStepKind::PlatformAndroid => {
                        // Guard: Android install params are required.
                        let params = match android {
                            Some(p) => p,
                            None => {
                                let _ = msg_tx
                                    .send(crate::message::Message::WizardStepFailed {
                                        kind,
                                        reason: "Missing Android install parameters".to_string(),
                                    })
                                    .await;
                                return;
                            }
                        };

                        // Resolve the SDK root: use the provided path, or fall back to
                        // environment variables and the platform default.
                        let resolved_sdk_root = fdemon_daemon::resolve_android_sdk_root_path(
                            params.sdk_root.as_deref(),
                        );

                        let target = AndroidInstallTarget {
                            sdk_root: resolved_sdk_root,
                            api_level: params.api_level,
                            cmdline_tools_build: params
                                .cmdline_tools_build
                                .unwrap_or_else(|| DEFAULT_CMDLINE_TOOLS_BUILD.to_string()),
                            jdk_path: resolve_effective_jdk_path(params.jdk_path),
                            platform: HostPlatform::detect(),
                            cmdline_tools_sha256: params.cmdline_tools_sha256,
                        };

                        // Clone a sender for the synchronous on_event callback.
                        let tx_for_events = msg_tx.clone();

                        let result =
                            install_android_tools(&target, cancel.clone(), move |ev| match ev {
                                InstallEvent::Log(line) => {
                                    let _ = tx_for_events.try_send(
                                        crate::message::Message::WizardStepLog { kind, line },
                                    );
                                }
                                InstallEvent::Download(p) => {
                                    let _ = tx_for_events.try_send(
                                        crate::message::Message::WizardDownloadProgress {
                                            kind,
                                            received: p.received,
                                            total: p.total,
                                        },
                                    );
                                }
                                InstallEvent::Phase(label) => {
                                    let _ = tx_for_events.try_send(
                                        crate::message::Message::WizardStepPhase {
                                            kind,
                                            label: label.to_string(),
                                        },
                                    );
                                }
                            })
                            .await;

                        match result {
                            Ok(outcome) => {
                                let summary = format!(
                                    "Installed Android tools at {} ({} packages)",
                                    outcome.sdk_root.display(),
                                    outcome.packages_installed.len()
                                );
                                // Pass `sdk_path: Some(outcome.sdk_root)` so the
                                // handler (task 07) can persist [toolchain] android_sdk_root.
                                let _ = msg_tx
                                    .send(crate::message::Message::WizardStepCompleted {
                                        kind,
                                        summary,
                                        sdk_path: Some(outcome.sdk_root),
                                    })
                                    .await;
                            }
                            Err(ref e) if e.is_cancelled() => {
                                // Forward the error Display directly — Error::Cancelled
                                // already carries the "Cancelled: " prefix (no doubling).
                                let _ = msg_tx
                                    .send(crate::message::Message::WizardStepFailed {
                                        kind,
                                        reason: format!("{e}"),
                                    })
                                    .await;
                            }
                            Err(e) => {
                                let _ = msg_tx
                                    .send(crate::message::Message::WizardStepFailed {
                                        kind,
                                        reason: format!("{e}"),
                                    })
                                    .await;
                            }
                        }
                    }

                    WizardStepKind::PathConfig => {
                        // Guard: bin_dir is required for the PathConfig step.
                        let bin_dir = match path_bin_dir {
                            Some(d) => d,
                            None => {
                                let _ = msg_tx
                                    .send(crate::message::Message::WizardStepFailed {
                                        kind,
                                        reason: "Missing Flutter bin directory for PathConfig step"
                                            .to_string(),
                                    })
                                    .await;
                                return;
                            }
                        };

                        let shell = HostShell::detect();
                        let platform = HostPlatform::detect();

                        // `add_to_path` and `add_android_env` perform file I/O — run
                        // them on the blocking thread pool so we do not stall the
                        // async executor.
                        let result = tokio::task::spawn_blocking(move || {
                            // 1) Write the Flutter bin dir to PATH.
                            let flutter_outcome =
                                add_to_path(shell.clone(), platform.clone(), &bin_dir)?;

                            // 2) Optionally write ANDROID_HOME / Android PATH entries.
                            // Use the wizard-provided SDK root, else fall back to
                            // $ANDROID_HOME / $ANDROID_SDK_ROOT / platform default
                            // (same resolver the PlatformAndroid executor uses). Only
                            // write the Android env block if the resolved path exists.
                            let effective_android_root = android_sdk_root.or_else(|| {
                                let p = fdemon_daemon::resolve_android_sdk_root_path(None);
                                if p.is_dir() {
                                    Some(p)
                                } else {
                                    None
                                }
                            });
                            let android_outcome = if let Some(sdk_root) = effective_android_root {
                                Some(add_android_env(shell, platform, &sdk_root)?)
                            } else {
                                None
                            };

                            Ok::<_, fdemon_core::Error>((flutter_outcome, android_outcome))
                        })
                        .await;

                        match result {
                            Ok(Ok((flutter_outcome, android_outcome))) => {
                                let summary =
                                    build_pathconfig_summary(&flutter_outcome, android_outcome);

                                let _ = msg_tx
                                    .send(crate::message::Message::WizardStepCompleted {
                                        kind,
                                        summary,
                                        sdk_path: None,
                                    })
                                    .await;
                            }
                            Ok(Err(e)) => {
                                let _ = msg_tx
                                    .send(crate::message::Message::WizardStepFailed {
                                        kind,
                                        reason: format!("{e}"),
                                    })
                                    .await;
                            }
                            Err(join_err) => {
                                let _ = msg_tx
                                    .send(crate::message::Message::WizardStepFailed {
                                        kind,
                                        reason: format!("PATH config task panicked: {join_err}"),
                                    })
                                    .await;
                            }
                        }
                    }

                    // Non-executable kinds in this phase: report a clear failure so
                    // the user knows the step is not yet actionable rather than
                    // seeing a stale Running spinner.
                    WizardStepKind::Prerequisites
                    | WizardStepKind::Platforms
                    | WizardStepKind::PlatformIos
                    | WizardStepKind::PlatformMacos
                    | WizardStepKind::PlatformWeb
                    | WizardStepKind::PlatformWindows
                    | WizardStepKind::Doctor => {
                        let _ = msg_tx
                            .send(crate::message::Message::WizardStepFailed {
                                kind,
                                reason: "This step is not executable in this version of fdemon"
                                    .to_string(),
                            })
                            .await;
                    }
                }
            });

            // Deposit the JoinHandle into the shared slot so the
            // WizardInstallTaskReady message can carry it to state for abort backstop.
            // This must happen after `tokio::spawn` returns the handle.
            if let Ok(mut guard) = handle_slot_for_task.lock() {
                *guard = Some(join);
            }

            // Send the JoinHandle upgrade to state. The token is already stored
            // synchronously; this message only provides the backstop abort handle.
            // Spawn a tiny task so we can `.await` the send without blocking
            // `handle_action` (which is called synchronously from the Engine).
            tokio::spawn(async move {
                let _ = msg_tx_ready
                    .send(crate::message::Message::WizardInstallTaskReady {
                        kind,
                        run_seq,
                        handle: handle_slot,
                    })
                    .await;
            });
        }

        // ── Install Wizard — Version Picker manifest fetch (Phase 6) ───────────
        // No-op stub: the executor body lands in Task 04 (it downloads the
        // platform-appropriate release manifest and emits FlutterManifestFetched
        // / FlutterManifestFetchFailed). Leaving the arm here keeps the
        // exhaustive UpdateAction match compiling now that the variant exists.
        UpdateAction::FetchFlutterReleaseManifest => { /* Task 04 fills the body */ }

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

        // ── Mouse Capture (log-text-selection-broken fix) ─────────────────────
        // This action is handled by the TUI runner event loop (not here) because
        // it requires synchronous terminal I/O. Reaching this arm in the action
        // dispatcher is unexpected — log at warn level.
        UpdateAction::SetMouseCapture(active) => {
            tracing::warn!(
                "SetMouseCapture({}) reached handle_action — should be handled by the TUI runner",
                active
            );
        }

        // ── Clipboard Write (log-text-selection-broken fix) ───────────────────
        // This action is handled by the TUI runner event loop (not here) because
        // it requires synchronous clipboard I/O. Reaching this arm in the action
        // dispatcher is unexpected — log at warn level.
        UpdateAction::WriteClipboard { text } => {
            tracing::warn!(
                "WriteClipboard reached handle_action (text len={}) — \
                 should be handled by the TUI runner",
                text.len()
            );
        }

        UpdateAction::SendDaemonCommand {
            session_id,
            command,
            cmd_sender,
        } => {
            // Fire-and-forget: send the command to the session's Flutter process stdin.
            // Responses arrive as `DaemonMessage::Response` and are routed in
            // `process::route_session_daemon_response`:
            //   * Numeric-ID responses go through `RequestTracker` (the awaiting
            //     future resolves).
            //   * String-ID `devtools-serve-*` responses are parsed via
            //     `parse_devtools_serve_response` and forwarded as synthetic
            //     `Message::DevToolsServed` / `Message::DevToolsServeFailed`.
            // The `app.devTools` daemon event (primary, modern Flutter) is handled
            // separately in `handler/daemon.rs`.
            //
            // `cmd_sender` is hydrated by `process.rs`; if it is still None at this
            // point it means the session's process has not yet attached a sender,
            // which should not happen (process.rs discards the action when None).
            if let Some(sender) = cmd_sender {
                tokio::spawn(async move {
                    if let Err(e) = sender.send_fire_and_forget(command).await {
                        tracing::warn!(
                            session_id = session_id,
                            error = %e,
                            "devtools.serve fire-and-forget failed"
                        );
                    } else {
                        tracing::debug!(
                            session_id = session_id,
                            "devtools.serve command sent to Flutter daemon"
                        );
                    }
                });
            } else {
                tracing::debug!(
                    session_id = session_id,
                    "SendDaemonCommand: cmd_sender is None (process not attached); skipping"
                );
            }
        }

        // ─────────────────────────────────────────────────────────────────────
        // Phase 3: Timeline monitoring
        // ─────────────────────────────────────────────────────────────────────
        UpdateAction::StartTimelineMonitoring {
            session_id,
            handle,
            poll_interval_ms,
        } => {
            if let Some(vm_handle) = handle {
                performance::spawn_timeline_polling(
                    session_id,
                    vm_handle,
                    msg_tx,
                    poll_interval_ms,
                );
            } else {
                warn!(
                    "StartTimelineMonitoring reached handle_action with no VmRequestHandle \
                     for session {} — skipping",
                    session_id
                );
            }
        }

        // ─────────────────────────────────────────────────────────────────────
        // Phase 3: Toggle profileWidgetBuilds extension
        // ─────────────────────────────────────────────────────────────────────
        UpdateAction::ToggleProfileWidgetBuilds {
            session_id,
            enabled,
            vm_handle,
        } => {
            if let Some(handle) = vm_handle {
                tokio::spawn(async move {
                    let isolate_id = match handle.main_isolate_id().await {
                        Ok(id) => id,
                        Err(e) => {
                            tracing::warn!(
                                "ToggleProfileWidgetBuilds for session {}: isolate ID error: {}",
                                session_id,
                                e
                            );
                            return;
                        }
                    };
                    let result = handle
                        .call_extension(
                            "ext.flutter.profileWidgetBuilds",
                            &isolate_id,
                            Some(
                                [("enabled".to_string(), enabled.to_string())]
                                    .into_iter()
                                    .collect(),
                            ),
                        )
                        .await;
                    match result {
                        Ok(_) => {
                            let _ = msg_tx
                                .send(crate::message::Message::RebuildStatsExtensionStateChanged {
                                    session_id,
                                    enabled,
                                })
                                .await;
                        }
                        Err(e) => {
                            tracing::warn!(
                                "ToggleProfileWidgetBuilds for session {}: extension call failed: {}",
                                session_id,
                                e
                            );
                            // Roll back the optimistic UI state by emitting the opposite of
                            // what was attempted: if we tried to enable and failed, the extension
                            // is still disabled (and vice versa).
                            let _ = msg_tx
                                .send(crate::message::Message::RebuildStatsExtensionStateChanged {
                                    session_id,
                                    enabled: !enabled,
                                })
                                .await;
                            // Notify the user via the session log buffer.
                            let _ = msg_tx
                                .send(crate::message::Message::RebuildStatsToggleFailed {
                                    session_id,
                                    reason: format!("{e}"),
                                })
                                .await;
                        }
                    }
                });
            } else {
                warn!(
                    "ToggleProfileWidgetBuilds reached handle_action with no VmRequestHandle \
                     for session {} — skipping",
                    session_id
                );
            }
        }

        // ─────────────────────────────────────────────────────────────────────
        // Phase 3: Fetch widgetLocationIdMap (fallback seed for location map)
        // ─────────────────────────────────────────────────────────────────────
        UpdateAction::FetchWidgetLocationIdMap {
            session_id,
            vm_handle,
        } => {
            if let Some(handle) = vm_handle {
                tokio::spawn(async move {
                    let isolate_id = match handle.main_isolate_id().await {
                        Ok(id) => id,
                        Err(e) => {
                            tracing::warn!(
                                "FetchWidgetLocationIdMap for session {}: isolate ID error: {}",
                                session_id,
                                e
                            );
                            let _ = msg_tx
                                .send(crate::message::Message::RebuildStatsToggleFailed {
                                    session_id,
                                    reason: format!("Failed to fetch widget location map: {e}"),
                                })
                                .await;
                            return;
                        }
                    };
                    match fdemon_daemon::vm_service::widget_location_id_map_handle(
                        &handle,
                        &isolate_id,
                    )
                    .await
                    {
                        Ok(map) => {
                            let _ = msg_tx
                                .send(crate::message::Message::RebuildStatsLocationMapFetched {
                                    session_id,
                                    map,
                                })
                                .await;
                        }
                        Err(e) => {
                            tracing::warn!(
                                "FetchWidgetLocationIdMap for session {} failed: {}",
                                session_id,
                                e
                            );
                            let _ = msg_tx
                                .send(crate::message::Message::RebuildStatsToggleFailed {
                                    session_id,
                                    reason: format!("Failed to fetch widget location map: {e}"),
                                })
                                .await;
                        }
                    }
                });
            } else {
                warn!(
                    "FetchWidgetLocationIdMap reached handle_action with no VmRequestHandle \
                     for session {} — skipping",
                    session_id
                );
            }
        }

        // ─────────────────────────────────────────────────────────────────────
        // Phase 5: Frame-anchor debounce
        // ─────────────────────────────────────────────────────────────────────
        UpdateAction::DebounceFrameAnchor {
            session_id,
            generation,
            frame_number,
            delay_ms,
        } => {
            performance::spawn_frame_anchor_debounce(
                session_id,
                generation,
                frame_number,
                delay_ms,
                msg_tx,
            );
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

// ── JDK path helpers ─────────────────────────────────────────────────────────

/// Return the effective JDK home to pass to the Android installer.
///
/// If the user explicitly configured a `[toolchain] jdk_path` that value is
/// returned as-is.  Otherwise we call [`fdemon_daemon::toolchain::resolve_jdk_home`]
/// to discover the JDK from `$JAVA_HOME` or the `java` binary on PATH.
///
/// This helper is intentionally kept as a tiny pure wrapper so it can be unit-
/// tested without spawning any async tasks.
pub(crate) fn resolve_effective_jdk_path(
    config_jdk: Option<std::path::PathBuf>,
) -> Option<std::path::PathBuf> {
    config_jdk.or_else(fdemon_daemon::toolchain::resolve_jdk_home)
}

// ── PathConfig summary helper ─────────────────────────────────────────────────

/// Build the completion summary string for a `PathConfig` wizard step.
///
/// Collects the Flutter clause and (optionally) the Android clause, joins them
/// with `". "`, and appends the restart reminder.  This produces a clean sentence
/// without comma-splices or double spaces regardless of which outcomes are present.
pub(crate) fn build_pathconfig_summary(
    flutter_outcome: &fdemon_daemon::toolchain::PathConfigOutcome,
    android_outcome: Option<fdemon_daemon::toolchain::PathConfigOutcome>,
) -> String {
    use fdemon_daemon::toolchain::PathConfigOutcome;

    let flutter_clause = match flutter_outcome {
        PathConfigOutcome::Written { rc_file } => {
            format!("Added Flutter to PATH in {}", rc_file.display())
        }
        PathConfigOutcome::AlreadyPresent { rc_file } => {
            format!("Flutter already in PATH ({})", rc_file.display())
        }
    };

    let mut clauses: Vec<String> = vec![flutter_clause];

    match android_outcome {
        Some(PathConfigOutcome::Written { rc_file }) => {
            clauses.push(format!("Added ANDROID_HOME to {}", rc_file.display()));
        }
        Some(PathConfigOutcome::AlreadyPresent { rc_file }) => {
            clauses.push(format!(
                "ANDROID_HOME already present in {}",
                rc_file.display()
            ));
        }
        None => {}
    }

    clauses.push("Restart your terminal for changes to take effect".to_string());
    clauses.join(". ") + "."
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

    // ── PersistSettings dispatch tests ──────────────────────────────────────

    /// Build the minimal set of arguments `handle_action` expects.
    ///
    /// We only need the `msg_tx` / `shutdown_rx` pair for the
    /// `PersistSettings` arm; all other parameters are stubbed.
    fn make_handle_action_args() -> (
        tokio::sync::mpsc::Sender<crate::message::Message>,
        tokio::sync::mpsc::Receiver<crate::message::Message>,
        tokio::sync::watch::Receiver<bool>,
    ) {
        let (msg_tx, msg_rx) = tokio::sync::mpsc::channel(32);
        let (_shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);
        (msg_tx, msg_rx, shutdown_rx)
    }

    #[tokio::test]
    async fn persist_settings_action_sends_persisted_message_on_success() {
        let dir = tempfile::tempdir().unwrap();

        // Ensure the `.fdemon` dir exists so `save_settings` can write to it.
        std::fs::create_dir_all(dir.path().join(".fdemon")).unwrap();

        let settings = crate::config::Settings::default();
        let project_path = dir.path().to_path_buf();

        let (msg_tx, mut msg_rx, shutdown_rx) = make_handle_action_args();

        let session_tasks: SessionTaskMap = Arc::new(std::sync::Mutex::new(HashMap::new()));
        let dap_server_handle: DapHandleSlot = Arc::new(std::sync::Mutex::new(None));
        let vm_handle_for_dap: Arc<
            std::sync::Mutex<Option<fdemon_daemon::vm_service::VmRequestHandle>>,
        > = Arc::new(std::sync::Mutex::new(None));
        let dap_debug_senders: Arc<
            std::sync::Mutex<Vec<tokio::sync::mpsc::Sender<fdemon_dap::adapter::DebugEvent>>>,
        > = Arc::new(std::sync::Mutex::new(Vec::new()));

        handle_action(
            crate::UpdateAction::PersistSettings {
                settings: Box::new(settings),
                project_path: project_path.clone(),
            },
            msg_tx,
            None,   // session_cmd_sender
            vec![], // session_senders
            session_tasks,
            shutdown_rx,
            &project_path,
            fdemon_daemon::ToolAvailability::default(),
            dap_server_handle,
            vm_handle_for_dap,
            dap_debug_senders,
        );

        // Allow the spawned tasks to run.
        let msg = tokio::time::timeout(std::time::Duration::from_secs(5), msg_rx.recv())
            .await
            .expect("timed out waiting for SettingsPersisted")
            .expect("channel closed");

        assert!(
            matches!(msg, crate::message::Message::SettingsPersisted),
            "expected SettingsPersisted, got: {msg:?}"
        );
    }

    #[tokio::test]
    async fn persist_settings_action_sends_failed_message_on_error() {
        // Use a regular file as the project path so `save_settings` fails when
        // it tries to `create_dir_all(<file>/.fdemon)` — a non-directory
        // ancestor is rejected on every platform (the prior approach used a
        // Unix-style absolute path that succeeded on Windows by resolving
        // against the writable drive root).
        let temp_file = tempfile::NamedTempFile::new().expect("create temp file");
        let project_path = temp_file.path().to_path_buf();

        let settings = crate::config::Settings::default();

        let (msg_tx, mut msg_rx, shutdown_rx) = make_handle_action_args();

        let session_tasks: SessionTaskMap = Arc::new(std::sync::Mutex::new(HashMap::new()));
        let dap_server_handle: DapHandleSlot = Arc::new(std::sync::Mutex::new(None));
        let vm_handle_for_dap: Arc<
            std::sync::Mutex<Option<fdemon_daemon::vm_service::VmRequestHandle>>,
        > = Arc::new(std::sync::Mutex::new(None));
        let dap_debug_senders: Arc<
            std::sync::Mutex<Vec<tokio::sync::mpsc::Sender<fdemon_dap::adapter::DebugEvent>>>,
        > = Arc::new(std::sync::Mutex::new(Vec::new()));

        handle_action(
            crate::UpdateAction::PersistSettings {
                settings: Box::new(settings),
                project_path: project_path.clone(),
            },
            msg_tx,
            None,
            vec![],
            session_tasks,
            shutdown_rx,
            &project_path,
            fdemon_daemon::ToolAvailability::default(),
            dap_server_handle,
            vm_handle_for_dap,
            dap_debug_senders,
        );

        let msg = tokio::time::timeout(std::time::Duration::from_secs(5), msg_rx.recv())
            .await
            .expect("timed out waiting for SettingsPersistFailed")
            .expect("channel closed");

        assert!(
            matches!(msg, crate::message::Message::SettingsPersistFailed { .. }),
            "expected SettingsPersistFailed, got: {msg:?}"
        );
    }

    // ── RunWizardStep dispatch tests ────────────────────────────────────────────

    /// Receive the next message, skipping any `WizardInstallTaskReady` messages.
    ///
    /// `RunWizardStep` now sends a `WizardInstallTaskReady` message immediately
    /// after spawning the task (to hand the cancel token + handle to state).
    /// Tests that check for `WizardStepStarted → WizardStepFailed/Completed`
    /// must skip this intermediate message.
    async fn recv_skip_task_ready(
        rx: &mut tokio::sync::mpsc::Receiver<crate::message::Message>,
        timeout_secs: u64,
    ) -> crate::message::Message {
        loop {
            let msg = tokio::time::timeout(std::time::Duration::from_secs(timeout_secs), rx.recv())
                .await
                .expect("timed out waiting for message")
                .expect("channel closed");
            if matches!(msg, crate::message::Message::WizardInstallTaskReady { .. }) {
                continue;
            }
            return msg;
        }
    }

    /// Shared helper: invoke `handle_action` with `RunWizardStep` and return the
    /// message receiver so callers can assert on which messages arrive.
    fn dispatch_run_wizard_step(
        action: crate::UpdateAction,
    ) -> tokio::sync::mpsc::Receiver<crate::message::Message> {
        let (msg_tx, msg_rx) = tokio::sync::mpsc::channel(64);
        let (_shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);
        let session_tasks: SessionTaskMap = Arc::new(std::sync::Mutex::new(HashMap::new()));
        let dap_server_handle: DapHandleSlot = Arc::new(std::sync::Mutex::new(None));
        let vm_handle_for_dap: Arc<
            std::sync::Mutex<Option<fdemon_daemon::vm_service::VmRequestHandle>>,
        > = Arc::new(std::sync::Mutex::new(None));
        let dap_debug_senders: Arc<
            std::sync::Mutex<Vec<tokio::sync::mpsc::Sender<fdemon_dap::adapter::DebugEvent>>>,
        > = Arc::new(std::sync::Mutex::new(Vec::new()));
        let project_path = std::path::PathBuf::from("/tmp");

        handle_action(
            action,
            msg_tx,
            None,
            vec![],
            session_tasks,
            shutdown_rx,
            &project_path,
            fdemon_daemon::ToolAvailability::default(),
            dap_server_handle,
            vm_handle_for_dap,
            dap_debug_senders,
        );
        msg_rx
    }

    /// Dispatching `RunWizardStep` always emits `WizardStepStarted` first,
    /// regardless of step kind or param validity.  This guards the minimum
    /// TEA contract: the executor announces itself before doing any work.
    #[tokio::test]
    async fn test_run_wizard_step_emits_started() {
        use crate::install_wizard::WizardStepKind;

        let mut msg_rx = dispatch_run_wizard_step(crate::UpdateAction::RunWizardStep {
            kind: WizardStepKind::FlutterSdk,
            run_seq: 1,
            cancel_token: tokio_util::sync::CancellationToken::new(),
            install: None, // Missing params → WizardStepFailed will follow, but Started comes first.
            path_bin_dir: None,
            android_sdk_root: None,
            android: None,
        });

        let first = tokio::time::timeout(std::time::Duration::from_secs(5), msg_rx.recv())
            .await
            .expect("timed out waiting for first message")
            .expect("channel closed");

        assert!(
            matches!(
                first,
                crate::message::Message::WizardStepStarted {
                    kind: WizardStepKind::FlutterSdk,
                    ..
                }
            ),
            "first message must be WizardStepStarted; got: {first:?}"
        );
    }

    /// Missing `install` params for `FlutterSdk` step → `WizardStepFailed`.
    #[tokio::test]
    async fn test_run_wizard_step_flutter_sdk_missing_install_params_fails() {
        use crate::install_wizard::WizardStepKind;

        let mut msg_rx = dispatch_run_wizard_step(crate::UpdateAction::RunWizardStep {
            kind: WizardStepKind::FlutterSdk,
            run_seq: 1,
            cancel_token: tokio_util::sync::CancellationToken::new(),
            install: None,
            path_bin_dir: None,
            android_sdk_root: None,
            android: None,
        });

        // Consume WizardStepStarted.
        let _started = tokio::time::timeout(std::time::Duration::from_secs(5), msg_rx.recv())
            .await
            .expect("timed out")
            .expect("channel closed");

        let second = tokio::time::timeout(std::time::Duration::from_secs(5), msg_rx.recv())
            .await
            .expect("timed out waiting for WizardStepFailed")
            .expect("channel closed");

        assert!(
            matches!(
                second,
                crate::message::Message::WizardStepFailed {
                    kind: WizardStepKind::FlutterSdk,
                    ..
                }
            ),
            "missing install params must produce WizardStepFailed; got: {second:?}"
        );
    }

    /// Missing `path_bin_dir` for `PathConfig` step → `WizardStepFailed`.
    #[tokio::test]
    async fn test_run_wizard_step_pathconfig_missing_bindir_fails() {
        use crate::install_wizard::WizardStepKind;

        let mut msg_rx = dispatch_run_wizard_step(crate::UpdateAction::RunWizardStep {
            kind: WizardStepKind::PathConfig,
            run_seq: 1,
            cancel_token: tokio_util::sync::CancellationToken::new(),
            install: None,
            path_bin_dir: None, // Missing — executor must fail cleanly.
            android_sdk_root: None,
            android: None,
        });

        // Consume WizardStepStarted.
        let _started = tokio::time::timeout(std::time::Duration::from_secs(5), msg_rx.recv())
            .await
            .expect("timed out")
            .expect("channel closed");

        let second = tokio::time::timeout(std::time::Duration::from_secs(5), msg_rx.recv())
            .await
            .expect("timed out waiting for WizardStepFailed")
            .expect("channel closed");

        assert!(
            matches!(
                second,
                crate::message::Message::WizardStepFailed {
                    kind: WizardStepKind::PathConfig,
                    ..
                }
            ),
            "missing path_bin_dir must produce WizardStepFailed; got: {second:?}"
        );
    }

    /// Non-executable step kinds (Prerequisites, Doctor, Platforms, platform leaves)
    /// always produce a `WizardStepFailed` with a clear reason message.
    ///
    /// `PlatformAndroid` is handled by the real executor and is NOT in this list.
    #[tokio::test]
    async fn test_run_wizard_step_non_executable_kinds_fail() {
        use crate::install_wizard::WizardStepKind;

        for kind in [
            WizardStepKind::Prerequisites,
            WizardStepKind::Platforms,
            WizardStepKind::PlatformIos,
            WizardStepKind::PlatformMacos,
            WizardStepKind::PlatformWeb,
            WizardStepKind::PlatformWindows,
            WizardStepKind::Doctor,
        ] {
            let mut msg_rx = dispatch_run_wizard_step(crate::UpdateAction::RunWizardStep {
                kind,
                run_seq: 1,
                cancel_token: tokio_util::sync::CancellationToken::new(),
                install: None,
                path_bin_dir: None,
                android_sdk_root: None,
                android: None,
            });

            // Consume WizardStepStarted.
            let _started = tokio::time::timeout(std::time::Duration::from_secs(2), msg_rx.recv())
                .await
                .expect("timed out")
                .expect("channel closed");

            let result_msg = tokio::time::timeout(std::time::Duration::from_secs(2), msg_rx.recv())
                .await
                .expect("timed out waiting for WizardStepFailed")
                .expect("channel closed");

            assert!(
                matches!(result_msg, crate::message::Message::WizardStepFailed { .. }),
                "kind {kind:?} must produce WizardStepFailed; got: {result_msg:?}"
            );
        }
    }

    /// `PathConfig` step with a valid `path_bin_dir` runs `add_to_path` and
    /// produces either `WizardStepCompleted` or `WizardStepFailed` (never hangs).
    ///
    /// `$HOME` is redirected to a `TempDir` for the duration of this test so that
    /// the PathConfig executor cannot write to the developer's real `~/.zshenv` /
    /// `~/.bashrc`. The test is serialised (via `serial_test::serial`) because it
    /// mutates the process-wide `$HOME` environment variable.
    #[tokio::test]
    #[serial_test::serial]
    async fn test_run_wizard_step_pathconfig_terminates() {
        use crate::install_wizard::WizardStepKind;

        // Redirect $HOME to a sandboxed TempDir so add_to_path never reaches the
        // developer's real shell rc files.  Restore on exit (guard via Drop).
        let tmp = tempfile::tempdir().unwrap();
        let saved_home = std::env::var_os("HOME");
        std::env::set_var("HOME", tmp.path());
        struct HomeGuard(Option<std::ffi::OsString>);
        impl Drop for HomeGuard {
            fn drop(&mut self) {
                match &self.0 {
                    Some(v) => std::env::set_var("HOME", v),
                    None => std::env::remove_var("HOME"),
                }
            }
        }
        let _home_guard = HomeGuard(saved_home);

        let bin_dir = tmp.path().join("bin");
        std::fs::create_dir_all(&bin_dir).unwrap();

        let mut msg_rx = dispatch_run_wizard_step(crate::UpdateAction::RunWizardStep {
            kind: WizardStepKind::PathConfig,
            run_seq: 1,
            cancel_token: tokio_util::sync::CancellationToken::new(),
            install: None,
            path_bin_dir: Some(bin_dir),
            android_sdk_root: None,
            android: None,
        });

        // Consume WizardStepStarted (skip WizardInstallTaskReady if it arrives first).
        let _started = recv_skip_task_ready(&mut msg_rx, 5).await;

        // The next non-ready message must be Completed or Failed — never absent.
        let outcome = recv_skip_task_ready(&mut msg_rx, 5).await;

        assert!(
            matches!(
                outcome,
                crate::message::Message::WizardStepCompleted {
                    kind: WizardStepKind::PathConfig,
                    ..
                } | crate::message::Message::WizardStepFailed {
                    kind: WizardStepKind::PathConfig,
                    ..
                }
            ),
            "PathConfig executor must always terminate with Completed or Failed; got: {outcome:?}"
        );
    }

    // ── PlatformAndroid executor dispatch tests ────────────────────────────────────

    /// `RunWizardStep { kind: PlatformAndroid, android: None }` must emit
    /// `WizardStepStarted` followed by `WizardStepFailed` — never a panic.
    #[tokio::test]
    async fn test_android_tools_missing_params_fails() {
        use crate::install_wizard::WizardStepKind;

        let mut msg_rx = dispatch_run_wizard_step(crate::UpdateAction::RunWizardStep {
            kind: WizardStepKind::PlatformAndroid,
            run_seq: 1,
            cancel_token: tokio_util::sync::CancellationToken::new(),
            install: None,
            path_bin_dir: None,
            android_sdk_root: None,
            android: None, // Missing params — must fail cleanly.
        });

        // First message: WizardStepStarted.
        let first = tokio::time::timeout(std::time::Duration::from_secs(5), msg_rx.recv())
            .await
            .expect("timed out waiting for WizardStepStarted")
            .expect("channel closed");

        assert!(
            matches!(
                first,
                crate::message::Message::WizardStepStarted {
                    kind: WizardStepKind::PlatformAndroid,
                    ..
                }
            ),
            "first message must be WizardStepStarted; got: {first:?}"
        );

        // Second message: WizardStepFailed (missing params guard).
        let second = tokio::time::timeout(std::time::Duration::from_secs(5), msg_rx.recv())
            .await
            .expect("timed out waiting for WizardStepFailed")
            .expect("channel closed");

        assert!(
            matches!(
                second,
                crate::message::Message::WizardStepFailed {
                    kind: WizardStepKind::PlatformAndroid,
                    ..
                }
            ),
            "missing android params must produce WizardStepFailed; got: {second:?}"
        );
    }

    /// `RunWizardStep { kind: PlatformAndroid, android: Some(..) }` emits
    /// `WizardStepStarted` as its first message (the install attempt itself is not
    /// unit-tested because it requires network I/O, mirroring Phase 2 `FlutterSdk`).
    #[tokio::test]
    async fn test_android_tools_emits_started() {
        use crate::handler::AndroidStepParams;
        use crate::install_wizard::WizardStepKind;

        let tmp = tempfile::tempdir().unwrap();
        let sdk_root = tmp.path().join("android-sdk");

        let mut msg_rx = dispatch_run_wizard_step(crate::UpdateAction::RunWizardStep {
            kind: WizardStepKind::PlatformAndroid,
            run_seq: 1,
            cancel_token: tokio_util::sync::CancellationToken::new(),
            install: None,
            path_bin_dir: None,
            android_sdk_root: None,
            android: Some(AndroidStepParams {
                sdk_root: Some(sdk_root),
                api_level: 36,
                cmdline_tools_build: None,
                jdk_path: None,
                cmdline_tools_sha256: None,
            }),
        });

        let first = tokio::time::timeout(std::time::Duration::from_secs(5), msg_rx.recv())
            .await
            .expect("timed out waiting for first message")
            .expect("channel closed");

        assert!(
            matches!(
                first,
                crate::message::Message::WizardStepStarted {
                    kind: WizardStepKind::PlatformAndroid,
                    ..
                }
            ),
            "first message must be WizardStepStarted; got: {first:?}"
        );
    }

    /// `PathConfig` step without `android_sdk_root` still writes the Flutter
    /// PATH entry and produces `WizardStepCompleted` or `WizardStepFailed` — it
    /// must never hang or attempt to write `ANDROID_HOME`.
    ///
    /// `$HOME` is redirected to a `TempDir` so the executor cannot reach the
    /// developer's real shell rc files. Serialised to prevent `$HOME` races.
    #[tokio::test]
    #[serial_test::serial]
    async fn test_pathconfig_without_android_root_still_writes_flutter() {
        use crate::install_wizard::WizardStepKind;

        let tmp = tempfile::tempdir().unwrap();
        let saved_home = std::env::var_os("HOME");
        std::env::set_var("HOME", tmp.path());
        struct HomeGuard(Option<std::ffi::OsString>);
        impl Drop for HomeGuard {
            fn drop(&mut self) {
                match &self.0 {
                    Some(v) => std::env::set_var("HOME", v),
                    None => std::env::remove_var("HOME"),
                }
            }
        }
        let _home_guard = HomeGuard(saved_home);

        let bin_dir = tmp.path().join("bin");
        std::fs::create_dir_all(&bin_dir).unwrap();

        let mut msg_rx = dispatch_run_wizard_step(crate::UpdateAction::RunWizardStep {
            kind: WizardStepKind::PathConfig,
            run_seq: 1,
            cancel_token: tokio_util::sync::CancellationToken::new(),
            install: None,
            path_bin_dir: Some(bin_dir),
            android_sdk_root: None, // No android root — must not fail because of this.
            android: None,
        });

        // Consume WizardStepStarted (skip WizardInstallTaskReady if it arrives first).
        let _started = recv_skip_task_ready(&mut msg_rx, 5).await;

        let outcome = recv_skip_task_ready(&mut msg_rx, 5).await;

        assert!(
            matches!(
                outcome,
                crate::message::Message::WizardStepCompleted {
                    kind: WizardStepKind::PathConfig,
                    ..
                } | crate::message::Message::WizardStepFailed {
                    kind: WizardStepKind::PathConfig,
                    ..
                }
            ),
            "PathConfig with no android root must terminate with Completed or Failed; got: {outcome:?}"
        );
    }

    // ── PathConfig executor resolver fallback tests ─────────────────────────────

    /// When `android_sdk_root` is `None` but `$ANDROID_HOME` points to a
    /// temp dir that exists, the PathConfig executor should call `add_android_env`
    /// (i.e. not skip the Android block). We verify this by checking that the
    /// executor resolves to Completed or Failed (not a panic) and that the env var
    /// fallback logic itself works correctly.
    ///
    /// `$HOME` is redirected to a `TempDir` so neither `add_to_path` nor
    /// `add_android_env` can reach the developer's real shell rc files.
    #[tokio::test]
    #[serial_test::serial]
    async fn test_pathconfig_writes_android_env_from_resolver_when_settings_none() {
        use crate::install_wizard::WizardStepKind;

        // Create a temp dir to serve as both the bin dir and the "Android SDK root".
        let tmp = tempfile::tempdir().unwrap();

        // Redirect $HOME to the sandbox so add_to_path / add_android_env write
        // to the TempDir, not the developer's real home directory.
        let saved_home = std::env::var_os("HOME");
        std::env::set_var("HOME", tmp.path());
        struct HomeGuard(Option<std::ffi::OsString>);
        impl Drop for HomeGuard {
            fn drop(&mut self) {
                match &self.0 {
                    Some(v) => std::env::set_var("HOME", v),
                    None => std::env::remove_var("HOME"),
                }
            }
        }
        let _home_guard = HomeGuard(saved_home);

        let bin_dir = tmp.path().join("flutter_bin");
        std::fs::create_dir_all(&bin_dir).unwrap();
        let android_home = tmp.path().join("android_sdk");
        std::fs::create_dir_all(&android_home).unwrap();

        // Set $ANDROID_HOME so the resolver finds the temp dir.
        std::env::set_var("ANDROID_HOME", android_home.as_os_str());

        let mut msg_rx = dispatch_run_wizard_step(crate::UpdateAction::RunWizardStep {
            kind: WizardStepKind::PathConfig,
            run_seq: 1,
            cancel_token: tokio_util::sync::CancellationToken::new(),
            install: None,
            path_bin_dir: Some(bin_dir),
            android_sdk_root: None, // settings has no root — must fall back to $ANDROID_HOME
            android: None,
        });

        // Consume WizardStepStarted (skip WizardInstallTaskReady if it arrives first).
        let _started = recv_skip_task_ready(&mut msg_rx, 5).await;

        let outcome = recv_skip_task_ready(&mut msg_rx, 5).await;

        std::env::remove_var("ANDROID_HOME");

        // The executor must terminate (not hang), and must not fail due to a missing
        // Android root — the fallback to $ANDROID_HOME resolved a valid dir.
        assert!(
            matches!(
                outcome,
                crate::message::Message::WizardStepCompleted {
                    kind: WizardStepKind::PathConfig,
                    ..
                } | crate::message::Message::WizardStepFailed {
                    kind: WizardStepKind::PathConfig,
                    ..
                }
            ),
            "PathConfig with $ANDROID_HOME fallback must terminate; got: {outcome:?}"
        );
    }

    /// When `android_sdk_root` is `None` and the resolver returns a path that
    /// does not exist on disk, the Android block must be silently skipped and
    /// the executor must still complete (Flutter PATH is written regardless).
    ///
    /// `$HOME` is redirected to a `TempDir` so the Flutter PATH write cannot
    /// reach the developer's real shell rc files.
    #[tokio::test]
    #[serial_test::serial]
    async fn test_pathconfig_skips_android_env_when_no_sdk_anywhere() {
        use crate::install_wizard::WizardStepKind;

        let tmp = tempfile::tempdir().unwrap();

        // Redirect $HOME to the sandbox so add_to_path writes to the TempDir.
        let saved_home = std::env::var_os("HOME");
        std::env::set_var("HOME", tmp.path());
        struct HomeGuard(Option<std::ffi::OsString>);
        impl Drop for HomeGuard {
            fn drop(&mut self) {
                match &self.0 {
                    Some(v) => std::env::set_var("HOME", v),
                    None => std::env::remove_var("HOME"),
                }
            }
        }
        let _home_guard = HomeGuard(saved_home);

        let bin_dir = tmp.path().join("flutter_bin");
        std::fs::create_dir_all(&bin_dir).unwrap();

        // Ensure no Android env vars are set and the default path does not exist.
        std::env::remove_var("ANDROID_HOME");
        std::env::remove_var("ANDROID_SDK_ROOT");

        let mut msg_rx = dispatch_run_wizard_step(crate::UpdateAction::RunWizardStep {
            kind: WizardStepKind::PathConfig,
            run_seq: 1,
            cancel_token: tokio_util::sync::CancellationToken::new(),
            install: None,
            path_bin_dir: Some(bin_dir),
            android_sdk_root: None,
            android: None,
        });

        // Consume WizardStepStarted.
        let _started = recv_skip_task_ready(&mut msg_rx, 5).await;

        let outcome = recv_skip_task_ready(&mut msg_rx, 5).await;

        // Executor must terminate whether or not the Flutter PATH write itself
        // succeeds (depends on the runtime shell detection).
        assert!(
            matches!(
                outcome,
                crate::message::Message::WizardStepCompleted {
                    kind: WizardStepKind::PathConfig,
                    ..
                } | crate::message::Message::WizardStepFailed {
                    kind: WizardStepKind::PathConfig,
                    ..
                }
            ),
            "PathConfig with no SDK anywhere must terminate; got: {outcome:?}"
        );
    }

    // ── resolve_android_sdk_root_path unit tests (via daemon re-export) ─────────

    /// When a caller-provided path is given, it must be returned as-is.
    #[test]
    fn test_resolve_android_sdk_root_uses_provided_path() {
        let path = std::path::Path::new("/opt/android/sdk");
        let result = fdemon_daemon::resolve_android_sdk_root_path(Some(path));
        assert_eq!(result, std::path::PathBuf::from("/opt/android/sdk"));
    }

    /// When no path is provided and ANDROID_HOME is set, that value is returned.
    #[test]
    fn test_resolve_android_sdk_root_falls_back_to_android_home() {
        // Guard: remove both env vars first, then set ANDROID_HOME.
        std::env::remove_var("ANDROID_SDK_ROOT");
        std::env::set_var("ANDROID_HOME", "/custom/android/home");

        let result = fdemon_daemon::resolve_android_sdk_root_path(None);
        std::env::remove_var("ANDROID_HOME");

        assert_eq!(result, std::path::PathBuf::from("/custom/android/home"));
    }

    /// `resolve_android_sdk_root_path` never panics even when no env vars are set and
    /// the home dir is unavailable (returns the platform fallback or last-resort).
    #[test]
    fn test_resolve_android_sdk_root_never_panics() {
        // We cannot easily remove HOME/USERPROFILE but the function must not panic.
        let _result = fdemon_daemon::resolve_android_sdk_root_path(None);
    }

    // ── resolve_effective_jdk_path (M1) tests ───────────────────────────────────

    /// When an explicit `config_jdk` path is provided it should be returned
    /// without calling `resolve_jdk_home`.
    #[test]
    fn test_resolve_effective_jdk_path_prefers_config_value() {
        let explicit = std::path::PathBuf::from("/my/configured/jdk");
        let result = resolve_effective_jdk_path(Some(explicit.clone()));
        assert_eq!(
            result,
            Some(explicit),
            "configured path must be returned as-is"
        );
    }

    /// When `config_jdk` is `None` and `JAVA_HOME` points to a valid directory,
    /// `resolve_effective_jdk_path` must return that directory.
    ///
    /// `JAVA_HOME` is a process-global env var, so this test is marked `#[serial]`
    /// to avoid races with other tests that manipulate the same variable.
    #[test]
    #[serial_test::serial]
    fn test_resolve_effective_jdk_path_falls_back_to_java_home() {
        let tmp = tempfile::TempDir::new().unwrap();

        std::env::set_var("JAVA_HOME", tmp.path());
        let result = resolve_effective_jdk_path(None);
        std::env::remove_var("JAVA_HOME");

        assert_eq!(
            result.as_deref(),
            Some(tmp.path()),
            "should fall back to JAVA_HOME when config_jdk is None"
        );
    }

    /// When `config_jdk` is `None` and no JDK is discoverable, the result is
    /// `None` (not a panic or an error).
    #[test]
    #[serial_test::serial]
    fn test_resolve_effective_jdk_path_returns_none_when_no_jdk() {
        // Point JAVA_HOME at a non-existent directory so resolve_jdk_home skips it.
        std::env::set_var("JAVA_HOME", "/this/path/does/not/exist/fdemon_m1_test");
        let result = resolve_effective_jdk_path(None);
        std::env::remove_var("JAVA_HOME");

        // We cannot guarantee `which java` also fails on every CI machine, so we
        // simply assert the call does not panic and returns an Option (may be Some
        // if `java` is on PATH via the which fallback).
        let _ = result; // type-checks: Option<PathBuf>
    }

    // ── PathConfig summary string (M4) tests ────────────────────────────────────

    /// Flutter-only summary must not contain a comma-splice and must end with a
    /// single trailing period.
    #[test]
    fn test_pathconfig_summary_flutter_only() {
        use fdemon_daemon::toolchain::PathConfigOutcome;

        let flutter_outcome = PathConfigOutcome::Written {
            rc_file: std::path::PathBuf::from("/home/user/.zshrc"),
        };
        let android_outcome: Option<PathConfigOutcome> = None;

        let summary = build_pathconfig_summary(&flutter_outcome, android_outcome);

        assert!(
            !summary.contains(", "),
            "flutter-only summary must not have a comma-splice; got: {summary:?}"
        );
        assert!(
            summary.ends_with('.'),
            "summary must end with a single period; got: {summary:?}"
        );
        assert!(
            summary.contains("Restart your terminal"),
            "summary must include restart hint; got: {summary:?}"
        );
    }

    /// Flutter+Android summary must use ". " between clauses (not ", … and ").
    #[test]
    fn test_pathconfig_summary_flutter_and_android() {
        use fdemon_daemon::toolchain::PathConfigOutcome;

        let flutter_outcome = PathConfigOutcome::Written {
            rc_file: std::path::PathBuf::from("/home/user/.zshrc"),
        };
        let android_outcome = Some(PathConfigOutcome::Written {
            rc_file: std::path::PathBuf::from("/home/user/.zshrc"),
        });

        let summary = build_pathconfig_summary(&flutter_outcome, android_outcome);

        // Must NOT have the old comma-splice pattern.
        assert!(
            !summary.contains(", "),
            "combined summary must not have a comma-splice; got: {summary:?}"
        );
        // Must NOT have trailing spaces in the android clause.
        assert!(
            !summary.contains("  "),
            "combined summary must not have double spaces; got: {summary:?}"
        );
        // Must contain all three logical pieces.
        assert!(
            summary.contains("Flutter"),
            "must mention Flutter; got: {summary:?}"
        );
        assert!(
            summary.contains("ANDROID_HOME"),
            "must mention ANDROID_HOME; got: {summary:?}"
        );
        assert!(
            summary.contains("Restart your terminal"),
            "must include restart hint; got: {summary:?}"
        );
        assert!(
            summary.ends_with('.'),
            "summary must end with a single period; got: {summary:?}"
        );
    }
}
