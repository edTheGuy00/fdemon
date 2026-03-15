## Task: Skip Pre-App Ready Check for Already-Running Shared Sources

**Objective**: When a second session launches with shared pre-app sources already running, skip the readiness check wait and proceed directly to `SpawnSession`. Only wait for non-running (non-shared or newly-spawned shared) pre-app sources.

**Depends on**: 04-tea-handlers, 05-spawn-shared-pre-app

### Scope

- `crates/fdemon-app/src/handler/new_session/launch_context.rs`: Modify launch decision
- `crates/fdemon-app/src/handler/update.rs`: Modify `AutoLaunchResult` handler

### Details

#### 1. Modify Launch Gating Decision

Currently the gate decision in `handle_launch()` and `AutoLaunchResult` is:

```rust
if state.settings.native_logs.enabled && state.settings.native_logs.has_pre_app_sources() {
    UpdateAction::SpawnPreAppSources { ... }
} else {
    UpdateAction::SpawnSession { ... }
}
```

This should change to account for shared sources that are already running:

```rust
let has_unstarted_pre_app = state.settings.native_logs.pre_app_sources().any(|s| {
    // Non-shared sources always need spawning (per-session)
    // Shared sources only need spawning if not already running
    !s.shared || !state.is_shared_source_running(&s.name)
});

if state.settings.native_logs.enabled && has_unstarted_pre_app {
    UpdateAction::SpawnPreAppSources { ... }
} else if state.settings.native_logs.enabled && state.settings.native_logs.has_pre_app_sources() {
    // All pre-app sources are shared and already running — skip gating
    UpdateAction::SpawnSession { ... }
} else {
    UpdateAction::SpawnSession { ... }
}
```

Or more simply:

```rust
let needs_pre_app_spawn = state.settings.native_logs.enabled
    && state.settings.native_logs.pre_app_sources().any(|s| {
        !s.shared || !state.is_shared_source_running(&s.name)
    });

if needs_pre_app_spawn {
    UpdateAction::SpawnPreAppSources { ... }
} else {
    UpdateAction::SpawnSession { ... }
}
```

#### 2. Apply Same Logic to Both Sites

This change must be applied in both:
- `launch_context.rs` — manual launch path
- `update.rs` — auto-launch path

### Acceptance Criteria

1. Second session with all-shared-already-running pre-app sources skips `SpawnPreAppSources` entirely
2. Second session with mixed sources (some shared running, some non-shared) still goes through `SpawnPreAppSources` for the non-shared ones
3. First session behavior unchanged (shared sources not yet running → normal gate)
4. All existing auto-launch and manual-launch tests pass

### Testing

```rust
#[test]
fn test_launch_skips_gate_when_all_shared_pre_app_running() { ... }

#[test]
fn test_launch_gates_when_non_shared_pre_app_present() { ... }

#[test]
fn test_launch_gates_when_shared_pre_app_not_yet_running() { ... }

#[test]
fn test_auto_launch_skips_gate_when_all_shared_pre_app_running() { ... }
```

### Notes

- The `SpawnPreAppSources` handler (in `spawn_pre_app_sources`) already handles the case where all sources are skipped — it sends `PreAppSourcesReady` immediately. So even if we don't optimize the gate here, it would work correctly (just with an unnecessary async task spawn). The optimization avoids the overhead.
