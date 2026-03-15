## Task: Modify `spawn_custom_sources` to Handle Shared Sources

**Objective**: Update `spawn_custom_sources` (the post-app path triggered on `AppStarted`) to skip shared sources already running and route new shared source events through `SharedSource*` message variants.

**Depends on**: 05-spawn-shared-pre-app

### Scope

- `crates/fdemon-app/src/actions/native_logs.rs`: Modify `spawn_custom_sources`
- `crates/fdemon-app/src/actions/mod.rs`: Pass running shared names to `spawn_native_log_capture`
- `crates/fdemon-app/src/handler/session.rs`: Pass running shared names through `StartNativeLogCapture`

### Details

#### 1. Extend `UpdateAction::StartNativeLogCapture`

Add `running_shared_names: Vec<String>` field to `StartNativeLogCapture` so `spawn_custom_sources` knows which shared sources to skip:

```rust
UpdateAction::StartNativeLogCapture {
    // ... existing fields ...
    running_source_names: Vec<String>,
    running_shared_names: Vec<String>,  // NEW
}
```

#### 2. Populate in `maybe_start_native_log_capture`

In `handler/session.rs`, when building `StartNativeLogCapture`:

```rust
running_shared_names: state.running_shared_source_names(),
```

#### 3. Pass Through to `spawn_custom_sources`

In `actions/mod.rs` dispatch and `native_logs::spawn_native_log_capture`, pass the new field through to `spawn_custom_sources`.

#### 4. Skip Logic in `spawn_custom_sources`

Add after the existing `start_before_app` skip:

```rust
if source_config.shared && running_shared_names.contains(&source_config.name) {
    tracing::debug!(
        "Skipping shared source '{}' in spawn_custom_sources (already running)",
        source_config.name
    );
    continue;
}
```

#### 5. Route Based on `shared` Flag

For new shared post-app sources (not yet running): send `SharedSourceStarted`/`SharedSourceLog`/`SharedSourceStopped` instead of the per-session variants.

### Acceptance Criteria

1. Already-running shared sources are skipped in `spawn_custom_sources`
2. New shared post-app sources send `SharedSource*` message variants
3. Non-shared post-app sources behavior unchanged
4. The `running_source_names` skip list (for pre-app sources) still works independently
5. All existing tests pass

### Testing

```rust
#[tokio::test]
async fn test_spawn_custom_sources_skips_running_shared() { ... }

#[tokio::test]
async fn test_spawn_custom_sources_shared_post_app_sends_shared_variants() { ... }
```

### Notes

- The two skip lists (`running_source_names` for pre-app, `running_shared_names` for shared) are independent — a shared pre-app source will be caught by either check
- This task modifies the same functions as task 05 but for the post-app code path
