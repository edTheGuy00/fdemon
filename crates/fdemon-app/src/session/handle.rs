//! Session handle — Flutter process control for a session.

use std::sync::Arc;

use fdemon_core::AppPhase;
use fdemon_daemon::{vm_service::VmRequestHandle, CommandSender, FlutterProcess, RequestTracker};

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
