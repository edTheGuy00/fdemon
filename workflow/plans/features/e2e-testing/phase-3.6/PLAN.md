# Plan: Phase 3.6 - Review Followup & TEA Compliance

## TL;DR

Address code quality issues identified in the Phase 3.5 Wave 4-6 review: fix weak OR assertions, extract duplicated `test_device()` helpers, refactor oversized `status_bar.rs`, improve TestTerminal encapsulation, and update ARCHITECTURE.md to accurately reflect TEA pattern dependencies.

---

## Background

The Phase 3.5 Wave 4-6 code review identified several code quality and organizational issues:

1. **Logic bugs** - Weak OR assertions in transition tests that should use AND logic
2. **Code duplication** - `test_device()` helper duplicated 5+ times across widget files
3. **File size violation** - `status_bar.rs` at 1031 lines (violates 500-line guideline)
4. **Encapsulation gap** - Public `terminal` field in TestTerminal bypasses wrapper API
5. **Documentation gap** - ARCHITECTURE.md doesn't reflect TUI→App dependency required for TEA

The review verdict was "APPROVED WITH CONCERNS" - no blocking bugs, but organizational debt should be addressed.

---

## TEA Pattern Compliance

### Current State

The `docs/ARCHITECTURE.md` claims:
```
| **TUI** | Presentation | Core |
```

But the actual implementation requires TUI→App dependency:
- `tui::render()` receives `&AppState` (from `app/state.rs`)
- `tui::test_utils::create_test_state()` creates `AppState` instances
- Widget tests import `crate::app::state::AppState`

### TEA Pattern Analysis

This dependency is **correct and necessary** for the TEA pattern:

```
TEA Pattern:
┌─────────────────────────────────────────────────────────┐
│  Model (AppState)  ←─────── State lives in App layer   │
│        ↓                                                │
│  Update (handler) ←──────── Pure function in App layer │
│        ↓                                                │
│  View (render)    ←──────── Renders Model to terminal  │
└─────────────────────────────────────────────────────────┘
```

The View function **must** receive the Model to render it. This is the fundamental TEA contract:
- View: `fn render(frame: &mut Frame, state: &AppState)`

### Required Documentation Change

Update ARCHITECTURE.md layer dependencies:

```diff
| Layer | Responsibility | Dependencies |
|-------|----------------|--------------|
-| **TUI** | Presentation | Core |
+| **TUI** | Presentation | Core, App (TEA View pattern) |
```

This is not a violation - it's documenting the intentional TEA architecture.

---

## Affected Modules

- `src/tui/test_utils.rs` - Add shared helpers, improve encapsulation
- `src/tui/render/tests.rs` - Fix OR→AND assertion logic
- `src/tui/widgets/status_bar.rs` → `src/tui/widgets/status_bar/` - Extract tests to module
- `src/tui/widgets/device_selector.rs` - Remove duplicated helper
- `src/tui/widgets/header.rs` - Remove duplicated helper
- `src/tui/widgets/confirm_dialog.rs` - Remove duplicated helper
- `src/tui/widgets/tabs.rs` - Remove duplicated helper
- `src/tui/widgets/startup_dialog/mod.rs` - Remove duplicated helper
- `docs/ARCHITECTURE.md` - Update dependency documentation

---

## Development Phases

### Wave 1: Critical Fixes (Required Before Merge)

**Goal**: Fix logic bugs and document public API

#### Tasks

1. **Fix OR→AND assertions** (render/tests.rs)
   - Line 279: `!before.contains("Select") || !before.contains("Device")` → `&&`
   - Line 287: Fix similar OR pattern
   - Line 306: `after.contains("Quit") || after.contains("quit")` - OK (case insensitive)
   - Line 334: Same case-insensitive pattern - OK
   - Add failure messages to assertions

2. **Document public terminal field**
   - Add doc comment explaining usage pattern
   - Explain why direct access is sometimes needed

**Milestone**: All transition test assertions use correct logic

---

### Wave 2: Code Deduplication

**Goal**: Eliminate test helper duplication

#### Tasks

3. **Extract test_device() to test_utils.rs**
   - Add flexible `test_device()` and `test_device_full()` helpers
   - Support all parameter variations:
     - `test_device(id, name)` - basic
     - `test_device_with_platform(id, name, platform)` - platform-specific
     - `test_device_full(id, name, platform, emulator)` - all params

4. **Migrate widget tests to shared helper**
   - Update `device_selector.rs` tests
   - Update `header.rs` tests
   - Update `status_bar.rs` tests
   - Update `tabs.rs` tests
   - Update `startup_dialog/mod.rs` tests
   - Remove all duplicated `test_device()` definitions

**Milestone**: Single source of truth for test device creation

---

### Wave 3: File Organization

**Goal**: Comply with 500-line guideline

#### Tasks

5. **Refactor status_bar.rs to directory module**
   - Create `src/tui/widgets/status_bar/` directory
   - Move widget code to `mod.rs` (~331 lines)
   - Move tests to `tests.rs` (~700 lines)
   - Update `widgets/mod.rs` imports

6. **Improve TestTerminal encapsulation**
   - Add `draw_with()` method for custom rendering
   - Update `render/tests.rs` to use `draw_with()`
   - Consider `pub(crate)` for terminal field
   - Add comprehensive doc comments

**Milestone**: All widget files under 500 lines

---

### Wave 4: Documentation & Cleanup

**Goal**: Update architecture docs for TEA compliance

#### Tasks

7. **Update ARCHITECTURE.md**
   - Fix TUI dependencies to include App
   - Add explanation of TEA View pattern
   - Document `render/` module structure
   - Add TestTerminal to test utility documentation

8. **Strengthen SearchInput test**
   - Replace weak `len() > 0` assertion
   - Create proper session with search input
   - Test actual content rendered

**Milestone**: Documentation accurately reflects codebase

---

## Edge Cases & Risks

### Test Isolation
- **Risk:** Shared test helpers could introduce coupling between tests
- **Mitigation:** Helpers create fresh Device instances, no shared mutable state

### Module Refactoring
- **Risk:** Moving status_bar.rs could break imports
- **Mitigation:** Use pub re-exports in mod.rs to maintain API

### Backward Compatibility
- **Risk:** Changing TestTerminal API could break existing tests
- **Mitigation:** Keep `terminal` field accessible (with docs), add `draw_with()` as additional method

---

## Task Dependency Graph

```
Wave 1 (Critical)
├── 01-fix-or-assertions
└── 02-document-terminal-field

Wave 2 (Deduplication)
├── 03-extract-test-device-helper
└── 04-migrate-widget-tests
    └── depends on: 03

Wave 3 (Organization)
├── 05-refactor-status-bar-module
└── 06-improve-testterminal-api
    └── depends on: 04 (uses shared helpers)

Wave 4 (Documentation)
├── 07-update-architecture-docs
│   └── depends on: 05
└── 08-strengthen-search-input-test
    └── depends on: 06
```

---

## Success Criteria

### Wave 1 Complete When:
- [ ] All transition tests use AND (`&&`) logic for proper validation
- [ ] Public `terminal` field has doc comment explaining usage
- [ ] `cargo test --lib` passes

### Wave 2 Complete When:
- [ ] `test_device()` exists only in `test_utils.rs`
- [ ] All widget tests use shared helper
- [ ] No duplicated test device creation code

### Wave 3 Complete When:
- [ ] `status_bar.rs` widget code < 500 lines
- [ ] Tests in separate `tests.rs` file
- [ ] `TestTerminal::draw_with()` method added

### Wave 4 Complete When:
- [ ] ARCHITECTURE.md shows `TUI | Presentation | Core, App`
- [ ] TEA View pattern documented
- [ ] SearchInput test verifies actual content

---

## References

- [Phase 3.5 Wave 4-6 Review](/workflow/reviews/features/phase-3.5-wave-4-6/REVIEW.md)
- [docs/ARCHITECTURE.md](/docs/ARCHITECTURE.md)
- [docs/CODE_STANDARDS.md](/docs/CODE_STANDARDS.md)
