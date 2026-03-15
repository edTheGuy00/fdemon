## Task: Implement TEA Handlers for Shared Source Messages

**Objective**: Handle `SharedSourceLog`, `SharedSourceStarted`, and `SharedSourceStopped` messages in the TEA update loop, broadcasting logs to all sessions and managing shared handles on `AppState`.

**Depends on**: 01-config-shared-field, 02-shared-source-handle, 03-message-variants

### Scope

- `crates/fdemon-app/src/handler/update.rs`: Add three new match arms in the message handler

### Details

#### 1. `SharedSourceLog` Handler

Broadcast the log event to all active sessions:

```rust
Message::SharedSourceLog { event } => {
    // Resolve per-tag min-level filter from settings
    let min_level = resolve_min_level_for_tag(&state.settings.native_logs, &event.tag);

    // Broadcast to all sessions
    for handle in state.session_manager.iter_mut() {
        // Observe tag for the T-overlay filter
        handle.native_tag_state.observe_tag(&event.tag);

        // Apply level filter
        if event.level.severity() < min_level.severity() {
            continue;
        }

        // Apply per-session tag visibility filter
        if !handle.native_tag_state.is_tag_visible(&event.tag) {
            continue;
        }

        let entry = LogEntry::new(
            event.level,
            LogSource::Native { tag: event.tag.clone() },
            event.message.clone(),
        );
        if handle.session.queue_log(entry) {
            handle.session.flush_batched_logs();
        }
    }
    UpdateResult::none()
}
```

#### 2. `SharedSourceStarted` Handler

Store the handle on `AppState.shared_source_handles`:

```rust
Message::SharedSourceStarted { name, shutdown_tx, task_handle, start_before_app } => {
    let extracted = task_handle.lock()
        .ok()
        .and_then(|mut slot| slot.take());

    state.shared_source_handles.push(SharedSourceHandle {
        name: name.clone(),
        shutdown_tx,
        task_handle: extracted,
        start_before_app,
    });

    tracing::info!("Shared source '{}' started", name);
    UpdateResult::none()
}
```

#### 3. `SharedSourceStopped` Handler

Remove from `AppState` and warn all sessions:

```rust
Message::SharedSourceStopped { name } => {
    state.shared_source_handles.retain(|h| h.name != name);

    // Log warning to all active sessions
    for handle in state.session_manager.iter_mut() {
        let entry = LogEntry::new(
            LogLevel::Warning,
            LogSource::Daemon,
            format!("Shared source '{}' has stopped", name),
        );
        handle.session.queue_log(entry);
    }

    tracing::warn!("Shared source '{}' stopped", name);
    UpdateResult::none()
}
```

### Acceptance Criteria

1. `SharedSourceLog` broadcasts log events to ALL active sessions with per-session tag filtering
2. `SharedSourceStarted` stores handle on `state.shared_source_handles`
3. `SharedSourceStopped` removes handle and logs warning to all sessions
4. Tag observations work across sessions (shared source tags appear in T-overlay for all sessions)
5. All existing tests pass

### Testing

```rust
#[test]
fn test_shared_source_log_broadcasts_to_all_sessions() { ... }

#[test]
fn test_shared_source_log_applies_tag_filter() { ... }

#[test]
fn test_shared_source_started_stores_handle() { ... }

#[test]
fn test_shared_source_stopped_removes_handle_and_warns() { ... }

#[test]
fn test_shared_source_log_with_no_sessions_is_noop() { ... }
```

### Notes

- The `SharedSourceLog` handler clones `event.tag` and `event.message` per session. This is acceptable — log events are small strings and session count is capped at 9
- Use the same `resolve_min_level_for_tag` helper as the existing `NativeLog` handler to avoid duplication
