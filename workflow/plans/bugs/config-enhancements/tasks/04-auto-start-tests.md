## Task: Auto-Start Tests

**Objective**: Update existing broken-behavior tests and add comprehensive unit tests for the auto-start startup flow.

**Depends on**: 03-fix-auto-start

### Scope

- `crates/fdemon-tui/src/startup.rs`: Update/replace existing tests, add new coverage
- `crates/fdemon-app/src/config/priority.rs`: Verify existing `get_first_auto_start` tests are adequate

### Details

**Tests to update in startup.rs:**

1. **Remove `test_startup_flutter_ignores_auto_start_setting`** (line 56-68) — This test enforced the broken behavior. Replace with tests that verify auto_start is respected.

2. **Update `test_startup_flutter_shows_new_session_dialog`** (line 43-54) — Verify this still passes for the no-auto_start case.

**New test cases for startup.rs:**

1. **`test_startup_flutter_returns_auto_start_when_launch_config_has_auto_start`**
   - Create a launch.toml in a tempdir with `auto_start = true` on one config
   - Call `startup_flutter()` with default settings
   - Assert returns `StartupAction::AutoStart { configs }`
   - Assert `state.ui_mode` is NOT `UiMode::Startup` (dialog not shown)

2. **`test_startup_flutter_returns_auto_start_when_behavior_auto_start_true`**
   - No launch.toml (or no auto_start in configs)
   - Set `settings.behavior.auto_start = true`
   - Assert returns `StartupAction::AutoStart`

3. **`test_startup_flutter_shows_dialog_when_no_auto_start`**
   - No auto_start in launch.toml, `behavior.auto_start = false`
   - Assert returns `StartupAction::Ready`
   - Assert `state.ui_mode == UiMode::Startup`

4. **`test_startup_flutter_prefers_launch_config_auto_start`**
   - `launch.toml` has `auto_start = true`, `behavior.auto_start = false`
   - Assert returns `StartupAction::AutoStart`

5. **`test_startup_flutter_auto_start_configs_passed_through`**
   - Verify the `configs` in `AutoStart` contain the loaded launch configs

6. **`test_startup_flutter_multiple_configs_one_auto_start`**
   - Multiple configs in launch.toml, only one with `auto_start = true`
   - Assert returns `StartupAction::AutoStart`
   - Assert configs contain all configurations (not just the auto_start one)

### Acceptance Criteria

1. Broken-behavior test is removed/replaced
2. All auto-start scenarios have passing tests
3. No-auto-start scenario still works correctly
4. Tests verify both `launch.toml` and `config.toml` auto_start paths
5. `cargo test -p fdemon-tui` passes with no regressions

### Testing

```bash
cargo test -p fdemon-tui -- startup
cargo test --workspace
```

### Notes

- Tests may need to create tempdir-based `.fdemon/launch.toml` files for `load_all_configs()` to find
- Alternatively, if `startup_flutter` can be tested with mock configs (pre-loaded), that's simpler
- The `StartupAction` enum needs to derive `Debug` and possibly `PartialEq` for test assertions

---

## Completion Summary

**Status:** Not Started
