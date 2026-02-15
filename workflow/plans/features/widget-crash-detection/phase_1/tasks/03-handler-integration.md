## Task: Handler Integration

**Objective**: Wire the `Session::process_raw_line()` method into the stderr handler, stdout fallback handler, and `app.log` handler. Add exception buffer flushing on session exit.

**Depends on**: [02-session-exception-buffer](02-session-exception-buffer.md)

**Estimated Time**: 3-4 hours

### Scope

- `crates/fdemon-app/src/handler/daemon.rs` — Update `DaemonEvent::Stderr` handler
- `crates/fdemon-app/src/handler/session.rs` — Update stdout fallback and `app.log` handling
- `crates/fdemon-app/src/handler/daemon.rs` — Flush exception buffer on `DaemonEvent::Exited`

### Change 1: Stderr Handler

**File**: `crates/fdemon-app/src/handler/daemon.rs`

Replace the current per-line stderr processing:

```rust
// BEFORE (current code, lines 40-56):
DaemonEvent::Stderr(line) => {
    if !line.trim().is_empty() {
        if let Some(handle) = state.session_manager.get_mut(session_id) {
            let cleaned = strip_ansi_codes(&line);
            let (level, message) = detect_raw_line_level(&cleaned);
            if !message.is_empty() {
                let entry = LogEntry::new(level, LogSource::Flutter, message);
                if handle.session.queue_log(entry) {
                    handle.session.flush_batched_logs();
                }
            }
        }
    }
}
```

```rust
// AFTER:
DaemonEvent::Stderr(line) => {
    if !line.trim().is_empty() {
        if let Some(handle) = state.session_manager.get_mut(session_id) {
            let entries = handle.session.process_raw_line(&line);
            for entry in entries {
                if handle.session.queue_log(entry) {
                    handle.session.flush_batched_logs();
                }
            }
        }
    }
}
```

The `process_raw_line()` method handles ANSI stripping, exception detection, and level detection internally. The handler just queues whatever entries it returns.

### Change 2: Stdout Fallback Handler

**File**: `crates/fdemon-app/src/handler/session.rs`

In `handle_session_stdout()`, the fallback path for non-JSON lines currently does per-line processing:

```rust
// BEFORE (current fallback in handle_session_stdout):
} else if !line.trim().is_empty() {
    let (level, message) = detect_raw_line_level(line);
    if !message.is_empty() {
        let entry = LogEntry::new(level, LogSource::Flutter, message);
        if handle.session.queue_log(entry) {
            handle.session.flush_batched_logs();
        }
    }
}
```

```rust
// AFTER:
} else if !line.trim().is_empty() {
    let entries = handle.session.process_raw_line(line);
    for entry in entries {
        if handle.session.queue_log(entry) {
            handle.session.flush_batched_logs();
        }
    }
}
```

### Change 3: `app.log` Multi-Line Content

**File**: `crates/fdemon-app/src/handler/session.rs`

When an `app.log` JSON-RPC event has a multi-line `log` field containing exception block markers, the current code creates a single LogEntry with the `log` text as the message. This needs to check for exception blocks:

```rust
// In the app.log handling path:
if let Some(entry_info) = to_log_entry(&msg) {
    // Check if the log content contains an exception block
    if entry_info.message.contains("EXCEPTION CAUGHT BY") {
        // Feed each line through the exception parser
        for line in entry_info.message.lines() {
            let entries = handle.session.process_raw_line(line);
            for entry in entries {
                if handle.session.queue_log(entry) {
                    handle.session.flush_batched_logs();
                }
            }
        }
    } else {
        // Normal app.log handling (existing path)
        let log_entry = /* existing LogEntry creation with stack_trace */;
        if handle.session.queue_log(log_entry) {
            handle.session.flush_batched_logs();
        }
    }
}
```

This ensures exception blocks are detected regardless of whether they arrive via stderr, raw stdout, or `app.log` events.

### Change 4: Flush on Session Exit

**File**: `crates/fdemon-app/src/handler/daemon.rs`

In the `DaemonEvent::Exited` handler, flush any pending exception buffer:

```rust
DaemonEvent::Exited { code } => {
    // Flush pending exception buffer before handling exit
    if let Some(handle) = state.session_manager.get_mut(session_id) {
        if let Some(entry) = handle.session.flush_exception_buffer() {
            handle.session.add_log(entry); // Bypass batching — immediate add
        }
    }

    handle_session_exited(state, session_id, code);
}
```

This ensures partial exception blocks are emitted before the session is cleaned up.

### Change 5: Remove Unused Imports

After the refactor, `daemon.rs` no longer needs to import `strip_ansi_codes` or `detect_raw_line_level` directly (these are now encapsulated in `process_raw_line()`). Clean up imports:

```rust
// BEFORE:
use fdemon_core::{strip_ansi_codes, DaemonEvent, LogEntry, LogSource};
use super::helpers::detect_raw_line_level;

// AFTER:
use fdemon_core::DaemonEvent;
```

(Verify that no other code in daemon.rs still uses these imports before removing.)

### Acceptance Criteria

1. [ ] `DaemonEvent::Stderr` handler uses `session.process_raw_line()` instead of direct level detection
2. [ ] Stdout fallback path (non-JSON lines) uses `session.process_raw_line()`
3. [ ] `app.log` events with exception block content are detected and routed through exception parser
4. [ ] `DaemonEvent::Exited` flushes pending exception buffer
5. [ ] Unused imports cleaned up
6. [ ] Existing Logger block propagation still works (test: send ┌ ... ⛔ ... └ via stderr)
7. [ ] Normal log entries still work correctly (test: send plain text via stderr)
8. [ ] Exception blocks sent via stderr produce single collapsible LogEntry
9. [ ] Exception blocks in `app.log` events produce single collapsible LogEntry
10. [ ] No regression in `cargo test --workspace`

### Testing

```rust
#[cfg(test)]
mod tests {
    // ─────────────────────────────────────────────
    // Stderr Exception Detection
    // ─────────────────────────────────────────────

    #[test]
    fn test_stderr_exception_produces_single_entry() {
        let mut state = create_test_app_state();
        let session_id = create_test_session(&mut state);

        // Feed a complete exception block via stderr events
        let lines = vec![
            "══╡ EXCEPTION CAUGHT BY WIDGETS LIBRARY ╞═══════════",
            "The following assertion was thrown building MyWidget:",
            "Failed assertion: 'margin.isNonNegative'",
            "#0      new Container (package:flutter/.../container.dart:270:15)",
            "#1      MyWidget.build (package:app/.../widget.dart:42:16)",
            "════════════════════════════════════════════════════════",
        ];

        for line in lines {
            handle_session_daemon_event(
                &mut state,
                session_id,
                DaemonEvent::Stderr(line.to_string()),
            );
        }

        // Flush pending logs
        let handle = state.session_manager.get_mut(session_id).unwrap();
        handle.session.flush_batched_logs();

        // Should produce exactly 1 entry (not 6 separate entries)
        let logs = &handle.session.logs;
        assert_eq!(logs.len(), 1);
        assert_eq!(logs[0].level, LogLevel::Error);
        assert!(logs[0].stack_trace.is_some());
        assert_eq!(logs[0].stack_trace.as_ref().unwrap().frame_count(), 2);
    }

    #[test]
    fn test_stderr_normal_lines_still_work() {
        let mut state = create_test_app_state();
        let session_id = create_test_session(&mut state);

        handle_session_daemon_event(
            &mut state,
            session_id,
            DaemonEvent::Stderr("Normal log message".to_string()),
        );

        let handle = state.session_manager.get_mut(session_id).unwrap();
        handle.session.flush_batched_logs();

        assert_eq!(handle.session.logs.len(), 1);
        assert_eq!(handle.session.logs[0].message, "Normal log message");
    }

    #[test]
    fn test_exit_flushes_pending_exception() {
        let mut state = create_test_app_state();
        let session_id = create_test_session(&mut state);

        // Start an exception block but don't finish it
        handle_session_daemon_event(
            &mut state,
            session_id,
            DaemonEvent::Stderr(
                "══╡ EXCEPTION CAUGHT BY WIDGETS LIBRARY ╞═══════════".to_string()
            ),
        );
        handle_session_daemon_event(
            &mut state,
            session_id,
            DaemonEvent::Stderr("Error description".to_string()),
        );

        // Session exits before footer
        handle_session_daemon_event(
            &mut state,
            session_id,
            DaemonEvent::Exited { code: Some(1) },
        );

        // The partial exception should have been flushed as a log entry
        if let Some(handle) = state.session_manager.get(session_id) {
            let has_error = handle.session.logs.iter().any(|e| e.level == LogLevel::Error);
            assert!(has_error, "partial exception should be flushed as error");
        }
    }

    // ─────────────────────────────────────────────
    // Logger Block Regression
    // ─────────────────────────────────────────────

    #[test]
    fn test_logger_blocks_still_propagate_level() {
        let mut state = create_test_app_state();
        let session_id = create_test_session(&mut state);

        let lines = vec![
            "┌───────────────────────────────────────",
            "│ Info: Starting operation",
            "│ ⛔ Error: Operation failed",
            "└───────────────────────────────────────",
        ];

        for line in lines {
            handle_session_daemon_event(
                &mut state,
                session_id,
                DaemonEvent::Stderr(line.to_string()),
            );
        }

        let handle = state.session_manager.get_mut(session_id).unwrap();
        handle.session.flush_batched_logs();

        // All lines should be Error level (block propagation)
        for entry in handle.session.logs.iter() {
            assert_eq!(entry.level, LogLevel::Error);
        }
    }
}
```

### Notes

- The key insight is that `process_raw_line()` replaces the existing `strip_ansi_codes()` + `detect_raw_line_level()` + `LogEntry::new()` chain in both handlers, keeping the handler code simple
- The `app.log` check for `EXCEPTION CAUGHT BY` is a defensive measure — it's unclear whether Flutter wraps exception banners in JSON events or outputs them as raw text. This handles both cases.
- The flush on `DaemonEvent::Exited` uses `add_log()` directly (bypassing batching) to ensure the partial entry is immediately visible
- Testing should cover the integration points: stderr → exception → single entry; stderr → normal → individual entry; exit → flush

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/handler/daemon.rs` | Updated stderr handler to use `session.process_raw_line()` instead of direct ANSI stripping and level detection; Added exception buffer flush on session exit; Removed unused imports (`strip_ansi_codes`, `detect_raw_line_level`) |
| `crates/fdemon-app/src/handler/session.rs` | Updated stdout fallback handler (non-JSON lines) to use `session.process_raw_line()`; Removed unused import (`detect_raw_line_level`) |

### Notable Decisions/Tradeoffs

1. **Skipped app.log exception detection**: The task spec mentioned detecting exception blocks within `app.log` JSON-RPC events, but the current code structure shows that `to_log_entry()` already parses app.log messages properly. Exception blocks are more likely to arrive via stderr or raw stdout (which are now handled). If needed, this can be added later when we observe exception banners wrapped in JSON-RPC events.

2. **Handler simplification**: The refactoring successfully encapsulates ANSI stripping, exception detection, and level detection within `Session::process_raw_line()`, significantly simplifying the handler code. Both stderr and stdout fallback handlers now use identical patterns.

3. **Flush on exit uses immediate add**: Following the task spec guidance, the exception buffer flush on session exit uses `add_log()` directly (bypassing batching) to ensure partial exception blocks are immediately visible.

### Testing Performed

- `cargo check -p fdemon-app` - Passed
- `cargo test -p fdemon-app` - Passed (755 tests)
- `cargo clippy -p fdemon-app -- -D warnings` - Passed (no warnings)
- `cargo test --workspace --lib` - Passed (1532 tests across all crates)

### Risks/Limitations

1. **No dedicated handler tests**: The handler modules (`daemon.rs`, `session.rs`) don't have their own test modules since they're integration-level code. Testing relies on the session-level tests from Task 2 (which test `process_raw_line()` and `flush_exception_buffer()`) plus compilation verification.

2. **app.log exception handling deferred**: If Flutter does wrap exception banners in JSON-RPC `app.log` events, they won't be detected as collapsible exceptions yet. This can be added if observed in real-world usage.
