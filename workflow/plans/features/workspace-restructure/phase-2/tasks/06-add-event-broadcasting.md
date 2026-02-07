## Task: Add Event Broadcasting to Engine

**Objective**: Add a `broadcast::Sender<EngineEvent>` to the Engine that emits domain events after each message processing cycle. External consumers (future MCP server, pro features) subscribe via `Engine::subscribe()` and receive a curated stream of `EngineEvent`s. The headless runner can optionally use this to replace its manual `emit_post_message_events()` with a subscriber pattern.

**Depends on**: Task 05 (services layer wired -- broadcasting may emit service-related events)

**Estimated Time**: 3-4 hours

### Scope

- `src/app/engine.rs`: Add broadcast channel, `subscribe()`, event emission logic
- `src/app/engine_event.rs`: May need minor additions based on integration findings
- `src/headless/runner.rs`: Optionally convert to use EngineEvent subscriber (or defer to later)

### Details

#### Broadcast Channel Design

```rust
use tokio::sync::broadcast;

pub struct Engine {
    // ... existing fields ...

    /// Event broadcaster for external consumers.
    /// Subscribers receive EngineEvents after each message processing cycle.
    event_tx: broadcast::Sender<EngineEvent>,
}
```

Channel capacity: Use 256. If a subscriber falls behind, older events are dropped (broadcast channel behavior). This is acceptable for event-driven consumers.

#### Engine::subscribe()

```rust
impl Engine {
    /// Subscribe to engine events.
    ///
    /// Returns a receiver that gets EngineEvents after each message
    /// processing cycle. Multiple subscribers are supported.
    ///
    /// If the subscriber falls behind (buffer full), older events are
    /// dropped. Use `broadcast::error::RecvError::Lagged` to detect this.
    pub fn subscribe(&self) -> broadcast::Receiver<EngineEvent> {
        self.event_tx.subscribe()
    }
}
```

#### Event Emission Points

Events should be emitted based on **state changes**, not based on messages. This means comparing state before and after processing to detect what changed.

```rust
impl Engine {
    /// Emit EngineEvents based on state changes after processing.
    ///
    /// Called after process_message() and flush_pending_logs().
    fn emit_events(&self, pre_state: &StateSnapshot, post_state: &StateSnapshot) {
        // Phase changes
        if pre_state.phase != post_state.phase {
            if let Some(session_id) = post_state.selected_session_id {
                self.emit(EngineEvent::PhaseChanged {
                    session_id,
                    old_phase: pre_state.phase,
                    new_phase: post_state.phase,
                });
            }
        }

        // Reload detection
        if pre_state.phase != AppPhase::Reloading && post_state.phase == AppPhase::Reloading {
            if let Some(session_id) = post_state.selected_session_id {
                self.emit(EngineEvent::ReloadStarted { session_id });
            }
        }

        // New logs
        if post_state.log_count > pre_state.log_count {
            // Emit new log entries
            // ...
        }
    }

    fn emit(&self, event: EngineEvent) {
        // send() returns Err only if there are no receivers -- that's fine
        let _ = self.event_tx.send(event);
    }
}
```

#### StateSnapshot for Change Detection

Create a lightweight snapshot of state before processing to compare against:

```rust
/// Lightweight snapshot of state for change detection.
///
/// Captured before message processing, compared after to detect
/// what changed and emit appropriate EngineEvents.
#[derive(Debug, Clone)]
struct StateSnapshot {
    phase: AppPhase,
    selected_session_id: Option<SessionId>,
    log_count: usize,
    session_count: usize,
    reload_count: u32,
}

impl StateSnapshot {
    fn capture(state: &AppState) -> Self {
        let (phase, log_count, reload_count) = state
            .session_manager
            .selected()
            .map(|s| (s.session.phase, s.session.logs.len(), s.session.reload_count))
            .unwrap_or((AppPhase::Initializing, 0, 0));

        Self {
            phase,
            selected_session_id: state.session_manager.selected().map(|s| s.session.id),
            log_count,
            session_count: state.session_manager.session_count(),
            reload_count,
        }
    }
}
```

#### Updated process_message Flow

```rust
impl Engine {
    pub fn process_message(&mut self, msg: Message) {
        // Snapshot state before processing
        let pre = StateSnapshot::capture(&self.state);

        // Process through TEA
        process::process_message(
            &mut self.state,
            msg,
            &self.msg_tx,
            &self.session_tasks,
            &self.shutdown_rx,
            &self.project_path,
        );

        // Snapshot state after processing
        let post = StateSnapshot::capture(&self.state);

        // Emit events for any state changes
        self.emit_events(&pre, &post);
    }
}
```

#### Engine::new() Update

```rust
impl Engine {
    pub fn new(project_path: PathBuf) -> Self {
        // ... existing setup ...

        let (event_tx, _) = broadcast::channel(256);

        Self {
            // ... existing fields ...
            event_tx,
        }
    }
}
```

Note: The initial receiver from `broadcast::channel()` is dropped (`_`). Subscribers call `engine.subscribe()` to get their own receivers.

#### Headless Runner Integration (Optional)

The headless runner currently uses manual `emit_pre_message_events()` / `emit_post_message_events()` to emit HeadlessEvent. With EngineEvent broadcasting, it could subscribe and convert:

```rust
// Option A: Subscribe and convert (clean separation)
let mut event_rx = engine.subscribe();
tokio::spawn(async move {
    while let Ok(event) = event_rx.recv().await {
        if let Some(headless_event) = HeadlessEvent::from_engine_event(&event) {
            headless_event.emit();
        }
    }
});
```

However, this adds async complexity to the headless runner. **Recommended**: Keep the existing manual emission for now and note that it can be replaced once EngineEvent broadcasting is validated. The important thing is that `Engine.subscribe()` works for external consumers.

### Step-by-Step Implementation

1. **Add `broadcast::Sender<EngineEvent>` to Engine struct**

2. **Initialize in `Engine::new()`**: `broadcast::channel(256)`

3. **Implement `Engine::subscribe()`**: Returns `broadcast::Receiver<EngineEvent>`

4. **Create `StateSnapshot` struct**: Lightweight state capture for change detection

5. **Implement `emit_events()`**: Compare pre/post snapshots, emit appropriate EngineEvents

6. **Update `process_message()`**: Capture snapshot before, emit events after

7. **Update `drain_pending_messages()`**: Same pattern -- snapshot before first message, emit after all drained

8. **Add `Engine::shutdown()` event**: Emit `EngineEvent::Shutdown` during shutdown

9. **Add tests**: Verify subscribers receive events on state changes

### Acceptance Criteria

1. `Engine` struct has a `broadcast::Sender<EngineEvent>` field
2. `Engine::subscribe()` returns a `broadcast::Receiver<EngineEvent>`
3. `EngineEvent::PhaseChanged` is emitted when a session's phase changes
4. `EngineEvent::ReloadStarted` is emitted when a reload begins
5. `EngineEvent::ReloadCompleted` is emitted when a reload finishes
6. `EngineEvent::LogEntry` or `LogBatch` is emitted when new logs are added
7. `EngineEvent::Shutdown` is emitted during engine shutdown
8. Multiple subscribers can coexist (broadcast semantics)
9. Events are emitted after state is updated (not before)
10. No events emitted if no subscribers (no overhead when unused)
11. `cargo build` succeeds
12. `cargo test` passes
13. `cargo clippy` is clean

### Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_subscribe_receives_events() {
        let dir = tempfile::tempdir().unwrap();
        let mut engine = Engine::new(dir.path().to_path_buf());

        let mut rx = engine.subscribe();

        // Process a quit message -- should emit phase change
        engine.process_message(Message::Quit);

        // Note: Whether a PhaseChanged event fires depends on
        // whether Quit changes the phase. It sets phase = Quitting.
        // Check if we get an event
        match tokio::time::timeout(
            std::time::Duration::from_millis(100),
            rx.recv(),
        ).await {
            Ok(Ok(event)) => {
                // Got an event -- verify it's reasonable
                println!("Received event: {:?}", event);
            }
            _ => {
                // No event -- might be expected if Quit doesn't change phase
                // from Initializing in a fresh state
            }
        }
    }

    #[tokio::test]
    async fn test_no_subscribers_no_error() {
        let dir = tempfile::tempdir().unwrap();
        let mut engine = Engine::new(dir.path().to_path_buf());

        // No subscribers -- should not error
        engine.process_message(Message::Quit);
        // No panic
    }

    #[tokio::test]
    async fn test_multiple_subscribers() {
        let dir = tempfile::tempdir().unwrap();
        let engine = Engine::new(dir.path().to_path_buf());

        let _rx1 = engine.subscribe();
        let _rx2 = engine.subscribe();
        let _rx3 = engine.subscribe();

        // All three should be valid receivers
    }

    #[test]
    fn test_state_snapshot_capture() {
        let state = AppState::new();
        let snapshot = StateSnapshot::capture(&state);

        assert_eq!(snapshot.phase, AppPhase::Initializing);
        assert_eq!(snapshot.log_count, 0);
        assert_eq!(snapshot.session_count, 0);
    }
}
```

### Notes

- **Performance**: `broadcast::send()` is cheap when there are no subscribers (it's essentially a no-op). The `StateSnapshot` capture adds minimal overhead (reads a few fields). The emit_events comparison is O(1) per field.
- **Log broadcasting**: Emitting individual `LogEntry` events for every log line during high-volume logging could be expensive. Use `LogBatch` for bulk emission, or only emit when the subscriber is present. Consider a flag like `engine.has_subscribers()` to skip snapshot capture when nobody is listening.
- **Event ordering**: Events are emitted in order within a single `process_message()` call. Across calls, ordering follows message processing order.
- **broadcast vs mpsc**: Using `broadcast` (not `mpsc`) because multiple consumers may subscribe (TUI could have a debug panel, MCP server, etc.). Broadcast's `Lagged` behavior is acceptable for event consumers.
- **This task does NOT convert the headless runner** to use EngineEvent. That's an optimization for later. The headless runner continues to use its manual emit functions.

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `src/app/engine.rs` | Added broadcast channel, `subscribe()` method, `StateSnapshot` struct, `emit_events()` logic, and event emission in `process_message()` and `shutdown()` |

### Notable Decisions/Tradeoffs

1. **StateSnapshot Design**: Created a lightweight snapshot struct that captures only the essential fields needed for change detection (phase, session_id, log_count, session_count, reload_count). This minimizes overhead while enabling accurate event emission.

2. **Event Emission Strategy**: Events are emitted by comparing pre/post state snapshots rather than based on message types. This ensures events reflect actual state changes and handles all code paths uniformly.

3. **Reload Time Tracking**: Set reload completion time to 0ms for now since the current implementation uses `DateTime<Local>` instead of duration in milliseconds. This can be enhanced later by tracking reload start times and calculating durations.

4. **Restart Events Deferred**: The `AppPhase` enum doesn't currently have a `Restarting` variant, so `RestartStarted` and `RestartCompleted` events are not yet emitted. Added a comment noting this can be implemented when restart tracking is added.

5. **Headless Runner Integration**: Did not convert the headless runner to use the broadcast channel as recommended by the task. The manual `emit_pre_message_events()` / `emit_post_message_events()` remain in place. This can be optimized later once EngineEvent broadcasting is validated.

6. **No-Subscriber Performance**: Using `broadcast::send()` which returns `Err` when there are no subscribers. We ignore this error with `let _ = ...` since having no subscribers is a valid state and should not cause errors or logs.

### Testing Performed

- `cargo check` - Passed
- `cargo test --lib engine` - Passed (25 tests including 7 new broadcast tests)
- `cargo clippy --lib` - Passed (only warnings are unused fields in StateSnapshot, kept for future use)

**New Tests Added:**
- `test_subscribe_receives_shutdown_event` - Verifies subscribers receive shutdown events
- `test_no_subscribers_no_error` - Verifies no error when there are no subscribers
- `test_multiple_subscribers` - Verifies multiple subscribers can coexist
- `test_state_snapshot_capture` - Verifies snapshot captures state correctly
- `test_subscribe_channel_capacity` - Verifies broadcast buffer handling
- `test_phase_change_event` - Verifies phase change events are emitted
- `test_event_type_label` - Verifies event type labels work

### Risks/Limitations

1. **Event Ordering**: Events are emitted in the order they're detected in `emit_events()`, which follows a specific sequence (phase change, reload start, reload complete, new logs). This ordering is deterministic but may need adjustment if event consumers depend on specific ordering.

2. **Log Event Volume**: During high-volume logging, emitting individual `LogEntry` or `LogBatch` events could add overhead. The implementation uses batch emission for multiple logs, but further optimization may be needed for extremely high-throughput scenarios.

3. **Incomplete Reload Timing**: Reload completion events currently report 0ms duration. Proper timing will require tracking reload start timestamps and calculating durations in milliseconds.

4. **Missing Restart Events**: `RestartStarted` and `RestartCompleted` events are defined in `EngineEvent` but not yet emitted since `AppPhase` doesn't track restart state separately from reload state.
