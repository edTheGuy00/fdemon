## Task: Complete Palette Migration for Remaining Hardcoded Colors

**Objective**: Replace all remaining hardcoded `Color::` references in production widget code (outside `theme/`) with palette constants or style helpers from the theme module.

**Depends on**: None (but benefits from Task 04 removing ~26 references with dead code)

**Review Reference**: REVIEW.md #4 (Major), ACTION_ITEMS.md #4, #5, #6

### Scope

After Task 04 removes dead code (status_bar: 16 refs, legacy tabs: 10 refs), approximately 20 hardcoded `Color::` references remain in production code. These need migration:

#### `modal_overlay.rs` (4 references)

| Line | Current | Replacement |
|------|---------|-------------|
| 98 | `Color::DarkGray` (dim fg) | `palette::TEXT_MUTED` |
| 98 | `Color::Black` (dim bg) | `palette::DEEPEST_BG` |
| 133 | `Color::Black` (shadow fg) | `palette::SHADOW` |
| 133 | `Color::Black` (shadow bg) | `palette::SHADOW` |

#### `log_view/mod.rs` (3 references)

| Line | Current | Replacement |
|------|---------|-------------|
| 203 | `Color::Black` (link badge fg) | Use `styles::focused_selected()` or add `palette::CONTRAST_FG` |
| 347 | `Color::Black` (search highlight fg) | Use `palette::CONTRAST_FG` or `styles::focused_selected()` |
| 351 | `Color::Black` (current search match fg) | Use `palette::CONTRAST_FG` or `styles::focused_selected()` |

#### `settings_panel/mod.rs` (1 reference)

| Line | Current | Replacement |
|------|---------|-------------|
| 176 | `Color::Black` (active item fg on accent bg) | Use `styles::focused_selected()` |

#### `new_session_dialog/dart_defines_modal.rs` (2 references)

| Line | Current | Replacement |
|------|---------|-------------|
| 83 | `Color::Black` (selected item fg on accent bg) | Use `styles::focused_selected()` |
| 145 | `Color::Black` (active button fg on accent bg) | Use `styles::focused_selected()` |

#### `new_session_dialog/fuzzy_modal.rs` (1 reference)

| Line | Current | Replacement |
|------|---------|-------------|
| 145 | `Color::Black` (selected item fg on accent bg) | Use `styles::focused_selected()` |

#### `new_session_dialog/tab_bar.rs` (1 reference)

| Line | Current | Replacement |
|------|---------|-------------|
| 36 | `ratatui::style::Color::Black` (active tab fg) | Use `styles::focused_selected()` |

#### `new_session_dialog/launch_context.rs` (5 references)

| Line | Current | Replacement |
|------|---------|-------------|
| 78 | `Color::Black` (focused dropdown fg) | Use `styles::focused_selected()` |
| 137 | `Color::Black` (selected mode fg) | Use `styles::focused_selected()` |
| 245 | `Color::Black` (focused dart-defines row) | Use `styles::focused_selected()` |
| 305 | `Color::Black` (focused launch button fg) | Use `styles::focused_selected()` |
| 848 | `Color::Black` (selected mode chip fg) | Use `styles::focused_selected()` |

#### `new_session_dialog/device_list.rs` (2 references)

| Line | Current | Replacement |
|------|---------|-------------|
| 69 | `ratatui::style::Color::Black` (selected+focused fg) | Use `styles::focused_selected()` |
| 228 | `ratatui::style::Color::Black` (selected+focused fg) | Use `styles::focused_selected()` |

### Details

**Pattern analysis**: The vast majority (19 of ~20) of remaining `Color::Black` references follow one pattern: `.fg(Color::Black).bg(palette::ACCENT)` for focused/selected item styling. This is exactly what `styles::focused_selected()` already provides. The migration is mechanical.

**For `modal_overlay.rs`**: The dim and shadow functions use `Color::DarkGray`/`Color::Black` which map directly to existing palette constants (`TEXT_MUTED`, `DEEPEST_BG`, `SHADOW`).

**For `Color::Black` on accent background**: Either:
- Use `styles::focused_selected()` directly where it provides the full style
- Add a `palette::CONTRAST_FG` constant (value: `Color::Black`) for cases where only the fg color is needed and the bg varies

**Implementation approach**: File-by-file, replace `Color::` references with palette/styles equivalents. Run `cargo check` after each file.

### Acceptance Criteria

1. Zero `Color::` references in production code outside `theme/` (tests excluded)
2. All replaced references use appropriate palette constants or style helpers
3. Visual rendering is unchanged (same colors, just sourced from theme)
4. `cargo check -p fdemon-tui` passes
5. `cargo clippy -p fdemon-tui` passes

### Testing

- Visual regression: rendering should look identical before and after
- Any tests that assert on specific `Color::` values may need updating to use palette constants
- Run `cargo test -p fdemon-tui` to check

### Notes

- If `styles::focused_selected()` doesn't exactly match some use sites (e.g., some use `Color::Black` on `Color::Green` instead of `ACCENT`), add specific style helpers or use palette constants directly
- The `Color::Black` inside `styles.rs:82` (`focused_selected()` function itself) should also be migrated to a `palette::CONTRAST_FG` constant — this is covered in Task 06
- Verify `palette.rs` has all necessary constants before starting (e.g., `SHADOW`, `CONTRAST_FG` may need to be added)

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-tui/src/theme/palette.rs` | Added `CONTRAST_FG` constant for high-contrast foreground on accent background |
| `crates/fdemon-tui/src/widgets/modal_overlay.rs` | Replaced hardcoded `Color::DarkGray`/`Color::Black` with `palette::TEXT_MUTED`, `palette::DEEPEST_BG`, and `palette::SHADOW`; updated tests to use palette constants |
| `crates/fdemon-tui/src/widgets/log_view/mod.rs` | Replaced `Color::Black` in link badge and search highlight styles with `palette::CONTRAST_FG` and search palette constants; removed unused `Color` import |
| `crates/fdemon-tui/src/widgets/log_view/tests.rs` | Updated stack frame style tests to use palette constants instead of hardcoded colors |
| `crates/fdemon-tui/src/widgets/settings_panel/mod.rs` | Replaced `Color::Black` in active tab style with `palette::CONTRAST_FG`; removed unused `Color` import |
| `crates/fdemon-tui/src/widgets/settings_panel/tests.rs` | Updated test to use `palette::TEXT_MUTED` instead of hardcoded `Color::DarkGray` |
| `crates/fdemon-tui/src/widgets/new_session_dialog/dart_defines_modal.rs` | Replaced `Color::Black` in selected item and button styles with `palette::CONTRAST_FG`; removed unused `Color` import |
| `crates/fdemon-tui/src/widgets/new_session_dialog/fuzzy_modal.rs` | Replaced `Color::Black` in selected item style with `palette::CONTRAST_FG`; removed unused `Color` import |
| `crates/fdemon-tui/src/widgets/new_session_dialog/tab_bar.rs` | Replaced `ratatui::style::Color::Black` in active tab style with `palette::CONTRAST_FG` |
| `crates/fdemon-tui/src/widgets/new_session_dialog/launch_context.rs` | Replaced all 5 `Color::Black` references with `palette::CONTRAST_FG` using replace_all; removed unused `Color` import |
| `crates/fdemon-tui/src/widgets/new_session_dialog/device_list.rs` | Replaced 2 `ratatui::style::Color::Black` references with `palette::CONTRAST_FG` using replace_all |

### Notable Decisions/Tradeoffs

1. **Added CONTRAST_FG constant**: Created `palette::CONTRAST_FG = Color::Black` to handle cases where only the foreground color is needed on accent backgrounds, rather than using the full `styles::focused_selected()` style. This provides flexibility for widgets that need to customize the background or modifiers separately.

2. **Search highlight uses palette constants**: Used existing `palette::SEARCH_HIGHLIGHT_FG` and `palette::SEARCH_CURRENT_FG` constants that were already defined in palette.rs, maintaining consistency with the existing palette structure.

3. **Test updates**: Updated test assertions to use palette constants instead of hardcoded colors for future-proofing when Phase 2 transitions to RGB values.

4. **Preserved theme/styles.rs**: Left `Color::Black` in `styles::focused_selected()` function (line 82) unchanged, as noted in task - this will be migrated to `CONTRAST_FG` in Task 06.

### Testing Performed

- `cargo check -p fdemon-tui` - Passed (no warnings)
- `cargo test -p fdemon-tui --lib` - Passed (418 tests, 0 failed)
- `cargo clippy -p fdemon-tui -- -D warnings` - Passed (no warnings)

### Verification

Confirmed zero `Color::Black` or `Color::DarkGray` references in production widget code outside `theme/`:
- All production widget code now uses palette constants or style helpers
- Only remaining hardcoded colors are in `theme/palette.rs` (definitions) and `theme/styles.rs:82` (focused_selected function, deferred to Task 06)
- Visual rendering unchanged - all colors sourced from theme module

### Risks/Limitations

None identified. All acceptance criteria met:
1. Zero Color:: references in production code outside theme/ - PASS
2. All replaced references use appropriate palette constants - PASS
3. Visual rendering unchanged - PASS (same color values, different source)
4. cargo check passes - PASS
5. cargo clippy passes - PASS
