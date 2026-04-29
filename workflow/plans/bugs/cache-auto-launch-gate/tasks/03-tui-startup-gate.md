# Task 03 — Re-gate TUI startup behind `[behavior] auto_launch`

**Plan:** [../BUG.md](../BUG.md) · **Index:** [../TASKS.md](../TASKS.md)
**Agent:** implementor
**Depends on:** Task 01 (new `auto_launch` field), Task 02 (`cache_allowed` plumbing)
**Wave:** 2 (parallel with Task 04)

## Goal

Modify the TUI startup gate so cache-driven auto-launch only fires when `settings.behavior.auto_launch == true`. Replace Task 02's hardcoded `cache_allowed: false` at the `Message::StartAutoLaunch` construction site with the real value from settings. Emit a one-time `info!` migration log when cache is present but `auto_launch` is unset, helping users who were quietly relying on commit `c5879fa`'s behavior. Update existing G1/G2/G3 tests and add G4/G5.

## Files Modified (Write)

| File | Change |
|------|--------|
| `crates/fdemon-tui/src/startup.rs` | (1) `cache_trigger` now requires `settings.behavior.auto_launch == true`. (2) Activate the currently-underscored `_settings` parameter. (3) Emit migration `info!` when cache is present but `auto_launch` unset and no auto_start config exists. (4) Update tests. |
| `crates/fdemon-tui/src/runner.rs` | At `dispatch_startup_action`'s `AutoStart` arm, replace Task 02's hardcoded `cache_allowed: false` with `engine.settings.behavior.auto_launch`. |

## Files Read (dependency)

- `crates/fdemon-app/src/config/types.rs` — `BehaviorSettings.auto_launch` (Task 01)
- `crates/fdemon-app/src/message.rs` — `Message::StartAutoLaunch` shape with `cache_allowed` (Task 02)

## Implementation Notes

### `startup_flutter` (startup.rs:49-73)

Current:
```rust
pub fn startup_flutter(
    state: &mut AppState,
    _settings: &config::Settings,
    project_path: &Path,
) -> StartupAction {
    let configs = load_all_configs(project_path);
    let has_auto_start_config = get_first_auto_start(&configs).is_some();
    let cache_trigger = !has_auto_start_config && has_cached_last_device(project_path);

    if has_auto_start_config || cache_trigger {
        return StartupAction::AutoStart { configs };
    }

    state.show_new_session_dialog(configs);
    state.ui_mode = UiMode::Startup;
    StartupAction::Ready
}
```

After:
```rust
pub fn startup_flutter(
    state: &mut AppState,
    settings: &config::Settings,
    project_path: &Path,
) -> StartupAction {
    let configs = load_all_configs(project_path);
    let has_auto_start_config = get_first_auto_start(&configs).is_some();
    let has_cache              = has_cached_last_device(project_path);
    let cache_opt_in           = settings.behavior.auto_launch;

    let cache_trigger = !has_auto_start_config && cache_opt_in && has_cache;

    // Migration nudge: user has a cached device but didn't opt in. Tell them
    // this once so they understand why fdemon didn't auto-launch like it used to.
    if !has_auto_start_config && has_cache && !cache_opt_in {
        tracing::info!(
            "settings.local.toml has a cached last_device but [behavior] auto_launch \
             is not set in config.toml. Auto-launch via cache is now opt-in. \
             Set `[behavior] auto_launch = true` to restore the previous behavior."
        );
    }

    if has_auto_start_config || cache_trigger {
        return StartupAction::AutoStart { configs };
    }

    state.show_new_session_dialog(configs);
    state.ui_mode = UiMode::Startup;
    StartupAction::Ready
}
```

### `dispatch_startup_action` in `crates/fdemon-tui/src/runner.rs`

Replace Task 02's placeholder:

```rust
// Task 02 (placeholder):
engine.process_message(Message::StartAutoLaunch { configs, cache_allowed: false });

// Task 03 (real value):
let cache_allowed = engine.settings.behavior.auto_launch;
engine.process_message(Message::StartAutoLaunch { configs, cache_allowed });
```

### Test updates

Existing tests in `crates/fdemon-tui/src/startup.rs` pass `Settings::default()` (which now defaults `auto_launch = false`). The cache-trigger test `test_startup_flutter_cache_last_device_triggers_auto_start` (G1) currently asserts `AutoStart` — under the new gate it should assert `Ready`. **This test is the user's repro and must flip its assertion.**

New / updated test matrix:

| Test | Setup | Expected |
|------|-------|----------|
| G1 (renamed: `cache_alone_does_not_trigger_auto_start`) | cache present, `auto_launch = false`, no auto_start configs | `Ready`, dialog shown |
| G2 (renamed: `cache_with_auto_launch_triggers_auto_start`) | cache present, `auto_launch = true`, no auto_start configs | `AutoStart` |
| G3 (kept: `auto_start_config_beats_cache_regardless_of_flag`) | cache present, `auto_launch = false`, auto_start config present | `AutoStart` |
| G4 (new: `auto_start_config_beats_cache_with_flag_set`) | cache present, `auto_launch = true`, auto_start config present | `AutoStart` |
| G5 (new: `nothing_set_shows_dialog`) | no cache, `auto_launch = false`, no auto_start configs | `Ready` |

Construct `Settings` via `Settings::default()` and mutate `settings.behavior.auto_launch` per case — do not rely on file-based config loading inside these unit tests.

## Verification

- `cargo check --workspace`
- `cargo test -p fdemon-tui startup`
- `cargo clippy --workspace -- -D warnings`
- Manual smoke: in `example/app2` (no `auto_launch` set, cache present) → New Session dialog appears, `info!` line in fdemon log file. Add `[behavior] auto_launch = true` → cache fires.

## Acceptance

- [ ] `startup_flutter` reads `settings.behavior.auto_launch` (parameter no longer underscored).
- [ ] `cache_trigger` requires `auto_launch == true`.
- [ ] Migration `info!` fires under the documented condition.
- [ ] `dispatch_startup_action` passes `engine.settings.behavior.auto_launch` as `cache_allowed`.
- [ ] G1 test assertion flipped (`AutoStart` → `Ready`); G2-G5 added/updated.
- [ ] Manual repro from BUG.md now shows the dialog by default.
