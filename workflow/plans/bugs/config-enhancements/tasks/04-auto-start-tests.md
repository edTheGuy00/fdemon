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

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-tui/src/startup.rs` | Added 3 new test functions: `test_startup_flutter_prefers_launch_config_auto_start`, `test_startup_flutter_auto_start_configs_passed_through`, `test_startup_flutter_multiple_configs_one_auto_start` |

### Notable Decisions/Tradeoffs

1. **Pre-existing tests covered most criteria**: Task 03-fix-auto-start already implemented the broken-behavior test removal and the basic new tests. This task added the 3 remaining test cases that weren't covered.

2. **PartialEq not added to StartupAction**: The new tests use `matches!` macro and `if let`/`match` destructuring, which avoids needing `PartialEq` on `StartupAction`. Since `LoadedConfigs` doesn't implement `PartialEq`, adding it to `StartupAction` would require additional derives elsewhere — unnecessary.

3. **All configs in AutoStart**: The `test_startup_flutter_multiple_configs_one_auto_start` test verifies that all 3 configs (not just the auto_start one) are carried in the `AutoStart` variant, confirming the implementation correctly passes the full `LoadedConfigs` struct.

4. **Snapshot test failures are pre-existing**: 4 snapshot tests fail due to a version string mismatch (`v0.1.0` vs `v0.2.1`) that predates this task. No new failures were introduced.

### Testing Performed

- `cargo fmt --all` - Passed
- `cargo check --workspace` - Passed
- `cargo test -p fdemon-tui -- startup` - Passed (7 tests)
- `cargo test -p fdemon-app -- auto_launch` - Passed (39 tests)
- `cargo test --workspace` - Passed (822 passed, 4 pre-existing snapshot failures unrelated to this task)
- `cargo clippy --workspace -- -D warnings` - Passed

### Risks/Limitations

1. **Snapshot test failures**: 4 pre-existing snapshot test failures remain (`render::tests::snapshot_normal_mode_*`) due to version string mismatch. These are not introduced by this task.
