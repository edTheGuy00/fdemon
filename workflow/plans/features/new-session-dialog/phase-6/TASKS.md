# Phase 6: Launch Context Widget - Task Index

## Overview

Create the Launch Context widget - the right pane of the NewSessionDialog. Contains configuration selection, mode selector, flavor, dart-defines, and launch button.

**Total Tasks:** 14 (5 original + 9 review fixes)
**Status:** ✅ All tasks complete

## UI Design

```
┌── ⚙️ Launch Context ─────────────────┐
│                                       │
│  Configuration:                       │
│  [ Development (Default)          ▼]  │  ← Opens fuzzy modal
│                                       │
│  Mode:                                │
│  (●) Debug  (○) Profile  (○) Release  │
│                                       │
│  Flavor:                              │
│  [ dev____________________        ▼]  │  ← Opens fuzzy modal (if editable)
│                                       │
│  Dart Defines:                        │
│  [ 3 items                        ▶]  │  ← Opens dart defines modal
│                                       │
│  [          🚀 LAUNCH (Enter)       ] │
│                                       │
└───────────────────────────────────────┘
```

## Config Editability Rules

| Config Source | Mode | Flavor | Dart Defines | Behavior |
|---------------|------|--------|--------------|----------|
| VSCode | Read-only | Read-only | Read-only | All fields disabled, show "(from config)" |
| FDemon | Editable | Editable | Editable | Changes auto-save to `.fdemon/launch.toml` |
| None selected | Editable | Editable | Editable | Transient values, not persisted |

## Task Dependency Graph

```
┌─────────────────────────────────────┐
│  01-launch-context-state            │
└────────────────┬────────────────────┘
                 │
                 ▼
┌─────────────────────────────────────┐
│  02-field-widgets                   │
└────────────────┬────────────────────┘
                 │
                 ▼
┌─────────────────────────────────────┐
│  03-config-auto-save                │
└────────────────┬────────────────────┘
                 │
                 ▼
┌─────────────────────────────────────┐
│  04-launch-context-widget           │
└────────────────┬────────────────────┘
                 │
                 ▼
┌─────────────────────────────────────┐
│  05-launch-context-messages         │
└─────────────────────────────────────┘
```

## Tasks

### Original Implementation Tasks

| # | Task | Status | Depends On | Est. | Modules |
|---|------|--------|------------|------|---------|
| 1 | [01-launch-context-state](tasks/01-launch-context-state.md) | ✅ Done | Phase 1 | 25m | `new_session_dialog/state.rs` |
| 2 | [02-field-widgets](tasks/02-field-widgets.md) | ✅ Done | 1 | 30m | `new_session_dialog/launch_context.rs` |
| 3 | [03-config-auto-save](tasks/03-config-auto-save.md) | ✅ Done | 2 | 20m | `config/writer.rs` |
| 4 | [04-launch-context-widget](tasks/04-launch-context-widget.md) | ✅ Done | 3 | 25m | `new_session_dialog/launch_context.rs` |
| 5 | [05-launch-context-messages](tasks/05-launch-context-messages.md) | ✅ Done | 4 | 15m | `app/message.rs`, `app/handler/update.rs` |

### Review Fix Tasks (Post-Review)

| # | Task | Status | Severity | Depends On | Modules |
|---|------|--------|----------|------------|---------|
| 6 | [06-fix-navigation-loop-bug](tasks/06-fix-navigation-loop-bug.md) | ✅ Done | 🔴 Critical | 5 | `new_session_dialog/state.rs` |
| 7 | [07-handler-unit-tests](tasks/07-handler-unit-tests.md) | ✅ Done | 🔴 Critical | 5 | `app/handler/tests.rs` |
| 8 | [08-implement-auto-save-action](tasks/08-implement-auto-save-action.md) | ✅ Done | 🔴 Critical | 5 | `tui/actions.rs` |
| 9 | [09-refactor-editability-checks](tasks/09-refactor-editability-checks.md) | ✅ Done | 🟠 Major | 6 | `app/handler/update.rs` |
| 10 | [10-improve-launch-validation](tasks/10-improve-launch-validation.md) | ✅ Done | 🟠 Major | 5 | `app/handler/update.rs` |
| 11 | [11-track-file-splitting](tasks/11-track-file-splitting.md) | ✅ Done | 🟠 Major | 5 | `workflow/`, `update.rs`, `state.rs` |
| 12 | [12-cleanup-unused-methods](tasks/12-cleanup-unused-methods.md) | ✅ Done | 🟡 Minor | 9 | `new_session_dialog/state.rs` |
| 13 | [13-config-writer-improvements](tasks/13-config-writer-improvements.md) | ✅ Done | 🟡 Minor | 8 | `config/writer.rs`, `Cargo.toml` |
| 14 | [14-consolidate-widget-rendering](tasks/14-consolidate-widget-rendering.md) | ✅ Done | 🟡 Minor | 5 | `new_session_dialog/launch_context.rs` |

## Review Fix Dependency Graph

```
Critical (Must Fix - Blocking)
┌───────────────────────────────────────────────────────────────────┐
│                                                                   │
│  ┌──────────────────────┐  ┌──────────────────────┐              │
│  │  06-fix-loop-bug     │  │  07-handler-tests    │              │
│  │  (infinite loop)     │  │  (test coverage)     │              │
│  └──────────┬───────────┘  └──────────────────────┘              │
│             │                                                     │
│             ▼                                                     │
│  ┌──────────────────────┐  ┌──────────────────────┐              │
│  │  09-refactor-edits   │  │  08-auto-save-action │              │
│  │  (uses fixed nav)    │  │  (implement stub)    │              │
│  └──────────┬───────────┘  └──────────┬───────────┘              │
│             │                         │                          │
└─────────────┼─────────────────────────┼──────────────────────────┘
              │                         │
Major (Should Fix)                      │
┌─────────────┼─────────────────────────┼──────────────────────────┐
│             ▼                         ▼                          │
│  ┌──────────────────────┐  ┌──────────────────────┐              │
│  │  12-cleanup-methods  │  │  13-writer-improve   │              │
│  │  (after refactor)    │  │  (after auto-save)   │              │
│  └──────────────────────┘  └──────────────────────┘              │
│                                                                   │
│  ┌──────────────────────┐  ┌──────────────────────┐              │
│  │  10-launch-validation│  │  11-file-splitting   │              │
│  │  (error messages)    │  │  (tracking doc)      │              │
│  └──────────────────────┘  └──────────────────────┘              │
│                                                                   │
└───────────────────────────────────────────────────────────────────┘

Minor (Consider)
┌───────────────────────────────────────────────────────────────────┐
│  ┌──────────────────────┐                                        │
│  │  14-widget-rendering │                                        │
│  │  (consolidate code)  │                                        │
│  └──────────────────────┘                                        │
└───────────────────────────────────────────────────────────────────┘
```

## Success Criteria

### Implementation Complete (Tasks 1-5)

- [x] `LaunchContextState` struct with config, mode, flavor, dart_defines
- [x] Configuration dropdown opens fuzzy modal
- [x] Mode radio buttons work (Debug/Profile/Release)
- [x] Flavor field opens fuzzy modal (when editable)
- [x] Dart Defines field opens dart defines modal (when editable)
- [x] Fields show disabled state for VSCode configs
- [x] FDemon config changes auto-save to file
- [x] Launch button renders with focus state
- [x] Up/Down navigation between fields
- [x] `cargo fmt && cargo check && cargo test && cargo clippy -- -D warnings` passes

### Review Fixes Complete (Tasks 6-14)

- [x] Infinite loop bug fixed in field navigation (Task 6)
- [x] Unit tests added for new handlers (Task 7)
- [x] AutoSaveConfig action implemented (Task 8)
- [x] Editability checks refactored to use state methods (Task 9)
- [x] Launch validation error messages improved (Task 10)
- [x] File splitting tracked with TODO comments (Task 11)
- [x] Unused LaunchContextState methods cleaned up (Task 12)
- [x] Config writer has file locking (Task 13)
- [x] Widget rendering code consolidated (Task 14)

## Field Navigation

- Up/Down moves between fields: Config → Mode → Flavor → Dart Defines → Launch
- Enter on Config/Flavor → opens fuzzy modal
- Enter on Dart Defines → opens dart defines modal
- Enter on Launch → triggers launch action
- Left/Right on Mode → changes mode selection

## Notes

- Field editability depends on selected config source
- VSCode configs show "(from config)" suffix
- FDemon configs auto-save on change
- No config selected → transient values
- Consider visual indication of which fields are editable

## Review Context

**Review Date:** 2026-01-15
**Review Verdict:** ⚠️ NEEDS WORK

Tasks 6-14 were created to address issues found in the code review:
- **3 Critical issues** (Tasks 6-8): Must fix before merging
- **3 Major issues** (Tasks 9-11): Should fix for maintainability
- **3 Minor issues** (Tasks 12-14): Consider fixing for code quality

See review documents:
- [REVIEW.md](../../../reviews/features/new-session-dialog-phase-6/REVIEW.md)
- [ACTION_ITEMS.md](../../../reviews/features/new-session-dialog-phase-6/ACTION_ITEMS.md)

### Recommended Execution Order

1. **Wave 1 (Critical - Parallel):** Tasks 6, 7, 8 can run in parallel
2. **Wave 2 (Major - Sequential):** Task 9 depends on 6, Task 10-11 can run after Wave 1
3. **Wave 3 (Minor - Parallel):** Tasks 12, 13, 14 can run in parallel after dependencies
