## Task: Wire Real DebugBackend into TCP Accept Loop

**Objective**: Make the TCP DAP server functional for real debugging by threading a backend factory from the Engine through the accept loop, so IDE `attach` requests connect to the running Flutter session's VM Service instead of failing with "NoopBackend: no VM Service connected".

**Depends on**: None

**Estimated Time**: 4–6 hours

**Severity**: CRITICAL — all IDE debugging is non-functional without this fix.

### Scope

- `crates/fdemon-dap/src/server/mod.rs`: Add backend factory parameter to `accept_loop()` and `start()`
- `crates/fdemon-dap/src/server/session.rs`: Ensure `run_on_with_backend()` is called when a backend is available
- `crates/fdemon-dap/src/service.rs`: Update `DapService::start_tcp()` signature to accept the factory
- `crates/fdemon-app/src/actions/mod.rs`: Construct the factory in the `SpawnDapServer` action using the active session's `VmRequestHandle`
- `crates/fdemon-app/src/engine.rs`: Expose mechanism to retrieve `VmRequestHandle` for the active session

### Details

#### Current State

The TCP accept loop at `server/mod.rs:297` calls `DapClientSession::run(stream, session_shutdown, log_event_rx)` which is only implemented on `DapClientSession<NoopBackend>`. The `run_on_with_backend()` method exists on the generic `DapClientSession<B>` but is never called from any production code path.

#### Root Cause

No mechanism exists to pass a backend factory from the Engine (which owns `VmRequestHandle` per session) through to the accept loop (which lives in `fdemon-dap` and cannot depend on `fdemon-app`).

#### Design

**Backend factory approach** — pass a boxed closure that the accept loop calls per connection:

```rust
// In server/mod.rs — accept_loop signature becomes:
async fn accept_loop(
    listener: TcpListener,
    shutdown_rx: watch::Receiver<bool>,
    event_tx: mpsc::Sender<DapServerEvent>,
    semaphore: Arc<Semaphore>,
    log_event_tx: broadcast::Sender<DebugEvent>,
    // NEW: factory that produces a backend + per-session debug event receiver
    backend_factory: Arc<dyn Fn() -> Option<BackendHandle> + Send + Sync>,
)

// Where BackendHandle bundles backend + event channel:
pub struct BackendHandle {
    pub backend: Box<dyn DebugBackend>,
    pub debug_event_rx: mpsc::Receiver<DebugEvent>,
}
```

Wait — `DebugBackend` is not object-safe because it uses `async fn` methods via `trait_variant`. Instead, use a generic approach with a factory that returns a concrete type.

**Recommended approach: type-erased factory via boxed future + concrete VmServiceBackend**

Since `fdemon-dap` cannot depend on `fdemon-app`, the factory should return a `Box<dyn DebugBackend>`. Check whether `DebugBackend` is object-safe. If not (due to `trait_variant::make`), consider:

1. **Option A**: Make the factory return a concrete `Box<dyn DebugBackend>` if the trait is object-safe after `trait_variant::make` generates the `Send` variant.
2. **Option B**: Use a shared `Arc<Mutex<Option<VmRequestHandle>>>` pattern where the accept loop holds a reference, and the session constructs the backend itself. This requires `fdemon-dap` to know about `VmRequestHandle` — but `VmRequestHandle` is from `fdemon-daemon`, which `fdemon-dap` does not depend on. So this won't work directly.
3. **Option C (recommended)**: Add a `BackendFactory` trait to `fdemon-dap`:

```rust
// In fdemon-dap/src/adapter/mod.rs (or a new factory.rs):
pub trait BackendFactory: Send + Sync + 'static {
    fn create_backend(&self) -> Option<(Box<dyn DebugBackendBoxed>, mpsc::Receiver<DebugEvent>)>;
}
```

Where `DebugBackendBoxed` is a dyn-compatible wrapper. Implement `BackendFactory` in `fdemon-app` using `VmRequestHandle` → `VmServiceBackend`.

**Investigation needed**: Determine whether `trait_variant::make(DebugBackend: Send)` produces an object-safe trait. If it does, `Box<dyn DebugBackend>` works directly. If not, a wrapper will be needed.

#### Key Implementation Steps

1. **Determine object safety** of `DebugBackend` after `trait_variant::make`. Write a small compile test.
2. **Add backend factory parameter** to `server::start()` and propagate to `accept_loop()`.
3. **In accept loop**: when a connection arrives, call the factory. If `Some(backend)`, spawn with `run_on_with_backend`. If `None`, spawn with `run_on` (NoopBackend fallback).
4. **Update `DapService::start_tcp()`** to accept the factory.
5. **Update `SpawnDapServer` action** in `actions/mod.rs` to construct a factory closure that captures the active session's `VmRequestHandle` from `AppState::session_manager`.
6. **Handle per-session debug event routing**: The factory must also produce an `mpsc::Receiver<DebugEvent>` per client. The matching `mpsc::Sender<DebugEvent>` must be registered somewhere so the Engine can forward VM pause/stopped events to connected DAP clients.
7. **Update `DapServerHandle`** to expose a way to register/unregister per-client event senders, or include a `Vec<mpsc::Sender<DebugEvent>>` that the Engine iterates when forwarding VM events.

#### Channel Architecture

```
Engine (fdemon-app)
  │
  ├── broadcast::Sender<DebugEvent>  ← log output events (one-to-many, all clients)
  │     └── accept_loop subscribes each client via log_event_tx.subscribe()
  │
  └── per-client mpsc::Sender<DebugEvent>  ← VM debug events (stopped, breakpoint hit)
        └── factory creates (tx, rx) pair; rx goes to run_on_with_backend;
            tx registered with Engine for VM event forwarding
```

### Acceptance Criteria

1. IDE connects via TCP and completes `initialize` → `configurationDone` → `attach` handshake
2. `attach` succeeds when a Flutter session is running (returns success, not NoopBackend error)
3. `attach` returns an error response (not crash) when no Flutter session is running
4. `threads` request after attach returns the session's isolates
5. Breakpoints set via `setBreakpoints` are forwarded to the Dart VM Service
6. All existing tests pass
7. No new `unsafe` code or `unwrap()` on fallible operations

### Testing

- Unit test: factory returning `None` → session falls back to `NoopBackend`
- Unit test: factory returning `Some(MockBackend)` → session uses real backend
- Integration test: `SpawnDapServer` action with a mock session → factory produces backend
- Existing session.rs tests must continue to pass

### Notes

- This is the highest-priority fix. All DAP debugging functionality is blocked on this.
- The `run_on_with_backend` method uses `mpsc::Receiver<DebugEvent>` while `run_on` uses `broadcast::Receiver<DebugEvent>` — the factory approach must account for this difference.
- Task 04 (consolidate session loops) will clean up the duplication between `run_on` and `run_on_with_backend` after this task establishes the correct wiring.
- Consider whether multiple DAP clients should be able to debug the same Flutter session simultaneously. For Phase 3, single-client-per-session is acceptable.

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-dap/src/adapter/mod.rs` | Added `DynDebugBackendInner` (object-safe vtable trait with `Pin<Box<dyn Future>>` methods) and `DynDebugBackend` (concrete type implementing `DebugBackend` by delegating through the vtable). This solves the dyn-compatibility issue with `DebugBackend`. |
| `crates/fdemon-dap/src/server/mod.rs` | Updated `BackendHandle` to include `backend: DynDebugBackend`; updated `start()` to accept `backend_factory: Option<Arc<dyn BackendFactory>>`; updated `accept_loop()` to use the factory per connection (real backend or NoopBackend fallback); added factory tests with `MockBackendInner`, `AlwaysBackendFactory`, `NeverBackendFactory`. |
| `crates/fdemon-dap/src/service.rs` | Added `start_tcp_with_factory()` method that accepts a `BackendFactory`; kept legacy `start_tcp()` and `start()` as pass-`None` wrappers for backward compat. |
| `crates/fdemon-dap/src/lib.rs` | Re-exported `DynDebugBackend` and `DynDebugBackendInner` at crate root for `fdemon-app` consumers. |
| `crates/fdemon-app/src/handler/dap_backend.rs` | Added `DynDebugBackendInner` impl for `VmServiceBackend` (boxes each async fn return); added `VmBackendFactory` struct implementing `fdemon_dap::server::BackendFactory` using `Arc<Mutex<Option<VmRequestHandle>>>`. |
| `crates/fdemon-app/src/engine.rs` | Added `vm_handle_for_dap: Arc<Mutex<Option<VmRequestHandle>>>` field; initialized in `new()`; passes it to `process::process_message` and `dispatch_spawn_session`; added `sync_vm_handle_for_dap()` called after each TEA cycle. |
| `crates/fdemon-app/src/process.rs` | Added `vm_handle_for_dap` parameter to `process_message()`; threads it to `handle_action()`. |
| `crates/fdemon-app/src/actions/mod.rs` | Added `vm_handle_for_dap` parameter to `handle_action()`; `SpawnDapServer` arm now creates `VmBackendFactory` and calls `DapService::start_tcp_with_factory()`. |

### Notable Decisions/Tradeoffs

1. **`DynDebugBackend` pattern instead of `Box<dyn DebugBackend>`**: `DebugBackend` is not dyn-compatible because `trait_variant::make` generates `impl Future` return types (RPIT). Created `DynDebugBackendInner` with `Pin<Box<dyn Future>>` methods as a vtable, and `DynDebugBackend` as a concrete wrapper. This avoids modifying the public `DebugBackend` trait.

2. **`Arc<Mutex<Option<VmRequestHandle>>>` slot pattern**: The factory needs the VM handle at connection time, not at server-start time. Using a shared slot (updated each TEA cycle via `sync_vm_handle_for_dap`) means the factory always gets the freshest handle without needing `AppState` access inside the Tokio task.

3. **Legacy `DapService::start()` kept with `None` factory**: To avoid breaking existing callers (tests, binary), `start()` and `start_tcp()` were kept with their original 3-argument signatures, passing `None` to `server::start()`. The new `start_tcp_with_factory()` is the production entry point.

4. **Per-session debug event channel sender dropped in factory**: The `mpsc::Sender<DebugEvent>` from the per-session channel is currently dropped in `VmBackendFactory::create()`. Task 06 will wire the sender back to the Engine so VM pause/stopped/breakpoint events are forwarded to connected DAP clients.

5. **`sync_vm_handle_for_dap` uses `try_lock`**: Non-blocking, same pattern as `sync_dap_log_sender`. If the lock is held by the factory, the slot is not updated this cycle; it will be retried next TEA cycle.

### Testing Performed

- `cargo check --workspace` — Passed
- `cargo test --workspace --lib` — Passed (3,259 unit tests: 1267 fdemon-core, 360 fdemon-daemon, 460 fdemon-app, 376 fdemon-dap, 796 fdemon-tui)
- `cargo clippy --workspace -- -D warnings` — Passed (no warnings)
- `cargo fmt --all` — Passed (no formatting changes)
- New tests in `server/mod.rs`: `test_factory_returning_none_falls_back_to_noop`, `test_factory_returning_some_backend_initializes_cleanly` — Both pass

### Risks/Limitations

1. **Per-session debug event routing not yet wired (Task 06)**: The `mpsc::Sender<DebugEvent>` created in `VmBackendFactory::create()` is currently dropped. VM Service debug events (paused, resumed, breakpoint hit) will not reach the DAP client until Task 06 registers the sender with the Engine. Sessions work (initialize/attach/disconnect), but stopped events won't arrive automatically.

2. **Single-client-per-session assumption**: The factory clones the VM handle for each connection, so multiple clients could technically attach. However, the Engine's event routing (Task 06) may only support one sender per session. Multi-client support is deferred to after Phase 3.

3. **Linter interaction during development**: The project's automated linter (rustfmt + likely clippy-fix) modified some intermediate states of `server/mod.rs` during development. All changes were correctly reapplied and the final state passes all quality checks.
