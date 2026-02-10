# Review: Redesign Cyber-Glass Phase 4 — Settings Panel

**Review Date:** 2026-02-10
**Branch:** `feat/redesign`
**Commit:** `22586aa`
**Verdict:** :warning: **NEEDS WORK**

## Summary

Phase 4 implements a complete visual redesign of the settings panel to match the Cyber-Glass design language. The changes span 5 source files with +3020/-323 lines across 7 tasks executed in wave-based dependency order. Core rendering, styling, icons, and tests were all updated.

**Files Modified:**
| File | Delta |
|------|-------|
| `crates/fdemon-tui/src/theme/icons.rs` | +87 |
| `crates/fdemon-tui/src/theme/palette.rs` | +5 |
| `crates/fdemon-tui/src/widgets/settings_panel/mod.rs` | +683 |
| `crates/fdemon-tui/src/widgets/settings_panel/styles.rs` | +99/-4 |
| `crates/fdemon-tui/src/widgets/settings_panel/tests.rs` | +350 |

## Agent Verdicts

| Agent | Verdict | Key Finding |
|-------|---------|-------------|
| Architecture Enforcer | :white_check_mark: **PASS** | No layer violations. All changes within fdemon-tui crate. |
| Code Quality Inspector | :warning: **CONCERNS** | mod.rs grew to ~1200 lines (exceeds 500-line split threshold). Code duplication in empty states. Dead-code annotations on now-used functions. |
| Logic & Reasoning Checker | :x: **FAIL** | Two critical rendering bugs confirmed — info banners always empty, empty states vertically centered instead of top-aligned. |
| Risks & Tradeoffs Analyzer | :warning: **CONCERNS** | IconMode hardcoded to Unicode. Content area reduced by 3 lines total. mod.rs growth trajectory. |

## Critical Issues

### 1. Info banner height allocation (Bug — user-reported)

**Source:** Logic & Reasoning Checker
**Files:** `mod.rs:507`, `mod.rs:910`
**Severity:** Critical

Info banners for USER and VSCODE tabs allocate `height: 3` but require `height: 4`. With `Borders::ALL`, `Block::inner()` yields inner height = 1. The guard `if inner.height < 2 { return; }` always triggers, rendering banners as empty boxes.

**Root cause:** `Rect::new(area.x, area.y, area.width, 3)` should be `Rect::new(area.x, area.y, area.width, 4)`.

### 2. Empty state vertical centering (Regression — user-reported)

**Source:** Logic & Reasoning Checker
**Files:** `mod.rs:794-868` (launch), `mod.rs:1031-1115` (vscode not found), `mod.rs:1117-1201` (vscode empty)
**Severity:** Critical

Empty states for Launch and VSCode tabs center content vertically and align text left. The previous behavior was top-aligned with horizontal centering, which the user prefers.

**Root cause:** `start_y = area.top() + area.height.saturating_sub(total_height) / 2` centers vertically. Should use `start_y = area.top()` or a small fixed offset.

## Major Issues

### 3. `#[allow(dead_code)]` on now-used style functions

**Source:** Code Quality Inspector
**Files:** `styles.rs:142-211`
**Severity:** Major (cleanup)

11 new style functions were annotated with `#[allow(dead_code)]` during early wave implementation. They are now actively called from mod.rs. The annotations should be removed.

### 4. mod.rs exceeds 500-line file size threshold

**Source:** Code Quality Inspector
**File:** `mod.rs` (~1200 lines)
**Severity:** Major (tech debt)

`CODE_STANDARDS.md` sets a 500-line split threshold. The file should be modularized (e.g., extract `header.rs`, `footer.rs`, `empty_states.rs`, `content.rs`). This is not blocking but should be tracked.

### 5. IconMode hardcoded to Unicode

**Source:** Risks & Tradeoffs Analyzer
**File:** `mod.rs` (multiple `IconSet::new(IconMode::Unicode)` calls)
**Severity:** Major

The settings panel has access to `self.settings` which contains `ui.icons` configuration, but all `IconSet::new()` calls hardcode `IconMode::Unicode` instead of reading the user preference. Users who configure NerdFonts will not see NerdFont icons in the settings panel.

## Minor Issues

### 6. Accent bar may lose selected row background

**Source:** Logic & Reasoning Checker
**File:** `mod.rs` (render_setting_row)

`buf.set_line()` for the accent bar `▎` replaces the cell style entirely, so the accent bar cell loses `SELECTED_ROW_BG`. The bar should combine both styles.

### 7. Code duplication in empty state functions

**Source:** Code Quality Inspector
**File:** `mod.rs`

`render_launch_empty_state()`, `render_vscode_not_found()`, and `render_vscode_empty()` share nearly identical layout logic (icon container + title + subtitle). Could extract a shared `render_empty_state()` helper.

### 8. Pre-existing `truncate_str` bug

**Source:** Logic & Reasoning Checker
**File:** `styles.rs:219-228`

`truncate_str("this is long", 8)` returns `"this is..."` (10 chars). Takes `max_len - 1` chars then appends `"..."` (3 chars) = max_len + 2 total. Pre-existing, not introduced by Phase 4.

## What Went Well

- Clean architecture: all changes stay within the fdemon-tui crate boundary
- Thorough test coverage: 8 new tests covering all visual changes, 441 tests passing
- Consistent design token usage through the styles module
- Wave-based task execution with proper dependency management
- No clippy warnings, clean formatting

## Verdict Rationale

Two critical rendering bugs prevent the settings panel from displaying correctly:
1. Info banners are visually empty (always early-return)
2. Empty states have regressed vertical alignment

Both are straightforward fixes (height constant, positioning formula). Once these are resolved along with the dead_code cleanup, the implementation quality is solid.
