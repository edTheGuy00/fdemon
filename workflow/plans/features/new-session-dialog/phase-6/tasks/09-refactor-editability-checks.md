# Task: Refactor Editability Check Duplication

## Summary

Refactor handlers to use existing `LaunchContextState` editability methods instead of duplicating the logic inline.

## Files

| File | Action |
|------|--------|
| `src/app/handler/update.rs` | Modify (refactor handlers) |

## Background

The code review identified that handlers duplicate logic that already exists in `LaunchContextState::is_mode_editable()` and similar methods. This duplication increases maintenance burden and risk of bugs.

**Current (duplicated):**
```rust
// In handler - duplicates logic
if config.source == ConfigSource::VSCode {
    return None; // Can't edit VSCode configs
}
```

**Better (use state method):**
```rust
// Use existing method
if !state.new_session_dialog_state.is_mode_editable() {
    return None;
}
```

## Implementation

### 1. Identify duplicated editability checks

Locations in `update.rs` (around lines 2027-2113):
- Mode change handlers checking `ConfigSource::VSCode`
- Flavor change handlers checking config source
- Dart defines handlers checking config source

### 2. Refactor mode handlers

```rust
// Before
fn handle_new_session_dialog_mode_next(state: &mut AppState) -> Option<UpdateAction> {
    if let Some(ref mut dialog) = state.new_session_dialog_state {
        // Duplicated check
        if let Some(config) = dialog.launch_context_state.selected_config() {
            if config.source == ConfigSource::VSCode {
                return None;
            }
        }
        // ... mode change logic
    }
    None
}

// After
fn handle_new_session_dialog_mode_next(state: &mut AppState) -> Option<UpdateAction> {
    if let Some(ref mut dialog) = state.new_session_dialog_state {
        // Use existing method
        if !dialog.launch_context_state.is_mode_editable() {
            return None;
        }
        // ... mode change logic
    }
    None
}
```

### 3. Refactor flavor handlers

```rust
// Use is_flavor_editable() instead of inline check
if !dialog.launch_context_state.is_flavor_editable() {
    return None;
}
```

### 4. Refactor dart defines handlers

```rust
// Use are_dart_defines_editable() instead of inline check
if !dialog.launch_context_state.are_dart_defines_editable() {
    return None;
}
```

### 5. Ensure state methods exist

Verify these methods exist on `LaunchContextState`:
- `is_mode_editable() -> bool`
- `is_flavor_editable() -> bool`
- `are_dart_defines_editable() -> bool`

If they don't exist, they should have been added in task 01. The logic is:
```rust
impl LaunchContextState {
    pub fn is_mode_editable(&self) -> bool {
        self.selected_config()
            .map(|c| c.source != ConfigSource::VSCode)
            .unwrap_or(true) // Editable if no config selected
    }

    pub fn is_flavor_editable(&self) -> bool {
        self.is_mode_editable() // Same logic for now
    }

    pub fn are_dart_defines_editable(&self) -> bool {
        self.is_mode_editable() // Same logic for now
    }
}
```

## Acceptance Criteria

1. Mode handlers use `is_mode_editable()` method
2. Flavor handlers use `is_flavor_editable()` method
3. Dart defines handlers use `are_dart_defines_editable()` method
4. No inline `ConfigSource::VSCode` checks in handlers
5. All existing tests pass
6. `cargo clippy` passes with no warnings

## Verification

```bash
cargo fmt && cargo check && cargo test && cargo clippy -- -D warnings
```

## Notes

- This is a refactoring task - no behavior change expected
- Single source of truth for editability logic
- Easier to maintain and extend (e.g., if we add new read-only config sources)

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `src/tui/widgets/new_session_dialog/state.rs` | Added editability methods to `NewSessionDialogState`: `is_mode_editable()`, `is_flavor_editable()`, `are_dart_defines_editable()` |
| `src/app/handler/update.rs` | Refactored 5 handlers to use state methods instead of inline `ConfigSource::VSCode` checks |

### Notable Decisions/Tradeoffs

1. **Added methods to `NewSessionDialogState`**: The task description referenced `LaunchContextState`, but the actual dialog uses `NewSessionDialogState` which stores fields directly (not nested). Added the editability methods to `NewSessionDialogState` to match the actual architecture.

2. **Simplified handler logic**: Refactored handlers to use single-line editability checks instead of nested conditionals. This eliminated all inline `ConfigSource::VSCode` checks and reduced code duplication by ~60 lines.

3. **Consistent early returns**: All refactored handlers now use consistent early-return pattern: check editability first, return if not editable, then proceed with logic.

### Handlers Refactored

1. **NewSessionDialogModeNext** - Lines 2027-2058: Uses `is_mode_editable()`
2. **NewSessionDialogModePrev** - Lines 2060-2091: Uses `is_mode_editable()`
3. **NewSessionDialogFlavorSelected** - Lines 2107-2144: Uses `is_flavor_editable()`
4. **NewSessionDialogDartDefinesUpdated** - Lines 2146-2183: Uses `are_dart_defines_editable()`
5. **LaunchContextField::Flavor activation** - Lines 1948-1963: Uses `is_flavor_editable()`
6. **LaunchContextField::DartDefines activation** - Lines 1965-1975: Uses `are_dart_defines_editable()`

### Testing Performed

- `cargo fmt` - Passed
- `cargo check` - Passed (0.81s)
- `cargo test --lib` - Passed (1608 tests passed, 0 failed)
- `cargo clippy -- -D warnings` - Passed (0 warnings)

### Verification

All inline `ConfigSource::VSCode` checks removed from `update.rs`:
```bash
$ grep "ConfigSource::VSCode" src/app/handler/update.rs
# No matches found
```

### Risks/Limitations

1. **None identified**: This is a pure refactoring with no behavior change. All tests pass and logic is equivalent to the original implementation.
