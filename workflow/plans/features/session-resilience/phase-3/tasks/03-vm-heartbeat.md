## Task: Add VM Service Heartbeat to forward_vm_events

**Objective**: Detect silently-dead VM Service connections (where no WebSocket close frame was received) by periodically probing the VM with a `getVersion` RPC and breaking the event loop after consecutive failures.

**Depends on**: 02-get-version-rpc

### Scope

- `crates/fdemon-app/src/actions.rs`: Add a third `tokio::select!` arm to `forward_vm_events` (~line 949)

### Details

#### Problem

`forward_vm_events` loops on `client.event_receiver().recv()`. If the WebSocket connection silently dies (no close frame, no TCP RST — e.g., network partition, VM process killed without closing the socket), the `recv()` call blocks indefinitely. The session's DevTools panels show stale data forever with no disconnect indication.

The daemon-layer reconnect logic only triggers on WebSocket read errors or clean close frames. A silent TCP death without keepalive produces no error.

#### Fix

Add a `tokio::time::interval(30s)` heartbeat arm to the `forward_vm_events` select loop. On each tick, call `client.get_version()` with a 5-second timeout. Track consecutive failures; after 3 consecutive failures, break the loop (triggering `VmServiceDisconnected`).

**Current structure** (2 arms):
```rust
loop {
    tokio::select! {
        event = client.event_receiver().recv() => { ... }
        _ = vm_shutdown_rx.changed() => { ... }
    }
}
```

**New structure** (3 arms):
```rust
const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(30);
const HEARTBEAT_TIMEOUT: Duration = Duration::from_secs(5);
const MAX_HEARTBEAT_FAILURES: u32 = 3;

let mut heartbeat = tokio::time::interval(HEARTBEAT_INTERVAL);
heartbeat.tick().await; // consume immediate first tick
let mut consecutive_failures: u32 = 0;

loop {
    tokio::select! {
        event = client.event_receiver().recv() => { ... }  // existing — unchanged
        _ = vm_shutdown_rx.changed() => { ... }            // existing — unchanged
        _ = heartbeat.tick() => {
            match tokio::time::timeout(HEARTBEAT_TIMEOUT, client.get_version()).await {
                Ok(Ok(_version)) => {
                    // VM is alive — reset failure counter
                    if consecutive_failures > 0 {
                        debug!(
                            "VM Service heartbeat recovered for session {} after {} failures",
                            session_id, consecutive_failures
                        );
                    }
                    consecutive_failures = 0;
                }
                Ok(Err(e)) => {
                    consecutive_failures += 1;
                    warn!(
                        "VM Service heartbeat failed for session {} ({}/{}): {}",
                        session_id, consecutive_failures, MAX_HEARTBEAT_FAILURES, e
                    );
                    if consecutive_failures >= MAX_HEARTBEAT_FAILURES {
                        error!(
                            "VM Service heartbeat failed {} consecutive times for session {}, disconnecting",
                            MAX_HEARTBEAT_FAILURES, session_id
                        );
                        break;
                    }
                }
                Err(_timeout) => {
                    consecutive_failures += 1;
                    warn!(
                        "VM Service heartbeat timed out for session {} ({}/{})",
                        session_id, consecutive_failures, MAX_HEARTBEAT_FAILURES
                    );
                    if consecutive_failures >= MAX_HEARTBEAT_FAILURES {
                        error!(
                            "VM Service heartbeat timed out {} consecutive times for session {}, disconnecting",
                            MAX_HEARTBEAT_FAILURES, session_id
                        );
                        break;
                    }
                }
            }
        }
    }
}
```

#### Key Design Decisions

1. **30-second interval**: A heartbeat every 30s is lightweight (one small JSON-RPC round-trip). Worst-case detection time is ~90s (3 failures x 30s interval). This is acceptable — hung connections are rare and this is a safety net.

2. **5-second timeout**: If `getVersion` doesn't respond in 5s, the connection is likely dead. The timeout wraps the full RPC round-trip (send + receive).

3. **3 consecutive failures**: Avoids premature disconnect from a single transient hiccup. The consecutive counter resets on any success.

4. **Interaction with reconnection**: During reconnection, the background `run_client_task` is retrying the WebSocket connection. RPC calls will fail with `Error::ChannelClosed` because the I/O loop is between connections. This counts as a heartbeat failure — which is correct behavior. However, the `Reconnecting` events will also arrive via the event channel, and the loop will process them before the next heartbeat tick. After reconnection succeeds, the next heartbeat will succeed and reset the counter. With 30s intervals and reconnect backoff topping at 30s (10 attempts), the reconnect will likely complete before 3 heartbeat failures accumulate. If reconnection itself is failing for 90+ seconds, it's appropriate to consider the connection dead.

5. **`client.get_version()` takes `&self`**: The heartbeat arm borrows `client` immutably for the RPC call. The `event_receiver().recv()` arm borrows `client` mutably. This is fine because `tokio::select!` only executes one arm at a time — the heartbeat tick won't fire while `recv()` is being polled within the same iteration.

   **Wait — this is a potential borrow conflict.** `event_receiver()` returns `&mut self.event_rx`, so the first arm holds a `&mut client` borrow for the `recv()` future. The heartbeat arm needs `&client` for `get_version()`. Tokio's `select!` creates all futures upfront, which means both borrows exist simultaneously.

   **Solution**: Extract the event receiver before the loop, similar to how existing code works:
   ```rust
   let event_rx = client.event_receiver();
   // But event_receiver() returns &mut mpsc::Receiver, not owned...
   ```

   Actually, looking at the existing code: `client.event_receiver().recv()` is called inside `select!`. Tokio's `select!` uses pin-project to poll futures lazily — only one branch is polled at a time. However, Rust's borrow checker is conservative and may reject this at compile time.

   **Practical solution**: Add a `get_version` method to `VmServiceClient` that goes through the same `cmd_tx` channel as `VmRequestHandle::request()`. Since `cmd_tx` is `Clone` and `mpsc::Sender::send()` takes `&self`, we can clone the `cmd_tx` before the loop and use it for heartbeat pings without conflicting with `event_rx`:

   ```rust
   let heartbeat_handle = client.request_handle();
   // ... inside the heartbeat arm:
   heartbeat_handle.request("getVersion", None).await
   ```

   This avoids the borrow conflict entirely because `VmRequestHandle` is a separate owned value with its own `cmd_tx` clone.

#### Revised Implementation

```rust
let heartbeat_handle = client.request_handle();
let mut heartbeat = tokio::time::interval(HEARTBEAT_INTERVAL);
heartbeat.tick().await;
let mut consecutive_failures: u32 = 0;

loop {
    tokio::select! {
        event = client.event_receiver().recv() => { ... }
        _ = vm_shutdown_rx.changed() => { ... }
        _ = heartbeat.tick() => {
            let probe = heartbeat_handle.request("getVersion", None);
            match tokio::time::timeout(HEARTBEAT_TIMEOUT, probe).await {
                Ok(Ok(_)) => {
                    if consecutive_failures > 0 {
                        debug!("VM heartbeat recovered for session {} after {} failures",
                            session_id, consecutive_failures);
                    }
                    consecutive_failures = 0;
                }
                Ok(Err(e)) => {
                    consecutive_failures += 1;
                    warn!("VM heartbeat failed for session {} ({}/{}): {}",
                        session_id, consecutive_failures, MAX_HEARTBEAT_FAILURES, e);
                    if consecutive_failures >= MAX_HEARTBEAT_FAILURES {
                        error!("VM heartbeat failed {} times for session {}, disconnecting",
                            MAX_HEARTBEAT_FAILURES, session_id);
                        break;
                    }
                }
                Err(_timeout) => {
                    consecutive_failures += 1;
                    warn!("VM heartbeat timed out for session {} ({}/{})",
                        session_id, consecutive_failures, MAX_HEARTBEAT_FAILURES);
                    if consecutive_failures >= MAX_HEARTBEAT_FAILURES {
                        error!("VM heartbeat timed out {} times for session {}, disconnecting",
                            MAX_HEARTBEAT_FAILURES, session_id);
                        break;
                    }
                }
            }
        }
    }
}
```

This uses `client.request_handle()` (which clones the `cmd_tx` sender) to make heartbeat RPCs independently of the event receiver borrow.

#### Constants

Define at the top of `actions.rs` alongside existing constants (`PERF_POLL_MIN_MS`, etc.):

```rust
/// Interval between VM Service heartbeat probes.
const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(30);

/// Maximum time to wait for a heartbeat response.
const HEARTBEAT_TIMEOUT: Duration = Duration::from_secs(5);

/// Number of consecutive heartbeat failures before declaring the connection dead.
const MAX_HEARTBEAT_FAILURES: u32 = 3;
```

### Acceptance Criteria

1. A `tokio::time::interval(30s)` heartbeat arm exists in the `forward_vm_events` select loop
2. Each tick sends a `getVersion` RPC with a 5-second timeout
3. Consecutive failure counter increments on RPC error or timeout
4. Counter resets to 0 on any successful response
5. After 3 consecutive failures, the loop breaks and `VmServiceDisconnected` is sent
6. No borrow conflict between `event_receiver().recv()` and the heartbeat probe
7. During normal operation (VM alive), the heartbeat is a no-op (counter stays 0)
8. `cargo check --workspace` passes
9. `cargo clippy --workspace -- -D warnings` clean

### Testing

The heartbeat is async runtime behavior that's difficult to unit test directly. Primary verification:

- `cargo check` confirms no borrow conflicts
- Manual testing: connect to a Flutter app, kill the VM Service process (but leave Flutter running), verify disconnect detected within ~90s
- The downstream `VmServiceDisconnected` handler is already well-tested

Optional: if a mock `VmServiceClient` exists or can be created, test the heartbeat failure counting logic in isolation.

### Notes

- The heartbeat does NOT replace the daemon-layer reconnect logic. If the WebSocket produces a close frame or read error, that path still fires first and is faster.
- The heartbeat is specifically for the "silent death" case: TCP connection open but no data flowing, no close frame.
- Using `request_handle()` instead of `client.get_version()` avoids the Rust borrow checker conflict and is actually cheaper (avoids going through `VmServiceClient::request` which just delegates anyway).
- The `VersionInfo` response is intentionally ignored (`Ok(Ok(_))`) — we only care that the round-trip succeeded.

---

## Completion Summary

**Status:** Not Started
