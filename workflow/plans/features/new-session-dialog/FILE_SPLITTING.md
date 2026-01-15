# File Splitting Plan: NewSessionDialog

## Overview

This document tracks the planned splitting of oversized files in the NewSessionDialog feature. Both files significantly exceed the 500-line guideline established in `docs/CODE_STANDARDS.md`.

## Files to Split

### 1. src/app/handler/update.rs (2,776 lines)

**Status:** 555% over guideline (2,276 lines over)

**Current Structure:**

The file contains the main TEA update function with message handlers for all application features:

- Core update function and routing (~50 lines)
- Session management handlers (~100 lines)
- Scroll handlers (~100 lines)
- Hot reload/restart handlers (~150 lines)
- Device selector handlers (~200 lines)
- Settings page handlers (~350 lines)
- **StartupDialog handlers (~200 lines)**
- **NewSessionDialog handlers (~650 lines)**
  - Target selector (~200 lines)
  - Launch context fields (~150 lines)
  - Fuzzy modal (~150 lines)
  - Dart defines modal (~150 lines)
- Log view handlers (~150 lines)
- Link mode handlers (~100 lines)
- Search handlers (~100 lines)
- Auto-launch handlers (~150 lines)
- Device/emulator discovery handlers (~150 lines)

**Proposed Structure:**

```
src/app/handler/
├── mod.rs                     # Main update fn, routing, re-exports
│                              # ~250 lines (update fn + core handlers)
├── update.rs                  # DEPRECATED - split into modules below
├── keys.rs                    # Existing key handler
├── helpers.rs                 # Existing helper functions
├── daemon.rs                  # Existing daemon event handler
├── session.rs                 # Session lifecycle handlers
│                              # ~200 lines (spawn, attach, close, switch)
├── scroll.rs                  # Scroll message handlers
│                              # ~150 lines (up, down, page, horizontal)
├── log_view.rs                # Log view handlers
│                              # ~200 lines (filter, search, clear, links)
├── device_selector.rs         # Legacy device selector handlers
│                              # ~200 lines (show, hide, select, launch)
├── settings.rs                # Settings page handlers
│                              # ~400 lines (navigation, edit, save)
├── startup_dialog.rs          # StartupDialog handlers
│                              # ~250 lines (navigation, selection, launch)
├── new_session/               # NewSessionDialog module
│   ├── mod.rs                 # Re-exports
│   ├── navigation.rs          # Pane/tab/field navigation
│   │                          # ~100 lines
│   ├── target_selector.rs     # Device list navigation/selection
│   │                          # ~200 lines
│   ├── launch_context.rs      # Config/mode/flavor/launch handlers
│   │                          # ~150 lines
│   ├── fuzzy_modal.rs         # Fuzzy modal handlers
│   │                          # ~150 lines
│   └── dart_defines_modal.rs  # Dart defines modal handlers
│       └── mod.rs             # ~150 lines
└── tests.rs                   # Existing tests (may also split)
```

**Approach:**

**Phase 1: Extract NewSessionDialog Module (~650 lines)**
1. Create `src/app/handler/new_session/` directory structure
2. Move NewSessionDialog handlers (lines ~1725-2470) to submodules:
   - `navigation.rs`: Pane/tab/field switching
   - `target_selector.rs`: Device list and bootable device handling
   - `launch_context.rs`: Config/mode/flavor/launch logic
   - `fuzzy_modal.rs`: Fuzzy search modal
   - `dart_defines_modal.rs`: Dart defines editor modal
3. Create `new_session/mod.rs` with re-exports
4. Update `handler/mod.rs` to use new module
5. Run tests to verify no regressions

**Phase 2: Extract StartupDialog Module (~200 lines)**
1. Create `src/app/handler/startup_dialog.rs`
2. Move StartupDialog handlers (lines ~1293-1560)
3. Update imports and verify

**Phase 3: Extract Other Large Handler Groups**
1. Extract `session.rs` (session lifecycle)
2. Extract `scroll.rs` (scroll handlers)
3. Extract `log_view.rs` (log filtering/search)
4. Extract `device_selector.rs` (legacy selector)
5. Extract `settings.rs` (settings page)

**Phase 4: Refactor Main Update Function**
1. Move core update logic and routing to `handler/mod.rs`
2. Remove now-empty `update.rs`
3. Update all imports throughout codebase

### 2. src/tui/widgets/new_session_dialog/state.rs (2,101 lines)

**Status:** 420% over guideline (1,601 lines over)

**Current Structure:**

- Enums (~100 lines):
  - `DialogPane` (12 lines)
  - `TargetTab` (45 lines)
  - `LaunchContextField` (100 lines)
  - `FuzzyModalType` (20 lines)
  - `DartDefinesPane` (10 lines)
  - `DartDefinesEditField` (25 lines)
- State structs with impl blocks (~850 lines):
  - `DartDefine` (15 lines struct + impl)
  - `LaunchContextState` (20 lines struct + 170 lines impl)
  - `FuzzyModalState` (20 lines struct + 100 lines impl)
  - `DartDefinesModalState` (25 lines struct + 190 lines impl)
  - `NewSessionDialogState` (65 lines struct + 380 lines impl)
- Tests (~1,150 lines):
  - Tests at line 1176 (~340 lines)
  - Tests at line 1517 (~75 lines)
  - Tests at line 1591 (~165 lines)
  - Tests at line 1756 (~345 lines)

**Proposed Structure:**

```
src/tui/widgets/new_session_dialog/
├── mod.rs                     # Widget rendering (existing)
├── state/                     # NEW: State module directory
│   ├── mod.rs                 # Re-exports all types
│   ├── types.rs               # Enums: DialogPane, TargetTab, LaunchContextField
│   │                          # ~100 lines (all field/pane/tab enums)
│   ├── dialog.rs              # NewSessionDialogState
│   │                          # ~450 lines (struct + impl)
│   ├── launch_context.rs      # LaunchContextState
│   │                          # ~200 lines (struct + impl)
│   ├── fuzzy_modal.rs         # FuzzyModalState + FuzzyModalType
│   │                          # ~150 lines (struct + impl + enum)
│   ├── dart_defines.rs        # DartDefinesModalState + related enums
│   │                          # ~250 lines (struct + impl + enums)
│   └── tests/                 # Test module directory
│       ├── mod.rs             # Test utilities
│       ├── dialog_tests.rs    # NewSessionDialogState tests (~400 lines)
│       ├── launch_context_tests.rs # LaunchContextState tests (~200 lines)
│       ├── fuzzy_modal_tests.rs    # FuzzyModalState tests (~240 lines)
│       └── dart_defines_tests.rs   # DartDefinesModalState tests (~350 lines)
└── state.rs                   # DEPRECATED - remove after split
```

**Type Dependencies (for split ordering):**

```
LaunchContextField (types.rs)
    ↓
LaunchContextState (launch_context.rs)
    ↓
NewSessionDialogState (dialog.rs)
    ↑
FuzzyModalState (fuzzy_modal.rs)
    ↑
DartDefinesModalState (dart_defines.rs)
```

**Approach:**

**Phase 1: Create Directory Structure**
1. Create `src/tui/widgets/new_session_dialog/state/` directory
2. Create `state/mod.rs` with skeleton
3. Create `state/tests/` directory

**Phase 2: Move Type Definitions (Foundation Layer)**
1. Create `state/types.rs`
2. Move `DialogPane` enum (~12 lines)
3. Move `TargetTab` enum (~45 lines)
4. Move `LaunchContextField` enum (~100 lines)
5. Update `state/mod.rs` to re-export
6. Run `cargo check` to verify

**Phase 3: Move FuzzyModalState (No Dependencies)**
1. Create `state/fuzzy_modal.rs`
2. Move `FuzzyModalType` enum (~20 lines)
3. Move `FuzzyModalState` struct + impl (~130 lines)
4. Extract fuzzy modal tests to `state/tests/fuzzy_modal_tests.rs` (~240 lines)
5. Update imports in `state/mod.rs`
6. Run `cargo test --lib` to verify

**Phase 4: Move DartDefinesModalState (No Dependencies)**
1. Create `state/dart_defines.rs`
2. Move `DartDefine` struct + impl (~15 lines)
3. Move `DartDefinesPane` enum (~10 lines)
4. Move `DartDefinesEditField` enum (~25 lines)
5. Move `DartDefinesModalState` struct + impl (~215 lines)
6. Extract dart defines tests to `state/tests/dart_defines_tests.rs` (~350 lines)
7. Update imports in `state/mod.rs`
8. Run `cargo test --lib` to verify

**Phase 5: Move LaunchContextState (Depends on LaunchContextField)**
1. Create `state/launch_context.rs`
2. Move `LaunchContextState` struct + impl (~190 lines)
3. Extract launch context tests to `state/tests/launch_context_tests.rs` (~200 lines)
4. Update imports in `state/mod.rs`
5. Run `cargo test --lib` to verify

**Phase 6: Move NewSessionDialogState (Depends on All Above)**
1. Create `state/dialog.rs`
2. Move `NewSessionDialogState` struct + impl (~445 lines)
3. Extract dialog tests to `state/tests/dialog_tests.rs` (~400 lines)
4. Update imports in `state/mod.rs`
5. Run `cargo test --lib` to verify

**Phase 7: Update External References**
1. Update all imports throughout codebase:
   - `use crate::tui::widgets::new_session_dialog::state::*;`
   - becomes `use crate::tui::widgets::new_session_dialog::state::{...};`
2. Remove empty `state.rs` file
3. Run full test suite: `cargo test`
4. Run clippy: `cargo clippy -- -D warnings`

## Priority

**Medium** - Not blocking current Phase 6 work, but should be addressed before Phase 7.

The files are functional but difficult to maintain and review. Splitting improves:
- Reviewability (smaller diffs, focused changes)
- Maintainability (easier to find and modify specific handlers)
- Test organization (co-located with implementation)
- Compile times (smaller compilation units)

## Dependencies

- Complete Phase 6 review fixes first
- Coordinate with any parallel development on feature branches
- Should be done as a dedicated refactoring task/PR

## Risks and Mitigation

| Risk | Impact | Mitigation |
|------|--------|------------|
| **Import path changes break external code** | High | Use compiler to find all references; search codebase for old imports |
| **Test regressions from split** | Medium | Run full test suite after each phase; keep git history clean for bisecting |
| **Merge conflicts with parallel work** | Medium | Coordinate timing; do split between major features |
| **Lost context from file structure** | Low | Keep logical groupings clear; use descriptive module names |
| **Over-splitting creates navigation overhead** | Low | Keep related handlers together; use clear module hierarchy |

**Verification at Each Phase:**
```bash
# After each extraction:
cargo fmt
cargo check
cargo test --lib
cargo clippy -- -D warnings

# Before committing:
cargo test  # Full test suite
cargo build --release  # Verify release build
```

## Estimated Effort

| Phase | Lines Moved | Estimated Time | Risk Level |
|-------|-------------|----------------|------------|
| update.rs: NewSessionDialog module | ~650 | 2-3 hours | Medium |
| update.rs: StartupDialog module | ~200 | 1 hour | Low |
| update.rs: Other handlers | ~1,500 | 3-4 hours | Medium |
| state.rs: Full split | ~2,100 | 4-5 hours | Medium-High |
| **Total** | **~4,450** | **10-13 hours** | **Medium** |

**Note:** Times include testing, fixing imports, and handling unexpected issues.

## Tracking Checklist

### update.rs Refactoring

- [ ] Phase 1: Extract `new_session/` module
  - [ ] Create directory structure
  - [ ] Extract `navigation.rs`
  - [ ] Extract `target_selector.rs`
  - [ ] Extract `launch_context.rs`
  - [ ] Extract `fuzzy_modal.rs`
  - [ ] Extract `dart_defines_modal.rs`
  - [ ] Create `new_session/mod.rs`
  - [ ] Update `handler/mod.rs`
  - [ ] Run tests and verify
- [ ] Phase 2: Extract `startup_dialog.rs`
  - [ ] Create module file
  - [ ] Move handlers
  - [ ] Update imports
  - [ ] Verify tests
- [ ] Phase 3: Extract other handler groups
  - [ ] Extract `session.rs`
  - [ ] Extract `scroll.rs`
  - [ ] Extract `log_view.rs`
  - [ ] Extract `device_selector.rs`
  - [ ] Extract `settings.rs`
- [ ] Phase 4: Finalize refactoring
  - [ ] Move remaining code to `handler/mod.rs`
  - [ ] Remove empty `update.rs`
  - [ ] Update all external imports
  - [ ] Full test suite passes
  - [ ] Clippy clean

### state.rs Refactoring

- [ ] Phase 1: Create structure
  - [ ] Create `state/` directory
  - [ ] Create `state/tests/` directory
  - [ ] Create skeleton `state/mod.rs`
- [ ] Phase 2: Move types (foundation)
  - [ ] Create `state/types.rs`
  - [ ] Move `DialogPane`
  - [ ] Move `TargetTab`
  - [ ] Move `LaunchContextField`
  - [ ] Update exports
  - [ ] Verify with `cargo check`
- [ ] Phase 3: Move FuzzyModalState
  - [ ] Create `state/fuzzy_modal.rs`
  - [ ] Move `FuzzyModalType` enum
  - [ ] Move `FuzzyModalState` struct + impl
  - [ ] Create `state/tests/fuzzy_modal_tests.rs`
  - [ ] Extract tests
  - [ ] Verify with `cargo test --lib`
- [ ] Phase 4: Move DartDefinesModalState
  - [ ] Create `state/dart_defines.rs`
  - [ ] Move `DartDefine`
  - [ ] Move `DartDefinesPane`
  - [ ] Move `DartDefinesEditField`
  - [ ] Move `DartDefinesModalState` struct + impl
  - [ ] Create `state/tests/dart_defines_tests.rs`
  - [ ] Extract tests
  - [ ] Verify with `cargo test --lib`
- [ ] Phase 5: Move LaunchContextState
  - [ ] Create `state/launch_context.rs`
  - [ ] Move `LaunchContextState` struct + impl
  - [ ] Create `state/tests/launch_context_tests.rs`
  - [ ] Extract tests
  - [ ] Verify with `cargo test --lib`
- [ ] Phase 6: Move NewSessionDialogState
  - [ ] Create `state/dialog.rs`
  - [ ] Move `NewSessionDialogState` struct + impl
  - [ ] Create `state/tests/dialog_tests.rs`
  - [ ] Extract tests
  - [ ] Verify with `cargo test --lib`
- [ ] Phase 7: Finalize
  - [ ] Update all external imports
  - [ ] Remove empty `state.rs`
  - [ ] Full test suite passes (`cargo test`)
  - [ ] Clippy clean (`cargo clippy -- -D warnings`)

## References

- **Code Standards:** `docs/CODE_STANDARDS.md` (500-line guideline)
- **Architecture:** `docs/ARCHITECTURE.md` (module organization)
- **Development:** `docs/DEVELOPMENT.md` (verification commands)
- **Related Feature:** `workflow/plans/features/new-session-dialog/PLAN.md`

## Notes

- This is a **tracking document only** - no code changes yet
- The split should be done on a **dedicated branch** with focused commits
- Each phase should be a **separate commit** for easy review and potential rollback
- File splitting can be done **incrementally** (one module at a time)
- Consider doing the split **between Phase 6 and Phase 7** of the feature work
- Both `update.rs` and `state.rs` can be split **in parallel** or sequentially
- The `state.rs` split is more straightforward (clear type dependencies)
- The `update.rs` split requires more care (message routing logic)
