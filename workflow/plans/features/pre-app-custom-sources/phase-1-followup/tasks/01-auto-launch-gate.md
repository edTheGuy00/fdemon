## Task: Fix Auto-Launch Path to Gate on Pre-App Sources

**Objective**: The `AutoLaunchResult` handler in `update.rs` bypasses pre-app source gating entirely, returning `SpawnSession` directly. Add the same conditional gate that exists in `handle_launch()` so that auto-launched sessions also wait for pre-app dependencies.

**Depends on**: None

**Severity**: Critical

### Scope

- `crates/fdemon-app/src/handler/update.rs`: Modify `AutoLaunchResult` success handler (~line 903)
- `crates/fdemon-app/src/handler/tests.rs`: Add test for auto-launch + pre-app gating

### Details

#### 1. The Bug

`handle_launch()` in `launch_context.rs:496-513` correctly gates:

```rust
if state.settings.native_logs.enabled
    && state.settings.native_logs.has_pre_app_sources()
{
    UpdateAction::SpawnPreAppSources { ... }
} else {
    UpdateAction::SpawnSession { ... }
}
```

But `AutoLaunchResult` in `update.rs:881-947` always returns `SpawnSession`:

```rust
UpdateResult::action(UpdateAction::SpawnSession {
    session_id,
    device,
    config: config.map(Box::new),
})
```

#### 2. Fix

In the `AutoLaunchResult` handler's `Ok(session_id)` branch (~line 919), replace the unconditional `SpawnSession` with the same conditional:

```rust
let action = if state.settings.native_logs.enabled
    && state.settings.native_logs.has_pre_app_sources()
{
    UpdateAction::SpawnPreAppSources {
        session_id,
        device,
        config: config.map(Box::new),
        settings: state.settings.native_logs.clone(),
        project_path: state.project_path.clone(),
    }
} else {
    UpdateAction::SpawnSession {
        session_id,
        device,
        config: config.map(Box::new),
    }
};
UpdateResult::action(action)
```

### Acceptance Criteria

1. `AutoLaunchResult` returns `SpawnPreAppSources` when `native_logs.enabled && native_logs.has_pre_app_sources()` is true
2. `AutoLaunchResult` returns `SpawnSession` when no pre-app sources exist (existing behavior preserved)
3. New test `test_auto_launch_with_pre_app_sources_returns_spawn_pre_app` asserts the gated path
4. Existing auto-launch tests continue to pass unchanged

### Testing

Add a test adjacent to the existing `test_auto_launch_flow_success` tests (~line 2040 in `tests.rs`):

```rust
#[test]
fn test_auto_launch_with_pre_app_sources_returns_spawn_pre_app() {
    // Setup: state with auto_start + native_logs.enabled + a custom source
    // with start_before_app = true
    // Process: send AutoLaunchResult { result: Ok(success) }
    // Assert: action is SpawnPreAppSources (not SpawnSession)
}
```

Also add the inverse test confirming that auto-launch WITHOUT pre-app sources still returns `SpawnSession`.

### Notes

- Mirror the exact condition from `launch_context.rs:496-513` — do not add extra logic
- The `SpawnPreAppSources` variant requires `settings` and `project_path` fields that `SpawnSession` does not — both are available on `state`
- The existing auto-launch tests at lines 2040-2219 all assert `SpawnSession` — they should still pass because none configure pre-app sources

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/handler/update.rs` | Replaced unconditional `SpawnSession` in `AutoLaunchResult` success handler with the same conditional gate from `handle_launch()`: dispatches `SpawnPreAppSources` when `native_logs.enabled && native_logs.has_pre_app_sources()`, otherwise `SpawnSession` |
| `crates/fdemon-app/src/handler/tests.rs` | Added `test_auto_launch_with_pre_app_sources_returns_spawn_pre_app` and `test_auto_launch_without_pre_app_sources_returns_spawn_session` inside the `auto_launch_tests` module, with a local `pre_app_source` helper mirroring the one in `launch_context.rs` |

### Notable Decisions/Tradeoffs

1. **Exact condition mirror**: Used `state.settings.native_logs.enabled && state.settings.native_logs.has_pre_app_sources()` verbatim from `launch_context.rs:497-498` with no additional logic, as the task specified.
2. **Local test helper**: Duplicated the `pre_app_source` helper inside `auto_launch_tests` rather than hoisting it to a shared location, matching the existing pattern where `launch_context.rs` defines its own copy.

### Testing Performed

- `cargo check -p fdemon-app` - Passed (2 pre-existing unrelated warnings)
- `cargo test -p fdemon-app` - Passed (1646 tests pass, 0 failed, 4 ignored)
- `cargo test -p fdemon-app test_auto_launch_with_pre_app_sources` - Passed (1 test)
- `cargo test -p fdemon-app test_auto_launch_without_pre_app_sources` - Passed (1 test)
- `cargo clippy -p fdemon-app -- -D warnings` - Passed (no new warnings)

### Risks/Limitations

1. **None identified**: The change is a direct port of existing logic already exercised in `launch_context.rs` tests; no new code paths or data structures were introduced.
