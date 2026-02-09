# Review: Phase 3 - New Session Modal Redesign (Cyber-Glass)

**Date:** 2026-02-09
**Branch:** feat/redesign
**Scope:** 25 files, +1540 / -726 lines
**Verdict:** NEEDS WORK

## Overview

Phase 3 transforms the New Session modal to the Cyber-Glass design with RGB palette migration, glass overlay/shadow, redesigned frame with header/footer, two-pane layout (target selector + launch context), themed nested modals, and kbd-style footer hints. The implementation was executed across 5 waves with parallel agents.

## Agent Verdicts

| Agent | Verdict | Critical Issues | Major Issues |
|-------|---------|----------------|--------------|
| Architecture Enforcer | CONCERNS | 2 | 0 |
| Code Quality Inspector | NEEDS WORK | 2 | 4 |
| Logic & Reasoning Checker | FAIL | 2 | 2 |
| Risks & Tradeoffs Analyzer | CONCERNS | 1 | 1 |

**Consolidated Verdict: NEEDS WORK** (2 critical issues found by all 4 agents)

## Critical Issues (Must Fix)

### 1. Missing Dart Defines Field from All Layouts

**Source:** All 4 agents
**File:** `crates/fdemon-tui/src/widgets/new_session_dialog/launch_context.rs`
**Lines:** 709-746 (layout), 736-746 (render)

The `LaunchContextField` enum in the app layer defines navigation order as:
Config -> Mode -> Flavor -> EntryPoint -> **DartDefines** -> Launch

But the TUI rendering completely omits DartDefines:
- `calculate_fields_layout()` allocates no slot for it
- `render_common_fields()` does not render it
- Compact mode also omits it (despite comments saying "only in compact mode")

**Impact:** Ghost field in navigation. When user presses Down from EntryPoint, focus moves to an invisible DartDefines field with no visual change. Pressing Enter on it opens the dart defines modal from nowhere. The `ActionField` widget exists and was designed for this purpose but is never used in any layout.

**Required Fix:** Either:
- (a) Add DartDefines rendering to both layouts using `ActionField`
- (b) Remove `DartDefines` from `LaunchContextField::next()`/`prev()` so EntryPoint navigates directly to Launch

### 2. Launch Button Ignores Focus State

**Source:** All 4 agents
**File:** `crates/fdemon-tui/src/widgets/new_session_dialog/launch_context.rs`
**Lines:** 374-406

`LaunchButton` has `is_focused: bool` field and `focused()` setter, and callers pass focus state, but `render()` only branches on `is_enabled`. The focused state is never used for visual feedback.

**Impact:** No visual indication when Launch button is selected via keyboard navigation. All other field widgets (DropdownField, ActionField) show focus via border/background changes. LaunchButton is the only widget that ignores focus.

**Required Fix:** Add focus-based styling in `render()`:
```rust
let (bg, fg, border) = if !self.is_enabled {
    (palette::SURFACE, palette::TEXT_MUTED, palette::BORDER_DIM)
} else if self.is_focused {
    (palette::GRADIENT_BLUE, palette::TEXT_BRIGHT, palette::BORDER_ACTIVE)
} else {
    (palette::GRADIENT_BLUE, palette::TEXT_BRIGHT, palette::GRADIENT_BLUE)
};
```

## Major Issues (Should Fix)

### 3. Stale `#[allow(dead_code)]` on Actively Used Constants

**File:** `crates/fdemon-tui/src/theme/palette.rs` lines 14, 43
**Constants:** `SURFACE` (used 11x), `GRADIENT_BLUE` (used 3x) still carry dead_code annotations from Task 01. `GRADIENT_INDIGO` (line 45) is genuinely unused and should keep the annotation.

### 4. Commented-Out Test Assertions

**File:** `launch_context.rs` lines 512, 1109, 1281, 1748
**Issue:** Assertions for DartDefines are commented out with misleading comments ("removed from normal layout, only in compact mode") rather than being properly removed or the underlying bug fixed.

### 5. Inconsistent Overlay for Dart Defines Modal

**File:** `crates/fdemon-tui/src/widgets/new_session_dialog/mod.rs` line 412
**Issue:** `render_dart_defines_modal()` uses `Clear.render()` while all other modals use `modal_overlay::dim_background()`. Task 07 aimed for consistency but this caller wasn't updated.

### 6. min_height() Potentially Incorrect

**File:** `launch_context.rs` line 769
**Issue:** Returns 21 but the comment arithmetic sums to 23. The actual layout may need 24 lines (9 chunks + spacer + 3-line button). May cause button clipping in tight terminals.

## Strengths

- **Layer compliance:** All changes respect workspace crate boundaries. TUI imports App for state types (TEA pattern), never imports Daemon directly.
- **Theme pipeline:** Clean palette -> styles -> widgets architecture with RGB design tokens.
- **Widget decomposition:** New `device_list.rs` and `tab_bar.rs` subwidgets are well-encapsulated with single responsibility.
- **Modal overlay:** Correctly leverages existing utilities (dim_background, render_shadow, clear_area).
- **Test coverage:** 428 TUI unit tests pass, 1553 total across workspace.
- **Palette consolidation:** Reduced from 40+ constants to a cleaner set of core design tokens.

## Recommendations

1. Fix the 2 critical issues before merge
2. Clean up stale `#[allow(dead_code)]` annotations
3. Remove commented-out test assertions
4. Replace `Clear.render()` with `dim_background()` in dart defines modal
5. Verify `min_height()` arithmetic

## Quality Gate

| Check | Result |
|-------|--------|
| `cargo fmt --all` | PASS |
| `cargo check --workspace` | PASS |
| `cargo test --workspace --lib` | PASS (1,553 tests) |
| `cargo clippy -- -D warnings` | PASS |
| E2E tests | 34 FAIL (pre-existing, no Flutter SDK) |

## Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-tui/src/theme/palette.rs` | RGB migration (33 constants) |
| `crates/fdemon-tui/src/theme/styles.rs` | Style function updates |
| `crates/fdemon-tui/src/widgets/new_session_dialog/mod.rs` | Frame, header, footer, overlay |
| `crates/fdemon-tui/src/widgets/new_session_dialog/target_selector.rs` | Left pane redesign |
| `crates/fdemon-tui/src/widgets/new_session_dialog/device_list.rs` | New device list subwidget |
| `crates/fdemon-tui/src/widgets/new_session_dialog/tab_bar.rs` | New tab toggle widget |
| `crates/fdemon-tui/src/widgets/new_session_dialog/launch_context.rs` | Right pane redesign |
| `crates/fdemon-tui/src/widgets/new_session_dialog/fuzzy_modal.rs` | Theme migration |
| `crates/fdemon-tui/src/widgets/new_session_dialog/dart_defines_modal.rs` | Theme migration |
| `crates/fdemon-tui/src/widgets/settings_panel/tests.rs` | Updated assertions |
| `crates/fdemon-tui/src/render/mod.rs` | Minor overlay changes |
| `crates/fdemon-tui/src/widgets/header.rs` | Minor changes |
| `crates/fdemon-tui/src/widgets/tabs.rs` | Minor changes |
| `crates/fdemon-tui/src/widgets/log_view/mod.rs` | Minor changes |
| `crates/fdemon-app/src/handler/tests.rs` | Test updates |
| `crates/fdemon-app/src/settings_items.rs` | Minor changes |
