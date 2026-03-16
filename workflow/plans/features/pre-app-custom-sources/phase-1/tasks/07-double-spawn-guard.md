## Task: Double-Spawn Prevention for Pre-App Sources

**Objective**: Ensure pre-app sources are not spawned again when `AppStarted` fires (which triggers the normal `spawn_native_log_capture()` for post-app sources), and add `start_before_app` tracking to `CustomSourceHandle`.

**Depends on**: Task 06 (spawn_pre_app_sources implementation)

### Scope

- `crates/fdemon-app/src/session/handle.rs`: Add `start_before_app` field to `CustomSourceHandle`
- `crates/fdemon-app/src/actions/native_logs.rs`: Skip `start_before_app` sources in `spawn_custom_sources()`
- `crates/fdemon-app/src/handler/update.rs`: Pass `start_before_app` in `CustomSourceStarted` handler

### Details

#### 1. Add `start_before_app` to `CustomSourceHandle`

In `session/handle.rs`, extend the struct:

```rust
pub struct CustomSourceHandle {
    pub name: String,
    pub shutdown_tx: Arc<tokio::sync::watch::Sender<bool>>,
    pub task_handle: Option<tokio::task::JoinHandle<()>>,
    /// Whether this source was started before the Flutter app (pre-app source).
    /// Used to skip re-spawning on `AppStarted` events.
    pub start_before_app: bool,
}
```

#### 2. Update `CustomSourceStarted` Message

The `CustomSourceStarted` message variant (in `message.rs`) needs the new field:

```rust
CustomSourceStarted {
    session_id: SessionId,
    name: String,
    shutdown_tx: Arc<watch::Sender<bool>>,
    task_handle: Arc<tokio::sync::Mutex<Option<JoinHandle<()>>>>,
    start_before_app: bool,  // NEW
},
```

#### 3. Update `CustomSourceStarted` Handler in `update.rs`

The handler (around line 2046 of `update.rs`) that constructs `CustomSourceHandle` needs to set the new field:

```rust
Message::CustomSourceStarted {
    session_id,
    name,
    shutdown_tx,
    task_handle,
    start_before_app,
} => {
    // ... existing logic to extract JoinHandle ...
    handle.custom_source_handles.push(CustomSourceHandle {
        name,
        shutdown_tx,
        task_handle: extracted_handle,
        start_before_app,
    });
}
```

#### 4. Update `spawn_custom_sources()` to Skip Pre-App Sources

In `actions/native_logs.rs`, the existing `spawn_custom_sources()` function (line 253) iterates over ALL `settings.custom_sources`. Modify it to skip sources that have `start_before_app = true`:

```rust
fn spawn_custom_sources(
    session_id: SessionId,
    settings: &NativeLogsSettings,
    project_path: &std::path::Path,
    msg_tx: &mpsc::Sender<Message>,
) {
    for source_config in &settings.custom_sources {
        // Skip pre-app sources — they were already started before Flutter launched.
        if source_config.start_before_app {
            tracing::debug!(
                "Skipping pre-app source '{}' in spawn_custom_sources (already running)",
                source_config.name
            );
            continue;
        }

        // ... existing validation + spawn logic ...
    }
}
```

#### 5. Update All `CustomSourceStarted` Send Sites

There are two places that send `CustomSourceStarted`:

1. **Existing `spawn_custom_sources()`** (native_logs.rs, around line 310): These are post-app sources, so set `start_before_app: false`.

2. **New `spawn_pre_app_sources()`** (from Task 06): These are pre-app sources, so set `start_before_app: true`.

Update both send sites:

```rust
// In spawn_custom_sources (post-app):
let _ = fwd_tx.send(Message::CustomSourceStarted {
    session_id,
    name: source_name.clone(),
    shutdown_tx: shutdown_tx_clone,
    task_handle: task_handle_clone,
    start_before_app: false,
}).await;

// In spawn_pre_app_sources (pre-app):
let _ = fwd_tx.send(Message::CustomSourceStarted {
    session_id,
    name: source_name.clone(),
    shutdown_tx: shutdown_tx_clone,
    task_handle: task_handle_clone,
    start_before_app: true,
}).await;
```

#### 6. Verify Existing Guard Still Works

The guard in `handler/session.rs:312`:

```rust
if handle.native_log_shutdown_tx.is_some() || !handle.custom_source_handles.is_empty() {
    return None;
}
```

This already prevents `maybe_start_native_log_capture()` from re-triggering if ANY custom sources are running. Since pre-app sources are added to `custom_source_handles` via `CustomSourceStarted` **before** `AppStarted` fires, this guard will short-circuit correctly — `custom_source_handles` is non-empty, so the guard returns `None`.

However, this means post-app custom sources (`start_before_app = false`) also won't be spawned on `AppStarted` because the guard fires early. **This is a problem.**

**Fix:** The guard needs refinement. Instead of checking `!custom_source_handles.is_empty()`, check if all expected captures are already running:

```rust
// In maybe_start_native_log_capture():
let all_custom_running = settings.native_logs.custom_sources.iter()
    .all(|cfg| handle.custom_source_handles.iter().any(|h| h.name == cfg.name));

if handle.native_log_shutdown_tx.is_some() && all_custom_running {
    return None;
}
```

Actually, a simpler approach: the guard should allow re-entry if there are post-app sources that haven't been spawned yet. The cleanest fix:

```rust
// Check if native platform capture is already running
let platform_capture_running = handle.native_log_shutdown_tx.is_some();

// Check if all custom sources are already running
let running_source_names: HashSet<_> = handle.custom_source_handles
    .iter()
    .map(|h| h.name.as_str())
    .collect();
let all_custom_running = state.settings.native_logs.custom_sources
    .iter()
    .all(|cfg| running_source_names.contains(cfg.name.as_str()));

if platform_capture_running && all_custom_running {
    tracing::debug!(
        "Native log capture already running for session {}",
        session_id
    );
    return None;
}
```

But this adds complexity. The **simplest correct approach**: since `spawn_custom_sources()` now skips `start_before_app = true` sources, and `spawn_native_log_capture()` calls `spawn_custom_sources()` for the remaining post-app sources, we need the guard to only check platform capture, not custom sources:

```rust
// Guard only on platform capture — custom sources have their own skip logic
if handle.native_log_shutdown_tx.is_some() {
    // Platform capture already running. But custom sources might still need spawning.
    // Check if all non-pre-app custom sources are running.
    let has_unstarted_post_app_sources = state.settings.native_logs.custom_sources
        .iter()
        .filter(|s| !s.start_before_app)
        .any(|s| !handle.custom_source_handles.iter().any(|h| h.name == s.name));

    if !has_unstarted_post_app_sources {
        tracing::debug!(
            "Native log capture already fully running for session {}",
            session_id
        );
        return None;
    }
    // Fall through to spawn remaining custom sources
}
// For non-platform sessions (Linux/Web): check if all custom sources are running
else if !handle.custom_source_handles.is_empty() {
    let has_unstarted_post_app_sources = state.settings.native_logs.custom_sources
        .iter()
        .filter(|s| !s.start_before_app)
        .any(|s| !handle.custom_source_handles.iter().any(|h| h.name == s.name));

    if !has_unstarted_post_app_sources {
        return None;
    }
}
```

This is getting complex. **Preferred approach**: Keep the original guard logic but also make `spawn_custom_sources()` idempotent by checking if a source is already running before spawning:

```rust
fn spawn_custom_sources(
    session_id: SessionId,
    settings: &NativeLogsSettings,
    project_path: &std::path::Path,
    msg_tx: &mpsc::Sender<Message>,
    running_source_names: &[String],  // NEW: names of already-running sources
) {
    for source_config in &settings.custom_sources {
        // Skip pre-app sources
        if source_config.start_before_app {
            continue;
        }

        // Skip already-running sources (idempotency guard)
        if running_source_names.iter().any(|n| n == &source_config.name) {
            continue;
        }

        // ... existing spawn logic ...
    }
}
```

And pass the running names from the caller (`spawn_native_log_capture()`):

```rust
let running_names: Vec<String> = /* get from session handle custom_source_handles */;
spawn_custom_sources(session_id, settings, project_path, msg_tx, &running_names);
```

This approach is simplest: the original guard at `session.rs:312` stays as-is for the platform capture, and custom source dedup is handled inside `spawn_custom_sources()`.

**Decision:** Use this last approach — pass `running_source_names` to `spawn_custom_sources()` and skip already-running sources. Keep the existing guard unchanged.

### Acceptance Criteria

1. `CustomSourceHandle` has `start_before_app: bool` field
2. `CustomSourceStarted` message carries `start_before_app: bool`
3. Pre-app sources are tagged with `start_before_app = true` in their handles
4. Post-app sources are tagged with `start_before_app = false`
5. `spawn_custom_sources()` skips sources with `start_before_app = true`
6. `spawn_custom_sources()` skips sources whose name already appears in `running_source_names`
7. Hot restart does not re-spawn pre-app sources (guard at session.rs:312 fires because `custom_source_handles` is non-empty)
8. Post-app sources still spawn correctly on `AppStarted` even when pre-app sources are already running
9. `cargo check -p fdemon-app` passes
10. `cargo test -p fdemon-app` passes

### Testing

```rust
#[test]
fn test_custom_source_handle_has_start_before_app() {
    let handle = CustomSourceHandle {
        name: "server".to_string(),
        shutdown_tx: /* ... */,
        task_handle: None,
        start_before_app: true,
    };
    assert!(handle.start_before_app);
}

#[test]
fn test_spawn_custom_sources_skips_pre_app() {
    // Create settings with one pre-app and one post-app source
    // Call spawn_custom_sources with empty running_names
    // Verify only the post-app source attempts to spawn
    // (This requires mocking or checking message sends)
}

#[test]
fn test_spawn_custom_sources_skips_already_running() {
    // Create settings with a post-app source named "watcher"
    // Call spawn_custom_sources with running_names = ["watcher"]
    // Verify the source is skipped
}
```

### Notes

- The guard logic change is the most delicate part of this task. The existing guard prevents duplicate spawning on hot restart. The new logic must preserve this protection while allowing post-app sources to spawn even when pre-app sources are already tracked.
- The `running_source_names` parameter approach is the least invasive — it doesn't change the guard at all, just makes `spawn_custom_sources()` smarter about what to skip.
- The `spawn_native_log_capture()` function needs access to `running_source_names`. Since it receives the `NativeLogsSettings` and `session_id`, it needs the caller to also pass the list of running source names from the session handle. Check the call site in `actions/mod.rs` to see what data is available.

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/handler/session.rs` | Replaced the simple `!custom_source_handles.is_empty()` guard with a fine-grained check that computes `has_unstarted_post_app` and only returns `None` when all post-app sources are already running. |
| `crates/fdemon-app/src/handler/tests.rs` | Added `test_guard_fires_on_hot_restart_with_pre_app_sources_only_running` (criterion 7) and `test_guard_allows_post_app_sources_when_only_pre_app_running` (criterion 8). |

### Notable Decisions/Tradeoffs

1. **Guard approach**: Tasks 01-06 already implemented criteria 1-6 fully (the `start_before_app` field on `CustomSourceHandle`, the message variant, the handler in `update.rs`, and the skip logic in `spawn_custom_sources()`). The only missing piece was the guard in `session.rs`. Changed the guard from the blunt `native_log_shutdown_tx.is_some() || !custom_source_handles.is_empty()` to a two-condition check that computes whether any post-app sources from config are still unstarted. This preserves the original hot-restart protection while allowing `AppStarted` to proceed when pre-app sources are tracked but post-app sources have not yet been spawned.

2. **`running_source_names` already collected**: Code after the guard was already collecting `running_source_names` and passing them to `StartNativeLogCapture` (added by Task 06). With the guard fix, this code now executes when pre-app sources are present, ensuring `spawn_custom_sources()` receives the names of already-running sources so it can skip them.

### Testing Performed

- `cargo fmt --all` - Passed
- `cargo check -p fdemon-app` - Passed
- `cargo test -p fdemon-app --lib` - Passed (1644 tests, 0 failed)
- `cargo clippy -p fdemon-app -- -D warnings` - Passed

### Risks/Limitations

1. **Guard complexity**: The new guard is more complex than the original. The `HashSet` construction on every `AppStart` event is cheap (typically O(n) with small n), but it adds a small allocation. This is acceptable given `AppStart` fires rarely (only on app launch/hot-restart).
2. **Test coverage of criterion 7**: The test for criterion 7 uses a Linux device (no platform capture). A scenario with an Android session + platform capture already running + pre-app sources could be added, but the logic is identical and covered by the condition structure.
