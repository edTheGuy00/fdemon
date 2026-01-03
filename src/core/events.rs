//! Domain event definitions

use crate::daemon::DaemonMessage;

/// Events from the Flutter daemon process
#[derive(Debug, Clone)]
pub enum DaemonEvent {
    /// Raw stdout line from daemon (JSON-RPC wrapped)
    Stdout(String),

    /// Parsed daemon message
    Message(DaemonMessage),

    /// Stderr output (usually errors/warnings)
    Stderr(String),

    /// Daemon process has exited
    Exited { code: Option<i32> },

    /// Process spawn failed
    SpawnFailed { reason: String },
}
