//! # DAP Transport Abstraction
//!
//! Defines the transport modes for the DAP server. The two supported modes are:
//!
//! - **TCP** — Listen on a TCP port for client connections (multi-client, long-lived server).
//! - **Stdio** — Use stdin/stdout for a single client connection (adapter subprocess mode).
//!
//! ## Why both modes?
//!
//! | IDE / tool    | Default transport    | Notes                                      |
//! |---------------|----------------------|--------------------------------------------|
//! | Zed           | stdio                | Spawns adapter as child process            |
//! | Helix         | stdio or TCP         | stdio is preferred; TCP via `transport`    |
//! | nvim-dap      | stdio or TCP         | Defaults to stdio                          |
//! | VS Code       | TCP                  | Separate adapter process with TCP fallback |
//!
//! TCP mode supports multiple concurrent sessions and is suitable for VS Code integration.
//! Stdio mode is preferred by Zed, Helix, and nvim-dap — each IDE launches fdemon as a
//! child process and communicates over the process's stdin/stdout pipes.

pub mod stdio;
pub mod tcp;

// ─────────────────────────────────────────────────────────────────────────────
// Transport mode
// ─────────────────────────────────────────────────────────────────────────────

/// Transport mode for the DAP server.
///
/// Controls how the DAP server accepts client connections:
/// - [`TransportMode::Tcp`] binds a TCP listener and accepts multiple clients.
/// - [`TransportMode::Stdio`] serves exactly one session over stdin/stdout.
///
/// These modes are mutually exclusive — stdio mode owns the process's
/// stdin/stdout pipes for the DAP wire protocol, making TUI mode and other
/// stdin/stdout users incompatible.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransportMode {
    /// Listen on a TCP port for client connections.
    ///
    /// Use `port = 0` to let the OS assign an ephemeral port. The actual bound
    /// port can be retrieved from [`DapServerHandle::port()`] after startup.
    Tcp {
        /// TCP port to bind on. Use `0` for OS-assigned ephemeral port.
        port: u16,
        /// Bind address string. Use `"127.0.0.1"` for local-only (recommended).
        bind_address: String,
    },

    /// Use stdin/stdout for a single client (adapter subprocess mode).
    ///
    /// This is the transport used when fdemon is launched as a DAP adapter
    /// subprocess by an IDE (Zed, Helix, nvim-dap). The process exits when
    /// the DAP client disconnects.
    ///
    /// **Important**: When this mode is active, all non-DAP output (tracing,
    /// status messages, etc.) must be routed to stderr to avoid corrupting
    /// the DAP wire protocol.
    Stdio,
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transport_mode_tcp_fields() {
        let mode = TransportMode::Tcp {
            port: 4711,
            bind_address: "127.0.0.1".to_string(),
        };
        match mode {
            TransportMode::Tcp { port, bind_address } => {
                assert_eq!(port, 4711);
                assert_eq!(bind_address, "127.0.0.1");
            }
            TransportMode::Stdio => panic!("Expected Tcp variant"),
        }
    }

    #[test]
    fn test_transport_mode_stdio() {
        let mode = TransportMode::Stdio;
        assert_eq!(mode, TransportMode::Stdio);
    }

    #[test]
    fn test_transport_mode_tcp_port_zero_is_valid() {
        let mode = TransportMode::Tcp {
            port: 0,
            bind_address: "127.0.0.1".to_string(),
        };
        assert!(matches!(mode, TransportMode::Tcp { port: 0, .. }));
    }

    #[test]
    fn test_transport_mode_clone() {
        let mode = TransportMode::Tcp {
            port: 4711,
            bind_address: "127.0.0.1".to_string(),
        };
        let cloned = mode.clone();
        assert_eq!(mode, cloned);
    }

    #[test]
    fn test_transport_mode_stdio_clone() {
        let mode = TransportMode::Stdio;
        let cloned = mode.clone();
        assert_eq!(mode, cloned);
    }
}
