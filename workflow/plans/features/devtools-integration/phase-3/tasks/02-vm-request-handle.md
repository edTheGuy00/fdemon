## Task: VM Service Request Handle

**Objective**: Extract a clonable, shareable request handle from `VmServiceClient` that allows on-demand VM Service RPC calls from outside the event forwarding loop. This enables periodic memory polling and future on-demand extension calls without restructuring the existing forwarding task.

**Depends on**: None (modifies existing infrastructure)

**Estimated Time**: 3-4 hours

### Scope

- `crates/fdemon-daemon/src/vm_service/client.rs`: Extract `VmRequestHandle` from `VmServiceClient`
- `crates/fdemon-daemon/src/vm_service/mod.rs`: Re-export `VmRequestHandle`
- `crates/fdemon-app/src/session.rs`: Add `vm_request_handle` field to `SessionHandle`
- `crates/fdemon-app/src/message.rs`: Add `VmServiceHandleReady` message variant
- `crates/fdemon-app/src/actions.rs`: Send handle before entering forwarding loop
- `crates/fdemon-app/src/handler/update.rs`: Handle the `VmServiceHandleReady` message

### Details

#### 1. The Problem

The `VmServiceClient` currently lives only inside `forward_vm_events()` in `actions.rs`. It is consumed by the forwarding loop and is not accessible from the TEA handler or other background tasks. This blocks Phase 3's requirement for periodic memory polling and on-demand RPC calls.

The `VmServiceClient` has two key parts:
- `cmd_tx: mpsc::Sender<ClientCommand>` — request-making channel (clonable)
- `event_rx: mpsc::Receiver<VmServiceEvent>` — stream event receiver (NOT clonable)

Since `mpsc::Sender` is `Clone`, we can extract a handle that wraps just the sender side and shares the connection/cache state.

#### 2. VmRequestHandle

Create a lightweight, clonable handle that wraps the request-making channel:

```rust
/// A clonable handle for making VM Service RPC requests.
///
/// This shares the underlying WebSocket connection with the `VmServiceClient`
/// that created it. Multiple handles can make concurrent requests through
/// the same background WebSocket task.
///
/// The handle becomes inoperable when the `VmServiceClient` (or its background
/// task) is dropped — requests will return `Error::ChannelClosed`.
#[derive(Clone)]
pub struct VmRequestHandle {
    cmd_tx: mpsc::Sender<ClientCommand>,
    state: Arc<std::sync::RwLock<ConnectionState>>,
    isolate_id_cache: Arc<Mutex<Option<String>>>,
}
```

#### 3. Move Shared Methods to VmRequestHandle

Move (or duplicate via delegation) the request-making methods to `VmRequestHandle`:

```rust
impl VmRequestHandle {
    /// Send a JSON-RPC request and wait for the response.
    pub async fn request(
        &self,
        method: &str,
        params: Option<serde_json::Value>,
    ) -> Result<serde_json::Value> {
        let (response_tx, response_rx) = oneshot::channel();
        self.cmd_tx
            .send(ClientCommand::SendRequest {
                method: method.to_string(),
                params,
                response_tx,
            })
            .await
            .map_err(|_| Error::ChannelClosed)?;
        response_rx.await.map_err(|_| Error::ChannelClosed)?
    }

    /// Return the current connection state.
    pub fn connection_state(&self) -> ConnectionState {
        self.state.read().unwrap_or_else(|e| e.into_inner()).clone()
    }

    /// Return `true` if the client is currently connected.
    pub fn is_connected(&self) -> bool {
        *self.state.read().unwrap_or_else(|e| e.into_inner()) == ConnectionState::Connected
    }

    /// Get the cached main isolate ID, discovering it if not yet cached.
    pub async fn main_isolate_id(&self) -> Result<String> {
        // Check cache first
        {
            let guard = self.isolate_id_cache.lock().await;
            if let Some(ref id) = *guard {
                return Ok(id.clone());
            }
        }
        // Discover via getVM
        let result = self.request("getVM", None).await?;
        let vm: VmInfo = serde_json::from_value(result)
            .map_err(|e| Error::vm_service(format!("parse getVM: {e}")))?;
        let isolate = vm.isolates.iter()
            .find(|iso| !iso.is_system_isolate.unwrap_or(false))
            .ok_or_else(|| Error::vm_service("no non-system isolate found"))?;
        let id = isolate.id.clone();
        {
            let mut guard = self.isolate_id_cache.lock().await;
            *guard = Some(id.clone());
        }
        Ok(id)
    }

    /// Call a Flutter service extension method.
    pub async fn call_extension(
        &self,
        method: &str,
        isolate_id: &str,
        args: Option<std::collections::HashMap<String, String>>,
    ) -> Result<serde_json::Value> {
        let mut params = serde_json::Map::new();
        params.insert("isolateId".to_string(), serde_json::json!(isolate_id));
        if let Some(extra) = args {
            for (k, v) in extra {
                params.insert(k, serde_json::json!(v));
            }
        }
        self.request(method, Some(serde_json::Value::Object(params))).await
    }
}
```

#### 4. Add Handle Factory to VmServiceClient

```rust
impl VmServiceClient {
    /// Create a clonable request handle that shares this client's connection.
    ///
    /// The handle can make RPC requests independently of the event receiver.
    /// Multiple handles can coexist; they all route through the same background
    /// WebSocket task.
    pub fn request_handle(&self) -> VmRequestHandle {
        VmRequestHandle {
            cmd_tx: self.cmd_tx.clone(),
            state: Arc::clone(&self.state),
            isolate_id_cache: Arc::clone(&self.isolate_id_cache),
        }
    }
}
```

#### 5. Refactor VmServiceClient to Delegate

Refactor `VmServiceClient`'s `request()`, `main_isolate_id()`, `call_extension()`, `connection_state()`, and `is_connected()` to delegate to an internal `VmRequestHandle`. This avoids code duplication:

```rust
pub struct VmServiceClient {
    handle: VmRequestHandle,
    event_rx: mpsc::Receiver<VmServiceEvent>,
}

impl VmServiceClient {
    pub async fn request(&self, method: &str, params: Option<Value>) -> Result<Value> {
        self.handle.request(method, params).await
    }

    pub fn request_handle(&self) -> VmRequestHandle {
        self.handle.clone()
    }

    pub fn event_receiver(&mut self) -> &mut mpsc::Receiver<VmServiceEvent> {
        &mut self.event_rx
    }

    // ... delegate other methods to self.handle
}
```

#### 6. Integration: Store Handle in SessionHandle

In `crates/fdemon-app/src/session.rs`, add a field to `SessionHandle`:

```rust
pub struct SessionHandle {
    // ... existing fields ...

    /// VM Service request handle for on-demand RPC calls.
    /// Set when the VM Service connects, cleared on disconnect.
    pub vm_request_handle: Option<VmRequestHandle>,
}
```

#### 7. Integration: Send Handle via Message

In `crates/fdemon-app/src/message.rs`, add a new variant:

```rust
/// VM Service request handle ready for on-demand calls.
VmServiceHandleReady {
    session_id: SessionId,
    handle: VmRequestHandle,
},
```

Note: `VmRequestHandle` is `Clone` (required by `Message`'s `Clone` derive). It must also impl `Debug` (required by `Message`'s `Debug` derive).

#### 8. Integration: actions.rs Changes

In `spawn_vm_service_connection`, after `VmServiceClient::connect()` succeeds, extract the handle and send it before entering the forwarding loop:

```rust
match connect_result {
    Ok(client) => {
        // ... existing stream subscription ...
        // ... existing shutdown channel setup ...

        // Extract request handle BEFORE entering the forwarding loop
        let handle = client.request_handle();
        let _ = msg_tx.send(Message::VmServiceHandleReady {
            session_id,
            handle,
        }).await;

        // ... existing VmServiceAttached + VmServiceConnected messages ...

        forward_vm_events(client, session_id, msg_tx, vm_shutdown_rx).await;
    }
    // ...
}
```

#### 9. Integration: Handler Changes

In `handler/update.rs`, handle `VmServiceHandleReady`:

```rust
Message::VmServiceHandleReady { session_id, handle } => {
    if let Some(session_handle) = state.session_manager.get_mut(&session_id) {
        session_handle.vm_request_handle = Some(handle);
    }
    UpdateResult::default()
}
```

### Acceptance Criteria

1. `VmRequestHandle` is `Clone`, `Debug`, and can make `request()` calls
2. `VmRequestHandle` shares the WebSocket connection with the original `VmServiceClient`
3. `VmServiceClient::request_handle()` returns a working handle
4. Multiple `VmRequestHandle` instances can make concurrent requests
5. Handle becomes inoperable (`ChannelClosed` error) when the client disconnects
6. `VmServiceClient` delegates to internal handle (no code duplication)
7. `SessionHandle.vm_request_handle` is set on connection, cleared on disconnect
8. `VmServiceHandleReady` message correctly stores the handle
9. Existing `forward_vm_events` behavior is unchanged
10. All existing VM Service tests pass without modification

### Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_request_handle_is_clone() {
        // VmRequestHandle must be Clone for Message derive
        fn assert_clone<T: Clone>() {}
        assert_clone::<VmRequestHandle>();
    }

    #[test]
    fn test_request_handle_is_debug() {
        // VmRequestHandle must be Debug for Message derive
        fn assert_debug<T: std::fmt::Debug>() {}
        assert_debug::<VmRequestHandle>();
    }

    #[tokio::test]
    async fn test_handle_channel_closed_after_drop() {
        // Create a mock channel and handle
        let (cmd_tx, _cmd_rx) = mpsc::channel::<ClientCommand>(1);
        let handle = VmRequestHandle {
            cmd_tx,
            state: Arc::new(std::sync::RwLock::new(ConnectionState::Connected)),
            isolate_id_cache: Arc::new(Mutex::new(None)),
        };
        // Drop the receiver to simulate disconnection
        drop(_cmd_rx);
        let result = handle.request("getVM", None).await;
        assert!(result.is_err());
    }
}
```

### Notes

- **`VmRequestHandle` must implement `Clone` and `Debug`** because `Message` derives both traits. `Clone` is natural (it's a channel sender clone). For `Debug`, use a manual implementation that doesn't try to format the channel internals — just show the connection state.
- **The refactor of `VmServiceClient` to delegate to an internal handle** is the cleanest approach. It avoids method duplication and ensures both the full client and the handle use exactly the same request path.
- **`ClientCommand` is `pub(super)` in `client.rs`** — `VmRequestHandle` must be in the same module (or `ClientCommand` visibility must be adjusted). Since both are in `vm_service/client.rs`, this works naturally.
- **Thread safety**: `VmRequestHandle` is `Send + Sync` because `mpsc::Sender` is `Send + Sync`, `Arc<RwLock<_>>` is `Send + Sync`, and `Arc<Mutex<_>>` (Tokio mutex) is `Send + Sync`.
- **The handle does NOT own the event receiver.** Events still flow through `forward_vm_events` exclusively. The handle is purely for request/response RPC.
- **On disconnect**: The TEA handler should set `vm_request_handle = None` when processing `VmServiceDisconnected`. The handle itself becomes inoperable (ChannelClosed) but clearing the field makes intent explicit.

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-daemon/src/vm_service/client.rs` | Added `VmRequestHandle` struct with `Clone` + manual `Debug`. Refactored `VmServiceClient` to hold an internal `VmRequestHandle` field and delegate `request()`, `main_isolate_id()`, `call_extension()`, `connection_state()`, `is_connected()` to it. Added `request_handle()` factory method. Added 6 new tests for the handle. |
| `crates/fdemon-daemon/src/vm_service/mod.rs` | Re-exported `VmRequestHandle` from `pub use client::...`. |
| `crates/fdemon-app/src/session.rs` | Added `vm_request_handle: Option<VmRequestHandle>` field to `SessionHandle`. Updated `Debug` impl and `new()` constructor. Added `fdemon_daemon::vm_service::VmRequestHandle` import. |
| `crates/fdemon-app/src/message.rs` | Added `VmServiceHandleReady { session_id, handle }` variant to `Message` enum. Added `VmRequestHandle` import from `fdemon_daemon::vm_service`. |
| `crates/fdemon-app/src/actions.rs` | Extracted `client.request_handle()` and sent `VmServiceHandleReady` message before entering the forwarding loop in `spawn_vm_service_connection`. |
| `crates/fdemon-app/src/handler/update.rs` | Added handler for `VmServiceHandleReady` that stores the handle in the session. Updated `VmServiceDisconnected` handler to also clear `vm_request_handle = None`. |

### Notable Decisions/Tradeoffs

1. **Delegation pattern**: `VmServiceClient` holds an internal `VmRequestHandle` field and delegates all request-making methods to it. This eliminates code duplication and ensures both the full client and the handle use the exact same request path.

2. **Manual `Debug` impl**: `VmRequestHandle` uses a manual `Debug` impl that shows only the `ConnectionState`, not channel internals. This is required because `mpsc::Sender<ClientCommand>` does not implement `Debug` (and `ClientCommand` contains oneshot senders which also lack `Debug`).

3. **Message ordering**: The `VmServiceHandleReady` message is sent first (before `VmServiceAttached` and `VmServiceConnected`) so the handle is stored before any other VM Service messages arrive. This ensures the handle is available for immediate use once the TEA handler processes `VmServiceConnected`.

4. **Explicit `None` on disconnect**: `vm_request_handle` is set to `None` in the `VmServiceDisconnected` handler. While the handle would return `Error::ChannelClosed` anyway (since the background task exited), clearing it explicitly communicates intent and helps future code avoid making redundant calls.

### Testing Performed

- `cargo fmt --all` - Passed (no formatting changes needed)
- `cargo check --workspace` - Passed
- `cargo test --lib --workspace` - Passed (1,847 tests: 772 + 314 + 319 + 446)
- `cargo clippy --workspace -- -D warnings` - Passed (no warnings)
- New `VmRequestHandle` tests specifically: 6 tests passed
  - `test_request_handle_is_clone`
  - `test_request_handle_is_debug`
  - `test_handle_channel_closed_after_drop`
  - `test_request_handle_debug_shows_state`
  - `test_request_handle_clone_shares_state`
  - `test_request_handle_is_send_sync`

### Risks/Limitations

1. **E2e tests**: 25 e2e integration tests in the binary crate fail, but these are pre-existing failures related to TUI terminal interaction tests requiring a full terminal environment. They are not caused by this change.

2. **`ClientCommand` visibility**: `ClientCommand` remains non-`pub` (private to `client.rs`). `VmRequestHandle` is in the same module so it can reference `ClientCommand` directly. This is the correct design — callers use the higher-level `request()` API, not the internal command type.
