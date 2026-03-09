# Task 03: Populate `extra_args` in `build_launch_params()`

**File:** `crates/fdemon-app/src/new_session_dialog/state.rs`
**Depends on:** Task 01, Task 02
**Wave:** 1

## What to do

1. In `build_launch_params()` (around line 900-918), add `extra_args` to the returned `LaunchParams`:
   ```rust
   Some(LaunchParams {
       device_id: device.id.clone(),
       mode: self.launch_context.mode,
       flavor: self.launch_context.flavor.clone(),
       dart_defines: self
           .launch_context
           .dart_defines
           .iter()
           .map(|d| d.to_arg())
           .collect(),
       config_name: self
           .launch_context
           .selected_config()
           .map(|c| c.display_name.clone()),
       entry_point: self.launch_context.entry_point.clone(),
       extra_args: self.launch_context.extra_args.clone(),  // <-- add this
   })
   ```

## Verification

- `cargo check -p fdemon-app` compiles
