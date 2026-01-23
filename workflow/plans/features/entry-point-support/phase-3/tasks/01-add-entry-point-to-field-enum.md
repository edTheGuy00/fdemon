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
