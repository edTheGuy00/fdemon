## Task: Wire Debug Event Channel from Engine to DAP Adapter

**Objective**: Fix the broken pause/resume/exception flow by wiring the `mpsc::Sender<DebugEvent>` from `VmBackendFactory::create()` back through the Engine so VM Service debug events (PauseBreakpoint, PauseException, Resume, IsolateStart, IsolateExit) are forwarded to connected DAP adapter sessions. This is the root cause of the Zed debugger's pause button never transitioning back to play, and uncaught exceptions leaving the debugger stuck.

**Depends on**: None

**Estimated Time**: 5–7 hours

**Severity**: CRITICAL — all DAP debugging event flows are non-functional without this fix.

### Root Cause Analysis

In `crates/fdemon-app/src/handler/dap_backend.rs:359`:
```rust
let (_, debug_event_rx) = tokio::sync::mpsc::channel::<fdemon_dap::adapter::DebugEvent>(64);
//   ^ sender immediately dropped — channel is dead from birth
```

The comment says "Task 06 will wire the sender back to the Engine" — but this wiring was never implemented.

**Effect chain:**
1. Dart VM sends `PauseBreakpoint`/`PauseException`/`Resume` on Debug stream
2. `forward_vm_events()` in `actions/vm_service.rs` parses → `Message::VmServiceDebugEvent`
3. `handler/devtools/debug.rs::handle_debug_event()` updates `DebugState` on the session
4. Returns `UpdateResult::none()` — **DEAD END**, no forwarding to DAP adapter
5. `session.rs` in `fdemon-dap` has `debug_events.recv()` arm in select loop — **NEVER FIRES** (sender dropped)
6. `adapter.handle_debug_event()` — **NEVER CALLED**
7. IDE never receives `stopped`/`continued`/`thread` events
8. Zed's `ThreadStatus` stays in `Running` state → pause button never becomes play button

### Scope

- `crates/fdemon-app/src/handler/dap_backend.rs`: Store the `mpsc::Sender<DebugEvent>` instead of dropping it
- `crates/fdemon-app/src/engine.rs`: Add storage for per-DAP-client event senders; expose `register_dap_event_sender()` / `unregister_dap_event_sender()`
- `crates/fdemon-app/src/handler/devtools/debug.rs`: Forward VM debug events to registered DAP senders after updating `DebugState`
- `crates/fdemon-app/src/handler/update.rs`: Forward isolate events similarly
- `crates/fdemon-dap/src/server/mod.rs`: Pass a registration callback to the backend factory
- `crates/fdemon-dap/src/adapter/mod.rs`: Verify `handle_debug_event()` correctly maps all event types

### Details

#### Architecture: Event Sender Registration

The key challenge: `VmBackendFactory::create()` runs inside a Tokio task spawned by the TCP accept loop (in `fdemon-dap`), but the TEA handler (in `fdemon-app`) needs to reach the sender. These cross the crate boundary.

**Recommended approach: Shared sender registry via `Arc<Mutex<Vec<mpsc::Sender<DebugEvent>>>>`**

```rust
// In engine.rs — add to Engine:
pub struct Engine {
    // ... existing fields ...
    /// Per-DAP-client debug event senders. The TEA handler iterates these
    /// after each VM debug event to forward to connected DAP adapters.
    dap_debug_senders: Arc<Mutex<Vec<mpsc::Sender<fdemon_dap::adapter::DebugEvent>>>>,
}

impl Engine {
    pub fn dap_debug_senders(&self) -> Arc<Mutex<Vec<mpsc::Sender<fdemon_dap::adapter::DebugEvent>>>> {
        self.dap_debug_senders.clone()
    }
}
```

```rust
// In dap_backend.rs — VmBackendFactory gets the registry:
pub struct VmBackendFactory {
    vm_handle: Arc<Mutex<Option<VmRequestHandle>>>,
    dap_debug_senders: Arc<Mutex<Vec<mpsc::Sender<fdemon_dap::adapter::DebugEvent>>>>,
}

impl BackendFactory for VmBackendFactory {
    fn create(&self) -> Option<BackendHandle> {
        let handle = self.vm_handle.lock().ok()?.as_ref()?.clone();
        let (debug_event_tx, debug_event_rx) = mpsc::channel::<DebugEvent>(64);

        // Register the sender so the TEA handler can forward events
        if let Ok(mut senders) = self.dap_debug_senders.lock() {
            senders.push(debug_event_tx);
        }

        let backend = VmServiceBackend::new(handle);
        // ... construct DynDebugBackend wrapper ...
        Some(BackendHandle { backend, debug_event_rx })
    }
}
```

```rust
// In handler/devtools/debug.rs — forward after DebugState update:
pub fn handle_debug_event(
    state: &mut AppState,
    session_id: &Uuid,
    event: &VmServiceDebugEvent,
    dap_debug_senders: &Arc<Mutex<Vec<mpsc::Sender<DebugEvent>>>>,
) -> UpdateResult {
    // ... existing DebugState update logic ...

    // Forward to DAP adapter(s)
    let dap_event = match event {
        VmServiceDebugEvent::PauseBreakpoint { isolate, .. } =>
            Some(DebugEvent::Paused {
                isolate_id: isolate.id.clone(),
                reason: PauseReason::Breakpoint,
            }),
        VmServiceDebugEvent::PauseException { isolate, exception, .. } =>
            Some(DebugEvent::Paused {
                isolate_id: isolate.id.clone(),
                reason: PauseReason::Exception,
            }),
        VmServiceDebugEvent::PauseInterrupted { isolate, .. } =>
            Some(DebugEvent::Paused {
                isolate_id: isolate.id.clone(),
                reason: PauseReason::Interrupted,
            }),
        VmServiceDebugEvent::Resume { isolate } =>
            Some(DebugEvent::Resumed {
                isolate_id: isolate.id.clone(),
            }),
        _ => None,
    };

    if let Some(ev) = dap_event {
        if let Ok(mut senders) = dap_debug_senders.lock() {
            // Remove closed senders while iterating
            senders.retain(|tx| tx.try_send(ev.clone()).is_ok());
        }
    }

    UpdateResult::none()
}
```

#### Also Forward Isolate Events

`IsolateStart` and `IsolateExit` events from the Isolate stream (handled via `VmServiceIsolateEvent` in `handler/update.rs`) must also be forwarded as `DebugEvent::IsolateStart` / `DebugEvent::IsolateExit` for the adapter's thread map to stay accurate.

#### Clean Up Stale Senders

When a DAP client disconnects, its `mpsc::Receiver` is dropped, causing `try_send` to return `Err(SendError)`. The `retain` call above handles this automatically — stale senders are pruned on the next event.

#### Verify `PauseReason::Exit` Mapping

The adapter maps `PauseReason::Exit` to DAP reason `"breakpoint"` (in `pause_reason_to_dap_str`). This is wrong for `PauseExit` (isolate about to exit). Fix: map to `"exit"` or handle as a special case that sends a `terminated` event instead.

### Acceptance Criteria

1. Pressing Pause in Zed pauses the Flutter app AND the button transitions to Play/Continue
2. Pressing Continue in Zed resumes the Flutter app AND the button transitions back to Pause
3. Hitting a breakpoint causes Zed to show the stopped state with step buttons
4. Uncaught exceptions show in Zed with exception details and can be resumed
5. `thread` events are sent when isolates start and exit
6. Stepping (next/stepIn/stepOut) correctly sends `stopped` then `continued` events
7. Stale DAP senders are cleaned up when clients disconnect
8. All existing tests pass with no regressions
9. 30+ new unit tests for event forwarding and sender lifecycle

### Testing

```rust
#[test]
fn test_debug_event_forwarded_to_dap_senders() {
    // Create a sender registry with one sender
    // Call handle_debug_event with PauseBreakpoint
    // Verify the receiver gets DebugEvent::Paused { reason: Breakpoint }
}

#[test]
fn test_stale_sender_pruned_on_event() {
    // Register two senders, drop one receiver
    // Send an event — verify it goes to the live sender
    // Verify the dead sender is removed from the registry
}

#[test]
fn test_resume_event_forwarded() {
    // Call handle_debug_event with Resume
    // Verify receiver gets DebugEvent::Resumed
}

#[test]
fn test_isolate_events_forwarded() {
    // Call handle_isolate_event with IsolateStart
    // Verify receiver gets DebugEvent::IsolateStart
}

#[test]
fn test_pause_exception_includes_reason() {
    // Call handle_debug_event with PauseException
    // Verify receiver gets DebugEvent::Paused { reason: Exception }
}
```

### Notes

- The `DebugEvent` enum in `fdemon-dap/adapter/mod.rs` must derive `Clone` for the `retain`-based broadcast pattern. Verify this is the case.
- The `dap_debug_senders` `Arc<Mutex<Vec<...>>>` is passed through the Engine → `process_message` → `handle_action` → `VmBackendFactory` chain. Follow the same pattern as `vm_handle_for_dap`.
- The `handle_debug_event` function signature must be extended to accept the sender registry. Thread it through `update()` → `devtools::debug::handle_debug_event()`.
- Consider using `tokio::sync::broadcast` instead of `Vec<mpsc::Sender>` for the many-sender pattern — but `mpsc` with retain is simpler and each sender gets independent backpressure.
