## Task: Fix headless runner log re-emission bug

**Objective**: Prevent duplicate log entries in headless NDJSON output by tracking which logs have already been emitted across message loop iterations.

**Review Issue**: #2 (MAJOR) - Headless runner re-emits last log on every message cycle

**Depends on**: None

### Scope

- `src/headless/runner.rs`: Rewrite `emit_post_message_events()` with index tracking
- `src/headless/mod.rs`: No changes needed (HeadlessEvent::emit() is fine)

### Details

#### The Bug

`emit_post_message_events()` (runner.rs:92-114) always takes the last log entry via `.iter().rev().take(1)` and emits it as NDJSON. There is no tracking of which logs have been previously emitted. The function is called on **every** message cycle -- including non-log messages like keyboard events, ticks, file watcher events, and reload completions. When these non-log messages arrive, no new log is added, but the last log is re-emitted.

The code itself contains TODO comments acknowledging this:
- Line 93-95: `"Note: This is a simplified version. In a full implementation, we'd track which logs have been emitted already."`
- Line 97: `"Get the last few logs (we'd ideally track the last emitted index)"`

#### Impact

E2E test consumers parsing headless output would see duplicate log lines. For example, if a session has 10 logs and 5 non-log messages arrive, log #10 would appear 6 times (1 original + 5 duplicates). This corrupts test assertions that count log events (e.g., `tests/e2e/scripts/test_startup.sh:160`).

#### The Fix

Add a `last_emitted_log_count: usize` local variable in `headless_event_loop()` and pass it to `emit_post_message_events()`. After emitting new logs, update the counter.

```rust
// In headless_event_loop():
let mut last_emitted_log_count: usize = 0;

// In the main loop body, after flush_pending_logs():
emit_post_message_events(&engine.state, &mut last_emitted_log_count);
```

Rewrite `emit_post_message_events()`:

```rust
fn emit_post_message_events(state: &AppState, last_emitted: &mut usize) {
    if let Some(session) = state.session_manager.selected() {
        let current_count = session.session.logs.len();

        // Handle VecDeque eviction: if logs were evicted from front,
        // our index may be past the current length
        if *last_emitted > current_count {
            *last_emitted = 0; // Reset -- we lost track due to eviction
        }

        if current_count > *last_emitted {
            // Emit only new logs (skip already-emitted ones)
            for log in session.session.logs.iter().skip(*last_emitted) {
                HeadlessEvent::Log {
                    message: log.message.clone(),
                    level: log.level.to_string().to_lowercase(),
                    timestamp: log.timestamp.timestamp_millis(),
                    source: log.source.to_string().to_lowercase(),
                }
                .emit();
            }
            *last_emitted = current_count;
        }
    }
}
```

#### Edge Case: VecDeque Ring Buffer Eviction

`Session.logs` is a `VecDeque<LogEntry>`. If the log buffer has a maximum size and older entries are evicted from the front, `current_count` could temporarily be less than `last_emitted`. The fix handles this by resetting `last_emitted` to 0 when this is detected. This may cause a few logs to be re-emitted after eviction, but this is the safe/correct behavior (better to duplicate a few than to miss new logs).

#### Why Not Use Engine's Broadcast Channel?

The Engine already correctly tracks log deltas via `StateSnapshot` and emits `EngineEvent::LogEntry`/`EngineEvent::LogBatch`. However, there is a timing issue: `process_message()` captures the post-snapshot BEFORE `flush_pending_logs()` runs, so logs added to the pending buffer during processing but only moved to `session.logs` by the subsequent `flush_pending_logs()` call may be missed by the Engine's diff. The local index tracking approach avoids this timing issue entirely.

### Acceptance Criteria

1. `emit_post_message_events()` only emits logs that haven't been previously emitted
2. Non-log messages (ticks, key events, reload completions) do NOT cause duplicate log output
3. VecDeque eviction edge case is handled (index clamping)
4. The TODO comments about tracking are removed (they are now addressed)
5. `cargo test --workspace --lib` passes
6. `cargo clippy --workspace --lib -- -D warnings` passes

### Testing

Add unit tests for the emission tracking logic. Since `emit_post_message_events` writes to stdout (via `HeadlessEvent::emit()`), test the index-tracking logic separately:

```rust
#[cfg(test)]
mod tests {
    #[test]
    fn test_last_emitted_advances_with_new_logs() {
        // Setup: session with 3 logs, last_emitted = 0
        // Act: call emit function
        // Assert: last_emitted now equals 3
    }

    #[test]
    fn test_no_emission_when_no_new_logs() {
        // Setup: session with 3 logs, last_emitted = 3
        // Act: call emit function
        // Assert: last_emitted still 3, no output
    }

    #[test]
    fn test_eviction_resets_index() {
        // Setup: last_emitted = 100, but session.logs.len() = 50 (eviction happened)
        // Act: call emit function
        // Assert: last_emitted reset, new logs emitted from beginning
    }
}
```

### Notes

- The state is tracked as a local variable in `headless_event_loop()`, NOT in Engine or AppState. This is headless-runner-specific emission tracking.
- A future architectural improvement (Option B from the review) would be to subscribe to Engine's broadcast channel, but that requires fixing the snapshot timing in Engine -- a larger change deferred to a separate task.

---

## Completion Summary

**Status:** Not Started
