## Task: Add DAP Message Variants, DapStatus, and Handler

**Objective**: Add DAP server lifecycle messages to the `Message` enum, a `DapStatus` enum to `AppState` for tracking server state, and a `handler/dap.rs` module for processing DAP messages through the TEA update cycle.

**Depends on**: 02 (DapSettings must exist for DapStatus defaults)

### Scope

- `crates/fdemon-app/src/message.rs` — Add DAP server Message variants
- `crates/fdemon-app/src/state.rs` — Add `DapStatus` enum, add `dap_status` field to `AppState`
- `crates/fdemon-app/src/handler/dap.rs` — **NEW FILE**: DAP message handler
- `crates/fdemon-app/src/handler/mod.rs` — Add `mod dap;`, add `UpdateAction` variants, route DAP messages to handler
- `crates/fdemon-app/src/handler/keys.rs` — (Deferred to Task 07 — keybinding toggle)

### Details

#### 1. DapStatus Enum (`state.rs`)

Add near the top of the file, alongside other status enums:

```rust
/// Status of the embedded DAP server.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum DapStatus {
    /// DAP server is not running.
    #[default]
    Off,
    /// DAP server is starting up (binding port, initializing).
    Starting,
    /// DAP server is running and accepting connections.
    Running {
        /// The TCP port the server is listening on.
        port: u16,
        /// Number of currently connected DAP clients.
        client_count: usize,
    },
    /// DAP server is shutting down (disconnecting clients, unbinding).
    Stopping,
}

impl DapStatus {
    /// Returns the port if the server is running.
    pub fn port(&self) -> Option<u16> {
        match self {
            DapStatus::Running { port, .. } => Some(*port),
            _ => None,
        }
    }

    /// Returns whether the server is running.
    pub fn is_running(&self) -> bool {
        matches!(self, DapStatus::Running { .. })
    }

    /// Returns the client count if running, otherwise 0.
    pub fn client_count(&self) -> usize {
        match self {
            DapStatus::Running { client_count, .. } => *client_count,
            _ => 0,
        }
    }
}
```

Add to `AppState` struct (after `devtools_view_state`):

```rust
pub struct AppState {
    // ... existing fields ...
    /// Status of the embedded DAP debug adapter server.
    pub dap_status: DapStatus,
}
```

Initialize in `AppState::with_settings()`:
```rust
dap_status: DapStatus::Off,
```

#### 2. Message Variants (`message.rs`)

Add a new section under a banner comment. Place after the existing `VmServiceDebugEvent`/`VmServiceIsolateEvent` variants:

```rust
// ─────────────────────────────────────────────────────────
// DAP Server Messages
// ─────────────────────────────────────────────────────────

/// Request to start the DAP server on the configured port.
StartDapServer,

/// Request to stop the DAP server and disconnect all clients.
StopDapServer,

/// Toggle DAP server on/off (keybinding handler).
ToggleDap,

/// DAP server successfully started and is listening.
DapServerStarted {
    port: u16,
},

/// DAP server has been stopped.
DapServerStopped,

/// DAP server failed to start.
DapServerFailed {
    reason: String,
},

/// A DAP client connected to the server.
DapClientConnected {
    client_id: String,
},

/// A DAP client disconnected from the server.
DapClientDisconnected {
    client_id: String,
},
```

#### 3. UpdateAction Variants (`handler/mod.rs`)

Add two new variants to `UpdateAction`:

```rust
/// Spawn the DAP TCP server as a background task.
SpawnDapServer {
    port: u16,
    bind_addr: String,
},

/// Stop the running DAP server and disconnect all clients.
StopDapServer,
```

Add routing in the main `update()` function to delegate DAP messages to `dap::handle_dap_message()`.

#### 4. DAP Handler (`handler/dap.rs`)

Create a new handler module following the pattern of `handler/devtools/debug.rs`:

```rust
//! DAP server lifecycle message handler.
//!
//! Processes DAP server state transitions through the TEA update cycle.
//! The actual server start/stop is performed by UpdateAction handlers in
//! the TUI/headless event loops — this module only manages AppState.

use crate::handler::UpdateResult;
use crate::message::Message;
use crate::state::{AppState, DapStatus};

/// Handle a DAP server lifecycle message.
pub fn handle_dap_message(state: &mut AppState, message: &Message) -> UpdateResult {
    match message {
        Message::StartDapServer => handle_start(state),
        Message::StopDapServer => handle_stop(state),
        Message::ToggleDap => handle_toggle(state),
        Message::DapServerStarted { port } => handle_started(state, *port),
        Message::DapServerStopped => handle_stopped(state),
        Message::DapServerFailed { reason } => handle_failed(state, reason),
        Message::DapClientConnected { client_id } => handle_client_connected(state, client_id),
        Message::DapClientDisconnected { client_id } => handle_client_disconnected(state, client_id),
        _ => UpdateResult::none(),
    }
}

fn handle_start(state: &mut AppState) -> UpdateResult {
    if state.dap_status.is_running() {
        return UpdateResult::none(); // Already running, no-op
    }
    state.dap_status = DapStatus::Starting;
    let port = state.settings.dap.port;
    let bind_addr = state.settings.dap.bind_address.clone();
    UpdateResult::with_action(UpdateAction::SpawnDapServer { port, bind_addr })
}

fn handle_stop(state: &mut AppState) -> UpdateResult {
    if !state.dap_status.is_running() {
        return UpdateResult::none(); // Not running, no-op
    }
    state.dap_status = DapStatus::Stopping;
    UpdateResult::with_action(UpdateAction::StopDapServer)
}

fn handle_toggle(state: &mut AppState) -> UpdateResult {
    if state.dap_status.is_running() {
        handle_stop(state)
    } else {
        handle_start(state)
    }
}

fn handle_started(state: &mut AppState, port: u16) -> UpdateResult {
    state.dap_status = DapStatus::Running { port, client_count: 0 };
    tracing::info!("DAP server listening on {}:{}", state.settings.dap.bind_address, port);
    UpdateResult::none()
}

fn handle_stopped(state: &mut AppState) -> UpdateResult {
    state.dap_status = DapStatus::Off;
    tracing::info!("DAP server stopped");
    UpdateResult::none()
}

fn handle_failed(state: &mut AppState, reason: &str) -> UpdateResult {
    state.dap_status = DapStatus::Off;
    tracing::error!("DAP server failed to start: {}", reason);
    UpdateResult::none()
}

fn handle_client_connected(state: &mut AppState, client_id: &str) -> UpdateResult {
    if let DapStatus::Running { client_count, .. } = &mut state.dap_status {
        *client_count += 1;
        tracing::info!("DAP client connected: {}", client_id);
    }
    UpdateResult::none()
}

fn handle_client_disconnected(state: &mut AppState, client_id: &str) -> UpdateResult {
    if let DapStatus::Running { client_count, .. } = &mut state.dap_status {
        *client_count = client_count.saturating_sub(1);
        tracing::info!("DAP client disconnected: {}", client_id);
    }
    UpdateResult::none()
}
```

#### 5. Message Routing (`handler/mod.rs`)

In the main `update()` function, add a match arm to route DAP messages to the handler:

```rust
Message::StartDapServer
| Message::StopDapServer
| Message::ToggleDap
| Message::DapServerStarted { .. }
| Message::DapServerStopped
| Message::DapServerFailed { .. }
| Message::DapClientConnected { .. }
| Message::DapClientDisconnected { .. } => dap::handle_dap_message(state, message),
```

### Acceptance Criteria

1. `DapStatus` enum compiles with `Debug`, `Clone`, `Default`, `PartialEq`, `Eq`
2. `DapStatus::default()` is `Off`
3. `DapStatus::port()`, `is_running()`, `client_count()` return correct values for each variant
4. All 8 DAP `Message` variants compile and are `Debug + Clone`
5. `UpdateAction::SpawnDapServer` and `StopDapServer` variants are defined
6. `handle_dap_message()` correctly transitions `DapStatus` for each message:
   - `StartDapServer` when Off → `Starting` + `SpawnDapServer` action
   - `StartDapServer` when already Running → no-op
   - `StopDapServer` when Running → `Stopping` + `StopDapServer` action
   - `StopDapServer` when Off → no-op
   - `ToggleDap` when Off → same as Start
   - `ToggleDap` when Running → same as Stop
   - `DapServerStarted { port }` → `Running { port, client_count: 0 }`
   - `DapServerStopped` → `Off`
   - `DapServerFailed { .. }` → `Off`
   - `DapClientConnected` → increments `client_count`
   - `DapClientDisconnected` → decrements `client_count` (saturating)
7. `AppState` has `dap_status: DapStatus` field initialized to `Off`
8. DAP messages are correctly routed from `update()` to `dap::handle_dap_message()`
9. `cargo check -p fdemon-app` passes
10. `cargo test -p fdemon-app` passes (no regressions)
11. `cargo clippy -p fdemon-app -- -D warnings` clean

### Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dap_status_default_is_off() {
        assert_eq!(DapStatus::default(), DapStatus::Off);
    }

    #[test]
    fn test_dap_status_port() {
        assert_eq!(DapStatus::Off.port(), None);
        assert_eq!(DapStatus::Starting.port(), None);
        assert_eq!(DapStatus::Running { port: 4711, client_count: 0 }.port(), Some(4711));
    }

    #[test]
    fn test_start_when_off_transitions_to_starting() {
        let mut state = test_state();
        let result = handle_dap_message(&mut state, &Message::StartDapServer);
        assert_eq!(state.dap_status, DapStatus::Starting);
        assert!(result.action.is_some()); // SpawnDapServer
    }

    #[test]
    fn test_start_when_running_is_noop() {
        let mut state = test_state();
        state.dap_status = DapStatus::Running { port: 4711, client_count: 0 };
        let result = handle_dap_message(&mut state, &Message::StartDapServer);
        assert!(state.dap_status.is_running());
        assert!(result.action.is_none());
    }

    #[test]
    fn test_toggle_when_off_starts() {
        let mut state = test_state();
        let result = handle_dap_message(&mut state, &Message::ToggleDap);
        assert_eq!(state.dap_status, DapStatus::Starting);
        assert!(result.action.is_some());
    }

    #[test]
    fn test_toggle_when_running_stops() {
        let mut state = test_state();
        state.dap_status = DapStatus::Running { port: 4711, client_count: 0 };
        let result = handle_dap_message(&mut state, &Message::ToggleDap);
        assert_eq!(state.dap_status, DapStatus::Stopping);
        assert!(result.action.is_some());
    }

    #[test]
    fn test_client_connected_increments_count() {
        let mut state = test_state();
        state.dap_status = DapStatus::Running { port: 4711, client_count: 0 };
        handle_dap_message(&mut state, &Message::DapClientConnected { client_id: "c1".into() });
        assert_eq!(state.dap_status.client_count(), 1);
    }

    #[test]
    fn test_client_disconnected_decrements_count() {
        let mut state = test_state();
        state.dap_status = DapStatus::Running { port: 4711, client_count: 2 };
        handle_dap_message(&mut state, &Message::DapClientDisconnected { client_id: "c1".into() });
        assert_eq!(state.dap_status.client_count(), 1);
    }

    #[test]
    fn test_client_disconnected_saturates_at_zero() {
        let mut state = test_state();
        state.dap_status = DapStatus::Running { port: 4711, client_count: 0 };
        handle_dap_message(&mut state, &Message::DapClientDisconnected { client_id: "c1".into() });
        assert_eq!(state.dap_status.client_count(), 0);
    }

    #[test]
    fn test_server_failed_resets_to_off() {
        let mut state = test_state();
        state.dap_status = DapStatus::Starting;
        handle_dap_message(&mut state, &Message::DapServerFailed { reason: "port in use".into() });
        assert_eq!(state.dap_status, DapStatus::Off);
    }
}
```

### Notes

- The `handler/dap.rs` module handles **server lifecycle only** (start/stop/status). It does NOT handle debugging operations (breakpoints, stepping, etc.) — those remain in `handler/devtools/debug.rs` and will be connected in Phase 3.
- `UpdateAction::SpawnDapServer` and `StopDapServer` are handled in the TUI/headless runner event loops (Task 05), not in `actions/mod.rs`. This is because the DAP server is an Engine-level service (like the file watcher), not a session-scoped action.
- The `client_id` in `DapClientConnected`/`DapClientDisconnected` is a string identifier assigned by the DAP server (e.g., remote address or UUID). It's used for logging only in Phase 2.
- The handler returns `UpdateResult` (not `Option<Message>` + `Option<UpdateAction>`), following the pattern established in Phase 1's `handler/devtools/debug.rs`.
