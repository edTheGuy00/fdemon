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

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `src/app/new_session_dialog/types.rs` | Added `EntryPoint` variant to `FuzzyModalType` enum, updated `title()` and `allows_custom()` methods, added unit tests |

### Notable Decisions/Tradeoffs

1. **allows_custom() returns true**: Following the task specification, `EntryPoint` allows custom input so users can type arbitrary paths not in the discovered list. This is consistent with the `Flavor` variant behavior.
2. **Test placement**: Added tests to the existing `#[cfg(test)] mod tests` block that was created by task 01, avoiding duplication of the tests module.

### Testing Performed

- `cargo fmt` - Passed
- `cargo check` - Expected compilation errors in other files (see explanation below)
- Unit tests added:
  - `test_fuzzy_modal_type_entry_point_title()` - Tests the title method
  - `test_fuzzy_modal_type_entry_point_allows_custom()` - Tests allows_custom for all variants

### Risks/Limitations

1. **Expected Compilation Errors**: The project currently has compilation errors in `src/app/handler/new_session/fuzzy_modal.rs` and `src/app/new_session_dialog/state.rs` where `match` statements on `FuzzyModalType` need to handle the new `EntryPoint` variant. Per task 01's notes and task 07's scope, these errors are expected and will be resolved by task 07 which adds the `handle_entry_point_selected()` handler and updates the match statements.

2. **Parallel Implementation**: This task was implemented in parallel with task 01, which modified the same file (`types.rs`) but a different enum (`LaunchContextField`). Both changes are present in the final file.

### Implementation Details

The `EntryPoint` variant was successfully added to `FuzzyModalType` with:
- Doc comment: "Entry point selection (discovered Dart files with main())"
- `title()` returns: "Select Entry Point"
- `allows_custom()` returns: `true` (allows typing custom paths)

All acceptance criteria met except full project compilation, which is blocked by expected missing match arms that will be added in task 07.
