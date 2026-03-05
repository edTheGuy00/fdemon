## Task: Add Stdio Transport for DAP

**Objective**: Add stdin/stdout transport mode to the DAP server so IDEs like Zed and Helix can launch `fdemon` as a DAP adapter subprocess, communicating over stdio instead of TCP.

**Depends on**: None

**Estimated Time**: 3-4 hours

### Scope

- `crates/fdemon-dap/src/transport/` — **NEW** module for transport abstraction
- `crates/fdemon-dap/src/server/mod.rs` — Refactor to support both TCP and stdio
- `crates/fdemon-dap/src/server/session.rs` — Generalize over `AsyncRead`/`AsyncWrite`
- `crates/fdemon-dap/src/lib.rs` — Re-export transport types
- `src/main.rs` (binary crate) — Add `--dap-stdio` CLI flag

### Details

#### Why Stdio Transport?

Both Zed and Helix use **stdio as the default transport** for DAP adapters:
- **Zed**: Spawns the adapter as a child process, communicates via stdin/stdout. TCP is available via `tcp_connection` but stdio is preferred.
- **Helix**: Uses `command` + optional `transport = "stdio"` in `languages.toml`. TCP is available but requires `port-arg` negotiation.
- **nvim-dap**: Also prefers stdio by default.

The current Phase 2 implementation is TCP-only. Adding stdio makes fdemon a first-class adapter for all four major editor targets.

#### Transport Abstraction

Create a transport layer that abstracts over the I/O source:

```rust
// crates/fdemon-dap/src/transport/mod.rs

pub mod stdio;
pub mod tcp;

use tokio::io::{AsyncRead, AsyncWrite};

/// Transport mode for the DAP server.
#[derive(Debug, Clone)]
pub enum TransportMode {
    /// Listen on a TCP port for client connections.
    Tcp { port: u16, bind_address: String },
    /// Use stdin/stdout for a single client (adapter mode).
    Stdio,
}
```

#### Stdio Transport Implementation

```rust
// crates/fdemon-dap/src/transport/stdio.rs

use tokio::io::{stdin, stdout, BufReader, BufWriter};

/// Runs a single DAP session over stdin/stdout.
///
/// This is the entry point for adapter mode. Unlike TCP mode which accepts
/// multiple clients, stdio mode serves exactly one session and exits when
/// the client disconnects.
pub async fn run_stdio_session(
    shutdown_rx: watch::Receiver<bool>,
    event_tx: mpsc::Sender<DapServerEvent>,
) -> Result<()> {
    let reader = BufReader::new(stdin());
    let writer = BufWriter::new(stdout());

    // Emit connected event
    event_tx.send(DapServerEvent::ClientConnected {
        client_id: "stdio".into(),
    }).await.ok();

    // Run session (reuse DapClientSession logic)
    let result = DapClientSession::run_on(reader, writer, shutdown_rx).await;

    // Emit disconnected event
    event_tx.send(DapServerEvent::ClientDisconnected {
        client_id: "stdio".into(),
    }).await.ok();

    result
}
```

#### Generalize `DapClientSession::run`

The current `run` method takes a `TcpStream`. Generalize it to work with any `AsyncRead + AsyncWrite`:

```rust
impl DapClientSession {
    /// Run the session on any async reader/writer pair.
    ///
    /// This is the generalized version used by both TCP and stdio transports.
    pub async fn run_on<R, W>(
        reader: R,
        writer: W,
        mut shutdown_rx: watch::Receiver<bool>,
    ) -> Result<()>
    where
        R: tokio::io::AsyncRead + Unpin + Send,
        W: tokio::io::AsyncWrite + Unpin + Send,
    {
        let mut reader = BufReader::new(reader);
        let mut session = Self::new();

        loop {
            tokio::select! {
                result = read_message(&mut reader) => {
                    // ... same dispatch logic as current run() ...
                }
                _ = shutdown_rx.changed() => {
                    // ... same shutdown logic ...
                }
            }
        }
    }

    /// Run on a TCP stream (convenience wrapper preserving existing API).
    pub async fn run(
        stream: tokio::net::TcpStream,
        shutdown_rx: watch::Receiver<bool>,
    ) -> Result<()> {
        let (reader, writer) = stream.into_split();
        Self::run_on(reader, writer, shutdown_rx).await
    }
}
```

#### Verify `read_message`/`write_message` Generics

The codec functions in `protocol/codec.rs` should already work with generic `AsyncRead`/`AsyncWrite`. Verify that their type bounds are:
```rust
pub async fn read_message<R: AsyncBufRead + Unpin>(reader: &mut R) -> Result<Option<DapMessage>>
pub async fn write_message<W: AsyncWrite + Unpin>(writer: &mut W, message: &DapMessage) -> Result<()>
```

If they currently take concrete `BufReader<OwnedReadHalf>`, generalize the signatures.

#### CLI Integration

Add a `--dap-stdio` flag to the binary crate:

```rust
// src/main.rs or src/cli.rs (wherever CLI args are parsed)

/// Run as a DAP adapter over stdin/stdout (for IDE integration).
#[arg(long, conflicts_with = "dap_port")]
pub dap_stdio: bool,
```

When `--dap-stdio` is set:
1. Do NOT start the TUI (stdin/stdout are used for DAP)
2. Start the Engine in headless mode
3. Run a single stdio DAP session
4. Exit when the DAP client disconnects

#### Important: Stdout Isolation

When running in stdio mode, **all non-DAP output must be suppressed from stdout**. Tracing logs, status messages, and any other output would corrupt the DAP wire protocol. Ensure:

- Tracing subscriber writes to stderr (not stdout) in stdio mode
- No `println!()` calls leak to stdout
- The TUI is disabled (it uses terminal raw mode which conflicts with stdio DAP)

#### DapService Update

Extend `DapService` to support both modes:

```rust
impl DapService {
    pub async fn start_tcp(config: DapServerConfig, event_tx: mpsc::Sender<DapServerEvent>) -> Result<DapServerHandle> {
        // existing TCP implementation
    }

    pub async fn start_stdio(event_tx: mpsc::Sender<DapServerEvent>) -> Result<DapServerHandle> {
        // stdio implementation
    }
}
```

### Acceptance Criteria

1. `fdemon --dap-stdio` runs as a single-session DAP adapter over stdin/stdout
2. `fdemon --dap-stdio` does NOT start the TUI
3. The DAP initialization handshake works over stdio (test with a pipe)
4. All tracing output goes to stderr, not stdout, in stdio mode
5. Existing TCP mode continues to work unchanged
6. `DapClientSession::run_on` is generic over `AsyncRead + AsyncWrite`
7. Both `read_message` and `write_message` accept generic reader/writer types
8. Unit tests verify stdio session lifecycle (connect → initialize → disconnect)

### Testing

```rust
// Test using in-memory byte streams (tokio::io::duplex or similar)
#[tokio::test]
async fn test_stdio_session_initialization() {
    let (client_reader, server_writer) = tokio::io::duplex(8192);
    let (server_reader, client_writer) = tokio::io::duplex(8192);

    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let (event_tx, mut event_rx) = mpsc::channel(16);

    // Spawn the server session
    let handle = tokio::spawn(async move {
        DapClientSession::run_on(server_reader, server_writer, shutdown_rx).await
    });

    // Send initialize from client side
    let mut writer = BufWriter::new(client_writer);
    let init_req = DapMessage::Request(DapRequest {
        seq: 1,
        command: "initialize".into(),
        arguments: None,
    });
    write_message(&mut writer, &init_req).await.unwrap();

    // Read response from server
    let mut reader = BufReader::new(client_reader);
    let response = read_message(&mut reader).await.unwrap().unwrap();
    assert!(matches!(response, DapMessage::Response(r) if r.success));

    // Clean shutdown
    shutdown_tx.send(true).unwrap();
    handle.await.unwrap().unwrap();
}
```

### Notes

- Stdio mode is mutually exclusive with TUI — they both need terminal control
- When `--dap-stdio` is used with `--dap-port`, reject with a clear error message
- The `DapServerEvent::ClientConnected` event uses `client_id: "stdio"` for stdio sessions
- The stdio session exits the process when the client disconnects (single-session mode)
- In the future, stdio mode could coexist with headless Engine mode for a pure "adapter + engine" binary
- Consider adding `--dap-stdio` to the binary's help text with examples for Zed/Helix configuration

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-dap/src/lib.rs` | Added `pub mod transport` and re-export of `TransportMode` |
| `crates/fdemon-dap/src/server/mod.rs` | Changed `DapServerHandle.port` from private to `pub(crate)` to allow stdio handle construction |
| `crates/fdemon-dap/src/server/session.rs` | Added generic `run_on<R, W>` method; refactored `run(TcpStream)` to delegate to `run_on` |
| `crates/fdemon-dap/src/service.rs` | Added `DapService::start_tcp` (renamed from `start`) and `DapService::start_stdio`; kept `start` as backwards-compat alias; added `watch` import |
| `crates/fdemon-dap/src/transport/mod.rs` | NEW — `TransportMode` enum (`Tcp { port, bind_address }` and `Stdio`); tests |
| `crates/fdemon-dap/src/transport/stdio.rs` | NEW — `run_stdio_session` function; comprehensive `run_on` test suite using `tokio::io::duplex` |
| `crates/fdemon-dap/src/transport/tcp.rs` | NEW — thin re-export of `crate::server::start` for symmetric API |
| `Cargo.toml` (workspace root) | Added `fdemon-dap.workspace = true` to binary `[dependencies]` |
| `src/main.rs` | Added `mod dap_stdio`; added `--dap-stdio` CLI flag (conflicts with `--dap-port`); added early-exit handling for `--dap-stdio` mode |
| `src/dap_stdio/mod.rs` | NEW — module header |
| `src/dap_stdio/runner.rs` | NEW — `run_dap_stdio` entry point for `--dap-stdio` mode |

### Notable Decisions/Tradeoffs

1. **`DapClientSession::run_on` generics**: Made generic over `BufReader<R>` + `W` where `R: AsyncRead + Unpin + Send` and `W: AsyncWrite + Unpin + Send`. The `run(TcpStream)` method is preserved as a convenience wrapper that splits the stream, wraps in `BufReader`, then delegates to `run_on`. This satisfies the task requirement with zero breaking changes.

2. **`DapServerHandle.port` visibility**: Changed from private to `pub(crate)` to allow `DapService::start_stdio` to construct a handle with `port: 0`. Consistent with how `shutdown_tx` and `task` are already `pub(crate)`.

3. **Stdio runner does not start the Engine**: Per the task instruction "Focus on the transport layer. Do NOT implement adapter/debugging logic", the `run_dap_stdio` runner only starts the DAP session over stdio and bridges lifecycle events to tracing. Engine/adapter integration is deferred to later tasks (03, 10).

4. **`DapService::start_tcp` + backwards-compat `start` alias**: Added `start_tcp` as the explicit name and preserved `start` as an alias to avoid breaking any callers in the existing codebase.

5. **In-memory duplex streams for tests**: All stdio transport tests use `tokio::io::duplex(8192)` pairs to avoid touching real stdin/stdout in the test harness. This provides full coverage of the session lifecycle without corrupting the test runner's terminal.

### Testing Performed

- `cargo check -p fdemon-dap` — Passed
- `cargo check --workspace` — Passed
- `cargo test -p fdemon-dap` — Passed (123 tests; 35 new tests added by this task)
- `cargo clippy -p fdemon-dap -- -D warnings` — Passed (0 warnings)
- `cargo clippy --workspace -- -D warnings` — Passed (0 warnings)
- `cargo fmt --all -- --check` — Passed (formatting applied and verified clean)

### Risks/Limitations

1. **Real stdin/stdout not exercised in tests**: `run_stdio_session` binds to `tokio::io::stdin()`/`stdout()` which cannot be safely tested in a multi-test harness. The session logic is fully covered via `DapClientSession::run_on` with duplex streams. A separate E2E test with a real subprocess would be needed to verify the actual stdin/stdout plumbing.

2. **Engine not yet wired**: `run_dap_stdio` starts the DAP protocol session but does not start a Flutter Engine or route debug commands to the Dart VM. This is intentional — adapter integration is task 03/10.
