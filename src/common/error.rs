//! Application error types with rich context

use std::path::PathBuf;
use thiserror::Error;

/// Result type alias using our Error type
pub type Result<T> = std::result::Result<T, Error>;

/// Application error types organized by layer/domain
#[derive(Debug, Error)]
pub enum Error {
    // ─────────────────────────────────────────────────────────────
    // Common/Infrastructure Errors
    // ─────────────────────────────────────────────────────────────
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON parsing error: {0}")]
    Json(#[from] serde_json::Error),

    // ─────────────────────────────────────────────────────────────
    // Terminal/TUI Errors
    // ─────────────────────────────────────────────────────────────
    #[error("Terminal error: {message}")]
    Terminal { message: String },

    #[error("Failed to initialize terminal: {0}")]
    TerminalInit(String),

    #[error("Failed to restore terminal: {0}")]
    TerminalRestore(String),

    // ─────────────────────────────────────────────────────────────
    // Flutter/Daemon Errors
    // ─────────────────────────────────────────────────────────────
    #[error("Flutter SDK not found. Ensure 'flutter' is in your PATH.")]
    FlutterNotFound,

    #[error("No Flutter project found in: {path}")]
    NoProject { path: PathBuf },

    #[error("Flutter daemon error: {message}")]
    Daemon { message: String },

    #[error("Flutter process error: {message}")]
    Process { message: String },

    #[error("Failed to spawn Flutter process: {reason}")]
    ProcessSpawn { reason: String },

    #[error("Flutter process exited unexpectedly with code: {code:?}")]
    ProcessExit { code: Option<i32> },

    #[error("Daemon protocol error: {message}")]
    Protocol { message: String },

    // ─────────────────────────────────────────────────────────────
    // Configuration Errors
    // ─────────────────────────────────────────────────────────────
    #[error("Configuration error: {message}")]
    Config { message: String },

    #[error("Configuration file not found: {path}")]
    ConfigNotFound { path: PathBuf },

    #[error("Invalid configuration: {message}")]
    ConfigInvalid { message: String },

    // ─────────────────────────────────────────────────────────────
    // Channel/Communication Errors
    // ─────────────────────────────────────────────────────────────
    #[error("Channel send error: {message}")]
    ChannelSend { message: String },

    #[error("Channel closed unexpectedly")]
    ChannelClosed,
}

// ─────────────────────────────────────────────────────────────────
// Convenience Constructors
// ─────────────────────────────────────────────────────────────────

impl Error {
    pub fn terminal(message: impl Into<String>) -> Self {
        Self::Terminal {
            message: message.into(),
        }
    }

    pub fn daemon(message: impl Into<String>) -> Self {
        Self::Daemon {
            message: message.into(),
        }
    }

    pub fn process(message: impl Into<String>) -> Self {
        Self::Process {
            message: message.into(),
        }
    }

    pub fn protocol(message: impl Into<String>) -> Self {
        Self::Protocol {
            message: message.into(),
        }
    }

    pub fn config(message: impl Into<String>) -> Self {
        Self::Config {
            message: message.into(),
        }
    }

    pub fn channel_send(message: impl Into<String>) -> Self {
        Self::ChannelSend {
            message: message.into(),
        }
    }

    /// Check if this is a recoverable error
    pub fn is_recoverable(&self) -> bool {
        matches!(
            self,
            Error::Daemon { .. } | Error::Protocol { .. } | Error::ChannelSend { .. }
        )
    }

    /// Check if this error should trigger application exit
    pub fn is_fatal(&self) -> bool {
        matches!(
            self,
            Error::FlutterNotFound
                | Error::NoProject { .. }
                | Error::ProcessSpawn { .. }
                | Error::TerminalInit(_)
        )
    }
}

// ─────────────────────────────────────────────────────────────────
// Error Context Extensions (for use with color-eyre)
// ─────────────────────────────────────────────────────────────────

/// Extension trait for adding context to Results
pub trait ResultExt<T> {
    /// Add context to an error
    fn context(self, context: impl Into<String>) -> Result<T>;

    /// Add context with a closure (lazy evaluation)
    fn with_context<F>(self, f: F) -> Result<T>
    where
        F: FnOnce() -> String;
}

impl<T, E: Into<Error>> ResultExt<T> for std::result::Result<T, E> {
    fn context(self, context: impl Into<String>) -> Result<T> {
        self.map_err(|e| {
            let err = e.into();
            tracing::error!("{}: {:?}", context.into(), err);
            err
        })
    }

    fn with_context<F>(self, f: F) -> Result<T>
    where
        F: FnOnce() -> String,
    {
        self.map_err(|e| {
            let err = e.into();
            tracing::error!("{}: {:?}", f(), err);
            err
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display_messages() {
        let err = Error::daemon("Connection lost");
        assert_eq!(err.to_string(), "Flutter daemon error: Connection lost");

        let err = Error::FlutterNotFound;
        assert!(err.to_string().contains("Flutter SDK not found"));
    }

    #[test]
    fn test_error_from_io() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let err: Error = io_err.into();
        assert!(matches!(err, Error::Io(_)));
    }

    #[test]
    fn test_error_is_fatal() {
        assert!(Error::FlutterNotFound.is_fatal());
        assert!(Error::NoProject {
            path: PathBuf::from("/test")
        }
        .is_fatal());
        assert!(!Error::daemon("test").is_fatal());
    }

    #[test]
    fn test_error_is_recoverable() {
        assert!(Error::daemon("test").is_recoverable());
        assert!(Error::protocol("parse error").is_recoverable());
        assert!(!Error::FlutterNotFound.is_recoverable());
    }

    #[test]
    fn test_error_constructors() {
        let _ = Error::terminal("test");
        let _ = Error::daemon("test");
        let _ = Error::process("test");
        let _ = Error::protocol("test");
        let _ = Error::config("test");
        let _ = Error::channel_send("test");
    }
}
