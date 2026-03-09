## Task: Wire Debug Events Through the Message Pipeline

**Objective**: Add `Message` variants for debug/isolate events, `UpdateAction` variants for debug RPCs, and a debug event handler that updates `DebugState` on incoming events. This connects the daemon-layer infrastructure (tasks 01-03) to the app-layer state management (task 04).

**Depends on**: 02-debug-stream-events, 03-debug-rpc-wrappers, 04-session-debug-state

### Scope

- `crates/fdemon-app/src/message.rs` — Add `VmServiceDebugEvent` and `VmServiceIsolateEvent` Message variants
- `crates/fdemon-app/src/handler/mod.rs` — Add `UpdateAction` variants for debug RPC triggers
- `crates/fdemon-app/src/handler/devtools/debug.rs` — **NEW FILE**: Debug event handler
- `crates/fdemon-app/src/handler/devtools/mod.rs` — Add `pub mod debug;` and route debug messages

### Details

#### 1. Add Message variants

In `crates/fdemon-app/src/message.rs`, add to the VM Service messages section (around line 725):

```rust
// --- Debug Events ---

/// A debug stream event from the VM Service (breakpoints, pause, resume, etc.).
VmServiceDebugEvent {
    session_id: SessionId,
    event: fdemon_daemon::vm_service::debugger_types::DebugEvent,
},

/// An isolate lifecycle event from the VM Service.
VmServiceIsolateEvent {
    session_id: SessionId,
    event: fdemon_daemon::vm_service::debugger_types::IsolateEvent,
},
```

These follow the pattern of existing VM Service message variants like `VmServiceLogRecord` (line ~670) and `VmServiceGcEvent` (line ~710).

#### 2. Add UpdateAction variants

In `crates/fdemon-app/src/handler/mod.rs`, add to the `UpdateAction` enum:

```rust
// --- Debug Actions ---

/// Pause an isolate in the VM.
PauseIsolate {
    session_id: SessionId,
    vm_handle: Option<VmRequestHandle>,
    isolate_id: String,
},

/// Resume an isolate, optionally with a step action.
ResumeIsolate {
    session_id: SessionId,
    vm_handle: Option<VmRequestHandle>,
    isolate_id: String,
    step: Option<fdemon_daemon::vm_service::debugger_types::StepOption>,
},

/// Set a breakpoint via URI.
AddBreakpoint {
    session_id: SessionId,
    vm_handle: Option<VmRequestHandle>,
    isolate_id: String,
    script_uri: String,
    line: i32,
    column: Option<i32>,
},

/// Remove a breakpoint by VM Service ID.
RemoveBreakpoint {
    session_id: SessionId,
    vm_handle: Option<VmRequestHandle>,
    isolate_id: String,
    breakpoint_id: String,
},

/// Set the exception pause mode for an isolate.
SetIsolatePauseMode {
    session_id: SessionId,
    vm_handle: Option<VmRequestHandle>,
    isolate_id: String,
    mode: fdemon_daemon::vm_service::debugger_types::ExceptionPauseMode,
},
```

These follow the established `vm_handle: Option<VmRequestHandle>` hydration pattern — the handle starts as `None` and is populated by `process.rs` before dispatch to the async executor.

#### 3. Create debug event handler

Create `crates/fdemon-app/src/handler/devtools/debug.rs`:

```rust
//! # Debug Event Handler
//!
//! Handles VM Service debug and isolate stream events, updating per-session
//! DebugState. This module is the app-layer counterpart to the daemon-layer
//! debug RPCs and event types.

use crate::handler::UpdateResult;
use crate::session::SessionId;
use crate::session::debug_state::PauseReason;
use crate::state::AppState;
use fdemon_daemon::vm_service::debugger_types::{DebugEvent, IsolateEvent};

/// Handles a debug stream event for the given session.
pub fn handle_debug_event(
    state: &mut AppState,
    session_id: SessionId,
    event: DebugEvent,
) -> UpdateResult {
    let Some(session) = state.session_manager.get_session_mut(session_id) else {
        return UpdateResult::none();
    };

    match event {
        DebugEvent::PauseStart { isolate, .. } => {
            session.debug.mark_paused(PauseReason::Entry, isolate.id);
        }
        DebugEvent::PauseBreakpoint { isolate, .. } => {
            session.debug.mark_paused(PauseReason::Breakpoint, isolate.id);
        }
        DebugEvent::PauseException { isolate, .. } => {
            session.debug.mark_paused(PauseReason::Exception, isolate.id);
        }
        DebugEvent::PauseExit { isolate, .. } => {
            session.debug.mark_paused(PauseReason::Exit, isolate.id);
        }
        DebugEvent::PauseInterrupted { isolate, .. } => {
            session.debug.mark_paused(PauseReason::Interrupted, isolate.id);
        }
        DebugEvent::PausePostRequest { isolate, .. } => {
            session.debug.mark_paused(PauseReason::PostRequest, isolate.id);
        }
        DebugEvent::Resume { .. } => {
            session.debug.mark_resumed();
        }
        DebugEvent::BreakpointAdded { breakpoint, .. } => {
            // Breakpoint tracking is managed by the DAP adapter (task 04 TrackedBreakpoint).
            // BreakpointAdded events confirm VM-side creation — log for debugging.
            tracing::debug!("Breakpoint added: {}", breakpoint.id);
        }
        DebugEvent::BreakpointResolved { breakpoint, .. } => {
            session.debug.mark_breakpoint_verified(&breakpoint.id);
        }
        DebugEvent::BreakpointRemoved { breakpoint, .. } => {
            tracing::debug!("Breakpoint removed: {}", breakpoint.id);
        }
        DebugEvent::BreakpointUpdated { breakpoint, .. } => {
            tracing::debug!("Breakpoint updated: {}", breakpoint.id);
        }
        DebugEvent::Inspect { inspectee, .. } => {
            tracing::debug!("Inspect event: {:?}", inspectee.kind);
        }
    }

    UpdateResult::none()
}

/// Handles an isolate lifecycle event for the given session.
pub fn handle_isolate_event(
    state: &mut AppState,
    session_id: SessionId,
    event: IsolateEvent,
) -> UpdateResult {
    let Some(session) = state.session_manager.get_session_mut(session_id) else {
        return UpdateResult::none();
    };

    match event {
        IsolateEvent::IsolateStart { isolate } => {
            session.debug.add_isolate(isolate);
        }
        IsolateEvent::IsolateRunnable { isolate } => {
            // Isolate is ready for VM Service commands.
            // Ensure it's tracked (IsolateStart may have been missed on reconnect).
            session.debug.add_isolate(isolate);
        }
        IsolateEvent::IsolateExit { isolate } => {
            session.debug.remove_isolate(&isolate.id);
            // If the paused isolate exited, clear pause state.
            if session.debug.paused_isolate_id.as_deref() == Some(&isolate.id) {
                session.debug.mark_resumed();
            }
        }
        IsolateEvent::IsolateUpdate { .. } => {
            // Name/metadata change — no action needed for debug state.
        }
        IsolateEvent::IsolateReload { .. } => {
            // Hot reload completed. Breakpoints may need re-verification.
            // Full handling happens in Phase 4 (coordinated reload).
            tracing::debug!("Isolate reload event for session {session_id}");
        }
        IsolateEvent::ServiceExtensionAdded { .. } => {
            // Service extensions are handled by the existing Extension stream handler.
        }
    }

    UpdateResult::none()
}
```

#### 4. Register in devtools handler module

In `crates/fdemon-app/src/handler/devtools/mod.rs`, add:

```rust
pub mod debug;
```

#### 5. Route messages in the main handler

In the main `update()` function (or the devtools dispatch), add match arms for the new message variants:

```rust
Message::VmServiceDebugEvent { session_id, event } => {
    devtools::debug::handle_debug_event(state, session_id, event)
}
Message::VmServiceIsolateEvent { session_id, event } => {
    devtools::debug::handle_isolate_event(state, session_id, event)
}
```

#### 6. Add event routing in process.rs

In the VM Service event processing code (where `VmClientEvent::StreamEvent` is matched by `stream_id`), add routing for the new streams:

```rust
// In the match on event.params.stream_id (or equivalent):
"Debug" => {
    if let Some(debug_event) = debugger_types::parse_debug_event(&event.kind, &event.data) {
        msg_tx.send(Message::VmServiceDebugEvent {
            session_id,
            event: debug_event,
        }).ok();
    }
}
"Isolate" => {
    if let Some(isolate_event) = debugger_types::parse_isolate_event(&event.kind, &event.data) {
        msg_tx.send(Message::VmServiceIsolateEvent {
            session_id,
            event: isolate_event,
        }).ok();
    }
}
```

### Acceptance Criteria

1. `Message::VmServiceDebugEvent` and `Message::VmServiceIsolateEvent` variants exist and compile
2. All `UpdateAction` debug variants compile with the `vm_handle: Option<VmRequestHandle>` pattern
3. `handle_debug_event()` correctly updates `DebugState` for all `DebugEvent` variants
4. `handle_isolate_event()` correctly tracks isolate lifecycle in `DebugState`
5. `PauseBreakpoint` → `mark_paused(Breakpoint, ...)`, `Resume` → `mark_resumed()`, etc.
6. `IsolateExit` clears pause state if the exited isolate was the paused one
7. `BreakpointResolved` → `mark_breakpoint_verified()`
8. Debug/Isolate stream events from `process.rs` are routed to the correct handlers
9. The main `update()` function dispatches the new Message variants to the debug handler
10. `cargo check --workspace` passes (all crates compile together)
11. `cargo test --workspace` passes (no regressions across all 2,525+ existing tests)
12. `cargo clippy --workspace -- -D warnings` passes

### Testing

Test the handler functions with mock `AppState` containing sessions with `DebugState`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_state_with_session() -> (AppState, SessionId) {
        // Create AppState with a session that has DebugState::default()
        // Follow the pattern from existing handler tests
        todo!()
    }

    #[test]
    fn test_pause_breakpoint_updates_debug_state() {
        let (mut state, session_id) = make_test_state_with_session();
        let event = DebugEvent::PauseBreakpoint {
            isolate: IsolateRef { id: "isolates/1".into(), name: Some("main".into()) },
            top_frame: None,
            breakpoint: None,
            pause_breakpoints: vec![],
            at_async_suspension: false,
        };

        let result = handle_debug_event(&mut state, session_id, event);
        assert!(result.action.is_none());

        let session = state.session_manager.get_session(session_id).unwrap();
        assert!(session.debug.paused);
        assert_eq!(session.debug.pause_reason, Some(PauseReason::Breakpoint));
    }

    #[test]
    fn test_resume_clears_pause_state() {
        let (mut state, session_id) = make_test_state_with_session();
        // First pause...
        let pause_event = DebugEvent::PauseBreakpoint {
            isolate: IsolateRef { id: "isolates/1".into(), name: Some("main".into()) },
            top_frame: None, breakpoint: None, pause_breakpoints: vec![], at_async_suspension: false,
        };
        handle_debug_event(&mut state, session_id, pause_event);

        // Then resume...
        let resume_event = DebugEvent::Resume {
            isolate: IsolateRef { id: "isolates/1".into(), name: Some("main".into()) },
        };
        handle_debug_event(&mut state, session_id, resume_event);

        let session = state.session_manager.get_session(session_id).unwrap();
        assert!(!session.debug.paused);
        assert!(session.debug.pause_reason.is_none());
    }

    #[test]
    fn test_isolate_start_tracks_isolate() {
        let (mut state, session_id) = make_test_state_with_session();
        let event = IsolateEvent::IsolateStart {
            isolate: IsolateRef { id: "isolates/1".into(), name: Some("main".into()) },
        };

        handle_isolate_event(&mut state, session_id, event);

        let session = state.session_manager.get_session(session_id).unwrap();
        assert_eq!(session.debug.isolates.len(), 1);
    }

    #[test]
    fn test_isolate_exit_clears_pause_if_paused_isolate() {
        let (mut state, session_id) = make_test_state_with_session();

        // Pause on isolate 1
        let pause = DebugEvent::PauseBreakpoint {
            isolate: IsolateRef { id: "isolates/1".into(), name: Some("main".into()) },
            top_frame: None, breakpoint: None, pause_breakpoints: vec![], at_async_suspension: false,
        };
        handle_debug_event(&mut state, session_id, pause);

        // Isolate 1 exits
        let exit = IsolateEvent::IsolateExit {
            isolate: IsolateRef { id: "isolates/1".into(), name: Some("main".into()) },
        };
        handle_isolate_event(&mut state, session_id, exit);

        let session = state.session_manager.get_session(session_id).unwrap();
        assert!(!session.debug.paused);
    }

    #[test]
    fn test_unknown_session_returns_none() {
        let mut state = AppState::default(); // or minimal test state
        let event = DebugEvent::Resume {
            isolate: IsolateRef { id: "isolates/1".into(), name: None },
        };
        let result = handle_debug_event(&mut state, 999, event);
        assert!(result.action.is_none());
        assert!(result.message.is_none());
    }
}
```

### Notes

- The `UpdateAction` debug variants are defined now but not dispatched by the TUI/headless runners until Phase 2. This is fine — they need to exist for the handler to produce them, but the async executor wiring comes with the DAP server.
- The `process.rs` routing for debug/isolate events uses `parse_debug_event()` / `parse_isolate_event()` from task 01. If those return `None` (unrecognized event), the event is silently dropped — this is the established pattern for forward-compatibility.
- The handler currently returns `UpdateResult::none()` for all debug events. In Phase 3-4, some events will return `UpdateAction` variants (e.g., `PauseBreakpoint` might trigger a DAP `stopped` event). The handler structure supports this — just change the return value.
- The `Debug` stream will fire events even when no DAP client is connected. The handler updates `DebugState` regardless, which is correct — the state should reflect reality, and the DAP adapter checks `dap_attached` when deciding whether to send DAP events.
- The `DebugEvent` and `IsolateEvent` types must implement `Send` to flow through the message channel. Since they only contain `String`, `Option`, `Vec`, and serde types, this is automatic.

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/message.rs` | Added `VmServiceDebugEvent` and `VmServiceIsolateEvent` Message variants (with doc comments), placed in a new "VM Service Debug Messages" section before the network messages |
| `crates/fdemon-app/src/handler/mod.rs` | Added 5 new `UpdateAction` debug variants: `PauseIsolate`, `ResumeIsolate`, `AddBreakpoint`, `RemoveBreakpoint`, `SetIsolatePauseMode`, each with `vm_handle: Option<VmRequestHandle>` and full doc comments |
| `crates/fdemon-app/src/handler/devtools/debug.rs` | **NEW FILE** - `handle_debug_event()` and `handle_isolate_event()` handler functions, plus 19 unit tests covering all pause reasons, resume, breakpoint resolved, isolate lifecycle, and unknown-session edge cases |
| `crates/fdemon-app/src/handler/devtools/mod.rs` | Added `pub mod debug;` declaration |
| `crates/fdemon-app/src/handler/update.rs` | Added routing arms for `VmServiceDebugEvent` and `VmServiceIsolateEvent` in the TEA `update()` function |
| `crates/fdemon-app/src/actions/vm_service.rs` | Added imports for `parse_debug_event` and `parse_isolate_event`; added Debug and Isolate stream event routing in `forward_vm_events()` using `stream_id` matching |
| `crates/fdemon-app/src/actions/mod.rs` | Added match arms for all 5 new `UpdateAction` debug variants (log at debug level, Phase 2 wiring placeholder) |

### Notable Decisions/Tradeoffs

1. **`get_session_mut()` vs `get_mut()`**: The task plan used `state.session_manager.get_session_mut(session_id)` in the handler template, but the actual API is `state.session_manager.get_mut(session_id)`. Used the correct API; handlers access `handle.session.debug` via the `SessionHandle`.

2. **`stream_id` routing vs parser-cascade pattern**: Existing code uses a cascade of parser calls without checking `stream_id` first. For Debug/Isolate events, we check `event.params.stream_id` before calling the specific parser. This is more correct because `parse_debug_event` and `parse_isolate_event` take separate `kind` and `data` arguments (not the whole `StreamEvent`), and routing by stream_id avoids attempting debug parsing on unrelated streams.

3. **Debug action placeholders**: The 5 new `UpdateAction` debug variants (`PauseIsolate`, `ResumeIsolate`, `AddBreakpoint`, `RemoveBreakpoint`, `SetIsolatePauseMode`) are wired with `tracing::debug!()` stubs in `actions/mod.rs`. They satisfy the exhaustive match requirement and will be wired to actual async executors in Phase 2 (DAP server).

4. **No process.rs hydration functions**: The new debug actions' `vm_handle` fields will be hydrated in Phase 2 when the DAP server properly integrates. The existing hydration chain in `process.rs` passes all unknown actions through unchanged via `Some(action)`, so the debug actions reach `handle_action` and hit the debug log stub.

### Testing Performed

- `cargo fmt --all` - Passed
- `cargo check --workspace` - Passed
- `cargo test --workspace` - Passed (2,824+ tests, 0 failures; 19 new tests in `handler::devtools::debug::tests`)
- `cargo clippy --workspace -- -D warnings` - Passed (0 warnings)

### Risks/Limitations

1. **Phase 2 hydration**: The debug `UpdateAction` variants reach `handle_action` without VM handle hydration. This is intentional — Phase 2 will add hydration functions in `process.rs` and actual async executor dispatch in `actions/mod.rs`.

2. **Parse-then-route ordering**: The Debug stream routing uses `continue` so that Debug/Isolate events are not double-parsed as GC or Log events. The ordering matters: Extension/Frame/GC/Log parsers run first (by design, as they only match specific event kinds), then stream_id routing handles Debug and Isolate.
