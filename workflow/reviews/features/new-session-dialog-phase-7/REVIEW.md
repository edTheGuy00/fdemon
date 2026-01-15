# Code Review: Phase 7 - Main Dialog Assembly

**Feature:** new-session-dialog
**Phase:** 7 - Main Dialog Assembly
**Review Date:** 2026-01-15
**Branch:** `feat/udpate-device-selector`

---

## Overall Verdict: **NEEDS WORK**

| Agent | Verdict | Summary |
|-------|---------|---------|
| Architecture Enforcer | **CONCERNS** | 3 critical layer violations; App importing TUI types |
| Code Quality Inspector | **NEEDS WORK** | Unsafe unwrap, missing error handling, broken tests |
| Logic Reasoning Checker | **CONCERNS** | Incomplete key routing, escape UX issues |
| Risks & Tradeoffs | **CONCERNS** | 168 broken tests = zero confidence |

---

## Summary

Phase 7 successfully implements the main NewSessionDialog assembly with:
- Modular state composition (TargetSelectorState + LaunchContextState)
- Two-pane layout widget (50/50 split)
- Modal overlay rendering (fuzzy modal with dim, dart defines full-screen)
- New messages: `OpenNewSessionDialog`, `CloseNewSessionDialog`, `NewSessionDialogEscape`

However, **critical issues** prevent approval:

1. **CRITICAL:** 168 test failures - test suite completely broken by API changes
2. **CRITICAL:** Layer boundary violations - App layer imports TUI types
3. **CRITICAL:** Incomplete key routing - most keys ignored in dialog
4. **MAJOR:** Unsafe `unwrap()` in launch handler

---

## Files Modified

| File | Changes | Lines |
|------|---------|-------|
| `src/app/handler/keys.rs` | Key routing, 'd' key opens dialog | +14/-4 |
| `src/app/handler/new_session/fuzzy_modal.rs` | Updated modal handlers | +6/-22 |
| `src/app/handler/new_session/launch_context.rs` | Nested state access | +91/-105 |
| `src/app/handler/new_session/navigation.rs` | New handlers for dialog lifecycle | +68/-55 |
| `src/app/handler/new_session/target_selector.rs` | Nested state access | +42/-38 |
| `src/app/handler/update.rs` | Wired new message handlers | +8/-18 |
| `src/app/message.rs` | Added 3 new message variants | +9/-0 |
| `src/app/state.rs` | Updated NewSessionDialogState::new() call | +2/-2 |
| `src/tui/widgets/new_session_dialog/mod.rs` | NewSessionDialog widget | +306/-1 |
| `src/tui/widgets/new_session_dialog/state/dialog.rs` | Complete state refactoring | ~350 changed |
| `src/tui/widgets/new_session_dialog/state/types.rs` | DialogPane rename + toggle | +9/-4 |

**Total:** 17 files, +1141/-545 lines

---

## Critical Issues

### 1. Test Suite Completely Broken (BLOCKING)

**Severity:** CRITICAL
**Source:** All Agents
**Location:** `src/app/handler/tests.rs`, `src/tui/widgets/new_session_dialog/state/tests/dialog_tests.rs`

**Problem:** 168 test failures due to API breaking changes. Tests reference:
- Old methods: `switch_tab()`, `open_fuzzy_modal()`, `target_up()`, `context_down()`
- Old fields: `loading_bootable`, `flavor`, `active_pane`, `target_tab`
- Old constructors: `NewSessionDialogState::new()` with no params

**Impact:** Zero confidence in system behavior. The refactoring may have introduced silent regressions that tests would have caught.

**Required Action:** Fix test suite compilation and update API references before merge.

---

### 2. Layer Boundary Violation (BLOCKING)

**Severity:** CRITICAL
**Source:** Architecture Enforcer
**Location:** Multiple files

**Problem:** App layer imports types from TUI layer:
- `src/app/state.rs:12` - imports `NewSessionDialogState` from TUI
- `src/app/message.rs:10` - imports `DartDefine`, `FuzzyModalType`, `TargetTab` from TUI
- `src/app/handler/keys.rs:680` - imports `TargetTab` from TUI

**Violation:** Per `docs/ARCHITECTURE.md`, the TUI layer depends on App (View depends on Model), NOT the reverse. State types belong in the App layer.

**Required Action:** Move all dialog state types from `tui/widgets/new_session_dialog/state/` to `app/new_session_dialog/` module.

---

### 3. Incomplete Key Routing (BLOCKING)

**Severity:** CRITICAL
**Source:** Architecture Enforcer, Logic Reasoning Checker
**Location:** `src/app/handler/keys.rs:679-703`

**Problem:** The `handle_key_new_session_dialog()` function only handles Tab, Escape, and tab shortcuts (1/2). All other keys (arrows, Enter, text input) return `None` and are ignored.

```rust
// Line 700-702
// All other keys - delegate to modal-aware handler
_ => None,  // Should actually route to modal/pane handlers
```

**Impact:** The dialog is barely usable - users cannot navigate device lists, select configs, or interact with modals.

**Required Action:** Implement complete key routing that delegates to:
- Fuzzy modal handler when modal is open
- Dart defines modal handler when modal is open
- Target selector handler when left pane focused
- Launch context handler when right pane focused

---

### 4. Unsafe Unwrap in Launch Handler

**Severity:** MAJOR
**Source:** Code Quality Inspector
**Location:** `src/app/handler/new_session/launch_context.rs:239-248`

**Problem:**
```rust
let device = state
    .new_session_dialog_state
    .selected_device()
    .unwrap()  // Can panic!
    .clone();
```

**Impact:** Panic if `build_launch_params()` returns `Some` but `selected_device()` returns `None` (logic error or race condition).

**Required Action:** Use proper error handling, not `unwrap()`. Per `docs/CODE_STANDARDS.md`: "Never use unwrap in library code."

---

## Major Issues

### 5. Missing Error Handling in Config Loading

**Source:** Code Quality Inspector
**Location:** `src/app/handler/new_session/navigation.rs:160-166`

```rust
let configs = crate::config::load_all_configs(&state.project_path);
```

No error handling if config loading fails. Should provide user feedback.

---

### 6. Modal State Corruption Risk

**Source:** Risks & Tradeoffs Analyzer
**Location:** `src/tui/widgets/new_session_dialog/state/dialog.rs`

No enforcement that only one modal can be open at a time. The `open_*_modal()` methods don't verify existing modal is closed.

**Recommended Fix:** Add debug assertions:
```rust
pub fn open_config_modal(&mut self) {
    debug_assert!(!self.has_modal_open(), "Modal already open");
    // ...
}
```

---

### 7. Escape Always Saves Dart Defines

**Source:** Logic Reasoning Checker
**Location:** `src/app/handler/new_session/navigation.rs:192-198`

Pressing Escape on dart defines modal calls `close_dart_defines_modal_with_changes()` which saves changes. There's no way to cancel/discard edits.

**UX Impact:** Users cannot discard unwanted edits to dart defines.

---

## Minor Issues

1. **Missing doc comments** on public handler functions (navigation.rs, launch_context.rs)
2. **Verbose nested access** patterns (4 levels deep)
3. **Hard-coded footer strings** in widget (should be constants)
4. **Silent failure** when pane toggle blocked by modal
5. **Session check uses `has_running_sessions()`** not `has_sessions()`

---

## Positive Observations

1. **Excellent modular refactoring** - State composition with TargetSelectorState + LaunchContextState follows TEA best practices

2. **Semantic naming improvement** - DialogPane variants changed from `Left`/`Right` to `TargetSelector`/`LaunchContext`

3. **Comprehensive unit tests written** - 13 new tests in dialog.rs for core functionality

4. **Proper widget composition** - NewSessionDialog correctly composes child widgets

5. **Modal overlay patterns** - Fuzzy modal dims background, dart defines is full-screen (good UX)

6. **LaunchParams struct** - Clean encapsulation of launch configuration

---

## Verification Status

| Command | Status |
|---------|--------|
| `cargo fmt --check` | PASS |
| `cargo check` | PASS |
| `cargo clippy --lib -- -D warnings` | PASS |
| `cargo test --lib` | **FAIL** (168 errors) |

---

## Recommendations

### Before Merge (BLOCKING)

1. **Fix test suite compilation** - Update API references in test files
2. **Fix layer boundary violations** - Move state types to App layer
3. **Complete key routing** - Implement full keyboard handling
4. **Remove unsafe unwrap** - Use proper error handling in launch

### Short-term (Next PR)

1. Add doc comments to all public handler functions
2. Add modal exclusivity assertions
3. Verify launch flow with manual testing
4. Run full test suite once fixed

### Medium-term

1. Consider convenience methods for verbose nested access
2. Add integration test for launch flow
3. Monitor performance with many dart-defines
4. Consider "cancel without saving" for dart defines modal

---

## Approval Condition

DO NOT MERGE until:
- [ ] Test suite compiles and core tests pass
- [ ] Layer boundary violations resolved OR documented exemption
- [ ] Key routing implementation complete
- [ ] Unsafe unwrap removed

See `ACTION_ITEMS.md` for detailed fix instructions.
