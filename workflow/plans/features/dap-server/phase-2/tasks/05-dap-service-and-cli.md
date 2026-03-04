## Task: DapService Lifecycle, CLI Flags, and Engine Integration

**Objective**: Create the `DapService` struct that manages the DAP server lifecycle (start/stop/status), add `--dap-port` CLI flag for scripting/CI, integrate DAP startup into the Engine, and wire `UpdateAction::SpawnDapServer`/`StopDapServer` handling into the TUI and headless runner event loops.

**Depends on**: 03 (DAP Messages + UpdateAction variants), 04 (TCP server)

### Scope

- `crates/fdemon-dap/src/service.rs` — **NEW**: DapService struct, start/stop lifecycle
- `crates/fdemon-dap/src/lib.rs` — Add `pub mod service;`, re-exports
- `crates/fdemon-app/src/engine.rs` — Add DAP server fields, integrate start/stop into Engine lifecycle
- `crates/fdemon-app/src/actions/mod.rs` — Handle `SpawnDapServer`/`StopDapServer` actions
- `crates/fdemon-app/Cargo.toml` — Add `fdemon-dap` dependency
- `src/main.rs` — Add `--dap-port` CLI flag (implies DAP enabled)
- `src/tui/runner.rs` — Wire DAP startup
- `src/headless/runner.rs` — Wire DAP startup

### Details

#### 1. DapService (`service.rs`)

A thin wrapper around `server::start()` that bridges `DapServerEvent` → `Message`:

```rust
use crate::server::{DapServerConfig, DapServerEvent, DapServerHandle, start};
use tokio::sync::mpsc;

/// Manages the DAP server lifecycle.
///
/// This struct does not hold the server handle directly — it creates one
/// via `start()` and returns it to the caller (the Engine).
pub struct DapService;

impl DapService {
    /// Start the DAP server and begin forwarding events to the Engine's message channel.
    ///
    /// Returns a `DapServerHandle` for lifecycle management.
    /// The caller must store this handle and call `stop()` on shutdown.
    pub async fn start(
        port: u16,
        bind_addr: String,
        msg_tx: mpsc::Sender<Message>,
    ) -> Result<DapServerHandle> {
        let (event_tx, mut event_rx) = mpsc::channel::<DapServerEvent>(32);

        let config = DapServerConfig { port, bind_addr };
        let handle = start(config, event_tx).await?;
        let actual_port = handle.port;

        // Spawn a bridge task: DapServerEvent → Message
        let bridge_tx = msg_tx.clone();
        tokio::spawn(async move {
            while let Some(event) = event_rx.recv().await {
                let message = match event {
                    DapServerEvent::ClientConnected { client_id } => {
                        Message::DapClientConnected { client_id }
                    }
                    DapServerEvent::ClientDisconnected { client_id } => {
                        Message::DapClientDisconnected { client_id }
                    }
                    DapServerEvent::ServerError { reason } => {
                        Message::DapServerFailed { reason }
                    }
                };
                if bridge_tx.send(message).await.is_err() {
                    break; // Engine channel closed
                }
            }
        });

        // Notify Engine that server started successfully
        let _ = msg_tx.send(Message::DapServerStarted { port: actual_port }).await;

        Ok(handle)
    }

    /// Stop a running DAP server.
    ///
    /// Signals shutdown and waits for the server task to complete.
    pub async fn stop(handle: DapServerHandle) {
        let _ = handle.shutdown_tx.send(true);
        // Wait for server task with timeout
        let _ = tokio::time::timeout(
            std::time::Duration::from_secs(5),
            handle.task,
        ).await;
    }
}
```

Note: `Message` here comes from `fdemon-app`. Since `fdemon-dap` doesn't depend on `fdemon-app`, this module needs the `Message` type injected. Two approaches:

**Option A (recommended)**: Define `DapService::start()` as a free function in `fdemon-app` (or the binary crate) that calls `fdemon_dap::server::start()` and does the bridging. This keeps `fdemon-dap` dependency-free from `fdemon-app`.

**Option B**: Use a generic callback/closure for the event bridge instead of `Message` directly.

The implementor should choose the approach that best fits the existing codebase patterns. The key contract is:
- `DapServerEvent` → `Message` mapping happens somewhere
- The Engine gets a `DapServerHandle` to manage lifecycle

#### 2. Engine Integration (`engine.rs`)

Add fields to the `Engine` struct:

```rust
pub struct Engine {
    // ... existing fields ...
    /// Handle for the running DAP server, if any.
    pub(crate) dap_server_handle: Option<DapServerHandle>,
}
```

Initialize in `Engine::new()`:
```rust
dap_server_handle: None,
```

**Shutdown integration** — in `Engine::shutdown()`, stop the DAP server if running:

```rust
// In Engine::shutdown():
if let Some(handle) = self.dap_server_handle.take() {
    tracing::info!("Stopping DAP server...");
    DapService::stop(handle).await;
    self.state.dap_status = DapStatus::Off;
}
```

#### 3. Action Handler (`actions/mod.rs`)

Add match arms for the new `UpdateAction` variants:

```rust
UpdateAction::SpawnDapServer { port, bind_addr } => {
    let msg_tx = msg_tx.clone();
    tokio::spawn(async move {
        match DapService::start(port, bind_addr, msg_tx.clone()).await {
            Ok(handle) => {
                // Handle is stored by the Engine when DapServerStarted message is processed
                // For now, we need to communicate the handle back to the Engine
                // This is handled via a oneshot channel or by storing directly
            }
            Err(e) => {
                let _ = msg_tx.send(Message::DapServerFailed {
                    reason: e.to_string(),
                }).await;
            }
        }
    });
}
UpdateAction::StopDapServer => {
    // Take the handle from Engine and stop
    if let Some(handle) = engine.dap_server_handle.take() {
        tokio::spawn(async move {
            DapService::stop(handle).await;
        });
        let _ = msg_tx.send(Message::DapServerStopped).await;
    }
}
```

**Handle storage challenge:** The `SpawnDapServer` action spawns an async task that returns a `DapServerHandle`, but the action handler doesn't have mutable access to the Engine. Two approaches:

1. **Pass handle via Message**: Add a `Message::DapServerHandleReady { handle }` variant (requires `DapServerHandle` to be `Clone` or wrapped in `Arc`)
2. **Store in a shared slot**: Use an `Arc<Mutex<Option<DapServerHandle>>>` that the action handler writes to and the Engine reads from

The implementor should choose the approach that best fits. Approach 1 is simpler and follows the TEA pattern.

#### 4. CLI Flags (`src/main.rs`)

The primary startup path is auto-detection (Task 07), not CLI flags. The CLI flag exists for scripting/CI scenarios where you need a fixed port.

Add to the `Args` struct:

```rust
#[derive(Parser, Debug)]
#[command(name = "fdemon", version)]
#[command(about = "A high-performance TUI for Flutter development", long_about = None)]
struct Args {
    /// Path to Flutter project
    #[arg(value_name = "PATH")]
    path: Option<PathBuf>,

    /// Run in headless mode (JSON output, no TUI)
    #[arg(long)]
    headless: bool,

    /// Start the DAP server on a specific port (implies DAP enabled)
    #[arg(long, value_name = "PORT")]
    dap_port: Option<u16>,
}
```

**No `--dap` flag.** The normal startup paths are:
1. Auto-start when IDE detected (zero config, handled by Task 07)
2. `dap.enabled = true` in config (for "always on" users)
3. `--dap-port PORT` (for scripting/CI with fixed port)
4. Press `D` at runtime (for ad-hoc toggle)

In `main()`, after Engine creation:

```rust
// --dap-port overrides config and forces DAP on
if let Some(port) = args.dap_port {
    engine.settings.dap.port = port;
    engine.settings.dap.enabled = true; // Force enable
}
// Auto-start evaluation happens in Task 07 (should_auto_start_dap)
```

#### 5. Runner Integration (`src/tui/runner.rs` and `src/headless/runner.rs`)

Both runners need to handle the DAP `UpdateAction` variants in their event loops. The exact pattern depends on how `UpdateAction` is currently dispatched — follow the existing pattern for `SpawnSession` or `ConnectVmService`.

### Acceptance Criteria

1. `DapService::start()` binds the TCP server and returns a `DapServerHandle`
2. `DapService::start()` sends `Message::DapServerStarted { port }` on success
3. `DapService::start()` sends `Message::DapServerFailed { reason }` on bind failure
4. `DapService::stop()` signals shutdown and waits for task completion with timeout
5. `DapServerEvent` → `Message` bridge correctly maps all event variants
6. Engine stores `dap_server_handle: Option<DapServerHandle>`
7. Engine shutdown stops the DAP server if running
8. `--dap-port PORT` CLI flag sets the port and forces `dap.enabled = true`
9. CLI values override config file settings
12. `UpdateAction::SpawnDapServer` spawns the DAP server as a background task
13. `UpdateAction::StopDapServer` stops the running DAP server
14. Both TUI and headless runners handle DAP actions
15. `cargo check --workspace` passes
16. `cargo test --workspace` passes (no regressions)
17. `cargo clippy --workspace -- -D warnings` clean

### Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_dap_service_start_and_stop() {
        let (msg_tx, mut msg_rx) = mpsc::channel(16);
        let handle = DapService::start(0, "127.0.0.1".to_string(), msg_tx).await.unwrap();
        assert!(handle.port > 0);

        // Should receive DapServerStarted
        let msg = msg_rx.recv().await.unwrap();
        assert!(matches!(msg, Message::DapServerStarted { .. }));

        DapService::stop(handle).await;
    }

    #[tokio::test]
    async fn test_dap_service_port_in_use_fails() {
        let (msg_tx1, _) = mpsc::channel(16);
        let handle = DapService::start(0, "127.0.0.1".to_string(), msg_tx1).await.unwrap();
        let port = handle.port;

        let (msg_tx2, _) = mpsc::channel(16);
        let result = DapService::start(port, "127.0.0.1".to_string(), msg_tx2).await;
        assert!(result.is_err());

        DapService::stop(handle).await;
    }
}
```

### Notes

- The `DapService` is designed as a stateless helper — all state is in the `DapServerHandle` and `AppState.dap_status`. This follows the Engine's existing pattern where services are managed via handles and messages rather than long-lived objects.
- The `DapServerHandle` must be `Send` (it contains `JoinHandle` and `watch::Sender`, both of which are `Send`).
- The bridge task (DapServerEvent → Message) runs until the event channel closes (when the server stops) or the msg_tx is dropped (when the Engine shuts down). Either way, it cleans up automatically.
- `--dap-port` is independent of `--headless`. Both can be combined: `fdemon --headless --dap-port 4711` runs headless with DAP on a fixed port. This mirrors the plan's design decision that DAP is a service, not a mode.
- In headless mode, the DAP port is printed to stdout as JSON: `{"dapPort": 54321}` so external tooling can discover it.
