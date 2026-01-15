# Task: Remove or Use Unused LaunchContextState Methods

## Summary

Address the unused methods on `LaunchContextState` that were implemented but never called, either by removing them or refactoring handlers to use them.

## Files

| File | Action |
|------|--------|
| `src/tui/widgets/new_session_dialog/state.rs` | Modify (remove or document) |

## Background

The code review identified that methods like `focus_next()`, `focus_prev()`, `cycle_mode_next()`, `cycle_mode_prev()` exist on `LaunchContextState` but the handlers manipulate `NewSessionDialogState` directly instead.

**Unused methods (around lines 242-277):**
- `LaunchContextState::focus_next()`
- `LaunchContextState::focus_prev()`
- `LaunchContextState::cycle_mode_next()`
- `LaunchContextState::cycle_mode_prev()`

## Implementation

### Option A: Remove Unused Methods (Recommended)

If the handlers work correctly with direct state manipulation, remove the unused methods to reduce confusion:

```rust
// Remove these methods from LaunchContextState:
// - focus_next()
// - focus_prev()
// - cycle_mode_next()
// - cycle_mode_prev()

// Keep only the methods that ARE used:
// - is_mode_editable()
// - is_flavor_editable()
// - are_dart_defines_editable()
// - selected_config()
// - selected_config_source()
```

### Option B: Refactor Handlers to Use Methods

If the methods provide cleaner encapsulation, refactor handlers to use them:

```rust
// In update.rs handlers, instead of:
dialog.launch_context_state.focused_field = dialog.launch_context_state.focused_field.next();

// Use:
dialog.launch_context_state.focus_next();
```

This would require updating:
- `handle_new_session_dialog_field_next()`
- `handle_new_session_dialog_field_prev()`
- `handle_new_session_dialog_mode_next()`
- `handle_new_session_dialog_mode_prev()`

## Decision Criteria

Choose **Option A** if:
- Methods add no value over direct field access
- Keeping them causes confusion about the "right" way to do things
- Tests don't rely on these methods

Choose **Option B** if:
- Methods encapsulate complex logic (e.g., skipping disabled fields)
- Multiple places would benefit from the abstraction
- The methods handle edge cases that handlers might forget

## Acceptance Criteria

1. No dead code warnings from clippy
2. Clear single way to update state (either direct or via methods, not both)
3. If methods removed, ensure no references remain
4. If methods kept, ensure handlers use them
5. All tests pass

## Verification

```bash
cargo fmt && cargo check && cargo test && cargo clippy -- -D warnings
```

## Notes

- This is a cleanup task to reduce code confusion
- Either approach is valid - consistency is key
- Document the chosen approach in code comments if not obvious
