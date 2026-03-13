## Task: Message & UpdateAction Variants for Pre-App Sources

**Objective**: Add the TEA message variants and `UpdateAction` variant needed to drive the pre-app source lifecycle through the event loop.

**Depends on**: Task 01 (config types — needs `CustomSourceConfig` with `start_before_app`)

### Scope

- `crates/fdemon-app/src/message.rs`: Add 3 message variants
- `crates/fdemon-app/src/handler/mod.rs`: Add 1 `UpdateAction` variant

### Details

#### 1. Add Message Variants

Add to the `Message` enum in `message.rs`. Place them near the existing native-log lifecycle messages (`NativeLogCaptureStarted`, `NativeLogCaptureStopped`, `CustomSourceStarted`, `CustomSourceStopped`):

```rust
/// All pre-app custom sources are ready (or individually timed out).
/// Triggers the Flutter session spawn that was gated on readiness.
PreAppSourcesReady {
    session_id: SessionId,
    device: Device,
    config: Option<Box<LaunchConfig>>,
},

/// A specific pre-app source's readiness check timed out.
/// Informational — logged as a warning. Does not block other sources.
PreAppSourceTimedOut {
    session_id: SessionId,
    source_name: String,
},

/// Progress update during pre-app source startup.
/// Displayed in the session's log buffer for user feedback.
PreAppSourceProgress {
    session_id: SessionId,
    message: String,
},
```

**Why these three:**
- `PreAppSourcesReady` is the gate-release signal — when received, the handler returns `UpdateAction::SpawnSession` to launch Flutter. It carries the same data as `SpawnSession` needs (`session_id`, `device`, `config`).
- `PreAppSourceTimedOut` is per-source, informational — the handler logs a prominent warning but does not block the launch.
- `PreAppSourceProgress` provides real-time feedback during the readiness wait (e.g., "Starting server 'my-server'...", "Server 'my-server' ready (3.2s)").

#### 2. Add `UpdateAction::SpawnPreAppSources` Variant

Add to the `UpdateAction` enum in `handler/mod.rs`, near `SpawnSession`:

```rust
/// Spawn pre-app custom sources and run their readiness checks before
/// launching the Flutter session.
///
/// Dispatched by `handle_launch()` when the config has custom sources with
/// `start_before_app = true`. On completion (all sources ready or timed out),
/// sends `Message::PreAppSourcesReady` which triggers `SpawnSession`.
SpawnPreAppSources {
    session_id: SessionId,
    device: Device,
    config: Option<Box<LaunchConfig>>,
    settings: NativeLogsSettings,
    project_path: std::path::PathBuf,
},
```

**Why this payload:**
- `session_id`, `device`, `config` are passed through to `PreAppSourcesReady` → `SpawnSession` (the data needed to launch Flutter).
- `settings` provides access to `custom_sources` (filtered for `start_before_app`), `exclude_tags`, `include_tags`.
- `project_path` is needed for `working_dir` default resolution when constructing daemon-layer configs.

#### 3. Add Stub Handler in `update.rs`

Add match arms for the new message variants in `handler::update()`. These are stubs that will be filled in by Task 05:

```rust
Message::PreAppSourcesReady { session_id, device, config } => {
    // Task 05 will implement: return UpdateAction::SpawnSession
    UpdateResult::none()
}
Message::PreAppSourceTimedOut { session_id, source_name } => {
    // Task 05 will implement: log warning to session
    UpdateResult::none()
}
Message::PreAppSourceProgress { session_id, message } => {
    // Task 05 will implement: add log entry to session
    UpdateResult::none()
}
```

#### 4. Add Stub Dispatch in `actions/mod.rs`

Add a match arm for the new action variant in `handle_action()`. This is a stub that will be filled in by Task 06:

```rust
UpdateAction::SpawnPreAppSources { session_id, device, config, settings, project_path } => {
    // Task 06 will implement: call native_logs::spawn_pre_app_sources()
    tracing::debug!("SpawnPreAppSources action dispatched for session {}", session_id);
}
```

### Acceptance Criteria

1. `Message` enum compiles with the 3 new variants
2. `UpdateAction` enum compiles with the new variant
3. All existing match arms on `Message` and `UpdateAction` are exhaustive (no new warnings)
4. Stub handlers exist for all new variants (no-op, but code compiles and routes correctly)
5. `cargo check -p fdemon-app` passes
6. `cargo test -p fdemon-app` passes (no regressions)

### Testing

This task is primarily type definitions. The key test is compilation. Optionally add a basic construction test:

```rust
#[test]
fn test_pre_app_message_variants_construct() {
    let _msg = Message::PreAppSourcesReady {
        session_id: 1,
        device: test_device(),
        config: None,
    };
    let _msg = Message::PreAppSourceTimedOut {
        session_id: 1,
        source_name: "server".to_string(),
    };
    let _msg = Message::PreAppSourceProgress {
        session_id: 1,
        message: "Starting server...".to_string(),
    };
}
```

### Notes

- The `Device` and `LaunchConfig` types are already imported/available in `message.rs`. Check existing imports.
- `NativeLogsSettings` needs to be available in `handler/mod.rs` for the `UpdateAction` variant. It should already be reachable via `crate::config::NativeLogsSettings`. Verify the import path.
- Stub handlers must be present so the compiler doesn't complain about non-exhaustive matches. Later tasks (05, 06) replace the stubs with real logic.

---

## Completion Summary

**Status:** Not Started
