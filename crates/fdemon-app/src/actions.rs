//! Action handlers: UpdateAction dispatch and background task spawning

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::{mpsc, watch};
use tracing::{error, info, warn};

use crate::config::LaunchConfig;
use crate::handler::Task;
use crate::message::{DebugOverlayKind, Message};
use crate::session::SessionId;
use crate::UpdateAction;
use fdemon_core::{DaemonEvent, DaemonMessage};
use fdemon_daemon::{
    vm_service::{
        enable_frame_tracking, ext, extract_layout_info, flutter_error_to_log_entry,
        parse_bool_extension_response, parse_diagnostics_node_response, parse_flutter_error,
        parse_frame_timing, parse_gc_event, parse_log_record, vm_log_to_log_entry, VmRequestHandle,
        VmServiceClient,
    },
    CommandSender, DaemonCommand, Device, FlutterProcess, RequestTracker, ToolAvailability,
};

/// Minimum polling interval for memory usage (500ms) to prevent excessive VM Service calls.
const PERF_POLL_MIN_MS: u64 = 500;

use super::spawn;

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
            spawn_session(
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
            let handle = spawn_vm_service_connection(session_id, ws_uri, msg_tx);
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
                spawn_performance_polling(
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
        } => {
            if let Some(handle) = vm_handle {
                spawn_fetch_widget_tree(session_id, handle, msg_tx, tree_max_depth);
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
                spawn_fetch_layout_data(session_id, node_id, handle, msg_tx);
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
                spawn_toggle_overlay(session_id, extension, handle, msg_tx);
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
                spawn_dispose_devtools_groups(session_id, handle);
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
                if let Err(e) = open_url_in_browser(&url, &browser) {
                    tracing::error!("Failed to open browser DevTools: {e}");
                }
            });
        }
    }
}

/// Spawn a Flutter session for a device (multi-session mode)
fn spawn_session(
    session_id: SessionId,
    device: Device,
    config: Option<Box<LaunchConfig>>,
    project_path: &Path,
    msg_tx: mpsc::Sender<Message>,
    session_tasks: SessionTaskMap,
    shutdown_rx: watch::Receiver<bool>,
) {
    let project_path = project_path.to_path_buf();
    let msg_tx_clone = msg_tx.clone();
    let session_tasks_clone = session_tasks.clone();
    let mut shutdown_rx_clone = shutdown_rx.clone();
    let device_id = device.id.clone();
    let device_name = device.name.clone();
    let device_platform = device.platform.clone();

    let handle = tokio::spawn(async move {
        info!(
            "Spawning Flutter session {} on device: {} ({})",
            session_id, device_name, device_id
        );

        // Create event channel for this session
        let (daemon_tx, mut daemon_rx) = mpsc::channel::<DaemonEvent>(256);

        // Spawn the Flutter process
        let spawn_result = if let Some(cfg) = config {
            // Build flutter args from config (conversion happens here in app layer)
            let args = cfg.build_flutter_args(&device_id);
            FlutterProcess::spawn_with_args(&project_path, args, daemon_tx).await
        } else {
            FlutterProcess::spawn_with_device(&project_path, &device_id, daemon_tx).await
        };

        match spawn_result {
            Ok(mut process) => {
                info!(
                    "Flutter process started for session {} (PID: {:?})",
                    session_id,
                    process.id()
                );

                // Create command sender for this session
                let request_tracker = Arc::new(RequestTracker::default());
                let session_sender = process.command_sender(request_tracker);

                // Send SessionProcessAttached to store cmd_sender in SessionHandle
                let _ = msg_tx_clone
                    .send(Message::SessionProcessAttached {
                        session_id,
                        cmd_sender: session_sender.clone(),
                    })
                    .await;

                // Send session started message
                let _ = msg_tx_clone
                    .send(Message::SessionStarted {
                        session_id,
                        device_id: device_id.clone(),
                        device_name: device_name.clone(),
                        platform: device_platform.clone(),
                        pid: process.id(),
                    })
                    .await;

                // Track app_id from events for shutdown
                let mut app_id: Option<String> = None;

                // Track if process has already exited (for fast shutdown path)
                let mut process_exited = false;

                // Forward daemon events to the main message channel
                // This runs until the process exits, main loop closes, or shutdown signal
                loop {
                    tokio::select! {
                        event = daemon_rx.recv() => {
                            match event {
                                Some(event) => {
                                    // Track exit events for fast shutdown
                                    if matches!(event, DaemonEvent::Exited { .. }) {
                                        process_exited = true;
                                    }

                                    // Capture app_id from stdout events
                                    if let DaemonEvent::Stdout(ref line) = event {
                                        if let Some(DaemonMessage::AppStart(app_start)) =
                                            fdemon_daemon::parse_daemon_message(line)
                                        {
                                            app_id = Some(app_start.app_id.clone());
                                        }
                                    }

                                    // Send event with session context for multi-session routing
                                    if msg_tx_clone
                                        .send(Message::SessionDaemon {
                                            session_id,
                                            event,
                                        })
                                        .await
                                        .is_err()
                                    {
                                        // Main loop closed, need to shutdown
                                        break;
                                    }
                                }
                                None => {
                                    // Channel closed, process likely ended
                                    process_exited = true;
                                    break;
                                }
                            }
                        }
                        _ = shutdown_rx_clone.changed() => {
                            // Shutdown signal received
                            info!(
                                "Shutdown signal received, stopping session {}...",
                                session_id
                            );
                            break;
                        }
                    }
                }

                // Fast shutdown path: skip shutdown commands if we know process already exited
                if process_exited {
                    info!(
                        "Session {} process already exited, skipping shutdown commands",
                        session_id
                    );
                } else {
                    // Graceful shutdown when loop ends - use session's own sender
                    info!("Session {} ending, initiating shutdown...", session_id);
                    if let Err(e) = process
                        .shutdown(app_id.as_deref(), Some(&session_sender))
                        .await
                    {
                        warn!(
                            "Shutdown error for session {} (process may already be gone): {}",
                            session_id, e
                        );
                    }
                }
            }
            Err(e) => {
                error!(
                    "Failed to spawn Flutter process for session {}: {}",
                    session_id, e
                );
                let _ = msg_tx_clone
                    .send(Message::SessionSpawnFailed {
                        session_id,
                        device_id,
                        error: e.to_string(),
                    })
                    .await;
            }
        }

        // Remove this session's task from the tracking map
        if let Ok(mut guard) = session_tasks_clone.lock() {
            guard.remove(&session_id);
            info!("Session {} task removed from tracking", session_id);
        } else {
            warn!(
                "Session {} task could not be removed from tracking (poisoned lock)",
                session_id
            );
        }
    });

    // Store the handle with session_id as key (allows multiple concurrent sessions)
    match session_tasks.lock() {
        Ok(mut guard) => {
            guard.insert(session_id, handle);
            info!(
                "Session {} task added to tracking (total: {})",
                session_id,
                guard.len()
            );
        }
        Err(e) => {
            warn!(
                "Session {} task handle could not be tracked (poisoned lock): {}",
                session_id, e
            );
        }
    }
}

/// Execute a task and send completion message
pub async fn execute_task(
    task: Task,
    msg_tx: mpsc::Sender<Message>,
    cmd_sender: Option<CommandSender>,
) {
    let Some(sender) = cmd_sender else {
        // No command sender available - send session-specific failure
        let msg = match task {
            Task::Reload { session_id, .. } => Message::SessionReloadFailed {
                session_id,
                reason: "Flutter not running".to_string(),
            },
            Task::Restart { session_id, .. } => Message::SessionRestartFailed {
                session_id,
                reason: "Flutter not running".to_string(),
            },
            Task::Stop { .. } => return, // Nothing to do
        };
        let _ = msg_tx.send(msg).await;
        return;
    };

    match task {
        Task::Reload { session_id, app_id } => {
            let start = std::time::Instant::now();
            info!(
                "Executing reload for session {} (app_id: {})",
                session_id, app_id
            );
            match sender.send(DaemonCommand::Reload { app_id }).await {
                Ok(response) => {
                    if response.success {
                        let time_ms = start.elapsed().as_millis() as u64;
                        let _ = msg_tx
                            .send(Message::SessionReloadCompleted {
                                session_id,
                                time_ms,
                            })
                            .await;
                    } else {
                        let reason = response
                            .error
                            .unwrap_or_else(|| "Unknown error".to_string());
                        let _ = msg_tx
                            .send(Message::SessionReloadFailed { session_id, reason })
                            .await;
                    }
                }
                Err(e) => {
                    let reason = e.to_string();
                    let _ = msg_tx
                        .send(Message::SessionReloadFailed { session_id, reason })
                        .await;
                }
            }
        }
        Task::Restart { session_id, app_id } => {
            info!(
                "Executing restart for session {} (app_id: {})",
                session_id, app_id
            );
            match sender.send(DaemonCommand::Restart { app_id }).await {
                Ok(response) => {
                    if response.success {
                        let _ = msg_tx
                            .send(Message::SessionRestartCompleted { session_id })
                            .await;
                    } else {
                        let reason = response
                            .error
                            .unwrap_or_else(|| "Unknown error".to_string());
                        let _ = msg_tx
                            .send(Message::SessionRestartFailed { session_id, reason })
                            .await;
                    }
                }
                Err(e) => {
                    let reason = e.to_string();
                    let _ = msg_tx
                        .send(Message::SessionRestartFailed { session_id, reason })
                        .await;
                }
            }
        }
        Task::Stop { session_id, app_id } => {
            info!(
                "Executing stop for session {} (app_id: {})",
                session_id, app_id
            );
            if let Err(e) = sender.send(DaemonCommand::Stop { app_id }).await {
                error!("Failed to stop app: {}", e);
            }
        }
    }
}

/// Minimum allocation profile polling interval (1000ms).
///
/// `getAllocationProfile` walks the entire Dart heap, making it significantly
/// more expensive than `getMemoryUsage`. A higher minimum ensures it is never
/// called more frequently than once per second even with aggressive settings.
const ALLOC_PROFILE_POLL_MIN_MS: u64 = 1000;

/// Spawn the periodic memory-usage polling task for a session.
///
/// Creates a `watch::channel(false)` shutdown channel outside the spawned task
/// so that both the sender and the `JoinHandle` are available to package into
/// `VmServicePerformanceMonitoringStarted`. The TEA layer can then:
/// - Signal the task to stop by sending `true` on the shutdown channel, and
/// - Abort the task directly via the `JoinHandle` if needed.
///
/// The polling loop runs until:
/// - The shutdown channel receives `true` (VM disconnected / session stopped), or
/// - The `msg_tx` channel is closed (engine shutting down).
///
/// **Memory tick** (every `performance_refresh_ms`, min 500ms):
/// 1. Calls `getMemoryUsage` → sends `VmServiceMemorySnapshot` (basic gauge).
/// 2. Calls `get_memory_sample` (combines `getMemoryUsage` + `getIsolate` RSS) →
///    sends `VmServiceMemorySample` (rich time-series). The two ring buffers stay
///    in sync because both are populated from the same tick.
///
/// **Allocation tick** (every `allocation_profile_interval_ms`, min 1000ms):
/// - Calls `getAllocationProfile` → sends `VmServiceAllocationProfileReceived`.
///   This is intentionally lower frequency than the memory tick because it is
///   expensive (forces the VM to walk the entire heap).
///
/// Transient errors from any RPC (e.g., isolate paused during hot reload) are
/// logged at debug level and skipped — the next tick will retry.
///
/// The `performance_refresh_ms` parameter controls the memory polling interval.
/// It is clamped to a minimum of [`PERF_POLL_MIN_MS`] (500ms).
///
/// The `allocation_profile_interval_ms` parameter controls the allocation profile
/// polling interval. It is clamped to a minimum of [`ALLOC_PROFILE_POLL_MIN_MS`]
/// (1000ms).
fn spawn_performance_polling(
    session_id: SessionId,
    handle: VmRequestHandle,
    msg_tx: mpsc::Sender<Message>,
    performance_refresh_ms: u64,
    allocation_profile_interval_ms: u64,
) {
    // Clamp intervals to their respective minimums.
    let memory_interval = Duration::from_millis(performance_refresh_ms.max(PERF_POLL_MIN_MS));
    let alloc_interval =
        Duration::from_millis(allocation_profile_interval_ms.max(ALLOC_PROFILE_POLL_MIN_MS));

    // Create the shutdown channel outside the task so both ends are available
    // before the task starts running.
    let (perf_shutdown_tx, mut perf_shutdown_rx) = tokio::sync::watch::channel(false);
    let perf_shutdown_tx = std::sync::Arc::new(perf_shutdown_tx);

    // The JoinHandle from `tokio::spawn` is only available after the call, but
    // the task will send it in `VmServicePerformanceMonitoringStarted` as the
    // first async operation. We use `Arc<Mutex<Option<>>>` as a rendezvous:
    // - We fill the slot after spawn returns (synchronously, before any await).
    // - The task reads from the slot when it sends the "started" message.
    // Because tokio tasks don't run until the current thread yields (or the
    // runtime schedules them), the slot is guaranteed to be filled before the
    // task's first `.await` point.
    let task_handle_slot: std::sync::Arc<std::sync::Mutex<Option<tokio::task::JoinHandle<()>>>> =
        std::sync::Arc::new(std::sync::Mutex::new(None));
    let task_handle_slot_for_msg = task_handle_slot.clone();

    let join_handle = tokio::spawn(async move {
        // Notify TEA that monitoring has started. The slot is populated
        // synchronously by the caller before this first `.await` runs.
        if msg_tx
            .send(Message::VmServicePerformanceMonitoringStarted {
                session_id,
                perf_shutdown_tx,
                perf_task_handle: task_handle_slot_for_msg,
            })
            .await
            .is_err()
        {
            // Channel closed — engine is shutting down.
            return;
        }

        let mut memory_tick = tokio::time::interval(memory_interval);
        let mut alloc_tick = tokio::time::interval(alloc_interval);

        loop {
            tokio::select! {
                _ = memory_tick.tick() => {
                    // Fetch the main isolate ID (cached after first call).
                    let isolate_id = match handle.main_isolate_id().await {
                        Ok(id) => id,
                        Err(e) => {
                            tracing::debug!(
                                "Could not get isolate ID for memory polling (session {}): {}",
                                session_id, e
                            );
                            continue;
                        }
                    };

                    // 1. Basic memory snapshot (existing behaviour — populates memory_history).
                    match fdemon_daemon::vm_service::get_memory_usage(&handle, &isolate_id).await {
                        Ok(memory) => {
                            if msg_tx
                                .send(Message::VmServiceMemorySnapshot {
                                    session_id,
                                    memory,
                                })
                                .await
                                .is_err()
                            {
                                // Engine shutting down.
                                break;
                            }
                        }
                        Err(e) => {
                            // Transient errors are expected during hot reload when
                            // the isolate is paused. Log at debug and continue.
                            tracing::debug!(
                                "Memory usage poll failed for session {}: {}",
                                session_id, e
                            );
                            continue;
                        }
                    }

                    // 2. Rich memory sample (new — populates memory_samples ring buffer).
                    //    Shares the same tick as the basic snapshot so both ring buffers
                    //    stay in sync. If `get_memory_sample` fails (e.g. getIsolate
                    //    unavailable), the basic VmServiceMemorySnapshot still succeeded
                    //    above, so the gauge fallback remains functional.
                    if let Some(sample) =
                        fdemon_daemon::vm_service::get_memory_sample(&handle, &isolate_id).await
                    {
                        if msg_tx
                            .send(Message::VmServiceMemorySample { session_id, sample })
                            .await
                            .is_err()
                        {
                            // Engine shutting down.
                            break;
                        }
                    } else {
                        tracing::debug!(
                            "Rich memory sample unavailable for session {} (non-fatal)",
                            session_id
                        );
                    }
                }

                _ = alloc_tick.tick() => {
                    // Allocation profile polling (lower frequency than memory polling).
                    // `getAllocationProfile` is expensive — it forces the VM to walk the
                    // entire Dart heap. Transient failures are silently skipped.
                    let isolate_id = match handle.main_isolate_id().await {
                        Ok(id) => id,
                        Err(e) => {
                            tracing::debug!(
                                "Could not get isolate ID for allocation polling (session {}): {}",
                                session_id, e
                            );
                            continue;
                        }
                    };

                    match fdemon_daemon::vm_service::get_allocation_profile(
                        &handle,
                        &isolate_id,
                        false, // gc=false — no forced GC before profiling
                    )
                    .await
                    {
                        Ok(profile) => {
                            if msg_tx
                                .send(Message::VmServiceAllocationProfileReceived {
                                    session_id,
                                    profile,
                                })
                                .await
                                .is_err()
                            {
                                // Engine shutting down.
                                break;
                            }
                        }
                        Err(e) => {
                            tracing::debug!(
                                "Allocation profile poll failed for session {}: {}",
                                session_id, e
                            );
                        }
                    }
                }

                _ = perf_shutdown_rx.changed() => {
                    if *perf_shutdown_rx.borrow() {
                        info!(
                            "Performance monitoring stopped for session {}",
                            session_id
                        );
                        break;
                    }
                }
            }
        }
    });

    // Synchronously store the JoinHandle in the slot. The task hasn't run yet
    // (tokio tasks don't run until the current thread yields to the runtime),
    // so the slot is populated before the first `.await` inside the task.
    if let Ok(mut slot) = task_handle_slot.lock() {
        *slot = Some(join_handle);
    };
}

/// Spawn a task that connects to the VM Service and forwards events as Messages.
fn spawn_vm_service_connection(
    session_id: SessionId,
    ws_uri: String,
    msg_tx: mpsc::Sender<Message>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let connect_result = tokio::time::timeout(
            std::time::Duration::from_secs(10),
            VmServiceClient::connect(&ws_uri),
        )
        .await;

        let connect_result = match connect_result {
            Ok(result) => result,
            Err(_) => {
                warn!(
                    "VM Service: connection timed out for session {} ({})",
                    session_id, ws_uri
                );
                let _ = msg_tx
                    .send(Message::VmServiceConnectionFailed {
                        session_id,
                        error: "Connection timed out".to_string(),
                    })
                    .await;
                return;
            }
        };

        match connect_result {
            Ok(client) => {
                // Subscribe to Extension and Logging streams
                let stream_errors = client.subscribe_flutter_streams().await;
                for err in &stream_errors {
                    warn!(
                        "VM Service: stream subscription failed for session {}: {}",
                        session_id, err
                    );
                }

                // Best-effort: enable Flutter frame timing event emission.
                // `Flutter.Frame` events may already arrive without this call;
                // this attempts to also enable `profileWidgetBuilds` for build
                // timing detail. Errors are silently ignored (profile mode, etc.).
                if let Ok(isolate_id) = client.main_isolate_id().await {
                    let _ = enable_frame_tracking(&client.request_handle(), &isolate_id).await;
                }

                // Extract the request handle BEFORE entering the forwarding loop.
                // This allows the TEA handler and background tasks to make on-demand
                // RPC calls through the same WebSocket connection without going through
                // the event-forwarding loop.
                let handle = client.request_handle();
                let _ = msg_tx
                    .send(Message::VmServiceHandleReady { session_id, handle })
                    .await;

                // Create shutdown channel — sender goes to the session handle,
                // receiver lets the forwarding loop exit cleanly on AppStop.
                let (vm_shutdown_tx, vm_shutdown_rx) = tokio::sync::watch::channel(false);
                let vm_shutdown_tx = std::sync::Arc::new(vm_shutdown_tx);

                // Attach shutdown sender to the session handle BEFORE notifying
                // about connection so the session can signal shutdown at any time.
                let _ = msg_tx
                    .send(Message::VmServiceAttached {
                        session_id,
                        vm_shutdown_tx,
                    })
                    .await;

                // Notify TEA that the VM Service is connected
                let _ = msg_tx
                    .send(Message::VmServiceConnected { session_id })
                    .await;

                // Forward events from the VM Service to the TEA message loop
                forward_vm_events(client, session_id, msg_tx, vm_shutdown_rx).await;
            }
            Err(e) => {
                warn!(
                    "VM Service: connection failed for session {}: {}",
                    session_id, e
                );
                let _ = msg_tx
                    .send(Message::VmServiceConnectionFailed {
                        session_id,
                        error: e.to_string(),
                    })
                    .await;
            }
        }
    })
}

/// Receive VM Service stream events and translate them into TEA Messages.
///
/// Runs until:
/// - The event receiver closes (client disconnects or is dropped), OR
/// - The shutdown watch channel receives `true` (session stopped/closed)
///
/// Sends `VmServiceDisconnected` when the loop exits.
async fn forward_vm_events(
    mut client: VmServiceClient,
    session_id: SessionId,
    msg_tx: mpsc::Sender<Message>,
    mut vm_shutdown_rx: tokio::sync::watch::Receiver<bool>,
) {
    loop {
        tokio::select! {
            event = client.event_receiver().recv() => {
                match event {
                    Some(event) => {
                        // Try parsing as Flutter.Error (Extension stream) — most critical.
                        if let Some(flutter_error) = parse_flutter_error(&event.params.event) {
                            let log_entry = flutter_error_to_log_entry(&flutter_error);
                            let _ = msg_tx
                                .send(Message::VmServiceFlutterError {
                                    session_id,
                                    log_entry,
                                })
                                .await;
                            continue;
                        }

                        // Try parsing as a Flutter.Frame event (frame timing).
                        // Checked after Flutter.Error because Flutter.Frame events share
                        // the Extension stream and are less critical than crash logs.
                        if let Some(timing) =
                            parse_frame_timing(&event.params.event)
                        {
                            let _ = msg_tx
                                .send(Message::VmServiceFrameTiming {
                                    session_id,
                                    timing,
                                })
                                .await;
                            continue;
                        }

                        // Try parsing as a GC event (GC stream).
                        if let Some(gc_event) = parse_gc_event(&event.params.event) {
                            let _ = msg_tx
                                .send(Message::VmServiceGcEvent {
                                    session_id,
                                    gc_event,
                                })
                                .await;
                            continue;
                        }

                        // Try parsing as a structured LogRecord (Logging stream).
                        if let Some(log_record) = parse_log_record(&event.params.event) {
                            let log_entry = vm_log_to_log_entry(&log_record);
                            let _ = msg_tx
                                .send(Message::VmServiceLogRecord {
                                    session_id,
                                    log_entry,
                                })
                                .await;
                            continue;
                        }

                        // Other event kinds (Isolate, Timeline, etc.) are intentionally ignored
                    }
                    None => {
                        // Event receiver closed — client disconnected
                        info!("VM Service event stream ended for session {}", session_id);
                        break;
                    }
                }
            }
            _ = vm_shutdown_rx.changed() => {
                if *vm_shutdown_rx.borrow() {
                    info!("VM Service shutdown signal received for session {}", session_id);
                    client.disconnect().await;
                    break;
                }
            }
        }
    }

    let _ = msg_tx
        .send(Message::VmServiceDisconnected { session_id })
        .await;
}

/// Spawn a background task that fetches the root widget tree via VM Service.
///
/// Uses `ext.flutter.inspector.getRootWidgetTree` (with automatic fallback to
/// `getRootWidgetSummaryTree` for older Flutter versions). An object group
/// named `"fdemon-inspector-1"` is created to scope the returned `valueId`
/// references.
///
/// When `tree_max_depth` is non-zero, the depth is passed as `subtreeDepth`
/// to the RPC call (if supported by the Flutter extension). If the parameter
/// is not supported, the tree is returned at unlimited depth.
///
/// Sends `Message::WidgetTreeFetched` on success or
/// `Message::WidgetTreeFetchFailed` on failure.
fn spawn_fetch_widget_tree(
    session_id: SessionId,
    handle: VmRequestHandle,
    msg_tx: mpsc::Sender<Message>,
    tree_max_depth: u32,
) {
    tokio::spawn(async move {
        let isolate_id = match handle.main_isolate_id().await {
            Ok(id) => id,
            Err(e) => {
                tracing::warn!(
                    "FetchWidgetTree: could not get isolate ID for session {}: {}",
                    session_id,
                    e
                );
                let _ = msg_tx
                    .send(Message::WidgetTreeFetchFailed {
                        session_id,
                        error: format!("Could not get isolate ID: {e}"),
                    })
                    .await;
                return;
            }
        };

        // Use a fixed object group name for the initial tree fetch.
        // A persistent ObjectGroupManager would be needed for multi-fetch
        // workflows; for the initial inspector view one group is sufficient.
        let object_group = "fdemon-inspector-1";

        // Dispose the previous object group before creating a new one.
        // This releases VM references from any prior tree fetch and prevents
        // memory from accumulating on the Flutter VM during repeated refreshes.
        // `disposeGroup` is idempotent — safe to call even on the first fetch.
        // Failure is non-fatal: log at debug level and continue with the fetch.
        {
            let mut dispose_args = HashMap::new();
            dispose_args.insert("objectGroup".to_string(), object_group.to_string());
            if let Err(e) = handle
                .call_extension(ext::DISPOSE_GROUP, &isolate_id, Some(dispose_args))
                .await
            {
                tracing::debug!(
                    "FetchWidgetTree: disposeGroup '{}' failed for session {} (non-fatal): {}",
                    object_group,
                    session_id,
                    e
                );
            }
        }

        // Build args for the newer getRootWidgetTree API.
        let mut newer_args = HashMap::new();
        newer_args.insert("objectGroup".to_string(), object_group.to_string());
        newer_args.insert("isSummaryTree".to_string(), "true".to_string());
        newer_args.insert("withPreviews".to_string(), "false".to_string());
        // Pass subtreeDepth when a limit is configured (0 = unlimited).
        if tree_max_depth > 0 {
            newer_args.insert("subtreeDepth".to_string(), tree_max_depth.to_string());
        }

        // Wrap the entire RPC call sequence in a 10-second timeout so that a
        // hung or very slow VM Service does not leave the UI in a permanent
        // loading state.
        const FETCH_TIMEOUT: Duration = Duration::from_secs(10);

        let fetch_result = tokio::time::timeout(FETCH_TIMEOUT, async {
            match handle
                .call_extension(ext::GET_ROOT_WIDGET_TREE, &isolate_id, Some(newer_args))
                .await
            {
                Ok(value) => parse_diagnostics_node_response(&value),
                Err(e) => {
                    // If the newer API is not registered, fall back to the older one.
                    if matches!(&e, fdemon_core::Error::Protocol { .. }) {
                        tracing::debug!(
                            "getRootWidgetTree not available for session {}, \
                             falling back to getRootWidgetSummaryTree: {}",
                            session_id,
                            e
                        );
                        let mut older_args = HashMap::new();
                        older_args.insert("objectGroup".to_string(), object_group.to_string());
                        match handle
                            .call_extension(
                                ext::GET_ROOT_WIDGET_SUMMARY_TREE,
                                &isolate_id,
                                Some(older_args),
                            )
                            .await
                        {
                            Ok(value) => parse_diagnostics_node_response(&value),
                            Err(fallback_err) => Err(fallback_err),
                        }
                    } else {
                        Err(e)
                    }
                }
            }
        })
        .await;

        let msg = match fetch_result {
            Err(_timeout) => {
                // 10-second deadline exceeded.
                tracing::warn!(
                    "FetchWidgetTree timed out after 10s for session {}",
                    session_id
                );
                Message::WidgetTreeFetchTimeout { session_id }
            }
            Ok(Ok(root)) => Message::WidgetTreeFetched {
                session_id,
                root: Box::new(root),
            },
            Ok(Err(e)) => {
                tracing::warn!("FetchWidgetTree failed for session {}: {}", session_id, e);
                Message::WidgetTreeFetchFailed {
                    session_id,
                    error: e.to_string(),
                }
            }
        };
        let _ = msg_tx.send(msg).await;
    });
}

/// Spawn a background task that flips a debug overlay extension via VM Service.
///
/// Reads the current boolean state with one RPC call, then sets the opposite
/// state with a second RPC call (matching the `flip_overlay` pattern but using
/// `VmRequestHandle` instead of `VmServiceClient`).
///
/// Sends `Message::DebugOverlayToggled` on success (including profile-mode
/// failures where the extension is not available — which are silently logged).
fn spawn_toggle_overlay(
    session_id: SessionId,
    extension: DebugOverlayKind,
    handle: VmRequestHandle,
    msg_tx: mpsc::Sender<Message>,
) {
    tokio::spawn(async move {
        let isolate_id = match handle.main_isolate_id().await {
            Ok(id) => id,
            Err(e) => {
                tracing::warn!(
                    "ToggleOverlay: could not get isolate ID for session {}: {}",
                    session_id,
                    e
                );
                // No message sent — the overlay state is unchanged.
                return;
            }
        };

        let method = match extension {
            DebugOverlayKind::RepaintRainbow => ext::REPAINT_RAINBOW,
            DebugOverlayKind::DebugPaint => ext::DEBUG_PAINT,
            DebugOverlayKind::PerformanceOverlay => ext::SHOW_PERFORMANCE_OVERLAY,
        };

        // Step 1: read the current state.
        let current = match handle.call_extension(method, &isolate_id, None).await {
            Ok(value) => match parse_bool_extension_response(&value) {
                Ok(v) => v,
                Err(e) => {
                    tracing::warn!(
                        "ToggleOverlay: failed to parse current state for {:?} \
                         (session {}): {}",
                        extension,
                        session_id,
                        e
                    );
                    return;
                }
            },
            Err(e) => {
                // Extension not available (e.g., profile/release build) — log and ignore.
                tracing::debug!(
                    "ToggleOverlay: extension {:?} not available for session {}: {}",
                    extension,
                    session_id,
                    e
                );
                return;
            }
        };

        // Step 2: set the opposite state.
        let mut args = HashMap::new();
        args.insert("enabled".to_string(), (!current).to_string());
        let new_state = match handle.call_extension(method, &isolate_id, Some(args)).await {
            Ok(value) => match parse_bool_extension_response(&value) {
                Ok(v) => v,
                Err(e) => {
                    tracing::warn!(
                        "ToggleOverlay: failed to parse new state for {:?} \
                         (session {}): {}",
                        extension,
                        session_id,
                        e
                    );
                    return;
                }
            },
            Err(e) => {
                tracing::warn!(
                    "ToggleOverlay: failed to set state for {:?} (session {}): {}",
                    extension,
                    session_id,
                    e
                );
                return;
            }
        };

        let _ = msg_tx
            .send(Message::DebugOverlayToggled {
                extension,
                enabled: new_state,
            })
            .await;
    });
}

/// Spawn a background task that fetches layout data for a widget node via VM Service.
///
/// Uses `ext.flutter.inspector.getLayoutExplorerNode` to retrieve the layout
/// properties (constraints, size, flex factor, flex fit) for the widget
/// identified by `node_id` (the `valueId` from a previously fetched
/// `DiagnosticsNode`).
///
/// Sends `Message::LayoutDataFetched` on success or
/// `Message::LayoutDataFetchFailed` on failure.
fn spawn_fetch_layout_data(
    session_id: SessionId,
    node_id: String,
    handle: VmRequestHandle,
    msg_tx: mpsc::Sender<Message>,
) {
    tokio::spawn(async move {
        let isolate_id = match handle.main_isolate_id().await {
            Ok(id) => id,
            Err(e) => {
                tracing::warn!(
                    "FetchLayoutData: could not get isolate ID for session {}: {}",
                    session_id,
                    e
                );
                let _ = msg_tx
                    .send(Message::LayoutDataFetchFailed {
                        session_id,
                        error: format!("Could not get isolate ID: {e}"),
                    })
                    .await;
                return;
            }
        };

        // Use a dedicated object group for the layout explorer.
        let layout_group = "devtools-layout";

        // Dispose the previous layout object group before creating a new one.
        // This releases VM references from any prior layout fetch and prevents
        // memory from accumulating on the Flutter VM during repeated refreshes.
        // `disposeGroup` is idempotent — safe to call even on the first fetch.
        // Failure is non-fatal: log at debug level and continue with the fetch.
        {
            let mut dispose_args = HashMap::new();
            dispose_args.insert("objectGroup".to_string(), layout_group.to_string());
            if let Err(e) = handle
                .call_extension(ext::DISPOSE_GROUP, &isolate_id, Some(dispose_args))
                .await
            {
                tracing::debug!(
                    "FetchLayoutData: disposeGroup '{}' failed for session {} (non-fatal): {}",
                    layout_group,
                    session_id,
                    e
                );
            }
        }

        let mut args = HashMap::new();
        // NOTE: Layout explorer uses "id" and "groupName", not "arg" and "objectGroup".
        args.insert("id".to_string(), node_id.clone());
        args.insert("groupName".to_string(), layout_group.to_string());
        args.insert("subtreeDepth".to_string(), "1".to_string());

        // Wrap the RPC call in a 10-second timeout so that a hung or slow VM
        // does not leave the Layout panel in a permanent loading state.
        const LAYOUT_FETCH_TIMEOUT: Duration = Duration::from_secs(10);

        let fetch_result = tokio::time::timeout(LAYOUT_FETCH_TIMEOUT, async {
            handle
                .call_extension(ext::GET_LAYOUT_EXPLORER_NODE, &isolate_id, Some(args))
                .await
        })
        .await;

        let raw_result = match fetch_result {
            Err(_timeout) => {
                tracing::warn!(
                    "FetchLayoutData timed out after 10s for session {}",
                    session_id
                );
                let _ = msg_tx
                    .send(Message::LayoutDataFetchTimeout { session_id })
                    .await;
                return;
            }
            Ok(Ok(v)) => v,
            Ok(Err(e)) => {
                tracing::warn!(
                    "FetchLayoutData: extension call failed for session {}: {}",
                    session_id,
                    e
                );
                let _ = msg_tx
                    .send(Message::LayoutDataFetchFailed {
                        session_id,
                        error: e.to_string(),
                    })
                    .await;
                return;
            }
        };

        // Parse the DiagnosticsNode and extract LayoutInfo.
        let node_value = raw_result.get("result").unwrap_or(&raw_result);
        let layout =
            match serde_json::from_value::<fdemon_core::DiagnosticsNode>(node_value.clone()) {
                Ok(node) => extract_layout_info(&node, node_value),
                Err(e) => {
                    tracing::warn!(
                        "FetchLayoutData: failed to parse layout node for session {}: {}",
                        session_id,
                        e
                    );
                    let _ = msg_tx
                        .send(Message::LayoutDataFetchFailed {
                            session_id,
                            error: format!("Failed to parse layout data: {e}"),
                        })
                        .await;
                    return;
                }
            };

        let _ = msg_tx
            .send(Message::LayoutDataFetched {
                session_id,
                layout: Box::new(layout),
            })
            .await;
    });
}

/// Spawn a background task that disposes both DevTools VM object groups.
///
/// Disposes `"fdemon-inspector-1"` (widget inspector) and `"devtools-layout"`
/// (layout explorer) groups. Called when the user exits DevTools mode to release
/// VM references held by the Flutter inspector and prevent memory accumulation
/// during long debugging sessions.
///
/// Both disposal calls are fire-and-forget: failures are logged at debug level
/// and do not surface to the UI. `disposeGroup` is idempotent, so calling it
/// when a group does not exist is also safe.
fn spawn_dispose_devtools_groups(session_id: SessionId, handle: VmRequestHandle) {
    tokio::spawn(async move {
        let isolate_id = match handle.main_isolate_id().await {
            Ok(id) => id,
            Err(e) => {
                tracing::debug!(
                    "DisposeDevToolsGroups: could not get isolate ID for session {} \
                     (non-fatal, VM may have disconnected): {}",
                    session_id,
                    e
                );
                return;
            }
        };

        for group in &["fdemon-inspector-1", "devtools-layout"] {
            let mut args = HashMap::new();
            args.insert("objectGroup".to_string(), (*group).to_string());
            if let Err(e) = handle
                .call_extension(ext::DISPOSE_GROUP, &isolate_id, Some(args))
                .await
            {
                tracing::debug!(
                    "DisposeDevToolsGroups: disposeGroup '{}' failed for session {} \
                     (non-fatal): {}",
                    group,
                    session_id,
                    e
                );
            } else {
                tracing::debug!(
                    "DisposeDevToolsGroups: disposed '{}' for session {}",
                    group,
                    session_id
                );
            }
        }
    });
}

// ─────────────────────────────────────────────────────────────────────────────
// Helper: Browser launcher
// ─────────────────────────────────────────────────────────────────────────────

/// Open a URL in the system browser (cross-platform, fire-and-forget).
///
/// If `browser` is non-empty, uses it as the browser command.
/// Otherwise uses the platform-default browser opener.
///
/// Called from the `handle_action` dispatch for
/// [`UpdateAction::OpenBrowserDevTools`].
fn open_url_in_browser(url: &str, browser: &str) -> std::io::Result<()> {
    use std::process::Command;

    if !browser.is_empty() {
        // Custom browser specified in settings.
        Command::new(browser).arg(url).spawn()?;
        return Ok(());
    }

    // Platform-default browser.
    #[cfg(target_os = "macos")]
    {
        Command::new("open").arg(url).spawn()?;
        return Ok(());
    }

    #[cfg(target_os = "linux")]
    {
        Command::new("xdg-open").arg(url).spawn()?;
        return Ok(());
    }

    #[cfg(target_os = "windows")]
    {
        Command::new("cmd").args(["/C", "start", "", url]).spawn()?;
        return Ok(());
    }

    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
        return Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "no browser opener available for this platform",
        ));
    }

    #[allow(unreachable_code)]
    Ok(())
}
