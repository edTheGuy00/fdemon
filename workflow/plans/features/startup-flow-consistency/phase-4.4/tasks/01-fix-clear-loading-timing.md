## Task: Fix clear_loading() Timing in AutoLaunchResult Handler

**Objective**: Move `clear_loading()` inside each match branch to prevent intermediate UI state.

**Depends on**: None

**Estimated Time**: 15 minutes

### Background

The current code calls `clear_loading()` BEFORE examining the result:

```rust
Message::AutoLaunchResult { result } => {
    state.clear_loading();  // <- Sets ui_mode = Normal

    match result {
        Ok(success) => { ... }
        Err(error_msg) => {
            state.show_startup_dialog(configs);  // <- Sets ui_mode = StartupDialog
        }
    }
}
```

This creates a state transition sequence on the error path:
1. `Loading` (from StartAutoLaunch)
2. `Normal` (from clear_loading)
3. `StartupDialog` (from show_startup_dialog)

The intermediate `Normal` state is never rendered because the message loop drains all messages before rendering, but this is an accidental fix that depends on timing.

### Scope

- `src/app/handler/update.rs`: Move `clear_loading()` inside match branches

### Changes Required

**Before:**
```rust
Message::AutoLaunchResult { result } => {
    // Clear loading overlay
    state.clear_loading();

    match result {
        Ok(success) => {
            // Create session and spawn
            let AutoLaunchSuccess { device, config } = success;
            // ...
        }
        Err(error_msg) => {
            // Device discovery failed, show startup dialog with error
            let configs = crate::config::load_all_configs(&state.project_path);
            state.show_startup_dialog(configs);
            state.startup_dialog_state.set_error(error_msg);
            UpdateResult::none()
        }
    }
}
```

**After:**
```rust
Message::AutoLaunchResult { result } => {
    match result {
        Ok(success) => {
            // Clear loading before transitioning to session
            state.clear_loading();

            // Create session and spawn
            let AutoLaunchSuccess { device, config } = success;
            // ...
        }
        Err(error_msg) => {
            // Clear loading before showing error dialog
            state.clear_loading();

            // Device discovery failed, show startup dialog with error
            let configs = crate::config::load_all_configs(&state.project_path);
            state.show_startup_dialog(configs);
            state.startup_dialog_state.set_error(error_msg);
            UpdateResult::none()
        }
    }
}
```

Note: Also need to add `clear_loading()` inside the inner `Err(e)` branch (session creation failure) since it currently relies on the outer `clear_loading()`:

```rust
Err(e) => {
    state.clear_loading();  // Add this
    // Session creation failed (e.g., max sessions reached) - show startup dialog with error
    let configs = crate::config::load_all_configs(&state.project_path);
    state.show_startup_dialog(configs);
    // ...
}
```

### Acceptance Criteria

1. `clear_loading()` called only after result is examined
2. Each branch (Ok outer, Err outer, Err inner) calls `clear_loading()` appropriately
3. `cargo check` passes
4. `cargo test --lib` passes
5. `cargo clippy -- -D warnings` passes

### Testing

Existing tests should continue to pass. No new tests needed - this is a refactoring for correctness.

### Notes

- This is a code quality fix, not a bug fix
- The app works correctly without this change due to message loop timing
- The fix makes the intent explicit and prevents future timing-related bugs

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `src/app/handler/update.rs` | Moved `clear_loading()` from before the match statement to inside each branch of the AutoLaunchResult handler (lines 1661-1718) |

### Notable Decisions/Tradeoffs

1. **Placement of clear_loading()**: Added `clear_loading()` at the beginning of each branch to ensure the loading state is cleared only after examining the result. This prevents the intermediate `Normal` UI state transition that was previously masked by message loop timing.
2. **Three clear_loading() calls**: Added calls in all three branches:
   - Ok(success) branch (line 1665) - clears before transitioning to session
   - Inner Err(e) branch for session creation failure (line 1695) - clears before showing error dialog
   - Outer Err(error_msg) branch for device discovery failure (line 1709) - clears before showing error dialog

### Testing Performed

- `cargo check` - Passed
- `cargo test --lib` - Passed (1349 tests)
- `cargo clippy -- -D warnings` - Passed

### Risks/Limitations

1. **None**: This is a pure refactoring that makes state transitions more explicit without changing behavior. The message loop still drains messages before rendering, so the functional behavior remains identical.
