## Task: Server Hardening — Connection Limit, Accept Backoff, Handle Visibility

**Objective**: Harden the DAP TCP server by adding a connection limit, accept-error backoff, and restricting `DapServerHandle` field visibility.

**Depends on**: merge (post-merge improvement)

**Priority**: LOW

**Review Source**: REVIEW.md Issues #7, #8, #13 (Risks & Tradeoffs Analyzer, Architecture Enforcer)

### Scope

- `crates/fdemon-dap/src/server/mod.rs`: Connection limit, accept backoff, field visibility

### Background

Three related server robustness issues:

1. **No connection limit** (Issue #7, line 161): The accept loop spawns a new tokio task per connection with no cap. A misbehaving client or port scanner could exhaust task resources.

2. **No accept-error backoff** (Issue #13, lines 220-228): When `listener.accept()` fails, the loop logs the error and immediately loops. A persistent OS error (e.g., file descriptor exhaustion) creates a tight error-spam loop.

3. **Public DapServerHandle fields** (Issue #8, lines 64-79): `shutdown_tx`, `task`, and `port` are all `pub`, allowing callers to bypass `DapService::stop` and directly manipulate internals.

### Details

#### 1. Connection Limit

Add a `tokio::sync::Semaphore` to cap concurrent client connections:

```rust
use tokio::sync::Semaphore;
use std::sync::Arc;

/// Maximum number of concurrent DAP client connections.
const MAX_CONCURRENT_CLIENTS: usize = 8;
```

In the accept loop, acquire a permit before spawning a client task:

```rust
let semaphore = Arc::new(Semaphore::new(MAX_CONCURRENT_CLIENTS));

loop {
    // ... select! ...
    Ok((stream, addr)) => {
        let permit = match semaphore.clone().try_acquire_owned() {
            Ok(permit) => permit,
            Err(_) => {
                tracing::warn!("DAP server: max concurrent clients reached, rejecting connection from {}", addr);
                drop(stream);
                continue;
            }
        };

        tokio::spawn(async move {
            // ... handle session ...
            drop(permit); // released when session ends
        });
    }
}
```

#### 2. Accept Error Backoff

Add a delay after accept failures to prevent tight error loops:

```rust
Err(e) => {
    tracing::error!("DAP server accept error: {}", e);
    let _ = event_tx.send(DapServerEvent::ServerError { reason: e.to_string() }).await;
    // Backoff to prevent tight error loop on persistent failures.
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
}
```

#### 3. Restrict DapServerHandle Visibility

Change field visibility from `pub` to `pub(crate)` and add a `port()` accessor:

```rust
pub struct DapServerHandle {
    /// The actual port the server is listening on.
    port: u16,

    /// Send `true` to signal shutdown.
    pub(crate) shutdown_tx: watch::Sender<bool>,

    /// Join handle for the accept-loop task.
    pub(crate) task: tokio::task::JoinHandle<()>,
}

impl DapServerHandle {
    /// Returns the port the server is listening on.
    pub fn port(&self) -> u16 {
        self.port
    }
}
```

Update all callers that access `handle.port` directly to use `handle.port()`. `shutdown_tx` and `task` are only accessed within `fdemon-dap` (by `DapService::stop` and tests), so `pub(crate)` is sufficient.

### Acceptance Criteria

1. `MAX_CONCURRENT_CLIENTS` constant exists (8)
2. Connections beyond the limit are rejected with a log warning
3. Accept errors trigger a 100ms backoff before retrying
4. `DapServerHandle::port` is accessed via `port()` accessor
5. `shutdown_tx` and `task` are `pub(crate)`
6. All existing server tests pass
7. `cargo test -p fdemon-dap` passes
8. `cargo clippy -p fdemon-dap -- -D warnings` clean

### Testing

1. **Connection limit**: Test that spawning `MAX_CONCURRENT_CLIENTS + 1` connections results in the last being rejected (or queued). This may be tricky in unit tests — consider an integration-style test.
2. **Accept backoff**: Difficult to unit test directly. Verify by code review.
3. **Handle visibility**: Compile-time enforced. If external code accessed `handle.port` directly, it would fail to compile.

### Notes

- The semaphore `try_acquire_owned` approach is non-blocking — it rejects immediately rather than making the accept loop wait. This prevents a full semaphore from blocking the shutdown signal check.
- The 100ms backoff is deliberately short — enough to break a tight loop but not so long that it delays recovery from transient errors.
- `DapServerHandle` tests within `fdemon-dap` use `pub(crate)` fields directly, which is fine since tests are in the same crate.
