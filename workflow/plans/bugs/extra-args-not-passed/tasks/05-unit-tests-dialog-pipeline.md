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
