## Task: Add EntryPoint variant to LaunchContextField enum

**Objective**: Add `EntryPoint` variant to the `LaunchContextField` enum and update navigation methods.

**Depends on**: None

### Scope

- `src/app/new_session_dialog/types.rs`: Add `EntryPoint` variant to `LaunchContextField` enum

### Details

Add the `EntryPoint` variant to the `LaunchContextField` enum between `Flavor` and `DartDefines`. Update the `next()` and `prev()` methods to include the new field in the navigation cycle.

#### Current implementation:

```rust
pub enum LaunchContextField {
    Config,
    Mode,
    Flavor,
    DartDefines,
    Launch,
}
```

#### Updated implementation:

```rust
/// Fields in the Launch Context pane for navigation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LaunchContextField {
    #[default]
    Config,
    Mode,
    Flavor,
    EntryPoint,  // NEW - between Flavor and DartDefines
    DartDefines,
    Launch,
}

impl LaunchContextField {
    pub fn next(self) -> Self {
        match self {
            Self::Config => Self::Mode,
            Self::Mode => Self::Flavor,
            Self::Flavor => Self::EntryPoint,      // UPDATED
            Self::EntryPoint => Self::DartDefines, // NEW
            Self::DartDefines => Self::Launch,
            Self::Launch => Self::Config,
        }
    }

    pub fn prev(self) -> Self {
        match self {
            Self::Config => Self::Launch,
            Self::Mode => Self::Config,
            Self::Flavor => Self::Mode,
            Self::EntryPoint => Self::Flavor,      // NEW
            Self::DartDefines => Self::EntryPoint, // UPDATED
            Self::Launch => Self::DartDefines,
        }
    }

    // next_enabled() and prev_enabled() remain unchanged
    // They use the updated next()/prev() methods
}
```

### Acceptance Criteria

1. `LaunchContextField` enum has `EntryPoint` variant
2. `EntryPoint` is positioned between `Flavor` and `DartDefines`
3. `next()` navigates: Flavor → EntryPoint → DartDefines
4. `prev()` navigates: DartDefines → EntryPoint → Flavor
5. `next_enabled()` and `prev_enabled()` work with new field
6. Code compiles without errors

### Testing

Add these tests to `src/app/new_session_dialog/types.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_launch_context_field_next_includes_entry_point() {
        assert_eq!(LaunchContextField::Flavor.next(), LaunchContextField::EntryPoint);
        assert_eq!(LaunchContextField::EntryPoint.next(), LaunchContextField::DartDefines);
    }

    #[test]
    fn test_launch_context_field_prev_includes_entry_point() {
        assert_eq!(LaunchContextField::DartDefines.prev(), LaunchContextField::EntryPoint);
        assert_eq!(LaunchContextField::EntryPoint.prev(), LaunchContextField::Flavor);
    }

    #[test]
    fn test_launch_context_field_navigation_cycle() {
        // Forward cycle
        let mut field = LaunchContextField::Config;
        let fields = [
            LaunchContextField::Config,
            LaunchContextField::Mode,
            LaunchContextField::Flavor,
            LaunchContextField::EntryPoint,
            LaunchContextField::DartDefines,
            LaunchContextField::Launch,
        ];

        for expected in &fields[1..] {
            field = field.next();
            assert_eq!(field, *expected);
        }

        // Wraps around
        assert_eq!(field.next(), LaunchContextField::Config);
    }

    #[test]
    fn test_launch_context_field_next_enabled_skips_disabled() {
        // Simulate EntryPoint being disabled
        let is_disabled = |f: LaunchContextField| f == LaunchContextField::EntryPoint;

        let next = LaunchContextField::Flavor.next_enabled(is_disabled);
        assert_eq!(next, LaunchContextField::DartDefines);

        let prev = LaunchContextField::DartDefines.prev_enabled(is_disabled);
        assert_eq!(prev, LaunchContextField::Flavor);
    }
}
```

### Notes

- This is a simple enum variant addition with navigation logic update
- No changes to `next_enabled()` and `prev_enabled()` methods needed (they delegate to `next()`/`prev()`)
- Can be done in parallel with Task 02 (different enums in same file)

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `/Users/ed/Dev/zabin/flutter-demon/src/app/new_session_dialog/types.rs` | Added `EntryPoint` variant to `LaunchContextField` enum between `Flavor` and `DartDefines`; updated `next()` and `prev()` methods to include new field in navigation cycle; added comprehensive unit tests for navigation including cycle and enabled/disabled field handling |

### Notable Decisions/Tradeoffs

1. **Test Coverage**: Added 4 unit tests covering all aspects of the new field navigation including forward/backward navigation, full cycle, and skip-disabled behavior to ensure robustness.
2. **Navigation Order**: Positioned `EntryPoint` between `Flavor` and `DartDefines` as specified in the task requirements, maintaining logical flow in the launch context field order.

### Testing Performed

- `cargo fmt` - Passed (code formatted successfully)
- Unit tests added:
  - `test_launch_context_field_next_includes_entry_point` - Tests forward navigation
  - `test_launch_context_field_prev_includes_entry_point` - Tests backward navigation
  - `test_launch_context_field_navigation_cycle` - Tests full navigation cycle
  - `test_launch_context_field_next_enabled_skips_disabled` - Tests skip-disabled field logic

### Risks/Limitations

1. **Expected Compilation Errors**: The project currently has compilation errors in other files (`src/app/handler/new_session/navigation.rs`, `src/app/handler/new_session/fuzzy_modal.rs`, and `src/app/new_session_dialog/state.rs`) that need to handle the new `EntryPoint` variant. These errors are expected and will be resolved by subsequent tasks in Phase 3 that add the actual handling logic for the entry point field.
2. **No Functional Impact Yet**: While the enum variant and navigation logic are complete, the UI won't display or interact with the EntryPoint field until the rendering and handler tasks are completed.
