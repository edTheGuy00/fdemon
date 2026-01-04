## Task 4b: Remove Message::Daemon Variant and Handlers

**Objective**: Eliminate the legacy `Message::Daemon` variant and all associated handling code. After Task 4a, all daemon events flow through `Message::SessionDaemon`, making this code path obsolete.

**Depends on**: Task 4a (auto-start must use sessions first)

---

### Background

The `Message::Daemon(DaemonEvent)` variant was the original single-session way to receive daemon events. It processes events through `handle_daemon_event()` which updates global `AppState` fields.

With multi-session support, all daemon events now come via `Message::SessionDaemon { session_id, event }` which routes to `handle_session_daemon_event()` and updates per-session state.

After Task 4a removes the auto-start direct process ownership, no code path produces `Message::Daemon` events anymore.

---

### Scope

#### `src/app/message.rs`

**Remove:**
```rust
// Lines 13-16
/// Event from Flutter daemon (legacy single-session mode)
Daemon(DaemonEvent),
```

#### `src/app/handler/daemon.rs`

**Remove `handle_daemon_event` function (lines 11-107):**
- This entire function handles legacy single-session mode
- ~97 lines of code to remove

**Remove `handle_daemon_message_state` function (lines 175-191):**
- Updates global `state.current_app_id` and `state.phase`
- Only called by `handle_daemon_event`
- ~17 lines to remove

**Keep:**
- `handle_session_daemon_event()` - still used for multi-session mode
- All imports needed by the remaining function

**Update module documentation:**
- Lines 1-2: Remove "legacy and" from comment
- Update to say "Multi-session daemon event handling"

#### `src/app/handler/update.rs`

**Remove Message::Daemon match arm (lines 43-47):**
```rust
Message::Daemon(event) => {
    handle_daemon_event(state, event);
    UpdateResult::none()
}
```

**Update imports:**
- Remove `handle_daemon_event` from imports (line 8)

#### `src/tui/process.rs`

**Remove `route_legacy_daemon_response` function (lines 67-82):**
```rust
/// Route JSON-RPC responses for legacy daemon events
fn route_legacy_daemon_response(message: &Message, cmd_sender: &Arc<Mutex<Option<CommandSender>>>) {
    if let Message::Daemon(DaemonEvent::Stdout(ref line)) = message {
        // ... 15 lines
    }
}
```

**Remove call to route_legacy_daemon_response (line 33):**
```rust
// Route responses from Message::Daemon events (legacy single-session mode)
route_legacy_daemon_response(&message, cmd_sender);
```

**Update module documentation:**
- Lines 4-5: Remove "for both legacy single-session and"
- Update to say "routes JSON-RPC responses for multi-session mode"

**Update imports:**
- Remove `DaemonEvent` import (only needed for legacy function)

#### `src/tui/runner.rs`

**Verify daemon_rx removal (should be done in 4a):**
- Ensure no `daemon_rx` channel exists
- Ensure no `Message::Daemon` is sent anywhere
- Remove `route_daemon_response` function if still present (lines 164-182)

#### `src/app/handler/mod.rs`

**Update module documentation (lines 5-6):**
```rust
// Before:
//! - `daemon`: Legacy and multi-session daemon event handling

// After:
//! - `daemon`: Multi-session daemon event handling
```

---

### Implementation Steps

1. **Remove Message::Daemon from message.rs**
   - Delete the variant and its comment
   - Compile to find all usage sites

2. **Remove match arm in update.rs**
   - Delete the `Message::Daemon(event) => ...` arm
   - Remove `handle_daemon_event` from imports

3. **Remove handle_daemon_event from daemon.rs**
   - Delete the function
   - Delete `handle_daemon_message_state` (only caller was handle_daemon_event)
   - Update module documentation

4. **Remove route_legacy_daemon_response from process.rs**
   - Delete the function
   - Remove the call site
   - Update imports and documentation

5. **Clean up runner.rs**
   - Verify no daemon_rx usage remains
   - Remove route_daemon_response if present

6. **Update handler/mod.rs documentation**
   - Remove "Legacy and" reference

7. **Compile and fix any remaining references**

---

### Files Changed Summary

| File | Lines Removed | Lines Changed |
|------|---------------|---------------|
| `message.rs` | 3 | 0 |
| `daemon.rs` | ~115 | 2 (docs) |
| `update.rs` | 4 | 1 (import) |
| `process.rs` | ~20 | 3 (docs, imports) |
| `runner.rs` | 0 (done in 4a) | 0 |
| `handler/mod.rs` | 0 | 1 (docs) |

**Total: ~142 lines removed**

---

### Acceptance Criteria

1. ✅ `Message::Daemon` variant does not exist in message.rs
2. ✅ `handle_daemon_event()` function removed from daemon.rs
3. ✅ `handle_daemon_message_state()` function removed from daemon.rs
4. ✅ `route_legacy_daemon_response()` function removed from process.rs
5. ✅ No code sends or matches `Message::Daemon`
6. ✅ `handle_session_daemon_event()` still works correctly
7. ✅ All documentation updated to remove "legacy" references
8. ✅ `cargo check` passes
9. ✅ `cargo clippy` shows no warnings
10. ✅ All remaining tests pass

---

### Testing

#### Compile-Time Verification
- `cargo check` passes with no errors
- `cargo clippy` shows no warnings about unused code
- No dead_code warnings for removed functions

#### Unit Tests
- Tests using `Message::Daemon` will fail to compile → remove them in Task 4g
- For now, comment them out or mark as `#[ignore]` with TODO

**Tests to disable temporarily:**
- `test_daemon_exited_event_logs_message`
- `test_daemon_exited_sets_quitting_phase`
- `test_daemon_exited_with_error_code_sets_quitting`

#### Runtime Testing
1. Start fdemon with device selector
2. Select device → verify session starts
3. Verify daemon events logged correctly
4. Verify hot reload works
5. Verify session exit handled properly

---

### Edge Cases

1. **No Message::Daemon events should exist**
   - If any code still sends Message::Daemon, compilation will fail
   - This is the desired behavior - compile-time enforcement

2. **SessionDaemon events must still work**
   - All multi-session functionality must be preserved
   - Run integration tests to verify

---

### Risk Mitigation

| Risk | Mitigation |
|------|------------|
| Missing usage of Message::Daemon | Compiler will catch all usages |
| Breaking SessionDaemon handling | Verify handle_session_daemon_event unchanged |
| Orphaned imports | Clippy will warn about unused imports |

---

### Estimated Effort

**1 hour**

- 0.5 hours: Remove functions and variant
- 0.25 hours: Update documentation
- 0.25 hours: Compile and fix any issues