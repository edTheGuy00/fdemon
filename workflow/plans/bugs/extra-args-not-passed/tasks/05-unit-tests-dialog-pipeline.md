# Task 05: Unit tests for `extra_args` in dialog pipeline

**Files:**
- `crates/fdemon-app/src/new_session_dialog/state.rs` (tests module)
- `crates/fdemon-app/src/handler/new_session/launch_context.rs` (tests module)

**Depends on:** Tasks 01-04
**Wave:** 2 (parallel with Task 06)

## What to do

### Test 1: `select_config` copies `extra_args`

In `state.rs` tests, add a test that:
1. Creates a `LaunchContextState` with a config containing `extra_args: vec!["--dart-define-from-file=envs/staging.env.json".to_string()]`
2. Calls `select_config(Some(0))`
3. Asserts `state.extra_args == vec!["--dart-define-from-file=envs/staging.env.json"]`

### Test 2: `build_launch_params` includes `extra_args`

In `state.rs` tests (or `NewSessionDialogState` tests), add a test that:
1. Sets up dialog state with a selected config that has `extra_args`
2. Calls `build_launch_params()`
3. Asserts the returned `LaunchParams` has the expected `extra_args`

### Test 3: `handle_launch` produces config with `extra_args`

In `launch_context.rs` tests, add a test that:
1. Sets up state where `LaunchParams` has `extra_args: vec!["--dart-define-from-file=env.json".to_string()]`
2. Calls `handle_launch()`
3. Asserts the resulting `SpawnSession` action contains a config with populated `extra_args`

## Verification

- `cargo test -p fdemon-app -- extra_args` passes

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/new_session_dialog/state.rs` | Added `test_select_config_copies_extra_args` and `test_build_launch_params_includes_extra_args` tests |
| `crates/fdemon-app/src/handler/new_session/launch_context.rs` | Added `test_handle_launch_extra_args_in_spawn_session_config` test |

### Notable Decisions/Tradeoffs

1. **Test 2 placed in `NewSessionDialogState` tests**: `build_launch_params()` is a method on `NewSessionDialogState`, so the test constructs that type (which contains both `launch_context` and `target_selector`). A connected device must be provided at `selected_index = 1` (because index 0 is the group header) to satisfy `build_launch_params`'s device requirement.

2. **Test 3 sets `extra_args` directly on `launch_context`**: Rather than going through `select_config`, the test directly assigns `launch_context.extra_args` to simulate the state that would exist after config selection. This tests the handler path in isolation, consistent with the existing test patterns in the file.

### Testing Performed

- `cargo test -p fdemon-app -- extra_args` - Passed (30 tests, all ok)
- `cargo clippy -p fdemon-app -- -D warnings` - Passed (no warnings)

### Risks/Limitations

1. **None**: All three tests exercise the pipeline end-to-end per the acceptance criteria.
