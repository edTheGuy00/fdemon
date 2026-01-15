# Task: Remove Unsafe Unwrap

## Summary

Replace `unwrap()` call in launch handler with proper error handling to prevent potential panics.

**Priority:** CRITICAL (Blocking merge)

## Files

| File | Action |
|------|--------|
| `src/app/handler/new_session/launch_context.rs` | Modify (lines 239-248) |

## Problem

Current code at `src/app/handler/new_session/launch_context.rs:239-248`:

```rust
let device = state
    .new_session_dialog_state
    .selected_device()
    .unwrap()  // Can panic!
    .clone();
```

Per `docs/CODE_STANDARDS.md`: "Never use unwrap in library code."

## Implementation

Replace with proper error handling:

```rust
pub fn handle_launch(state: &mut AppState) -> UpdateResult {
    // Build launch params (already validates device exists)
    let params = match state.new_session_dialog_state.build_launch_params() {
        Some(p) => p,
        None => {
            // Should never happen if build_launch_params returned Some,
            // but handle gracefully
            state.new_session_dialog_state.target_selector
                .set_error("No device selected".to_string());
            return UpdateResult::none();
        }
    };

    // Get device reference without unwrap
    let device = match state.new_session_dialog_state.selected_device() {
        Some(d) => d.clone(),
        None => {
            state.new_session_dialog_state.target_selector
                .set_error("Device no longer available".to_string());
            return UpdateResult::none();
        }
    };

    UpdateResult::action(UpdateAction::LaunchFlutterSession {
        device,
        mode: params.mode,
        flavor: params.flavor,
        dart_defines: params.dart_defines,
        config_name: params.config_name,
    })
}
```

## Acceptance Criteria

1. No `unwrap()` calls in handler code
2. Errors shown to user via `set_error()` method
3. No panics possible from missing device
4. Function returns gracefully on error

## Verification

```bash
# Check for unwrap calls in handlers
grep -n "unwrap()" src/app/handler/new_session/*.rs

# Should return empty or only acceptable uses (e.g., in tests)
```

## Testing

```bash
cargo fmt && cargo check && cargo clippy -- -D warnings
```

## Notes

- This is a quick fix (15 minutes)
- The `set_error()` method should already exist on `TargetSelectorState`
- If `set_error()` doesn't exist, add it or use an alternative like `tracing::warn!()`
