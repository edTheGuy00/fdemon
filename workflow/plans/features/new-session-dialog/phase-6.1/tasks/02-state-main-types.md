## Task: Split state.rs - Main State Types

**Objective**: Extract `LaunchContextState` and `NewSessionDialogState` to complete the state module split.

**Depends on**: 01-state-types-and-modals

**Estimated Time**: 90 minutes

### Scope

- Complete the `state/` module by extracting:
  - `src/tui/widgets/new_session_dialog/state/launch_context.rs`
  - `src/tui/widgets/new_session_dialog/state/dialog.rs`
  - `src/tui/widgets/new_session_dialog/state/tests/launch_context_tests.rs`
  - `src/tui/widgets/new_session_dialog/state/tests/dialog_tests.rs`
- Remove `src/tui/widgets/new_session_dialog/state.rs` (now empty)

### Details

#### Step 1: Create state/launch_context.rs

Move `LaunchContextState` struct + impl block.

Dependencies:
- `types::LaunchContextField`
- `crate::core::RunMode`
- `crate::config::LaunchConfig`

```rust
// state/launch_context.rs
use super::types::LaunchContextField;
use crate::config::LaunchConfig;
use crate::core::RunMode;

pub struct LaunchContextState {
    // ... fields
}

impl LaunchContextState {
    // ... methods
}
```

#### Step 2: Create state/dialog.rs

Move `NewSessionDialogState` struct + impl block.

Dependencies:
- All types from `types.rs`
- `LaunchContextState` from `launch_context.rs`
- `FuzzyModalState` from `fuzzy_modal.rs`
- `DartDefinesModalState` from `dart_defines.rs`
- External: `BootableDevice`, `FlutterDevice`, `LaunchConfig`, etc.

```rust
// state/dialog.rs
use super::{
    types::{DialogPane, TargetTab, LaunchContextField},
    launch_context::LaunchContextState,
    fuzzy_modal::{FuzzyModalState, FuzzyModalType},
    dart_defines::DartDefinesModalState,
};
use crate::daemon::BootableDevice;
// ... other imports

pub struct NewSessionDialogState {
    // ... fields
}

impl NewSessionDialogState {
    // ... methods
}
```

#### Step 3: Extract Tests

Move tests to appropriate test files:
- `LaunchContextState` tests → `tests/launch_context_tests.rs`
- `NewSessionDialogState` tests → `tests/dialog_tests.rs`

Update `tests/mod.rs`:
```rust
// state/tests/mod.rs
mod fuzzy_modal_tests;
mod dart_defines_tests;
mod launch_context_tests;
mod dialog_tests;
```

#### Step 4: Update state/mod.rs

```rust
// state/mod.rs
mod types;
mod fuzzy_modal;
mod dart_defines;
mod launch_context;
mod dialog;

pub use types::*;
pub use fuzzy_modal::*;
pub use dart_defines::*;
pub use launch_context::*;
pub use dialog::*;

#[cfg(test)]
mod tests;
```

#### Step 5: Remove Old state.rs

Once all types are moved and tests pass:
```bash
rm src/tui/widgets/new_session_dialog/state.rs
```

#### Step 6: Update Parent mod.rs

Update `src/tui/widgets/new_session_dialog/mod.rs`:
```rust
// Change from:
mod state;
pub use state::*;

// To:
pub mod state;
pub use state::*;
```

### Type Dependencies (final structure)

```
types.rs (no deps)
    ↓
fuzzy_modal.rs (no deps)
    ↓
dart_defines.rs (no deps)
    ↓
launch_context.rs (depends on: types)
    ↓
dialog.rs (depends on: types, launch_context, fuzzy_modal, dart_defines)
```

### Acceptance Criteria

1. `state/launch_context.rs` contains `LaunchContextState` and impl
2. `state/dialog.rs` contains `NewSessionDialogState` and impl
3. All tests moved to `state/tests/` directory
4. Original `state.rs` file removed
5. All imports throughout codebase updated
6. Public API unchanged (same exports from `state`)
7. `cargo test` passes (all tests, not just lib)

### Testing

```bash
cargo fmt
cargo check
cargo test --lib new_session_dialog
cargo test  # Full suite
cargo clippy -- -D warnings
```

### Notes

- This completes the `state.rs` split
- ~2,100 lines → 5 files of ~100-450 lines each
- Test files may be the largest, which is acceptable
- Verify no external code changes (only import paths)

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `src/tui/widgets/new_session_dialog/state/launch_context.rs` | Created (161 lines) - Extracted LaunchContextState struct and impl |
| `src/tui/widgets/new_session_dialog/state/dialog.rs` | Created (467 lines) - Extracted NewSessionDialogState struct and impl |
| `src/tui/widgets/new_session_dialog/state/tests/launch_context_tests.rs` | Created (250 lines) - Extracted LaunchContextState tests |
| `src/tui/widgets/new_session_dialog/state/tests/dialog_tests.rs` | Created (330 lines) - Extracted NewSessionDialogState tests |
| `src/tui/widgets/new_session_dialog/state/tests/mod.rs` | Modified - Updated to import new test modules (6 lines total) |
| `src/tui/widgets/new_session_dialog/state/mod.rs` | Modified - Updated to import new modules (18 lines total, reduced from 638 lines) |

### Notable Decisions/Tradeoffs

1. **File Organization**: Split the monolithic state/mod.rs into focused files:
   - `launch_context.rs` - 161 lines (LaunchContextState)
   - `dialog.rs` - 467 lines (NewSessionDialogState)
   - Both are well within the 500-line guideline

2. **Test Organization**: Separated tests into dedicated files:
   - `launch_context_tests.rs` - 250 lines (23 tests)
   - `dialog_tests.rs` - 330 lines (24 tests)
   - Clean separation allows easy test discovery

3. **Import Structure**: Maintained backward compatibility through re-exports in mod.rs, ensuring no external code changes required

4. **Type Dependencies**: Followed the planned dependency chain:
   - types.rs (no deps) → fuzzy_modal.rs → dart_defines.rs → launch_context.rs → dialog.rs

### Testing Performed

- `cargo fmt` - Passed (code formatted)
- `cargo check` - Passed (compilation successful)
- `cargo test --lib new_session_dialog` - Passed (189 tests)
- `cargo test --lib` - Passed (1603 tests, 0 failed)
- `cargo clippy -- -D warnings` - Passed (no warnings)

### Risks/Limitations

1. **None Identified**: All tests pass, public API unchanged, clean compilation with no warnings
