## Task: Launch Flow Modification — Pre-App Source Gating

**Objective**: Modify `handle_launch()` to conditionally return `SpawnPreAppSources` instead of `SpawnSession` when pre-app sources exist, and implement the message handlers that gate the Flutter launch on readiness.

**Depends on**: Task 01 (config types), Task 03 (message + action variants)

### Scope

- `crates/fdemon-app/src/handler/new_session/launch_context.rs`: Modify `handle_launch()` return path
- `crates/fdemon-app/src/handler/update.rs`: Implement handlers for `PreAppSourcesReady`, `PreAppSourceTimedOut`, `PreAppSourceProgress`

### Details

#### 1. Modify `handle_launch()` in `launch_context.rs`

The current happy path (around line 520-546 of `launch_context.rs`) returns:

```rust
UpdateResult::action(UpdateAction::SpawnSession {
    session_id,
    device,
    config: config.map(Box::new),
})
```

Change this to conditionally check for pre-app sources:

```rust
// After session is created and selected...
state.hide_new_session_dialog();
state.ui_mode = UiMode::Normal;

// Check if any custom sources need to start before the app
if state.settings.native_logs.enabled && state.settings.native_logs.has_pre_app_sources() {
    UpdateResult::action(UpdateAction::SpawnPreAppSources {
        session_id,
        device,
        config: config.map(Box::new),
        settings: state.settings.native_logs.clone(),
        project_path: state.project_path.clone(),
    })
} else {
    UpdateResult::action(UpdateAction::SpawnSession {
        session_id,
        device,
        config: config.map(Box::new),
    })
}
```

**Key design point:** The session handle is already created in `SessionManager` at this point. The `SpawnPreAppSources` action starts the custom sources and waits for readiness, then sends `PreAppSourcesReady` which triggers `SpawnSession`. The session exists but has no `FlutterProcess` attached yet — this is the same state it's briefly in during the normal `SpawnSession` flow before the async spawn completes.

#### 2. Handle `Message::PreAppSourcesReady` in `update.rs`

Replace the stub from Task 03 with:

```rust
Message::PreAppSourcesReady { session_id, device, config } => {
    // Gate has lifted — launch Flutter now.
    // The session already exists in SessionManager (created by handle_launch).
    if state.session_manager.get(session_id).is_some() {
        UpdateResult::action(UpdateAction::SpawnSession {
            session_id,
            device,
            config,
        })
    } else {
        // Session was closed during the readiness wait — no-op.
        tracing::warn!(
            "PreAppSourcesReady for session {} but session no longer exists",
            session_id
        );
        UpdateResult::none()
    }
}
```

#### 3. Handle `Message::PreAppSourceTimedOut` in `update.rs`

```rust
Message::PreAppSourceTimedOut { session_id, source_name } => {
    if let Some(handle) = state.session_manager.get_mut(session_id) {
        handle.session.queue_log(LogEntry::new(
            LogLevel::Warning,
            LogSource::Daemon,
            format!(
                "Pre-app source '{}' readiness check timed out. Proceeding with launch.",
                source_name
            ),
        ));
    }
    UpdateResult::none()
}
```

#### 4. Handle `Message::PreAppSourceProgress` in `update.rs`

```rust
Message::PreAppSourceProgress { session_id, message } => {
    if let Some(handle) = state.session_manager.get_mut(session_id) {
        handle.session.queue_log(LogEntry::new(
            LogLevel::Info,
            LogSource::Daemon,
            message,
        ));
    }
    UpdateResult::none()
}
```

#### 5. Session Phase During Wait

When `SpawnPreAppSources` is dispatched, the session is in its initial phase (likely `AppPhase::Idle` or whatever the default is). The progress messages will appear in the log view. No explicit phase change is needed — the session simply hasn't been attached to a Flutter process yet. The progress messages provide the user feedback.

Optionally, if the current default phase shows "Idle" or similar, update the session phase immediately in `handle_launch()` before returning:

```rust
if let Some(handle) = state.session_manager.get_mut(session_id) {
    handle.session.phase = AppPhase::Initializing;
}
```

This ensures the UI shows a meaningful state during the pre-app wait. Check what `AppPhase::Initializing` renders as and whether it's appropriate.

### Acceptance Criteria

1. When `native_logs.enabled = true` and `has_pre_app_sources()` returns true, `handle_launch()` returns `SpawnPreAppSources` instead of `SpawnSession`
2. When `native_logs.enabled = false` or no pre-app sources, `handle_launch()` returns `SpawnSession` as before (zero behavioral change)
3. `PreAppSourcesReady` handler returns `UpdateAction::SpawnSession` with the same session_id/device/config
4. `PreAppSourcesReady` for a closed session is a no-op (no panic)
5. `PreAppSourceTimedOut` adds a warning log entry to the session
6. `PreAppSourceProgress` adds an info log entry to the session
7. `cargo check -p fdemon-app` passes
8. `cargo test -p fdemon-app` passes

### Testing

```rust
#[test]
fn test_handle_launch_returns_spawn_pre_app_when_pre_app_sources() {
    let mut state = test_state_with_pre_app_source();
    // Set up state so handle_launch can build params and find a device...
    let result = handle_launch(&mut state);
    assert!(matches!(
        result.action,
        Some(UpdateAction::SpawnPreAppSources { .. })
    ));
}

#[test]
fn test_handle_launch_returns_spawn_session_when_no_pre_app_sources() {
    let mut state = test_state_without_pre_app_sources();
    let result = handle_launch(&mut state);
    assert!(matches!(
        result.action,
        Some(UpdateAction::SpawnSession { .. })
    ));
}

#[test]
fn test_pre_app_sources_ready_triggers_spawn_session() {
    let mut state = test_state();
    // Create a session first
    let session_id = create_test_session(&mut state);
    let device = test_device();

    let result = handler::update(
        &mut state,
        Message::PreAppSourcesReady {
            session_id,
            device: device.clone(),
            config: None,
        },
    );

    assert!(matches!(
        result.action,
        Some(UpdateAction::SpawnSession { .. })
    ));
}

#[test]
fn test_pre_app_sources_ready_noop_for_closed_session() {
    let mut state = test_state();
    let result = handler::update(
        &mut state,
        Message::PreAppSourcesReady {
            session_id: 99999, // non-existent
            device: test_device(),
            config: None,
        },
    );

    assert!(result.action.is_none());
}

#[test]
fn test_pre_app_progress_adds_log_entry() {
    let mut state = test_state();
    let session_id = create_test_session(&mut state);

    handler::update(
        &mut state,
        Message::PreAppSourceProgress {
            session_id,
            message: "Starting server...".to_string(),
        },
    );

    let handle = state.session_manager.get(session_id).unwrap();
    // Verify a log entry was queued with the progress message
    // (check handle.session.log_batch or pending logs)
}

#[test]
fn test_pre_app_timed_out_adds_warning() {
    let mut state = test_state();
    let session_id = create_test_session(&mut state);

    handler::update(
        &mut state,
        Message::PreAppSourceTimedOut {
            session_id,
            source_name: "server".to_string(),
        },
    );

    // Verify warning log was queued
}
```

### Notes

- The `NativeLogsSettings` clone in `SpawnPreAppSources` is intentional — the action runs asynchronously and needs an owned copy. The settings struct is small.
- `handle_launch()` is the single convergence point for both auto-launch and manual device selection — so this change covers both paths.
- The session exists in `SessionManager` but has no `FlutterProcess` — this is a new intermediate state. Verify that the UI handles this gracefully (no "session stopped" false positives, no render panics).
- Use `LogSource::Daemon` for progress/timeout messages since that's the closest existing source type. If a dedicated `LogSource::System` is needed, that's a future enhancement.

---

## Completion Summary

**Status:** Not Started
