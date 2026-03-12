## Task: App Layer Integration for Custom Sources

**Objective**: Wire custom log source processes into the session lifecycle — spawn them alongside platform capture after `AppStarted`, integrate with the tag filter UI, and shut them down on session end.

**Depends on**: 03-custom-source-runner

### Scope

- `crates/fdemon-app/src/actions/native_logs.rs` — Extend to spawn custom sources
- `crates/fdemon-app/src/session/handle.rs` — Store multiple custom source handles
- `crates/fdemon-app/src/handler/session.rs` — Trigger custom source spawning
- `crates/fdemon-app/src/handler/update.rs` — Handle custom source events (may need new message variant or reuse existing `NativeLog`)
- `crates/fdemon-app/src/message.rs` — Add message variants if needed for custom source lifecycle

### Details

#### Session Handle Changes

Add storage for custom source handles alongside the existing platform capture handle:

```rust
// In session/handle.rs
pub struct SessionHandle {
    // ... existing fields ...
    pub native_log_shutdown_tx: Option<Arc<watch::Sender<bool>>>,
    pub native_log_task_handle: Option<JoinHandle<()>>,
    pub native_tag_state: NativeTagState,

    // NEW: custom source handles (one per configured custom source)
    pub custom_source_handles: Vec<CustomSourceHandle>,
}

pub struct CustomSourceHandle {
    pub name: String,
    pub shutdown_tx: Arc<watch::Sender<bool>>,
    pub task_handle: Option<JoinHandle<()>>,
}
```

Update `shutdown_native_logs()` to also shut down all custom sources:

```rust
pub fn shutdown_native_logs(&mut self) {
    // Shut down platform capture
    if let Some(tx) = self.native_log_shutdown_tx.take() {
        let _ = tx.send(true);
    }

    // Shut down all custom sources
    for handle in &self.custom_source_handles {
        let _ = handle.shutdown_tx.send(true);
    }
    self.custom_source_handles.clear();
}
```

#### Action Layer Changes

Extend `spawn_native_log_capture()` in `actions/native_logs.rs`:

```rust
pub async fn spawn_native_log_capture(
    session_id: Uuid,
    platform: String,
    device_id: String,
    device_name: String,
    app_id: Option<String>,
    settings: Settings,
    msg_tx: mpsc::UnboundedSender<Message>,
) {
    // ... existing platform capture logic ...

    // After platform capture, spawn custom sources
    spawn_custom_sources(session_id, &settings, &msg_tx).await;
}

async fn spawn_custom_sources(
    session_id: Uuid,
    settings: &Settings,
    msg_tx: &mpsc::UnboundedSender<Message>,
) {
    for source_config in &settings.native_logs.custom_sources {
        // Validate config
        if source_config.name.trim().is_empty() || source_config.command.trim().is_empty() {
            tracing::warn!(
                "Skipping custom source with empty name or command"
            );
            continue;
        }

        // Build daemon-layer config from app-layer config
        let daemon_config = CustomSourceConfig {
            name: source_config.name.clone(),
            command: source_config.command.clone(),
            args: source_config.args.clone(),
            format: source_config.format.clone(),
            working_dir: source_config.working_dir.clone(),
            env: source_config.env.clone(),
            exclude_tags: settings.native_logs.exclude_tags.clone(),
            include_tags: settings.native_logs.include_tags.clone(),
        };

        let capture = create_custom_log_capture(daemon_config);

        if let Some(handle) = capture.spawn() {
            let shutdown_tx = Arc::new(handle.shutdown_tx);
            let task_handle = Arc::new(Mutex::new(Some(handle.task_handle)));
            let source_name = source_config.name.clone();

            // Send the custom source started message
            let _ = msg_tx.send(Message::CustomSourceStarted {
                session_id,
                name: source_name.clone(),
                shutdown_tx: shutdown_tx.clone(),
                task_handle: task_handle.clone(),
            });

            // Forward events (same pattern as platform capture)
            let msg_tx = msg_tx.clone();
            let sid = session_id;
            tokio::spawn(async move {
                let mut event_rx = handle.event_rx;
                while let Some(event) = event_rx.recv().await {
                    if msg_tx.send(Message::NativeLog {
                        session_id: sid,
                        event,
                    }).is_err() {
                        break;
                    }
                }
                let _ = msg_tx.send(Message::CustomSourceStopped {
                    session_id: sid,
                    name: source_name,
                });
            });
        }
    }
}
```

#### Message Variants

Consider whether to add new message variants or reuse existing ones:

**Option A (recommended)**: Reuse `Message::NativeLog` for events (custom source events are identical to platform events). Add thin lifecycle messages:

```rust
// In message.rs
Message::CustomSourceStarted {
    session_id: Uuid,
    name: String,
    shutdown_tx: Arc<watch::Sender<bool>>,
    task_handle: Arc<Mutex<Option<JoinHandle<()>>>>,
}

Message::CustomSourceStopped {
    session_id: Uuid,
    name: String,
}
```

**Option B**: Reuse `NativeLogCaptureStarted` / `NativeLogCaptureStopped` for lifecycle too — but these are singular (one platform capture per session) and the handler expects a single shutdown_tx. Adding a vec of handles is cleaner with dedicated messages.

#### Handler Changes

In `handler/update.rs`:

```rust
Message::CustomSourceStarted { session_id, name, shutdown_tx, task_handle } => {
    if let Some(handle) = state.session_manager.get_mut(&session_id) {
        handle.custom_source_handles.push(CustomSourceHandle {
            name,
            shutdown_tx,
            task_handle: task_handle.lock().unwrap().take(),
        });
    }
}

Message::CustomSourceStopped { session_id, name } => {
    if let Some(handle) = state.session_manager.get_mut(&session_id) {
        handle.custom_source_handles.retain(|h| h.name != name);
    }
    tracing::debug!("Custom source '{}' stopped for session {}", name, session_id);
}
```

`Message::NativeLog` handling is unchanged — custom source events flow through the same handler, get the same min_level filtering, same `observe_tag()` call, same tag visibility check.

### Acceptance Criteria

1. Custom sources spawn after `AppStarted` alongside platform capture
2. Each custom source gets its own `CustomSourceHandle` stored on `SessionHandle`
3. Custom source events flow through `Message::NativeLog` and appear in the log view
4. Custom source tags appear in `NativeTagState` and the tag filter overlay
5. All custom sources shut down when the session ends
6. Custom source start/stop logged at debug level
7. Invalid configs (empty name/command) are skipped with a warning
8. Multiple custom sources can run simultaneously per session

### Testing

```rust
#[test]
fn test_custom_source_started_stores_handle() {
    // Process CustomSourceStarted message
    // Verify handle added to session.custom_source_handles
}

#[test]
fn test_custom_source_stopped_removes_handle() {
    // Add handle, then process CustomSourceStopped
    // Verify handle removed
}

#[test]
fn test_custom_source_events_use_native_log_handler() {
    // Send NativeLog event with tag matching a custom source name
    // Verify it goes through the same handler path as platform events
}

#[test]
fn test_session_shutdown_cleans_custom_sources() {
    // Add custom source handles, shut down session
    // Verify all handles cleaned up
}
```

### Notes

- Custom source events reuse the existing `NativeLog` message and handler — this means custom source tags get the same `min_level` filtering, tag visibility, and UI treatment as platform tags. No special-casing needed.
- The `custom_source_handles` Vec on `SessionHandle` is the main structural change. Keep it simple — no HashMap by name, just a Vec. Duplicate names are the user's problem (they'll both appear as the same tag).
- Consider whether custom sources should spawn even if `settings.native_logs.enabled == false`. Recommendation: respect the master toggle — if native logs are disabled, skip custom sources too.
