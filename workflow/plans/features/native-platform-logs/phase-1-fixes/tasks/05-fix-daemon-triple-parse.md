## Task: Parse Daemon Message Once in Stdout Path

**Objective**: Refactor the `DaemonEvent::Stdout` handler to call `parse_daemon_message` once instead of up to 3 times per line, eliminating redundant JSON deserialization.

**Depends on**: None

**Review Issue:** #8 (Minor)

### Scope

- `crates/fdemon-app/src/handler/daemon.rs`: Refactor `Stdout` arm (lines 44-83)

### Details

#### Problem

The current `DaemonEvent::Stdout` handler in `daemon.rs:44-83` parses the same line up to 3 times:

```rust
DaemonEvent::Stdout(line) => {
    // Parse 1: Check for AppDebugPort
    let vm_action =
        if let Some(msg @ DaemonMessage::AppDebugPort(_)) = parse_daemon_message(&line) {
            maybe_connect_vm_service(state, session_id, &msg)
        } else {
            None
        };

    // Parse 2: Boolean check for AppStart
    let is_app_start = matches!(
        parse_daemon_message(&line),
        Some(DaemonMessage::AppStart(_))
    );

    handle_session_stdout(state, session_id, &line);

    // Parse 3: Re-extract AppStart data for native log capture
    let native_log_action = if is_app_start {
        if let Some(msg @ DaemonMessage::AppStart(_)) = parse_daemon_message(&line) {
            maybe_start_native_log_capture(state, session_id, &msg)
        } else {
            None
        }
    } else {
        None
    };

    match vm_action.or(native_log_action) {
        Some(action) => UpdateResult::action(action),
        None => UpdateResult::none(),
    }
}
```

Each `parse_daemon_message` call involves `serde_json` deserialization (JSON bracket stripping → `RawMessage::parse` → `parse_event`). While not on a hot path (stdout lines are typically low-frequency protocol messages), the redundancy is unnecessary.

#### Fix

Parse once, bind the result, and branch on the parsed `DaemonMessage`:

```rust
DaemonEvent::Stdout(line) => {
    // Parse once — used for VM connection, native log capture, and state mutation.
    let parsed = parse_daemon_message(&line);

    // Check for AppDebugPort → VM Service connection.
    let vm_action = match &parsed {
        Some(msg @ DaemonMessage::AppDebugPort(_)) => {
            maybe_connect_vm_service(state, session_id, msg)
        }
        _ => None,
    };

    // Mutate state (logs the line, updates session phase, etc.).
    handle_session_stdout(state, session_id, &line);

    // Check for AppStart → native log capture.
    // This runs after handle_session_stdout so session.app_id is set.
    let native_log_action = match &parsed {
        Some(msg @ DaemonMessage::AppStart(_)) => {
            maybe_start_native_log_capture(state, session_id, msg)
        }
        _ => None,
    };

    match vm_action.or(native_log_action) {
        Some(action) => UpdateResult::action(action),
        None => UpdateResult::none(),
    }
}
```

**Key constraint preserved:** `handle_session_stdout` must run between the VM action check and the native log action check. The parsed `DaemonMessage` is an owned value with its own copy of the data (e.g., `AppStart { app_id }` is a `String` clone from the JSON), so mutating `session.app_id` inside `handle_session_stdout` does not affect the parsed value. The ordering is:
1. Parse once → `parsed: Option<DaemonMessage>`
2. Check `AppDebugPort` on `&parsed` → `vm_action`
3. Call `handle_session_stdout` (mutates state)
4. Check `AppStart` on `&parsed` → `native_log_action`

This preserves the exact same semantics while eliminating 2 redundant parses.

**Note:** `handle_session_stdout` (session.rs) internally calls `parse_daemon_message` again. That fourth parse is inside a different function and out of scope for this task — refactoring it would require changing the `handle_session_stdout` signature to accept a pre-parsed message, which is a larger change.

### Acceptance Criteria

1. `parse_daemon_message` is called exactly once in the `Stdout` arm
2. VM service connection (AppDebugPort) still works correctly
3. Native log capture (AppStart) still triggers correctly
4. `handle_session_stdout` is still called between the two checks
5. `cargo check -p fdemon-app` passes
6. `cargo test -p fdemon-app --lib` passes
7. `cargo clippy -p fdemon-app -- -D warnings` passes

### Testing

No new tests needed — this is a pure refactor with identical behavior. Existing tests for `handle_daemon_event` cover both the `AppDebugPort` and `AppStart` paths.

### Notes

- `parse_daemon_message` is defined in `fdemon-daemon/src/protocol.rs:103` and returns `Option<DaemonMessage>`.
- `DaemonMessage` is an enum defined in `fdemon-core/src/events.rs:113` with variants like `AppDebugPort`, `AppStart`, `AppStop`, etc.
- The `AppDebugPort` and `AppStart` events are always separate messages (they never appear in the same stdout line), so `vm_action` and `native_log_action` are mutually exclusive. The `.or()` combiner is correct.

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/handler/daemon.rs` | Refactored `DaemonEvent::Stdout` arm to parse once via `let parsed = parse_daemon_message(&line)`, then branch on `&parsed` for both `AppDebugPort` and `AppStart` checks using `match` expressions instead of repeated parse calls |

### Notable Decisions/Tradeoffs

1. **Borrow via `&parsed`**: Both match arms borrow `&parsed` rather than consuming it, preserving the ability to use the same binding for both checks without cloning. The `msg` binding inside each arm borrows from the outer `Option<DaemonMessage>`.
2. **`handle_session_stdout` ordering preserved**: The function is still called between the `vm_action` and `native_log_action` checks, as required by the task spec and the comment about `session.app_id` being set by that call.

### Testing Performed

- `cargo check -p fdemon-app` - Passed
- `cargo test -p fdemon-app --lib` - Passed (1464 tests, 0 failed, 4 ignored)
- `cargo clippy -p fdemon-app -- -D warnings` - Passed

### Risks/Limitations

1. **`handle_session_stdout` still re-parses internally**: As noted in the task, that function internally calls `parse_daemon_message` again, which is out of scope for this task. Total parse count per line is now 2 (one in the `Stdout` arm, one inside `handle_session_stdout`) rather than the prior 3-4.
