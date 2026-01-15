## Task: Split state.rs - Types and Modal States

**Objective**: Create the `state/` module directory and extract foundational types, FuzzyModalState, and DartDefinesModalState.

**Depends on**: Phase 6 complete

**Estimated Time**: 90 minutes

### Scope

- `src/tui/widgets/new_session_dialog/state.rs` → split into:
  - `src/tui/widgets/new_session_dialog/state/mod.rs`
  - `src/tui/widgets/new_session_dialog/state/types.rs`
  - `src/tui/widgets/new_session_dialog/state/fuzzy_modal.rs`
  - `src/tui/widgets/new_session_dialog/state/dart_defines.rs`
  - `src/tui/widgets/new_session_dialog/state/tests/mod.rs`
  - `src/tui/widgets/new_session_dialog/state/tests/fuzzy_modal_tests.rs`
  - `src/tui/widgets/new_session_dialog/state/tests/dart_defines_tests.rs`

### Details

#### Step 1: Create Directory Structure

```bash
mkdir -p src/tui/widgets/new_session_dialog/state/tests
```

#### Step 2: Create state/types.rs

Move these enums (no dependencies on other types):
- `DialogPane` enum
- `TargetTab` enum
- `LaunchContextField` enum

```rust
// state/types.rs
use crate::core::RunMode;

/// Represents which pane of the NewSessionDialog has focus
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DialogPane {
    #[default]
    TargetSelector,
    LaunchContext,
}

/// Tabs in the Target Selector pane
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TargetTab {
    #[default]
    Connected,
    Bootable,
}

/// Fields in the Launch Context pane for navigation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LaunchContextField {
    #[default]
    Configuration,
    Mode,
    Flavor,
    DartDefines,
    Launch,
}
```

#### Step 3: Create state/fuzzy_modal.rs

Move these types (no dependencies):
- `FuzzyModalType` enum
- `FuzzyModalState` struct + impl

Extract related tests to `state/tests/fuzzy_modal_tests.rs`.

#### Step 4: Create state/dart_defines.rs

Move these types (no dependencies):
- `DartDefine` struct + impl
- `DartDefinesPane` enum
- `DartDefinesEditField` enum
- `DartDefinesModalState` struct + impl

Extract related tests to `state/tests/dart_defines_tests.rs`.

#### Step 5: Create state/mod.rs

```rust
// state/mod.rs
mod types;
mod fuzzy_modal;
mod dart_defines;

// Re-export all types for backward compatibility
pub use types::*;
pub use fuzzy_modal::*;
pub use dart_defines::*;

// Remaining types still in this file (to be moved in task 02)
// - LaunchContextState
// - NewSessionDialogState

#[cfg(test)]
mod tests;
```

### Type Dependencies (verify no circular imports)

```
types.rs (no deps)
    ↓
fuzzy_modal.rs (no deps)
    ↓
dart_defines.rs (no deps)
```

### Acceptance Criteria

1. `state/` directory created with `mod.rs`, `types.rs`, `fuzzy_modal.rs`, `dart_defines.rs`
2. `state/tests/` directory created with test modules
3. All moved types accessible via `state::*` imports
4. No changes to public API (same exports)
5. `cargo check` passes
6. `cargo test --lib new_session_dialog` passes (all existing tests)

### Testing

After each extraction:
```bash
cargo fmt
cargo check
cargo test --lib new_session_dialog
cargo clippy -- -D warnings
```

### Notes

- Keep `LaunchContextState` and `NewSessionDialogState` in original location for now (Task 02)
- Preserve exact behavior - only move code, no refactoring
- Tests may need import updates but logic should not change
- Use `pub(crate)` for internal types if needed

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `/Users/ed/Dev/zabin/flutter-demon/src/tui/widgets/new_session_dialog/state/mod.rs` | Created module file with re-exports, kept LaunchContextState and NewSessionDialogState in place for task 02 |
| `/Users/ed/Dev/zabin/flutter-demon/src/tui/widgets/new_session_dialog/state/types.rs` | Extracted DialogPane, TargetTab, LaunchContextField enums with all impl methods |
| `/Users/ed/Dev/zabin/flutter-demon/src/tui/widgets/new_session_dialog/state/fuzzy_modal.rs` | Extracted FuzzyModalType enum and FuzzyModalState struct with all impl methods |
| `/Users/ed/Dev/zabin/flutter-demon/src/tui/widgets/new_session_dialog/state/dart_defines.rs` | Extracted DartDefine struct, DartDefinesPane, DartDefinesEditField enums, and DartDefinesModalState struct with all impl methods |
| `/Users/ed/Dev/zabin/flutter-demon/src/tui/widgets/new_session_dialog/state/tests/mod.rs` | Created test module with main dialog tests and LaunchContextState tests |
| `/Users/ed/Dev/zabin/flutter-demon/src/tui/widgets/new_session_dialog/state/tests/fuzzy_modal_tests.rs` | Extracted fuzzy modal tests (6 tests) |
| `/Users/ed/Dev/zabin/flutter-demon/src/tui/widgets/new_session_dialog/state/tests/dart_defines_tests.rs` | Extracted dart defines tests (13 tests) |
| `/Users/ed/Dev/zabin/flutter-demon/src/tui/widgets/new_session_dialog/state.rs` | Removed (replaced with state/ directory) |

### Notable Decisions/Tradeoffs

1. **Test organization**: Moved tests to `state/tests/mod.rs` instead of `state/tests.rs` to match the directory-based test structure specified in the task, avoiding file/directory naming conflicts
2. **Module ordering**: cargo fmt automatically alphabetized module declarations (dart_defines, fuzzy_modal, types) which is fine for dependency-free modules
3. **Exact preservation**: All functionality preserved byte-for-byte - no logic changes, only structural reorganization
4. **DialogPane variant names**: Kept original Left/Right naming in existing code (task spec showed TargetSelector/LaunchContext but that would be a semantic change)

### Testing Performed

- `cargo fmt` - Passed (formatted all new files)
- `cargo check` - Passed (no compilation errors)
- `cargo test --lib new_session_dialog` - Passed (189 tests, 0 failures)
- `cargo clippy -- -D warnings` - Passed (no warnings)

### Risks/Limitations

1. **None identified**: All acceptance criteria met, all tests passing, no clippy warnings, API unchanged
