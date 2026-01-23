## Task: Add EntryPoint variant to FuzzyModalType enum

**Objective**: Add `EntryPoint` variant to the `FuzzyModalType` enum so the fuzzy modal can be used for entry point selection.

**Depends on**: None

### Scope

- `src/app/new_session_dialog/types.rs`: Add `EntryPoint` variant to `FuzzyModalType` enum

### Details

Add the `EntryPoint` variant to `FuzzyModalType` and implement the `title()` and `allows_custom()` methods.

#### Current implementation:

```rust
pub enum FuzzyModalType {
    /// Configuration selection (from LoadedConfigs)
    Config,
    /// Flavor selection (from project + custom)
    Flavor,
}
```

#### Updated implementation:

```rust
/// Type of fuzzy modal
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FuzzyModalType {
    /// Configuration selection (from LoadedConfigs)
    Config,
    /// Flavor selection (from project + custom)
    Flavor,
    /// Entry point selection (discovered Dart files with main())
    EntryPoint,
}

impl FuzzyModalType {
    /// Get the modal title
    pub fn title(&self) -> &'static str {
        match self {
            Self::Config => "Select Configuration",
            Self::Flavor => "Select Flavor",
            Self::EntryPoint => "Select Entry Point",
        }
    }

    /// Whether custom input is allowed
    pub fn allows_custom(&self) -> bool {
        match self {
            Self::Config => false,     // Must select from list
            Self::Flavor => true,      // Can type custom flavor
            Self::EntryPoint => true,  // Can type custom path
        }
    }
}
```

### Acceptance Criteria

1. `FuzzyModalType` enum has `EntryPoint` variant
2. `title()` returns `"Select Entry Point"` for `EntryPoint`
3. `allows_custom()` returns `true` for `EntryPoint` (users can type custom paths)
4. Code compiles without errors

### Testing

Add these tests to the `mod tests` block in `src/app/new_session_dialog/types.rs`:

```rust
#[test]
fn test_fuzzy_modal_type_entry_point_title() {
    assert_eq!(FuzzyModalType::EntryPoint.title(), "Select Entry Point");
}

#[test]
fn test_fuzzy_modal_type_entry_point_allows_custom() {
    // EntryPoint should allow custom input for typing arbitrary paths
    assert!(FuzzyModalType::EntryPoint.allows_custom());

    // Verify other types for consistency
    assert!(!FuzzyModalType::Config.allows_custom());
    assert!(FuzzyModalType::Flavor.allows_custom());
}
```

### Notes

- `allows_custom() = true` enables users to type custom paths not in the discovered list
- This is important for entry points in non-standard locations or not yet created
- Can be done in parallel with Task 01 (different enums in same file)

### Rationale for `allows_custom() = true`

Entry point selection should allow custom input because:

1. **Non-standard locations**: Entry points might be in subdirectories like `lib/flavors/main_dev.dart`
2. **New files**: User might want to specify an entry point they're about to create
3. **Testing**: User might want to run a specific test entry point not discovered automatically
4. **Fallback**: Discovery might miss files with unusual main() signatures

The fuzzy modal will show discovered entry points first, but the user can type any path.
