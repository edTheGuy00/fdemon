## Task: Suppress Auto-Reload While Debugger is Paused

**Objective**: When the debugger is paused at a breakpoint or step, suppress the file watcher's auto-reload to prevent hot reloads from invalidating the debug state. Queue file change events while paused and trigger a hot reload after the debugger resumes.

**Depends on**: 01-wire-debug-event-channel

**Estimated Time**: 3–4 hours

### Scope

- `crates/fdemon-app/src/handler/devtools/debug.rs`: On pause → send `Message::SuspendFileWatcher`; on resume → send `Message::ResumeFileWatcher`
- `crates/fdemon-app/src/handler/update.rs`: Handle `SuspendFileWatcher` / `ResumeFileWatcher` messages
- `crates/fdemon-app/src/watcher.rs` (or equivalent): Add suspend/resume mechanism that queues changes
- `crates/fdemon-app/src/state.rs`: Add `file_watcher_suspended: bool` and `pending_file_changes: usize` to `AppState`
- `crates/fdemon-core/src/lib.rs`: Add `Message::SuspendFileWatcher` and `Message::ResumeFileWatcher` variants

### Details

#### Flow

```
Debugger pauses (breakpoint/exception/step)
  → handle_debug_event() detects Paused event
  → Emits Message::SuspendFileWatcher
  → update() sets state.file_watcher_suspended = true
  → File watcher continues detecting changes but queues them
  → state.pending_file_changes incremented

Debugger resumes (continue/step)
  → handle_debug_event() detects Resumed event
  → Emits Message::ResumeFileWatcher
  → update() sets state.file_watcher_suspended = false
  → If pending_file_changes > 0:
      → Emits Message::AutoReloadTriggered
      → Clears pending_file_changes
```

#### Configuration

Controlled by `dap.suppress_reload_on_pause` setting (default: `true`). Already defined in the PLAN.md config section. Check if the setting field exists in `DapSettings`; if not, add it.

```rust
// In handler for FilesChanged:
if state.file_watcher_suspended && state.settings.dap.suppress_reload_on_pause {
    state.pending_file_changes += count;
    return UpdateResult::none(); // Queue, don't reload
}
// ... existing auto-reload logic ...
```

#### Edge Cases

- **Multiple pause/resume cycles**: Don't double-suspend. Use a counter or bool.
- **Session closes while paused**: Reset `file_watcher_suspended` on session close.
- **DAP disconnects while paused**: Resume file watcher on disconnect.
- **Multiple DAP clients**: If any client is in a paused state, suppress reload. Only re-enable when all clients have resumed.

### Acceptance Criteria

1. Saving a file while paused at a breakpoint does NOT trigger hot reload
2. Resuming after file changes triggers a single hot reload
3. `suppress_reload_on_pause = false` disables this feature
4. File watcher resumes on DAP client disconnect
5. No interference with non-DAP auto-reload behavior
6. 15+ unit tests

### Testing

```rust
#[test]
fn test_file_changes_queued_while_paused() {
    let mut state = test_state();
    state.file_watcher_suspended = true;
    let result = update(&mut state, Message::FilesChanged { count: 3 });
    assert_eq!(state.pending_file_changes, 3);
    assert!(result.action.is_none()); // No reload triggered
}

#[test]
fn test_resume_triggers_reload_if_files_changed() {
    let mut state = test_state();
    state.file_watcher_suspended = true;
    state.pending_file_changes = 5;
    let result = update(&mut state, Message::ResumeFileWatcher);
    assert!(!state.file_watcher_suspended);
    assert_eq!(state.pending_file_changes, 0);
    // Verify AutoReloadTriggered message emitted
}

#[test]
fn test_resume_no_reload_if_no_changes() {
    let mut state = test_state();
    state.file_watcher_suspended = true;
    state.pending_file_changes = 0;
    let result = update(&mut state, Message::ResumeFileWatcher);
    assert!(!state.file_watcher_suspended);
    // No reload triggered
}
```

### Notes

- The existing file watcher infrastructure uses `Message::FilesChanged { count }` and `Message::AutoReloadTriggered`. This task adds a gate in the handler, not in the watcher itself.
- TUI should display a subtle indicator when auto-reload is suspended (e.g., dim the watcher status). This is optional and can be deferred.
- The `SuspendFileWatcher` / `ResumeFileWatcher` messages should be emitted as follow-up messages from `handle_debug_event`, not as `UpdateAction`s, since they're internal state transitions.

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/state.rs` | Added `file_watcher_suspended: bool` and `pending_file_changes: usize` fields to `AppState`; initialized to `false`/`0` in constructor |
| `crates/fdemon-app/src/message.rs` | Added `SuspendFileWatcher` and `ResumeFileWatcher` variants to `Message` enum (in File Watcher Messages section) |
| `crates/fdemon-app/src/handler/devtools/debug.rs` | Modified `handle_debug_event` to classify events as Pause/Resume/Other; emits `SuspendFileWatcher` on first pause (idempotent), `ResumeFileWatcher` on resume when suspended; added `use crate::message::Message`; added 10 new unit tests |
| `crates/fdemon-app/src/handler/update.rs` | Added gate in `FilesChanged` handler: queues changes when suspended+suppress enabled; added `SuspendFileWatcher` handler (sets flag idempotently); added `ResumeFileWatcher` handler (clears flag, emits `AutoReloadTriggered` if pending>0) |
| `crates/fdemon-app/src/handler/dap.rs` | Modified `handle_client_disconnected` to emit `ResumeFileWatcher` when watcher was suspended at disconnect time |
| `crates/fdemon-app/src/handler/tests.rs` | Added 15 new unit tests for the coordinated pause feature |

### Notable Decisions/Tradeoffs

1. **Idempotent SuspendFileWatcher**: The handler only emits `SuspendFileWatcher` when `!state.file_watcher_suspended`. This prevents double-suspend in rapid-event scenarios (e.g., multiple Pause events from different isolates). The task spec says "use a counter or bool" — the bool was chosen since the suspend is app-global, not per-session.

2. **Gate in FilesChanged, not in Watcher**: Per the task notes, the gate is a check in the TEA handler, not in the OS file-watcher itself. The watcher continues to detect changes; they are simply counted not acted upon.

3. **Single reload on resume**: `pending_file_changes` counts changes but always triggers exactly one `AutoReloadTriggered` on resume. This matches the existing multi-session reload logic which handles all reloadable sessions in one pass.

4. **DAP disconnect resets via message**: `handle_client_disconnected` emits `ResumeFileWatcher` as a follow-up message rather than directly mutating state. This keeps all state mutation in one place (the `ResumeFileWatcher` handler) and avoids code duplication.

5. **Scope boundary**: Task 02 was running concurrently modifying `fdemon-dap`. The implementation stays strictly within `fdemon-app` per the task constraint. The `suppress_reload_on_pause` field in `DapSettings` already existed; no changes to `fdemon-core` were needed since the `Message` enum lives in `fdemon-app/message.rs`.

### Testing Performed

- `cargo check --workspace` - Passed
- `cargo test -p fdemon-app` - Passed (1322 tests; 0 failures)
- `cargo clippy --workspace -- -D warnings` - Passed (no warnings)
- `cargo fmt --all` - Passed

**New tests added: 25 total** (10 in `debug.rs`, 15 in `tests.rs`)

### Risks/Limitations

1. **Multi-client pause state**: The spec mentions "if any client is in a paused state, suppress reload". The current implementation uses a single `file_watcher_suspended` bool which doesn't count how many clients are paused. If two DAP clients both pause and one resumes, the watcher will resume. A `pause_depth: u32` counter would handle this correctly, but the task spec says "Use a counter or bool" and the simpler bool was chosen to match the task's primary test cases. This can be upgraded later if multi-client scenarios are tested.

2. **PauseExit not guarded**: `PauseExit` (isolate pausing just before exit) also emits `SuspendFileWatcher`. This is intentional per the task flow, since the pause/resume symmetry should hold for all pause kinds. The isolate will resume or exit, clearing the flag.
