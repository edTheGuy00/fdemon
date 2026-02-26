## Task: Update launch guard to use phase-aware device check

**Objective**: Change the duplicate-device check in `handle_launch` to use `find_active_by_device_id` instead of `find_by_device_id`, so stopped sessions no longer block new session creation on the same device.

**Depends on**: 02-find-active-device-id

### Scope

- `crates/fdemon-app/src/handler/new_session/launch_context.rs`: Update the device guard at lines 421–435

### Details

Change the guard from:

```rust
// Check if device already has a running session
if state
    .session_manager
    .find_by_device_id(&device.id)
    .is_some()
{
    state
        .new_session_dialog_state
        .target_selector
        .set_error(format!(
            "Device '{}' already has an active session",
            device.name
        ));
    return UpdateResult::none();
}
```

To:

```rust
// Check if device already has an active session (skip stopped sessions)
if state
    .session_manager
    .find_active_by_device_id(&device.id)
    .is_some()
{
    state
        .new_session_dialog_state
        .target_selector
        .set_error(format!(
            "Device '{}' already has an active session",
            device.name
        ));
    return UpdateResult::none();
}
```

The only change is `find_by_device_id` → `find_active_by_device_id`. The error message text, early return, and all other behavior remain identical.

### Acceptance Criteria

1. `handle_launch` uses `find_active_by_device_id` instead of `find_by_device_id`
2. Stopped sessions no longer block new session creation on the same device
3. Running/Initializing/Reloading sessions still correctly block duplicate launches
4. The comment above the check says "active session" (already does)
5. All existing `handle_launch` tests still pass

### Testing

Tested via task 04 (integration-level tests for the full launch flow with pre-existing sessions).

### Notes

- This is a one-line change (method name swap). The risk is extremely low.
- Two sessions with the same `device_id` can now coexist in `SessionManager` (one stopped, one active). This is safe because daemon events route by `session_id`, not `device_id`.
- `find_by_app_id` is not affected — stopped sessions have `app_id = None`.

---

## Completion Summary

**Status:** Not Started
