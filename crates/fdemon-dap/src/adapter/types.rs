//! # Shared Types, Constants, and Helper Functions
//!
//! This module contains the supporting types used throughout the DAP adapter:
//! step modes, breakpoint results, error types, debug events, pause reasons,
//! numeric error codes, capacity constants, and small conversion helpers.

// ─────────────────────────────────────────────────────────────────────────────
// Supporting types
// ─────────────────────────────────────────────────────────────────────────────

/// Step mode for resume operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepMode {
    /// Step over the current statement (stay in the same function).
    Over,
    /// Step into a function call.
    Into,
    /// Step out of the current function.
    Out,
}

/// Result from adding a breakpoint via the VM Service.
#[derive(Debug, Clone)]
pub struct BreakpointResult {
    /// The VM Service breakpoint ID.
    pub vm_id: String,
    /// Whether the breakpoint has been resolved to source.
    pub resolved: bool,
    /// The actual line the breakpoint was placed on (may differ from requested).
    pub line: Option<i32>,
    /// The actual column (if supported).
    pub column: Option<i32>,
}

/// Errors returned by [`DebugBackend`](crate::adapter::backend::DebugBackend) implementations.
#[derive(Debug, Clone, thiserror::Error)]
pub enum BackendError {
    /// The requested isolate was not found or is no longer running.
    #[error("isolate not found: {0}")]
    IsolateNotFound(String),

    /// A VM Service RPC call failed.
    #[error("VM Service error: {0}")]
    VmServiceError(String),

    /// The backend is not connected to a VM Service.
    #[error("not connected")]
    NotConnected,

    /// The operation is not supported by this backend.
    #[error("not supported: {0}")]
    NotSupported(String),
}

/// Exception pause mode as specified in DAP `setExceptionBreakpoints`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DapExceptionPauseMode {
    /// Pause on all exceptions (caught and uncaught).
    All,
    /// Pause only on uncaught exceptions.
    Unhandled,
    /// Do not pause on exceptions.
    #[default]
    None,
}

// ─────────────────────────────────────────────────────────────────────────────
// Debug events
// ─────────────────────────────────────────────────────────────────────────────

/// Debug events forwarded from the Engine to the adapter.
///
/// These are translated from Dart VM Service stream events and sent to the
/// adapter via the event channel. The adapter converts them to DAP events and
/// forwards them to the client.
#[derive(Debug, Clone)]
pub enum DebugEvent {
    /// An isolate paused (e.g., at a breakpoint, step, or exception).
    Paused {
        /// Dart VM isolate ID (e.g., `"isolates/12345"`).
        isolate_id: String,
        /// Why the isolate paused.
        reason: PauseReason,
        /// The VM Service breakpoint ID that triggered the pause, if the pause
        /// reason is [`PauseReason::Breakpoint`]. Used by the adapter to look
        /// up the breakpoint's condition and hit-condition.
        ///
        /// `None` for non-breakpoint pauses (exceptions, steps, interrupts).
        breakpoint_id: Option<String>,
    },
    /// An isolate resumed execution.
    Resumed {
        /// Dart VM isolate ID.
        isolate_id: String,
    },
    /// A new isolate started (e.g., an isolate spawned by the Flutter app).
    IsolateStart {
        /// Dart VM isolate ID.
        isolate_id: String,
        /// Human-readable name of the isolate.
        name: String,
    },
    /// An isolate exited.
    IsolateExit {
        /// Dart VM isolate ID.
        isolate_id: String,
    },
    /// An isolate became runnable (fully initialized and ready for breakpoints).
    ///
    /// This is the correct trigger for re-applying breakpoints after a hot
    /// restart. The isolate must be fully initialized before breakpoints can
    /// be set. On hot restart, the sequence is:
    ///
    /// 1. `IsolateExit` (old isolate) — clear active breakpoints
    /// 2. `IsolateStart` (new isolate) — register thread
    /// 3. `IsolateRunnable` (new isolate) — re-apply all desired breakpoints
    IsolateRunnable {
        /// Dart VM isolate ID.
        isolate_id: String,
    },
    /// A breakpoint was resolved to a specific source location by the VM.
    BreakpointResolved {
        /// The VM Service breakpoint ID.
        vm_breakpoint_id: String,
        /// The resolved source line (1-based).
        line: Option<i32>,
        /// The resolved source column (1-based), if applicable.
        column: Option<i32>,
    },
    /// The Flutter app process exited.
    AppExited {
        /// The process exit code, if available.
        exit_code: Option<i64>,
    },
    /// A Flutter application log message to forward to the debug console.
    ///
    /// The `level` field uses a lowercase string representation of the log
    /// level (e.g., `"error"`, `"warning"`, `"info"`, `"debug"`) to keep
    /// the `DebugEvent` enum independent of fdemon-core's `LogLevel` type.
    /// Use [`log_level_to_category`] to map it to a DAP output category.
    LogOutput {
        /// The log message text.
        message: String,
        /// Log level as a lowercase string (`"error"`, `"warning"`, `"info"`, `"debug"`).
        level: String,
        /// Optional source file URI (`"file:///path/to/file.dart"`).
        source_uri: Option<String>,
        /// Optional source line number (1-based).
        line: Option<i32>,
    },

    /// The Flutter app has fully started and is ready for interaction.
    ///
    /// Triggers the `flutter.appStarted` custom DAP event. This variant is
    /// emitted by the Engine integration layer when the session phase
    /// transitions to `Running`.
    AppStarted,
}

/// Reason for a pause event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PauseReason {
    /// Paused at a breakpoint.
    Breakpoint,
    /// Paused due to an exception.
    Exception,
    /// Paused after a step operation.
    Step,
    /// Paused by a `pause` request (user-initiated).
    Interrupted,
    /// Paused at isolate entry (before any user code).
    Entry,
    /// Paused at isolate exit.
    Exit,
}

// ─────────────────────────────────────────────────────────────────────────────
// Log level → DAP output category mapping
// ─────────────────────────────────────────────────────────────────────────────

/// Map a log level string to a DAP output event `category` field.
///
/// | Log level      | DAP category | Notes                                    |
/// |----------------|--------------|------------------------------------------|
/// | `"error"`      | `"stderr"`   | Red/highlighted in most IDE consoles     |
/// | `"info"`       | `"stdout"`   | Standard application output              |
/// | `"warning"`    | `"console"`  | Informational messages                   |
/// | `"debug"`      | `"console"`  | Debug-level messages                     |
/// | anything else  | `"console"`  | Fallback (verbose, unknown, etc.)        |
///
/// The `"telemetry"` category is intentionally not used — it is hidden by
/// most IDEs and is reserved for machine-readable telemetry data.
pub fn log_level_to_category(level: &str) -> &'static str {
    match level {
        "error" => "stderr",
        "info" => "stdout",
        _ => "console",
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Capacity constant
// ─────────────────────────────────────────────────────────────────────────────

/// Capacity of the event channel from the Engine to the adapter.
///
/// Bounded to prevent unbounded memory growth if the session writer falls
/// behind. 64 events is sufficient for typical debugging workloads; the
/// Engine will block if the channel fills (which indicates a slow writer).
pub(crate) const EVENT_CHANNEL_CAPACITY: usize = 64;

// ─────────────────────────────────────────────────────────────────────────────
// Rate limiting and timeout constants
// ─────────────────────────────────────────────────────────────────────────────

/// Maximum number of variable children returned per `variables` request.
///
/// Prevents the IDE from fetching the entire object graph when a collection
/// has thousands of elements (e.g., a 10,000-element `List`). IDEs that
/// support DAP paging use the `start`/`count` fields to fetch additional pages.
pub(crate) const MAX_VARIABLES_PER_REQUEST: usize = 100;

/// Timeout for individual backend requests (VM Service RPC calls).
///
/// If a VM Service call does not return within this duration the adapter
/// returns an error response rather than hanging indefinitely. Slow devices
/// may require a longer timeout — future work can expose this via `DapSettings`.
///
/// Currently documented here; active wrapping of backend calls with this timeout
/// is deferred to integration once tokio::time::timeout is wired in.
#[allow(dead_code)]
pub(crate) const REQUEST_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

/// Numeric error code: VM Service not connected.
#[allow(dead_code)]
pub(crate) const ERR_NOT_CONNECTED: i64 = 1000;

/// Numeric error code: no active debug session (no paused isolate).
#[allow(dead_code)]
pub(crate) const ERR_NO_DEBUG_SESSION: i64 = 1001;

/// Numeric error code: thread / isolate not found.
#[allow(dead_code)]
pub(crate) const ERR_THREAD_NOT_FOUND: i64 = 1002;

/// Numeric error code: evaluation failed.
#[allow(dead_code)]
pub(crate) const ERR_EVAL_FAILED: i64 = 1003;

/// Numeric error code: backend request timed out.
#[allow(dead_code)]
pub(crate) const ERR_TIMEOUT: i64 = 1004;

/// Numeric error code: VM Service disconnected (app exited mid-session).
pub(crate) const ERR_VM_DISCONNECTED: i64 = 1005;
