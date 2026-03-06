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
