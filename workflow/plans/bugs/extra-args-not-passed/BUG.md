# Bug: `extra_args` from `launch.toml` Not Passed to Flutter Process via New Session Dialog

**Status:** Confirmed
**Severity:** High - `extra_args` (including `--dart-define-from-file`) is silently dropped when launching through the new session dialog
**Reported by:** GitHub user
**Date:** 2026-03-09

## Symptom

User has a `launch.toml` with `extra_args = ["--dart-define-from-file=envs/staging.env.json"]`. The flavor from the same config works, but `extra_args` is silently ignored. The env file is never loaded.

## Root Cause

**`LaunchParams` is missing an `extra_args` field.** When the new session dialog launches a Flutter session, the flow is:

1. User selects a config in the dialog
2. `LaunchContextState::select_config()` copies `mode`, `flavor`, `entry_point`, `dart_defines` from the selected config — but **NOT `extra_args`** (`state.rs:506-526`)
3. `LaunchContextState` itself has **no `extra_args` field** (`state.rs:407-434`)
4. `build_launch_params()` builds a `LaunchParams` which also **has no `extra_args` field** (`types.rs:169-176`)
5. `handle_launch()` reconstructs a `LaunchConfig` from `LaunchParams` using `..Default::default()`, which sets `extra_args: vec![]` (`launch_context.rs:443-449`)

The `extra_args` data is lost at step 2 and never recoverable after that.

### Why flavor works but extra_args doesn't

`flavor` has a dedicated field in `LaunchContextState`, `LaunchParams`, and is explicitly set in the `LaunchConfig` construction. `extra_args` was never added to any of these intermediate types.

## Affected Code Path (Dialog Launch - Path B)

```
User selects config in dialog
  → select_config() copies mode, flavor, entry_point, dart_defines ← extra_args MISSING
    → build_launch_params() → LaunchParams { ... } ← no extra_args field
      → handle_launch() → LaunchConfig { ..Default::default() } ← extra_args = vec![]
        → build_flutter_args() → args list ← extra_args empty, nothing appended
          → FlutterProcess::spawn_with_args(args) ← --dart-define-from-file absent
```

## Unaffected Code Path (Auto-launch - Path A)

The auto-launch path works correctly because it passes the full `LaunchConfig` loaded from TOML directly, without going through `LaunchParams`:

```
find_auto_launch_target() → AutoLaunchSuccess { config: Some(sourced.config.clone()) }
  → spawn_session() → cfg.build_flutter_args() ← extra_args included via line 334
    → FlutterProcess::spawn_with_args(args) ← --dart-define-from-file present
```

## Key Files

| File | Lines | Role |
|------|-------|------|
| `crates/fdemon-app/src/config/types.rs` | 39-41 | `LaunchConfig.extra_args: Vec<String>` definition |
| `crates/fdemon-app/src/config/launch.rs` | 334 | `args.extend(self.extra_args.clone())` - correctly appends |
| `crates/fdemon-app/src/new_session_dialog/types.rs` | 169-176 | `LaunchParams` - **missing `extra_args`** |
| `crates/fdemon-app/src/new_session_dialog/state.rs` | 407-434 | `LaunchContextState` - **missing `extra_args`** |
| `crates/fdemon-app/src/new_session_dialog/state.rs` | 506-526 | `select_config()` - **doesn't copy `extra_args`** |
| `crates/fdemon-app/src/new_session_dialog/state.rs` | 900-918 | `build_launch_params()` - **doesn't include `extra_args`** |
| `crates/fdemon-app/src/handler/new_session/launch_context.rs` | 438-462 | `handle_launch()` - uses `..Default::default()` |
| `crates/fdemon-app/src/actions/session.rs` | 59-65 | `spawn_session()` - calls `build_flutter_args` |
| `crates/fdemon-daemon/src/process.rs` | 64-80 | `spawn_with_args()` - passes args verbatim (correct) |

## Fix Strategy

### Option A: Pass `extra_args` through the dialog pipeline (Recommended)

Thread `extra_args` through all intermediate types so it survives the dialog path:

1. **Add `extra_args: Vec<String>` to `LaunchContextState`** (`state.rs`)
2. **Copy `extra_args` in `select_config()`** when user selects a config (`state.rs:506-526`)
3. **Add `extra_args: Vec<String>` to `LaunchParams`** (`types.rs:169-176`)
4. **Populate `extra_args` in `build_launch_params()`** (`state.rs:900-918`)
5. **Use `params.extra_args` in `handle_launch()`** instead of `..Default::default()` for `extra_args` (`launch_context.rs:443-449`)
6. **Update the condition at line 438** to also check `!params.extra_args.is_empty()`

### Option B: Use the original `LaunchConfig` directly in `handle_launch`

Instead of reconstructing a `LaunchConfig` from `LaunchParams`, look up the original config by name and use it directly. This avoids the field-by-field copy problem but changes the architecture more significantly.

**Recommendation:** Option A - it follows the existing pattern and is less invasive.

## Test Plan

1. Add unit tests verifying `extra_args` survives the full dialog pipeline
2. Test `build_launch_params()` includes `extra_args` from selected config
3. Test `handle_launch()` produces a `LaunchConfig` with populated `extra_args`
4. Manual test with example app using `--dart-define-from-file`

## Example App for Testing

Create `.fdemon/launch.toml` and `envs/staging.env.json` in `example/app1/` to reproduce and verify the fix. See `example/app1/envs/` for the test env file and updated `launch.toml`.
