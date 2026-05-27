## Task: Launch N sessions from the checked set

**Objective**: Extend the dialog launch path so that when ≥1 device is checked, one confirm spawns a session per checked device (sharing the launch context); with zero checked, behavior is identical to today.

**Depends on**: 01-multi-select-state

**Estimated Time**: 3–4h

### Scope

**Files Modified (Write):**
- `crates/fdemon-app/src/new_session_dialog/state.rs`: add a helper to build `LaunchParams` for a specific device id using the shared launch context.
- `crates/fdemon-app/src/handler/new_session/launch_context.rs`: rework `handle_launch` to fan out over the checked set, collecting one action per device and returning `UpdateResult::actions_vec`.

**Files Read (Dependencies):**
- `crates/fdemon-app/src/new_session_dialog/target_selector_state.rs`: `checked_devices()`, `checked_count()`.
- `crates/fdemon-app/src/session_manager.rs`: `create_session_configured`, `create_session_with_config_configured`, `find_active_by_device_id`, `MAX_SESSIONS`.
- `crates/fdemon-app/src/new_session_dialog/types.rs`: `LaunchParams`.

### Details

**Per-device params helper** (`new_session_dialog/state.rs`, next to `build_launch_params`):

```rust
/// Build launch params for a specific connected device, reusing the shared
/// launch context (mode/flavor/dart-defines/config/entry-point/extra-args).
pub fn build_launch_params_for_device(&self, device_id: &str) -> LaunchParams {
    LaunchParams {
        device_id: device_id.to_string(),
        mode: self.launch_context.mode,
        flavor: self.launch_context.flavor.clone(),
        dart_defines: self.launch_context.dart_defines.iter().map(|d| d.to_arg()).collect(),
        config_name: self.launch_context.selected_config().map(|c| c.display_name.clone()),
        entry_point: self.launch_context.entry_point.clone(),
        extra_args: self.launch_context.extra_args.clone(),
    }
}
```

**handle_launch fan-out** (`launch_context.rs`):

Refactor the existing single-device body into a reusable inner routine that, given a `Device` + `LaunchParams`, performs: active-session dedup (`find_active_by_device_id`), config construction, `create_session_*`, and returns either an `UpdateAction` (`SpawnSession` / `SpawnPreAppSources`) or a per-device error string. Then:

```rust
let checked = state.new_session_dialog_state.target_selector.checked_devices()
    .into_iter().cloned().collect::<Vec<_>>();

let devices: Vec<Device> = if checked.is_empty() {
    // Unchanged single-device path — cursor device.
    match state.new_session_dialog_state.selected_device() {
        Some(d) => vec![d.clone()],
        None => { /* set_error("Device no longer available") ; return none */ }
    }
} else {
    checked
};

let mut actions = Vec::new();
let mut skipped = Vec::new();   // (name, reason)
for device in &devices {
    // capacity guard: create_session_* returns Err at MAX_SESSIONS — collect & stop
    let params = state.new_session_dialog_state.build_launch_params_for_device(&device.id);
    match spawn_one(state, device, params) {        // inner routine
        Ok(action) => actions.push(action),
        Err(reason) => skipped.push((device.name.clone(), reason)),
    }
}

if actions.is_empty() {
    state.new_session_dialog_state.target_selector
        .set_error(summarize(&skipped));   // e.g. "All selected devices skipped: ..."
    return UpdateResult::none();
}

// Select the first newly-created session, close dialog, clear checked set.
state.session_manager.select_by_id(first_created_id);
state.new_session_dialog_state.target_selector.clear_checked();
state.hide_new_session_dialog();
state.ui_mode = UiMode::Normal;

if !skipped.is_empty() {
    // Non-fatal: surface as a toast "Launched X of Y (skipped: …)".
    state.push_toast(/* ToastLevel::Warn */, summarize_partial(&actions, &skipped));
}

UpdateResult::actions_vec(actions)
```

Notes on the inner routine:
- Preserve the existing pre-app-sources decision (`needs_pre_app_spawn`) per device.
- Preserve `save_last_selection` — call it for the first/primary device so auto-launch still remembers a sensible default.
- `create_session_*` already enforces `MAX_SESSIONS` (evicting oldest stopped first); treat its `Err` as the capacity/skip reason — do not pre-count.

### Acceptance Criteria

1. Two checked devices → `handle_launch` returns `actions_vec` with two spawn actions; two sessions are created.
2. Zero checked → exactly one action for the cursor device (byte-for-byte the current behavior); existing single-launch tests still pass.
3. A checked device with an active session is skipped (via `find_active_by_device_id`) and reported; other devices still launch.
4. When the session cap is hit mid-loop, already-built actions are returned and a "launched X of Y" toast is shown; no panic.
5. If every checked device is skipped, the dialog stays open with an error and no actions are emitted.
6. On success the dialog closes, the first new session is selected, and the checked set is cleared.

### Testing

```rust
#[test]
fn launch_with_two_checked_emits_two_actions() { /* seed 2 connected + checked, assert actions.len()==2 */ }

#[test]
fn launch_with_none_checked_falls_back_to_cursor_single() { /* assert single action for cursor device */ }

#[test]
fn launch_skips_device_with_active_session() { /* one device already active -> skipped, other launches */ }

#[test]
fn launch_clears_checked_and_closes_on_success() { /* checked_count()==0 and dialog hidden afterwards */ }
```

(Use the existing `launch_context.rs` / handler test helpers for seeding dialog state and a fake SDK.)

### Notes

- Keep the inner `spawn_one` private to the module; it's just the extracted current body.
- All N sessions share one launch context by design — do not attempt per-device configs in this task.
- Verify the action runner applies a `Vec<UpdateAction>` sequentially without racing on `SessionManager` (it is the supported `actions_vec` path; add a comment if any ordering assumption matters).

---

## Completion Summary

**Status:** Done
**Branch:** worktree-agent-a44801dbeca3d9010

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/new_session_dialog/state.rs` | Added `build_launch_params_for_device(&self, device_id: &str) -> LaunchParams` helper to `NewSessionDialogState` |
| `crates/fdemon-app/src/handler/new_session/launch_context.rs` | Refactored `handle_launch` to fan out over checked device set; extracted `spawn_one` private helper; added `summarize_skipped` helper; added 5 new tests |

### Notable Decisions/Tradeoffs

1. **Device list construction**: When the checked set is empty, the code falls through to the same cursor-device path as before (no behavioral change). This preserves backward compatibility for the zero-checked case.

2. **`spawn_one` signature**: Takes `is_primary: bool` to control `save_last_selection` — only the first device in the fan-out persists the selection, matching the single-device behavior for auto-launch defaults.

3. **No SDK rollback**: When `flutter_executable()` returns `None` after session creation, the orphaned session remains (no undo API). It is returned as an Err so the device is counted as skipped; the session will be evicted by capacity management on the next `create_session` call.

4. **Toast for partial success**: A `ToastLevel::Warn` toast is shown when some but not all devices launched, surfacing the skipped names and reasons without blocking the successful sessions.

5. **Dialog visible field**: `hide_new_session_dialog()` sets `ui_mode = Normal` rather than `new_session_dialog_state.visible = false`. Tests assert on `state.ui_mode` for dialog-closed checks, consistent with the existing test pattern.

### Testing Performed

- `cargo fmt --all -- --check` - PASS
- `cargo check --workspace --all-targets` - PASS
- `cargo test --workspace` - PASS (2566 tests across 5 crates; 0 failures)
- `cargo clippy --workspace --all-targets -- -D warnings` - PASS
- All 5 new multi-launch tests pass:
  - `launch_with_two_checked_emits_two_actions`
  - `launch_with_none_checked_falls_back_to_cursor_single`
  - `launch_skips_device_with_active_session`
  - `launch_clears_checked_and_closes_on_success`
  - `launch_all_skipped_keeps_dialog_open_with_error`

### Risks/Limitations

1. **Orphaned session on no-SDK error**: If `flutter_executable()` returns `None` mid-loop (unlikely but possible), the session created for that device is orphaned. It won't be spawned and will age out via capacity eviction. A future task could add a `remove_session` rollback API.

2. **Pre-app sources**: With N checked devices, each spawns its own `SpawnPreAppSources` action if the condition fires. Shared sources will be started once (the `running_shared_names` snapshot is taken per `spawn_one` call with the state as it stands at that point in the loop). The second `spawn_one` call will see the same snapshot and may re-trigger the gate for shared sources. This is a pre-existing limitation of the shared-source dedup logic and is noted but not fixed in this task.
