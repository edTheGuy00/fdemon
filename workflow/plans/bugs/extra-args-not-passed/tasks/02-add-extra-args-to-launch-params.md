# Task 02: Add `extra_args` to `LaunchParams`

**File:** `crates/fdemon-app/src/new_session_dialog/types.rs`
**Depends on:** None (can be done in parallel with Task 01)
**Wave:** 1

## What to do

1. Add `pub extra_args: Vec<String>` field to `LaunchParams` struct (line ~175, after `entry_point`):
   ```rust
   pub struct LaunchParams {
       pub device_id: String,
       pub mode: crate::config::FlutterMode,
       pub flavor: Option<String>,
       pub dart_defines: Vec<String>,
       pub config_name: Option<String>,
       pub entry_point: Option<PathBuf>,
       pub extra_args: Vec<String>,  // <-- add this
   }
   ```

2. Fix any compilation errors from existing `LaunchParams` construction sites — they'll need `extra_args: vec![]` or the actual value. Search for `LaunchParams {` across the crate.

## Verification

- `cargo check -p fdemon-app` compiles (may need Tasks 03/04 first for full compilation)
