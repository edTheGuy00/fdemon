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
