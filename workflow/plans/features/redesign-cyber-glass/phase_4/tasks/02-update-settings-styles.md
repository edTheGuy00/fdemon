## Task: Update Settings Panel Style Functions

**Objective**: Refactor `settings_panel/styles.rs` to align with the Cyber-Glass design — update existing style functions to match the new design tokens, add new style functions for group headers with icons, selected row accent bars, info banners, and empty states.

**Depends on**: 01-add-settings-icons

### Scope

- `crates/fdemon-tui/src/widgets/settings_panel/styles.rs` — Update existing + add new style functions

### Details

#### Current State (140 lines, 15 functions)

The styles module already references `theme::palette` colors from Phase 1. However, several functions need updating to match the Cyber-Glass design:

#### Changes to Existing Functions

**1. `section_header_style()` (line 73)**
- **Current**: `STATUS_YELLOW` fg + BOLD
- **Target**: `ACCENT_DIM` fg + BOLD (group headers should be dimmed accent, not yellow)

**2. `label_style(is_selected: bool)` (line 57)**
- **Current**: BOLD when selected, default otherwise
- **Target**: `TEXT_PRIMARY` + BOLD when selected, `TEXT_SECONDARY` when not selected (design shows selected labels are brighter)

**3. `indicator_style(is_selected: bool)` (line 37)**
- **Current**: `ACCENT` when selected, default otherwise
- **Target**: `ACCENT` when selected (for the `▎` accent bar character), no change needed

**4. `description_style()` (line 90)**
- **Current**: `TEXT_MUTED` fg
- **Target**: `TEXT_MUTED` fg + `Modifier::ITALIC` (design shows italic descriptions)

#### New Style Functions to Add

**5. `group_header_icon_style() -> Style`**
- Returns: `ACCENT_DIM` fg
- Usage: For the icon glyph in group headers (e.g., the ⚡ before "BEHAVIOR")

**6. `selected_row_bg() -> Style`**
- Returns: Style with subtle `ACCENT` background tint
- Implementation: Use a very dark blue-tinted background, e.g., `Rgb(17, 25, 40)` — approximation of `ACCENT` at 10% opacity on `CARD_BG`
- Add this as a new palette constant `SELECTED_ROW_BG` in `palette.rs` if desired, or define inline
- Usage: Background fill for selected setting rows

**7. `accent_bar_style() -> Style`**
- Returns: `ACCENT` fg
- Usage: For the `▎` left border indicator on selected rows

**8. `kbd_badge_style() -> Style`**
- Returns: `TEXT_SECONDARY` fg + `POPUP_BG` bg
- Usage: For keyboard shortcut badges in footer (e.g., `[Tab]`, `[Esc]`)

**9. `kbd_label_style() -> Style`**
- Returns: `TEXT_MUTED` fg
- Usage: For the description text after kbd badges (e.g., "Switch tabs")

**10. `kbd_accent_style() -> Style`**
- Returns: `ACCENT` fg
- Usage: For the emphasized `Ctrl+S` shortcut in footer

**11. `info_banner_bg() -> Style`**
- Returns: Style with `ACCENT` tinted background (same approach as `selected_row_bg` but possibly slightly different shade)
- Implementation: `Rgb(17, 25, 40)` bg — approximation of `ACCENT` at 10%
- Usage: Background for User tab info banner

**12. `info_banner_border_style() -> Style`**
- Returns: `ACCENT_DIM` fg
- Usage: Border style for info banner (replacing current `STATUS_BLUE`)

**13. `empty_state_icon_style() -> Style`**
- Returns: `TEXT_MUTED` fg
- Usage: For the large icon in empty states (Launch tab)

**14. `empty_state_title_style() -> Style`**
- Returns: `TEXT_PRIMARY` fg + BOLD
- Usage: Title text in empty states (replacing current `STATUS_YELLOW`)

**15. `empty_state_subtitle_style() -> Style`**
- Returns: `TEXT_MUTED` fg + `Modifier::ITALIC`
- Usage: Subtitle text in empty states

#### Layout Constant Updates

Review and adjust if needed:

```rust
pub const INDICATOR_WIDTH: u16 = 3;         // Keep: "▎ " (accent bar + space) or "▶ "
pub const LABEL_WIDTH: u16 = 25;            // Keep: matches design ~200px equivalent
pub const VALUE_WIDTH: u16 = 15;            // Keep: matches design ~150px equivalent
```

No changes expected to layout constants — they already approximate the design reference's column proportions.

#### Palette Addition

Add one new color constant to `crates/fdemon-tui/src/theme/palette.rs`:

```rust
/// Subtle accent-tinted background for selected rows and info banners.
/// Approximates ACCENT at 10% opacity on CARD_BG.
pub const SELECTED_ROW_BG: Color = Color::Rgb(17, 25, 40);  // #111928
```

### Acceptance Criteria

1. `section_header_style()` returns `ACCENT_DIM` fg (not `STATUS_YELLOW`)
2. `label_style(true)` returns `TEXT_PRIMARY` fg + BOLD
3. `label_style(false)` returns `TEXT_SECONDARY` fg
4. `description_style()` returns `TEXT_MUTED` fg + ITALIC
5. New functions exist: `group_header_icon_style()`, `selected_row_bg()`, `accent_bar_style()`, `kbd_badge_style()`, `kbd_label_style()`, `kbd_accent_style()`, `info_banner_bg()`, `info_banner_border_style()`, `empty_state_icon_style()`, `empty_state_title_style()`, `empty_state_subtitle_style()`
6. `SELECTED_ROW_BG` constant added to palette.rs
7. `cargo check -p fdemon-tui` passes
8. `cargo clippy -p fdemon-tui` passes

### Testing

- Existing style tests will need updating for changed return values (section_header, label, description)
- Add tests for new style functions verifying correct fg/bg colors
- Test that `selected_row_bg()` returns a style with a background color set

### Notes

- **Style function consumers**: Tasks 03-06 will call these style functions. If a function signature or return value needs adjustment during those tasks, update it directly and note the change.
- **ITALIC modifier**: Ratatui supports `Modifier::ITALIC`. Most modern terminals render it correctly. If terminal compatibility is a concern, this can be made conditional later.
- **Palette color**: The `SELECTED_ROW_BG` color is an approximation. The design uses `bg-blue-500/10` which on the `CARD_BG` background (#12151c) blends to approximately `#111928`. Fine-tune if needed during visual testing.

---

## Completion Summary

**Status:** Not Started
