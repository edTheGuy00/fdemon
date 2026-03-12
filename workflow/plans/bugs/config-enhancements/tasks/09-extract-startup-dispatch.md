## Task: Extract Shared Startup Dispatch Helper

**Objective**: Eliminate the duplicated 13-line startup dispatch block in `runner.rs` and consolidate the two-step startup mutation in `startup.rs`.

**Depends on**: None

**Priority**: Consider (optional improvement)

### Scope

- `crates/fdemon-tui/src/runner.rs`: Extract shared helper from lines 45-57 and 131-143
- `crates/fdemon-tui/src/startup.rs`: Consolidate `show_new_session_dialog()` + `ui_mode = Startup` into one method (optional)

### Details

#### Runner duplication (runner.rs)

The identical block appears in both `run_with_project()` and `run_with_project_and_dap()`:

```rust
match startup_result {
    startup::StartupAction::AutoStart { configs } => {
        engine.process_message(Message::StartAutoLaunch { configs });
    }
    startup::StartupAction::Ready => {
        spawn::spawn_device_discovery(engine.msg_sender());
    }
}
```

Extract to:
```rust
fn dispatch_startup_action(engine: &mut Engine, action: startup::StartupAction) {
    match action {
        startup::StartupAction::AutoStart { configs } => {
            engine.process_message(Message::StartAutoLaunch { configs });
        }
        startup::StartupAction::Ready => {
            spawn::spawn_device_discovery(engine.msg_sender());
        }
    }
}
```

#### Startup state consolidation (startup.rs, optional)

Lines 44-45 make two sequential mutations:
```rust
state.show_new_session_dialog(configs);
state.ui_mode = UiMode::Startup;
```

The Architecture Enforcer noted this is fragile — `show_new_session_dialog` sets `ui_mode = NewSessionDialog` which is immediately overridden. Consider consolidating into `state.prepare_startup_dialog(configs)` that does both in one call.

### Acceptance Criteria

1. Single `dispatch_startup_action` helper replaces both duplicated blocks
2. Both `run_with_project` and `run_with_project_and_dap` call the helper
3. No behavior change — same messages sent, same side effects
4. All existing tests pass

### Testing

```bash
cargo test -p fdemon-tui
cargo clippy -p fdemon-tui -- -D warnings
```

### Notes

- The review suggested tracking this for extraction "when a third call site or variant is added." Creating this task now for tracking purposes — implementation can be deferred if preferred.
- The startup consolidation is lower priority since the two-step pattern is a documented exception for TUI startup initialization.
