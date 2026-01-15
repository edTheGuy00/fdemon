# Task: Plan and Track File Splitting

## Summary

Create tracking documentation for splitting the oversized `update.rs` (2,835 lines) and `state.rs` (2,058 lines) files. Both exceed the 500-line guideline by 400-500%.

## Files

| File | Action |
|------|--------|
| `workflow/plans/features/new-session-dialog/FILE_SPLITTING.md` | Create (tracking doc) |
| `src/app/handler/update.rs` | Modify (add TODO comment) |
| `src/tui/widgets/new_session_dialog/state.rs` | Modify (add TODO comment) |

## Background

The code review identified critical file size violations:
- `update.rs`: 2,835 lines (567% over 500-line guideline)
- `state.rs`: 2,058 lines (412% over 500-line guideline)

This task creates tracking for a future refactoring effort rather than doing the split now.

## Implementation

### 1. Create tracking document

Create `workflow/plans/features/new-session-dialog/FILE_SPLITTING.md`:

```markdown
# File Splitting Plan: NewSessionDialog

## Overview

Track the planned splitting of oversized files in the NewSessionDialog feature.

## Files to Split

### 1. src/app/handler/update.rs (2,835 lines)

**Current Structure:**
- Core update function and routing
- Session handlers
- NewSessionDialog handlers (~400 lines)
- Fuzzy modal handlers (~150 lines)
- Dart defines modal handlers (~150 lines)
- Startup dialog handlers (~200 lines)
- Various other handlers

**Proposed Structure:**
```
src/app/handler/
├── mod.rs           (main update fn, routing) ~200 lines
├── keys.rs          (existing)
├── helpers.rs       (existing)
├── session.rs       (session handlers) ~300 lines
├── new_session_dialog.rs (~400 lines)
│   - Field navigation
│   - Mode cycling
│   - Config/flavor selection
│   - Launch action
├── fuzzy_modal.rs   (~150 lines)
├── dart_defines_modal.rs (~150 lines)
├── startup_dialog.rs (~200 lines)
└── tests.rs         (existing, may also split)
```

**Approach:**
1. Extract `new_session_dialog.rs` first (most isolated)
2. Extract `fuzzy_modal.rs` and `dart_defines_modal.rs`
3. Extract `startup_dialog.rs`
4. Review remaining code in `update.rs`

### 2. src/tui/widgets/new_session_dialog/state.rs (2,058 lines)

**Current Structure:**
- NewSessionDialogState (~200 lines)
- LaunchContextState (~150 lines)
- LaunchContextField enum (~100 lines)
- FuzzyModalState (~150 lines)
- DartDefinesModalState (~200 lines)
- Various enums and types (~50 lines)
- Tests (~1,200 lines)

**Proposed Structure:**
```
src/tui/widgets/new_session_dialog/
├── mod.rs
├── state/
│   ├── mod.rs           (re-exports)
│   ├── dialog.rs        (NewSessionDialogState) ~200 lines
│   ├── launch_context.rs (LaunchContextState) ~250 lines
│   ├── fuzzy_modal.rs   (FuzzyModalState) ~150 lines
│   ├── dart_defines.rs  (DartDefinesModalState) ~200 lines
│   ├── types.rs         (enums: Field, Tab, Pane) ~100 lines
│   └── tests/
│       ├── mod.rs
│       ├── dialog_tests.rs
│       ├── launch_context_tests.rs
│       ├── fuzzy_modal_tests.rs
│       └── dart_defines_tests.rs
└── ...
```

**Approach:**
1. Create `state/` directory structure
2. Move types to `types.rs` first
3. Extract each state struct to its own file
4. Split tests into corresponding test files
5. Update imports throughout codebase

## Priority

**Medium** - Not blocking current work but should be addressed before Phase 7.

## Dependencies

- Complete Phase 6 review fixes first
- Coordinate with any parallel development

## Risks

- Import path changes may break external references
- Test refactoring may introduce regressions
- Should be done on a separate branch with careful review

## Tracking

- [ ] Extract new_session_dialog handlers
- [ ] Extract fuzzy_modal handlers
- [ ] Extract dart_defines_modal handlers
- [ ] Create state/ directory structure
- [ ] Move state types
- [ ] Extract state structs
- [ ] Split tests
- [ ] Update all imports
- [ ] Verify all tests pass
```

### 2. Add TODO comment to update.rs

Add at the top of the file (after imports):

```rust
// TODO: This file exceeds 500 lines (currently ~2,835). Planned split:
// - new_session_dialog.rs: NewSessionDialog handlers
// - fuzzy_modal.rs: Fuzzy modal handlers
// - dart_defines_modal.rs: Dart defines modal handlers
// - startup_dialog.rs: Startup dialog handlers
// See: workflow/plans/features/new-session-dialog/FILE_SPLITTING.md
```

### 3. Add TODO comment to state.rs

Add at the top of the file (after imports):

```rust
// TODO: This file exceeds 500 lines (currently ~2,058). Planned split:
// - state/dialog.rs: NewSessionDialogState
// - state/launch_context.rs: LaunchContextState
// - state/fuzzy_modal.rs: FuzzyModalState
// - state/dart_defines.rs: DartDefinesModalState
// - state/types.rs: Shared enums
// See: workflow/plans/features/new-session-dialog/FILE_SPLITTING.md
```

## Acceptance Criteria

1. `FILE_SPLITTING.md` created with detailed split plan
2. TODO comment added to `update.rs` referencing the plan
3. TODO comment added to `state.rs` referencing the plan
4. Plan includes module structure diagrams
5. Plan includes approach and risks

## Verification

```bash
# Verify files created/modified
ls -la workflow/plans/features/new-session-dialog/FILE_SPLITTING.md
grep -n "TODO.*FILE_SPLITTING" src/app/handler/update.rs
grep -n "TODO.*FILE_SPLITTING" src/tui/widgets/new_session_dialog/state.rs
```

## Notes

- This task creates tracking only - actual splitting is a separate future task
- The split should be done on a dedicated branch
- Consider splitting after Phase 6 review fixes are complete
- File splitting can be done incrementally (one module at a time)

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `workflow/plans/features/new-session-dialog/FILE_SPLITTING.md` | Created comprehensive 14KB tracking document with detailed split plan, module diagrams, phased approach, and checklist |
| `src/app/handler/update.rs` | Added TODO comment (lines 3-11) referencing FILE_SPLITTING.md |
| `src/tui/widgets/new_session_dialog/state.rs` | Added TODO comment (lines 3-10) referencing FILE_SPLITTING.md |

### Notable Decisions/Tradeoffs

1. **Comprehensive Planning**: Created a very detailed FILE_SPLITTING.md (14KB, ~450 lines) that includes:
   - Current structure analysis for both files
   - Proposed module structures with ASCII diagrams
   - Phased approach with dependency ordering
   - Risk analysis and mitigation strategies
   - Effort estimates (10-13 hours total)
   - Detailed tracking checklists (40+ items)
   - Verification commands for each phase

2. **Module Structure for update.rs**: Proposed a `new_session/` subdirectory approach rather than flat files, which better organizes the 5 NewSessionDialog-related handler files (navigation, target_selector, launch_context, fuzzy_modal, dart_defines_modal)

3. **state.rs Split Ordering**: Identified type dependencies and proposed bottom-up extraction (types → fuzzy_modal → dart_defines → launch_context → dialog) to minimize import churn

4. **Test Organization**: Proposed moving tests to dedicated `state/tests/` directory with separate files per module, improving test organization and reducing file sizes

### Testing Performed

- `cargo check` - Passed (0.92s)
- `cargo test --lib` - Passed (1608 tests, 0 failed)
- File verification:
  - `ls -la FILE_SPLITTING.md` - Confirmed created (14,739 bytes)
  - `grep "FILE_SPLITTING" update.rs` - Confirmed TODO at line 11
  - `grep "FILE_SPLITTING" state.rs` - Confirmed TODO at line 10

### Risks/Limitations

1. **Documentation Only**: This task creates no functional changes - actual refactoring is tracked but not implemented
2. **Line Count Estimates**: Actual line counts (2,776 and 2,101) slightly differ from task spec (2,835 and 2,058) but proportions remain the same
3. **Effort Estimates**: 10-13 hour estimate may vary based on unexpected import issues or test complications
4. **Coordination Needed**: Both splits should coordinate with any parallel feature development to avoid merge conflicts
