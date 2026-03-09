# Task 04: Use `extra_args` in `handle_launch()`

**File:** `crates/fdemon-app/src/handler/new_session/launch_context.rs`
**Depends on:** Task 02
**Wave:** 1

## What to do

1. Update the condition at line 438 to include `extra_args`:
   ```rust
   let config = if params.config_name.is_some()
       || params.flavor.is_some()
       || !params.dart_defines.is_empty()
       || params.entry_point.is_some()
       || !params.extra_args.is_empty()  // <-- add this
   {
   ```

2. Set `extra_args` in the `LaunchConfig` construction (line 443-449) instead of relying on `..Default::default()`:
   ```rust
   let mut cfg = LaunchConfig {
       name: params.config_name.unwrap_or_else(|| "Session".to_string()),
       device: device.id.clone(),
       mode: params.mode,
       flavor: params.flavor,
       entry_point: params.entry_point,
       extra_args: params.extra_args,  // <-- add this
       ..Default::default()
   };
   ```

## Verification

- `cargo check -p fdemon-app` compiles
- `cargo test -p fdemon-app -- handler::new_session` passes

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/handler/new_session/launch_context.rs` | Added `|| !params.extra_args.is_empty()` to the config-build condition; added `extra_args: params.extra_args` to the `LaunchConfig` struct literal |

### Notable Decisions/Tradeoffs

1. **Minimal change**: Only the two lines specified in the task were touched. The existing `..Default::default()` spread is retained for all other fields, so future additions to `LaunchConfig` default safely.

### Testing Performed

- `cargo check -p fdemon-app` - Passed
- `cargo test -p fdemon-app -- handler::new_session` - Passed (56 tests)
- `cargo clippy -p fdemon-app -- -D warnings` - Passed (no warnings)

### Risks/Limitations

1. **No new unit tests added**: The task did not request new tests. Existing tests cover the launch path but do not exercise the `extra_args` branch specifically — that coverage belongs to a follow-on test task if needed.
