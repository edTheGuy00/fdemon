## Task: Fix SettingsToggleEdit to Dispatch Toggle for Booleans

**Objective**: Make the `SettingsToggleEdit` handler dispatch `SettingsToggleBool` directly for boolean items instead of the current no-op behavior.

**Depends on**: 01-implement-toggle-handler

### Scope

- `src/app/handler/update.rs:1024-1028` - Replace no-op with dispatch

### Details

Currently, when a user presses Enter on a boolean setting, the `SettingsToggleEdit` handler recognizes it as a boolean but does nothing:

```rust
// CURRENT (no-op) - lines 1024-1028
match &item.value {
    SettingValue::Bool(_) | SettingValue::Enum { .. } => {
        // These don't use traditional edit mode
        // Bool toggles directly, Enum cycles
    }
    // ... other types enter edit mode
}
```

The comment says "Bool toggles directly" but no toggle actually happens. This is confusing UX.

**Fix:** Make the handler dispatch the appropriate message for booleans and enums:

```rust
// FIXED
match &item.value {
    SettingValue::Bool(_) => {
        // Bool toggles directly without edit mode
        return update(state, Message::SettingsToggleBool);
    }
    SettingValue::Enum { .. } => {
        // Enums cycle through options
        return update(state, Message::SettingsCycleEnumNext);
    }
    SettingValue::Number(n) => {
        state.settings_view_state.start_editing(&n.to_string());
    }
    SettingValue::Float(f) => {
        state.settings_view_state.start_editing(&f.to_string());
    }
    SettingValue::String(s) => {
        state.settings_view_state.start_editing(s);
    }
    SettingValue::List(_) => {
        // List starts with empty buffer to add new item
        state.settings_view_state.start_editing("");
    }
}
```

### Acceptance Criteria

1. Pressing Enter on a boolean setting toggles the value immediately
2. Pressing Enter on an enum setting cycles to next option
3. Other setting types (Number, Float, String, List) still enter edit mode as before
4. All quality gates pass (`cargo fmt`, `cargo check`, `cargo test`, `cargo clippy`)

### Testing

```bash
# Run handler tests
cargo test handler --lib

# Run E2E settings tests
cargo test settings_page --test e2e
```

Verify manually:
1. Run `cargo run` in a Flutter project
2. Press `,` to open settings
3. Navigate to "Auto Reload" (boolean) and press Enter → should toggle
4. Navigate to "Theme" (enum) and press Enter → should cycle
5. Navigate to "Log Buffer Size" (number) and press Enter → should enter edit mode

### Notes

- This change improves UX by making Enter key work intuitively for all setting types
- The recursive `update(state, Message::...)` call is the established pattern in this codebase
- Enum cycling uses `SettingsCycleEnumNext` which may also be a stub - verify it works
- If `SettingsCycleEnumNext` is also broken, that's a separate bug (out of scope)

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `src/app/handler/update.rs` | Modified `SettingsToggleEdit` handler (lines 1024-1032) to dispatch `SettingsToggleBool` for boolean settings and `SettingsCycleEnumNext` for enum settings instead of no-op |

### Notable Decisions/Tradeoffs

1. **Recursive update() call pattern**: Used the established pattern `return update(state, Message::SettingsToggleBool)` to dispatch to the toggle handler, consistent with codebase conventions
2. **Enum cycling is a stub**: The `SettingsCycleEnumNext` message is dispatched correctly, but its handler only marks dirty without actually cycling enum values. This is out of scope for this task (noted in task description as potentially broken)
3. **Split Bool and Enum cases**: Separated the combined `Bool(_) | Enum { .. }` pattern into individual arms to allow different message dispatch for each type

### Testing Performed

- `cargo fmt` - Passed
- `cargo check` - Passed (compiled in 1.31s)
- `cargo test --lib handler` - Passed (231 tests passed, 1 ignored)
- `cargo clippy -- -D warnings` - Passed (no warnings)
- Full test suite: Unit tests passed (1329 tests)

Note: E2E tests have some snapshot failures, but these appear to be pre-existing issues unrelated to this change (snapshots were already modified before implementation based on initial git status).

### Risks/Limitations

1. **Enum cycling incomplete**: While this task correctly dispatches `SettingsCycleEnumNext`, the handler for that message is still a stub. Users pressing Enter on enum settings will mark the config as dirty but won't actually cycle values. This is a known limitation noted in the task description as out of scope.
2. **No integration test**: While unit tests verify the toggle behavior works, there are no integration tests specifically for the Enter key dispatch path. Manual testing is recommended to verify end-to-end behavior.
