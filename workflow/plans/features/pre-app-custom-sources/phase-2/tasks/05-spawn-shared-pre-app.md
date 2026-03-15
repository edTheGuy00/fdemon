## Task: Modify `spawn_pre_app_sources` to Handle Shared Sources

**Objective**: Update `spawn_pre_app_sources` to check if shared pre-app sources are already running on `AppState` before spawning them. New shared sources send `SharedSourceStarted`/`SharedSourceLog` instead of `CustomSourceStarted`/`NativeLog`.

**Depends on**: 01-config-shared-field, 02-shared-source-handle, 03-message-variants

### Scope

- `crates/fdemon-app/src/actions/native_logs.rs`: Modify `spawn_pre_app_sources` and `spawn_one_pre_app_source`
- `crates/fdemon-app/src/actions/mod.rs`: Pass shared source names to `spawn_pre_app_sources`

### Details

#### 1. Update `SpawnPreAppSources` Action to Carry Running Shared Names

In `actions/mod.rs`, when dispatching `spawn_pre_app_sources`, pass a list of already-running shared source names from `state.shared_source_handles`:

```rust
UpdateAction::SpawnPreAppSources { session_id, device, config, settings, project_path } => {
    let running_shared = state.running_shared_source_names();
    native_logs::spawn_pre_app_sources(
        session_id, device, config, &settings, &project_path, &msg_tx, &running_shared,
    );
}
```

#### 2. Update `spawn_pre_app_sources` Signature

Add `running_shared_names: &[String]` parameter.

#### 3. Skip Already-Running Shared Sources

In the pre-app source loop, before spawning:

```rust
if source_config.shared && running_shared_names.contains(&source_config.name) {
    tracing::debug!(
        "Skipping shared pre-app source '{}' (already running)",
        source_config.name
    );
    continue;  // Don't count toward sources_with_checks either
}
```

#### 4. Route Messages Based on `shared` Flag

In `spawn_one_pre_app_source`, when creating the forwarding task:

- If `source_config.shared`: send `Message::SharedSourceStarted` and `Message::SharedSourceLog` and `Message::SharedSourceStopped`
- If `!source_config.shared`: send existing `Message::CustomSourceStarted` and `Message::NativeLog` and `Message::CustomSourceStopped` (unchanged behavior)

The `session_id` is still passed for non-shared sources. Shared sources do not capture `session_id` in the forwarding closure.

#### 5. Ready Check Still Applies for New Shared Sources

If a shared source is being spawned for the first time (not skipped), its ready check runs normally. Only already-running shared sources skip the ready check.

### Acceptance Criteria

1. Already-running shared sources are skipped (not spawned again)
2. New shared sources send `SharedSourceStarted`/`SharedSourceLog`/`SharedSourceStopped`
3. Non-shared sources behavior is unchanged (still per-session)
4. Ready checks still run for newly-spawned shared sources
5. `PreAppSourcesReady` is sent when all sources (shared + non-shared) are ready or skipped
6. All existing tests pass

### Testing

```rust
#[tokio::test]
async fn test_spawn_pre_app_skips_running_shared_sources() { ... }

#[tokio::test]
async fn test_spawn_pre_app_shared_sends_shared_source_started() { ... }

#[tokio::test]
async fn test_spawn_pre_app_non_shared_unchanged() { ... }
```

### Notes

- The coordinator still waits for all non-skipped sources' ready checks before sending `PreAppSourcesReady`
- If ALL pre-app sources are shared and already running, the fast path sends `PreAppSourcesReady` immediately (same as the existing "no pre-app sources" fast path)
