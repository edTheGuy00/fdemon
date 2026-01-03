//! Domain event definitions

/// Events from the Flutter daemon process
#[derive(Debug, Clone)]
pub enum DaemonEvent {
    /// Raw stdout line from daemon (JSON-RPC wrapped)
    Stdout(String),

    /// Stderr output (usually errors/warnings)
    Stderr(String),

    /// Daemon process has exited
    Exited { code: Option<i32> },

    /// Process spawn failed
    SpawnFailed { reason: String },
}
