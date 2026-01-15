## Task: Cleanup and Final Verification

**Objective**: Update all imports throughout the codebase, remove deprecated files, and verify the refactoring is complete.

**Depends on**: 04-handler-remaining

**Estimated Time**: 60 minutes

### Scope

- Update imports in all files referencing moved types/functions
- Remove empty/deprecated files
- Verify all tests pass
- Verify clippy is clean
- Document the new module structure

### Details

#### Step 1: Search for Old Import Paths

Find all files importing from old locations:

```bash
# Find state.rs imports
rg "use.*new_session_dialog::state::" --type rust

# Find update.rs direct references
rg "handler::update::" --type rust

# Find any remaining direct type imports
rg "use.*state::(DialogPane|TargetTab|LaunchContextField)" --type rust
```

#### Step 2: Update Import Paths

For state types, imports should now be:
```rust
// Before:
use crate::tui::widgets::new_session_dialog::state::NewSessionDialogState;

// After (same - re-exported):
use crate::tui::widgets::new_session_dialog::state::NewSessionDialogState;
```

The re-exports in `state/mod.rs` should make this transparent, but verify.

For handler functions, if any external code directly referenced:
```rust
// Before:
use crate::app::handler::update::some_handler;

// After:
use crate::app::handler::new_session::some_handler;
```

#### Step 3: Remove Deprecated Files

If original files are now empty or only contain re-exports:

```bash
# Only if completely empty after moves:
rm src/tui/widgets/new_session_dialog/state.rs  # Done in Task 02
# update.rs should remain with routing logic
```

#### Step 4: Verify Line Counts

Check that all files are under the 500-line guideline:

```bash
wc -l src/tui/widgets/new_session_dialog/state/*.rs
wc -l src/app/handler/*.rs
wc -l src/app/handler/new_session/*.rs
```

Expected results:
- All state submodules: < 500 lines each
- All handler modules: < 500 lines each
- `update.rs`: < 500 lines (routing only)

#### Step 5: Run Full Verification

```bash
# Format
cargo fmt

# Check compilation
cargo check

# Run all tests
cargo test

# Run clippy
cargo clippy -- -D warnings

# Build release
cargo build --release
```

#### Step 6: Update FILE_SPLITTING.md

Mark completed items in the tracking checklist:

```markdown
### state.rs Refactoring
- [x] Phase 1: Create structure
- [x] Phase 2: Move types (foundation)
- [x] Phase 3: Move FuzzyModalState
- [x] Phase 4: Move DartDefinesModalState
- [x] Phase 5: Move LaunchContextState
- [x] Phase 6: Move NewSessionDialogState
- [x] Phase 7: Finalize

### update.rs Refactoring
- [x] Phase 1: Extract `new_session/` module
- [x] Phase 2: Extract `startup_dialog.rs`
- [x] Phase 3: Extract other handler groups
- [x] Phase 4: Finalize refactoring
```

### Acceptance Criteria

1. No compilation errors
2. No clippy warnings
3. All tests pass
4. No file exceeds 500 lines (check with `wc -l`)
5. All old import paths work via re-exports
6. FILE_SPLITTING.md checklist updated
7. `cargo build --release` succeeds

### Final File Structure Verification

```
src/tui/widgets/new_session_dialog/
├── mod.rs
├── state/
│   ├── mod.rs
│   ├── types.rs          (< 150 lines)
│   ├── fuzzy_modal.rs    (< 200 lines)
│   ├── dart_defines.rs   (< 300 lines)
│   ├── launch_context.rs (< 250 lines)
│   ├── dialog.rs         (< 500 lines)
│   └── tests/
│       ├── mod.rs
│       ├── fuzzy_modal_tests.rs
│       ├── dart_defines_tests.rs
│       ├── launch_context_tests.rs
│       └── dialog_tests.rs

src/app/handler/
├── mod.rs
├── update.rs             (< 500 lines - routing only)
├── keys.rs
├── helpers.rs
├── daemon.rs
├── session.rs            (< 250 lines)
├── scroll.rs             (< 200 lines)
├── log_view.rs           (< 250 lines)
├── device_selector.rs    (< 250 lines)
├── settings.rs           (< 450 lines)
├── startup_dialog.rs     (< 300 lines)
├── new_session/
│   ├── mod.rs
│   ├── navigation.rs     (< 150 lines)
│   ├── target_selector.rs(< 250 lines)
│   ├── launch_context.rs (< 200 lines)
│   ├── fuzzy_modal.rs    (< 200 lines)
│   └── dart_defines_modal.rs (< 200 lines)
└── tests.rs
```

### Notes

- This is a verification task, not a code-writing task
- If issues are found, create follow-up tasks
- Document any deviations from the original plan
- The split should be transparent to external code
