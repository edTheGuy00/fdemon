# Task 01: Add `extra_args` to `LaunchContextState` and copy in `select_config()`

**File:** `crates/fdemon-app/src/new_session_dialog/state.rs`
**Depends on:** None
**Wave:** 1 (sequential)

## What to do

1. Add `pub extra_args: Vec<String>` field to `LaunchContextState` (around line 431, before `focused_field`)

2. Initialize it in `LaunchContextState::new()` as `extra_args: Vec::new()`

3. In `select_config()` (around line 506-526), after the `dart_defines` copy block, add:
   ```rust
   if !config.config.extra_args.is_empty() {
       self.extra_args = config.config.extra_args.clone();
   }
   ```

## Verification

- `cargo check -p fdemon-app` compiles
- Existing tests in `state.rs` still pass: `cargo test -p fdemon-app -- new_session_dialog::state`
