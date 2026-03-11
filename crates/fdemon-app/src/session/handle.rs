//! Session handle — Flutter process control for a session.

use std::sync::Arc;

use fdemon_core::AppPhase;
use fdemon_daemon::{vm_service::VmRequestHandle, CommandSender, FlutterProcess, RequestTracker};

use super::native_tags::NativeTagState;
use super::session::Session;

/// Handle for controlling a session's Flutter process
pub struct SessionHandle {
    /// The session state
    pub session: Session,

    /// The Flutter process (if running)
    pub process: Option<FlutterProcess>,

    /// Command sender for this session
    pub cmd_sender: Option<CommandSender>,

    /// Request tracker for response matching
    pub request_tracker: Arc<RequestTracker>,

    /// Shutdown sender for the VM Service event forwarding task.
    ///
    /// Sending `true` signals the forwarding task to disconnect and stop.
    /// Stored as `Arc` because the `Message` enum requires `Clone`.
    pub vm_shutdown_tx: Option<std::sync::Arc<tokio::sync::watch::Sender<bool>>>,

    /// VM Service request handle for on-demand RPC calls.
    ///
    /// Set when the VM Service connects (via `VmServiceHandleReady` message),
    /// cleared on disconnect. Use this to issue JSON-RPC requests from outside
    /// the event forwarding loop (e.g. periodic memory polling).
    pub vm_request_handle: Option<VmRequestHandle>,

    /// Shutdown sender for the performance monitoring polling task.
    ///
    /// Sending `true` stops the polling loop cleanly. Stored as `Arc` because
    /// the `Message` enum (which carries the initial sender) requires `Clone`.
    /// Set by `VmServicePerformanceMonitoringStarted`, cleared on disconnect.
    pub perf_shutdown_tx: Option<std::sync::Arc<tokio::sync::watch::Sender<bool>>>,

    /// JoinHandle for the performance monitoring polling task.
    ///
    /// Aborted on session close or VM disconnect to prevent zombie polling
    /// tasks from continuing after the session has ended. Set by
    /// `VmServicePerformanceMonitoringStarted`, cleared on disconnect/close.
    pub perf_task_handle: Option<tokio::task::JoinHandle<()>>,

    /// Shutdown sender for the network monitoring polling task.
    ///
    /// Sending `true` stops the network polling loop cleanly. Stored as `Arc`
    /// because the `Message` enum (which carries the initial sender) requires
    /// `Clone`. Set by `VmServiceNetworkMonitoringStarted`, cleared on
    /// disconnect.
    pub network_shutdown_tx: Option<std::sync::Arc<tokio::sync::watch::Sender<bool>>>,

    /// JoinHandle for the network monitoring polling task.
    ///
    /// Aborted on session close or VM disconnect to prevent zombie polling
    /// tasks from continuing after the session has ended. Set by
    /// `VmServiceNetworkMonitoringStarted`, cleared on disconnect/close.
    pub network_task_handle: Option<tokio::task::JoinHandle<()>>,

    /// Shutdown signal for the debug event monitoring task.
    ///
    /// Sending `true` signals the debug event forwarding task to stop.
    /// Stored as `Arc` because the `Message` enum requires `Clone`.
    /// Initialized to `None`; set in Phase 2 when the DAP server starts a
    /// per-session debug task.
    pub debug_shutdown_tx: Option<std::sync::Arc<tokio::sync::watch::Sender<bool>>>,

    /// Handle for the debug event monitoring task.
    ///
    /// Aborted on session close or DAP client disconnect to prevent zombie
    /// tasks. Initialized to `None`; set in Phase 2 when the DAP server
    /// spawns the per-session debug event forwarding task.
    pub debug_task_handle: Option<tokio::task::JoinHandle<()>>,

    /// Shutdown sender for the native platform log capture task.
    ///
    /// Sending `true` signals the native log capture forwarding task to stop.
    /// Stored as `Arc` because the `Message` enum requires `Clone`.
    /// Set by `NativeLogCaptureStarted`, cleared on session stop or capture exit.
    pub native_log_shutdown_tx: Option<std::sync::Arc<tokio::sync::watch::Sender<bool>>>,

    /// JoinHandle for the native log capture forwarding task.
    ///
    /// Aborted on session close or app stop to prevent zombie capture tasks
    /// from continuing after the session has ended. Set by `NativeLogCaptureStarted`,
    /// cleared on session stop or capture exit.
    pub native_log_task_handle: Option<tokio::task::JoinHandle<()>>,

    /// Per-session native log tag discovery and visibility state.
    ///
    /// Tracks every distinct tag seen in this session's native log stream
    /// and allows the user to toggle individual tags on/off via the tag
    /// filter UI. Reset to default when the session is stopped or restarted.
    pub native_tag_state: NativeTagState,
}

impl std::fmt::Debug for SessionHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SessionHandle")
            .field("session", &self.session)
            .field("has_process", &self.process.is_some())
            .field("has_cmd_sender", &self.cmd_sender.is_some())
            .field("has_vm_shutdown", &self.vm_shutdown_tx.is_some())
            .field("vm_request_handle", &self.vm_request_handle)
            .field("has_perf_shutdown", &self.perf_shutdown_tx.is_some())
            .field("has_perf_task", &self.perf_task_handle.is_some())
            .field("has_network_shutdown", &self.network_shutdown_tx.is_some())
            .field("has_network_task", &self.network_task_handle.is_some())
            .field("has_debug_shutdown", &self.debug_shutdown_tx.is_some())
            .field("has_debug_task", &self.debug_task_handle.is_some())
            .field(
                "has_native_log_shutdown",
                &self.native_log_shutdown_tx.is_some(),
            )
            .field(
                "has_native_log_task",
                &self.native_log_task_handle.is_some(),
            )
            .field("native_tag_count", &self.native_tag_state.tag_count())
            .finish()
    }
}

impl SessionHandle {
    /// Create a new session handle
    pub fn new(session: Session) -> Self {
        Self {
            session,
            process: None,
            cmd_sender: None,
            request_tracker: Arc::new(RequestTracker::default()),
            vm_shutdown_tx: None,
            vm_request_handle: None,
            perf_shutdown_tx: None,
            perf_task_handle: None,
            network_shutdown_tx: None,
            network_task_handle: None,
            debug_shutdown_tx: None,
            debug_task_handle: None,
            native_log_shutdown_tx: None,
            native_log_task_handle: None,
            native_tag_state: NativeTagState::default(),
        }
    }

    /// Shut down the native platform log capture task (if running).
    ///
    /// Sends `true` on the shutdown channel to signal graceful stop, then
    /// aborts the task as a fallback. Clears both fields on the handle.
    pub fn shutdown_native_logs(&mut self) {
        if let Some(tx) = self.native_log_shutdown_tx.take() {
            let _ = tx.send(true);
            tracing::debug!(
                "Sent native log shutdown signal for session {}",
                self.session.id
            );
        }
        if let Some(handle) = self.native_log_task_handle.take() {
            handle.abort();
        }
    }

    /// Attach a Flutter process to this session
    pub fn attach_process(&mut self, process: FlutterProcess) {
        let sender = process.command_sender(self.request_tracker.clone());
        self.cmd_sender = Some(sender);
        self.process = Some(process);
        self.session.phase = AppPhase::Initializing;
    }

    /// Check if process is running
    pub fn has_process(&self) -> bool {
        self.process.is_some()
    }

    /// Get the app_id if available
    pub fn app_id(&self) -> Option<&str> {
        self.session.app_id.as_deref()
    }
}
