# Phase 6.1: File Splitting Refactoring - Task Index

## Overview

Refactor oversized files identified in the code review before proceeding to Phase 7. Both `state.rs` (2,101 lines) and `update.rs` (2,776 lines) significantly exceed the 500-line guideline.

**Total Tasks:** 6
**Estimated Time:** 7-9 hours

## Rationale

This intermediate phase is necessary because:
- Phase 7 modifies `state.rs` and `update.rs` extensively
- Phase 8 removes code from these files
- Splitting first prevents merge conflicts and makes future changes more manageable
- Reviewability improves with smaller, focused modules

## Files to Split

| File | Lines | Over Guideline | Priority |
|------|-------|----------------|----------|
| `src/tui/widgets/new_session_dialog/state.rs` | 2,101 | 420% | High (simpler) |
| `src/app/handler/update.rs` | 2,776 | 555% | High (complex) |

## Task Dependency Graph

```
┌─────────────────────────────────────────────────────────────────┐
│  01-state-types-and-modals          02-state-main-types         │
│  (types, fuzzy, dart_defines)       (launch_context, dialog)    │
└─────────────────┬───────────────────────────┬───────────────────┘
                  │                           │
                  └─────────────┬─────────────┘
                                ▼
┌─────────────────────────────────────────────────────────────────┐
│                   03-handler-new-session                         │
│                   (new_session/ module)                          │
└─────────────────────────────────┬───────────────────────────────┘
                                  │
                                  ▼
┌─────────────────────────────────────────────────────────────────┐
│                   04-handler-remaining                           │
│                   (startup_dialog, session, etc.)                │
└─────────────────────────────────┬───────────────────────────────┘
                                  │
                                  ▼
┌─────────────────────────────────────────────────────────────────┐
│                   05-cleanup-verification                        │
│                   (imports, remove old files, tests)             │
└─────────────────────────────────┬───────────────────────────────┘
                                  │
                                  ▼
┌─────────────────────────────────────────────────────────────────┐
│                   06-update-downstream-tasks                     │
│                   (update Phase 7 & 8 task file paths)           │
└─────────────────────────────────────────────────────────────────┘
```

## Tasks

| # | Task | Status | Depends On | Est. | Modules |
|---|------|--------|------------|------|---------|
| 1 | [01-state-types-and-modals](tasks/01-state-types-and-modals.md) | Done | Phase 6 | 90m | `new_session_dialog/state/` |
| 2 | [02-state-main-types](tasks/02-state-main-types.md) | Done | 1 | 90m | `new_session_dialog/state/` |
| 3 | [03-handler-new-session](tasks/03-handler-new-session.md) | Done | 1, 2 | 90m | `app/handler/new_session/` |
| 4 | [04-handler-remaining](tasks/04-handler-remaining.md) | Done | 3 | 90m | `app/handler/` |
| 5 | [05-cleanup-verification](tasks/05-cleanup-verification.md) | Done | 4 | 60m | Multiple |
| 6 | [06-update-downstream-tasks](tasks/06-update-downstream-tasks.md) | Done | 5 | 30m | `workflow/plans/` (docs only) |

## Target Module Structures

### state.rs → state/ module

```
src/tui/widgets/new_session_dialog/
├── mod.rs                     # Widget rendering (existing)
├── state/                     # NEW: State module directory
│   ├── mod.rs                 # Re-exports all types
│   ├── types.rs               # DialogPane, TargetTab, LaunchContextField (~100 lines)
│   ├── fuzzy_modal.rs         # FuzzyModalState + FuzzyModalType (~150 lines)
│   ├── dart_defines.rs        # DartDefinesModalState + enums (~250 lines)
│   ├── launch_context.rs      # LaunchContextState (~200 lines)
│   ├── dialog.rs              # NewSessionDialogState (~450 lines)
│   └── tests/                 # Test module directory
│       ├── mod.rs             # Test utilities
│       ├── dialog_tests.rs    # NewSessionDialogState tests
│       ├── launch_context_tests.rs
│       ├── fuzzy_modal_tests.rs
│       └── dart_defines_tests.rs
└── state.rs                   # REMOVED after split
```

### update.rs → handler modules

```
src/app/handler/
├── mod.rs                     # Main update fn, routing, re-exports (~250 lines)
├── keys.rs                    # Existing key handler
├── helpers.rs                 # Existing helper functions
├── daemon.rs                  # Existing daemon event handler
├── session.rs                 # Session lifecycle handlers (~200 lines)
├── scroll.rs                  # Scroll handlers (~150 lines)
├── log_view.rs                # Log view handlers (~200 lines)
├── device_selector.rs         # Legacy device selector (~200 lines)
├── settings.rs                # Settings page handlers (~400 lines)
├── startup_dialog.rs          # StartupDialog handlers (~250 lines)
├── new_session/               # NewSessionDialog module
│   ├── mod.rs                 # Re-exports
│   ├── navigation.rs          # Pane/tab/field navigation (~100 lines)
│   ├── target_selector.rs     # Device list handlers (~200 lines)
│   ├── launch_context.rs      # Config/mode/flavor handlers (~150 lines)
│   ├── fuzzy_modal.rs         # Fuzzy modal handlers (~150 lines)
│   └── dart_defines_modal.rs  # Dart defines modal handlers (~150 lines)
├── tests.rs                   # Existing tests
└── update.rs                  # REMOVED after split
```

## Success Criteria

Phase 6.1 is complete when:

- [x] `state.rs` split into `state/` module with 5+ submodules
- [x] All state types re-exported correctly from `state/mod.rs`
- [x] `update.rs` split into handler modules
- [x] NewSessionDialog handlers in dedicated `handler/new_session/` module
- [x] All imports updated throughout codebase
- [x] No files exceed 500 lines (guideline)
- [x] All existing tests pass without modification
- [x] `cargo fmt && cargo check && cargo test && cargo clippy -- -D warnings` passes
- [x] Phase 7 and Phase 8 task files updated with new file paths

## Verification Commands

Run after each task:
```bash
cargo fmt
cargo check
cargo test --lib
cargo clippy -- -D warnings
```

Run before marking phase complete:
```bash
cargo test  # Full test suite
cargo build --release  # Verify release build
```

## Risks and Mitigation

| Risk | Impact | Mitigation |
|------|--------|------------|
| Import path changes break external code | High | Use compiler to find all references |
| Test regressions from split | Medium | Run tests after each extraction |
| Lost code during move | Low | Use git diff to verify no code lost |
| Over-splitting creates navigation overhead | Low | Keep related code together |

## Notes

- Each task should be a **separate commit** for easy review/rollback
- Keep tests co-located with implementation where possible
- Preserve existing public API (same exports from `state` and `handler`)
- The `state.rs` split is simpler (clear type dependencies)
- The `update.rs` split requires more care (message routing logic)

## References

- [FILE_SPLITTING.md](../FILE_SPLITTING.md) - Detailed splitting plan
- [CODE_STANDARDS.md](/docs/CODE_STANDARDS.md) - 500-line guideline
- [ARCHITECTURE.md](/docs/ARCHITECTURE.md) - Module organization
